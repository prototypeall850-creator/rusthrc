use anyhow::{anyhow, Context, Result};
use async_openai::config::OpenAIConfig;
use async_openai::types::{
    ChatCompletionRequestAssistantMessageArgs, ChatCompletionRequestMessage,
    ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessageArgs,
    CreateChatCompletionRequestArgs,
};
use async_openai::Client;

use crate::env::Environment;

pub struct LlmOptions {
    pub api_key: String,
    pub base_url: String,
    pub model: String,
}

#[derive(Clone)]
pub enum Turn {
    User(String),
    Assistant(String),
}

pub struct Suggestion {
    pub command: Option<String>,
    pub explanation: Option<String>,
    pub raw: String,
}

pub fn system_prompt(env: &Environment) -> String {
    format!(
        "You are rusthrc, a terminal-native assistant that turns a natural-language request \
         into a single, ready-to-run shell command.\n\n\
         User environment:\n\
         - OS: {os_pretty} ({os}, {arch})\n\
         - Shell: {shell}\n\
         - Working directory: {cwd}\n\
         - Top-level entries in the working directory: {entries}\n\n\
         Rules:\n\
         1. Reply with EXACTLY ONE fenced code block containing the command. No prose before or after it.\n\
         2. The command must be valid for {shell} on {os}.\n\
         3. It will be executed from the working directory shown above; never `cd` first.\n\
         4. Prefer the simplest correct command. Never invent files or paths; use only what the \
         environment or the user provides.\n\
         5. If required information is missing, or the request cannot be fulfilled with a shell \
         command, reply with one short plain-text question or explanation and NO code block.\n\
         6. The user may write in English or Indonesian (Bahasa Indonesia); understand both, and \
         write any explanation in the user's language.",
        os_pretty = env.os_pretty,
        os = env.os,
        arch = env.arch,
        shell = env.shell,
        cwd = env.cwd,
        entries = env.cwd_summary,
    )
}

pub async fn generate(opts: &LlmOptions, env: &Environment, history: &[Turn]) -> Result<Suggestion> {
    let config = OpenAIConfig::new()
        .with_api_key(opts.api_key.clone())
        .with_api_base(opts.base_url.clone());
    let client = Client::with_config(config);

    let mut messages = vec![system_message(system_prompt(env))];
    for turn in history {
        messages.push(match turn {
            Turn::User(content) => user_message(content),
            Turn::Assistant(content) => assistant_message(content),
        });
    }

    let request = CreateChatCompletionRequestArgs::default()
        .model(opts.model.clone())
        .messages(messages)
        .build()
        .context("failed to build the chat completion request")?;

    let response = client
        .chat()
        .create(request)
        .await
        .map_err(|e| anyhow!("model request failed: {e}"))?;

    let content = response
        .choices
        .first()
        .and_then(|choice| choice.message.content.clone())
        .unwrap_or_default();

    Ok(parse_response(&content))
}

fn system_message(content: String) -> ChatCompletionRequestMessage {
    ChatCompletionRequestSystemMessageArgs::default()
        .content(content)
        .build()
        .expect("system message is valid")
        .into()
}

fn user_message(content: &str) -> ChatCompletionRequestMessage {
    ChatCompletionRequestUserMessageArgs::default()
        .content(content)
        .build()
        .expect("user message is valid")
        .into()
}

fn assistant_message(content: &str) -> ChatCompletionRequestMessage {
    ChatCompletionRequestAssistantMessageArgs::default()
        .content(content)
        .build()
        .expect("assistant message is valid")
        .into()
}

/// Split a model reply into an optional command (first fenced code block, or a
/// bare single line) and an optional explanation (everything outside the fences).
pub fn parse_response(raw: &str) -> Suggestion {
    match extract_first_code_block(raw) {
        Some(command) => {
            let outside = text_outside_fences(raw);
            Suggestion {
                command: Some(command),
                explanation: if outside.is_empty() { None } else { Some(outside) },
                raw: raw.to_string(),
            }
        }
        None => {
            let trimmed = raw.trim();
            let (command, explanation) = if trimmed.is_empty() || trimmed.starts_with("```") {
                (None, Some("(the model returned an empty reply)".to_string()))
            } else if trimmed.lines().count() == 1 && looks_like_command(trimmed) {
                (Some(trimmed.to_string()), None)
            } else {
                (None, Some(trimmed.to_string()))
            };
            Suggestion {
                command,
                explanation,
                raw: raw.to_string(),
            }
        }
    }
}

/// A bare single-line reply only counts as a command when it does not read
/// like a sentence — clarifying questions from the model must stay prose.
fn looks_like_command(line: &str) -> bool {
    let t = line.trim();
    if t.is_empty() || t.ends_with('?') {
        return false;
    }
    let lower = t.to_lowercase();
    const QUESTION_STARTS: &[&str] = &[
        // English
        "which ", "what ", "where ", "how ", "why ", "who ", "can ", "could ", "should ",
        "would ", "will ", "do ", "does ", "did ", "is ", "are ", "should i ",
        // Indonesian
        "apa ", "apakah ", "bagaimana ", "dimana ", "di mana ", "kenapa ", "mengapa ",
        "siapa ", "kapan ", "bisakah ", "mohon ", "tolong ",
    ];
    !QUESTION_STARTS.iter().any(|q| lower.starts_with(q))
}

fn extract_first_code_block(md: &str) -> Option<String> {
    let lines: Vec<&str> = md.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        if lines[i].trim_start().starts_with("```") {
            let mut body = Vec::new();
            let mut j = i + 1;
            while j < lines.len() && !lines[j].trim_start().starts_with("```") {
                body.push(lines[j]);
                j += 1;
            }
            let text = body.join("\n").trim().to_string();
            if !text.is_empty() {
                return Some(text);
            }
            i = j + 1;
        } else {
            i += 1;
        }
    }
    None
}

fn text_outside_fences(md: &str) -> String {
    let mut kept = Vec::new();
    let mut inside = false;
    for line in md.lines() {
        if line.trim_start().starts_with("```") {
            inside = !inside;
            continue;
        }
        if !inside {
            kept.push(line);
        }
    }
    kept.join("\n").trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_fenced_command_and_explanation() {
        let s = parse_response("Sure!\n```bash\nls -la\n```\nHope that helps");
        assert_eq!(s.command.as_deref(), Some("ls -la"));
        assert!(s.explanation.as_deref().unwrap().contains("Sure!"));
    }

    #[test]
    fn plain_single_line_is_command() {
        let s = parse_response("ls -la");
        assert_eq!(s.command.as_deref(), Some("ls -la"));
        assert!(s.explanation.is_none());
    }

    #[test]
    fn prose_without_fence_is_explanation() {
        let s = parse_response("Which directory do you want to list?");
        assert!(s.command.is_none());
        assert!(s.explanation.is_some());
    }

    #[test]
    fn indonesian_question_is_explanation() {
        let s = parse_response("Apakah folder ini harus dihapus?");
        assert!(s.command.is_none());
    }

    #[test]
    fn multiline_command_in_fence() {
        let s = parse_response("```\necho one \\\n  two\n```");
        assert_eq!(s.command.as_deref(), Some("echo one \\\n  two"));
    }

    #[test]
    fn empty_fence_is_not_a_command() {
        let s = parse_response("```bash\n```");
        assert!(s.command.is_none());
    }

    #[test]
    fn fence_language_tag_is_stripped() {
        let s = parse_response("```powershell\nGet-ChildItem\n```");
        assert_eq!(s.command.as_deref(), Some("Get-ChildItem"));
    }
}
