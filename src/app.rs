use std::io::{IsTerminal, Read};

use anyhow::{Context, Result};
use colored::Colorize;
use serde::Serialize;

use crate::cli::{Cli, ConfigCmd, KeyStore, Subcommands};
use crate::config;
use crate::env::Environment;
use crate::exec;
use crate::llm::{self, LlmOptions, Suggestion, Turn};
use crate::render;

pub async fn dispatch(args: Cli) -> i32 {
    if let Some(sub) = &args.command {
        return match sub {
            Subcommands::Login { key, store } => store_key_command(key.clone(), *store),
            Subcommands::Logout => logout(),
            Subcommands::Config { cmd } => config_command(cmd.clone()),
        };
    }
    match prompt_flow(args).await {
        Ok(code) => code,
        Err(e) => {
            render::error(&format!("{e:#}"));
            1
        }
    }
}

async fn prompt_flow(args: Cli) -> Result<i32> {
    let cfg = config::load();
    let model = args
        .model
        .clone()
        .or_else(|| cfg.model.clone())
        .unwrap_or_else(|| config::DEFAULT_MODEL.to_string());
    let base_url = args
        .base_url
        .clone()
        .or_else(|| cfg.api_base.clone())
        .unwrap_or_else(|| config::DEFAULT_BASE_URL.to_string());
    let env = Environment::sniff();

    let prompt = gather_prompt(&args)?;
    if prompt.is_empty() {
        render::error("empty prompt");
        return Ok(1);
    }

    let (stored, warnings) = config::resolve_api_key();
    for w in &warnings {
        render::status(w);
    }
    let api_key = match stored {
        Some(key) => key.value,
        None if base_url.trim_end_matches('/') != config::DEFAULT_BASE_URL => {
            render::status("no API key set — sending an unauthenticated request to a custom base URL");
            String::new()
        }
        None => {
            render::error(
                "no API key configured — run `rusthrc login`, `rusthrc config set-key <KEY>`, \
                 or export OPENAI_API_KEY",
            );
            return Ok(1);
        }
    };

    let opts = LlmOptions {
        api_key,
        base_url,
        model: model.clone(),
    };
    // Headless when either side of the terminal is piped, or the user asked for it.
    let headless =
        args.json || args.print || !std::io::stdout().is_terminal() || !std::io::stdin().is_terminal();

    let mut history = vec![Turn::User(prompt)];
    loop {
        render::status(&format!("consulting {} …", opts.model));
        let suggestion = llm::generate(&opts, &env, &history).await?;
        render::clear_status();

        if args.json {
            print_json(&suggestion, &model, &env);
            return Ok(0);
        }
        if headless {
            return Ok(match suggestion.command {
                Some(cmd) => {
                    println!("{cmd}");
                    0
                }
                None => {
                    render::error(&format!(
                        "model replied without a command: {}",
                        suggestion.explanation.as_deref().unwrap_or("(empty)")
                    ));
                    1
                }
            });
        }

        match &suggestion.command {
            Some(cmd) => {
                render::print_command_panel(
                    cmd,
                    suggestion.explanation.as_deref(),
                    &model,
                    &env.summary_line(),
                );
                if let Some(reason) = exec::risk_warning(cmd) {
                    render::warning(reason);
                }
                loop {
                    match menu(true)? {
                        MenuAction::Run => {
                            println!();
                            let status = exec::run(&env, cmd)?;
                            return Ok(exec::exit_code(&status));
                        }
                        MenuAction::Copy => {
                            if let Err(e) = copy_to_clipboard(cmd) {
                                render::error(&format!("{e:#}"));
                            } else {
                                render::success("command copied to clipboard");
                            }
                        }
                        MenuAction::Revise => {
                            if revise(&mut history, &suggestion)? {
                                break;
                            }
                        }
                        MenuAction::Cancel => {
                            render::status("cancelled — nothing was executed");
                            return Ok(0);
                        }
                    }
                }
            }
            None => {
                render::print_explanation_panel(
                    suggestion.explanation.as_deref().unwrap_or("(empty reply)"),
                    &model,
                    &env.summary_line(),
                );
                loop {
                    match menu(false)? {
                        MenuAction::Revise => {
                            if revise(&mut history, &suggestion)? {
                                break;
                            }
                        }
                        MenuAction::Cancel => {
                            render::status("cancelled");
                            return Ok(0);
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

fn gather_prompt(args: &Cli) -> Result<String> {
    if !args.prompt.is_empty() {
        return Ok(args.prompt.join(" "));
    }
    if !std::io::stdin().is_terminal() {
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .context("failed to read the prompt from stdin")?;
        return Ok(buf.trim().to_string());
    }
    Err(anyhow::anyhow!(
        "no prompt given — usage: rusthrc \"<prompt>\"  (or pipe it: echo \"<prompt>\" | rusthrc)"
    ))
}

fn print_json(suggestion: &Suggestion, model: &str, env: &Environment) {
    let out = HeadlessOutput {
        command: suggestion.command.as_deref(),
        explanation: suggestion.explanation.as_deref(),
        model,
        context: ContextSummary {
            os: env.os,
            os_pretty: &env.os_pretty,
            arch: env.arch,
            shell: &env.shell,
            cwd: &env.cwd,
        },
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&out).unwrap_or_else(|_| "{}".to_string())
    );
}

#[derive(Serialize)]
struct HeadlessOutput<'a> {
    command: Option<&'a str>,
    explanation: Option<&'a str>,
    model: &'a str,
    context: ContextSummary<'a>,
}

#[derive(Serialize)]
struct ContextSummary<'a> {
    os: &'a str,
    os_pretty: &'a str,
    arch: &'a str,
    shell: &'a str,
    cwd: &'a str,
}

#[derive(Clone, Copy)]
enum MenuAction {
    Run,
    Copy,
    Revise,
    Cancel,
}

impl std::fmt::Display for MenuAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            MenuAction::Run => "Run",
            MenuAction::Copy => "Copy to clipboard",
            MenuAction::Revise => "Revise",
            MenuAction::Cancel => "Cancel",
        })
    }
}

fn menu(has_command: bool) -> Result<MenuAction> {
    let options: Vec<MenuAction> = if has_command {
        vec![
            MenuAction::Run,
            MenuAction::Copy,
            MenuAction::Revise,
            MenuAction::Cancel,
        ]
    } else {
        vec![MenuAction::Revise, MenuAction::Cancel]
    };
    match inquire::Select::new("What next?", options)
        .with_help_message("↑/↓ to move · Enter to select · Esc to cancel")
        .with_render_config(inquire::ui::RenderConfig::default_colored())
        .prompt()
    {
        Ok(action) => Ok(action),
        Err(inquire::InquireError::OperationCanceled)
        | Err(inquire::InquireError::OperationInterrupted) => Ok(MenuAction::Cancel),
        Err(e) => Err(e.into()),
    }
}

/// Returns true when the model should regenerate from the added revision turn.
fn revise(history: &mut Vec<Turn>, suggestion: &Suggestion) -> Result<bool> {
    let instruction = match inquire::Text::new("Describe the change")
        .with_help_message("e.g. 'also include hidden files' — Esc to cancel")
        .prompt()
    {
        Ok(text) if !text.trim().is_empty() => text.trim().to_string(),
        Ok(_) => {
            render::status("empty revision — keeping the current command");
            return Ok(false);
        }
        Err(inquire::InquireError::OperationCanceled)
        | Err(inquire::InquireError::OperationInterrupted) => {
            render::status("revision cancelled");
            return Ok(false);
        }
        Err(e) => return Err(e.into()),
    };
    history.push(Turn::Assistant(suggestion.raw.clone()));
    history.push(Turn::User(format!(
        "Please revise the previous command: {instruction}\nReply again with exactly one fenced code block."
    )));
    Ok(true)
}

fn copy_to_clipboard(command: &str) -> Result<()> {
    let mut clipboard =
        arboard::Clipboard::new().context("no clipboard is available in this session")?;
    clipboard
        .set_text(command.to_string())
        .context("failed to copy to the clipboard")?;
    Ok(())
}

fn store_key_command(key: Option<String>, store: KeyStore) -> i32 {
    let key = match key {
        Some(k) if !k.trim().is_empty() => k.trim().to_string(),
        _ => match inquire::Password::new("OpenAI API key")
            .with_help_message("input is hidden; keys usually look like `sk-…`")
            .prompt()
        {
            Ok(k) if !k.trim().is_empty() => k.trim().to_string(),
            Ok(_) => {
                render::error("empty key — aborted");
                return 1;
            }
            Err(_) => {
                render::status("cancelled");
                return 1;
            }
        },
    };
    match config::store_api_key(&key, store) {
        Ok(where_) => {
            let hint = match store {
                KeyStore::Keyring => " (service: rusthrc, account: openai-api-key)",
                KeyStore::File => "",
            };
            render::success(&format!("API key stored in {where_}{hint}"));
            0
        }
        Err(e) => {
            render::error(&format!("could not store the API key: {e:#}"));
            eprintln!(
                "  hint: use `--store file` to keep the key in the config file, \
                 or export OPENAI_API_KEY per session"
            );
            1
        }
    }
}

fn logout() -> i32 {
    match config::clear_api_key() {
        Ok(notes) => {
            for note in notes {
                render::status(&note);
            }
            render::success("API key removed");
            0
        }
        Err(e) => {
            render::error(&format!("{e:#}"));
            1
        }
    }
}

fn config_command(cmd: ConfigCmd) -> i32 {
    match cmd {
        ConfigCmd::SetKey { key, store } => store_key_command(key, store),
        ConfigCmd::SetModel { model } => {
            let mut cfg = config::load();
            cfg.model = Some(model.clone());
            match config::save(&cfg) {
                Ok(()) => {
                    render::success(&format!("default model set to `{model}`"));
                    0
                }
                Err(e) => {
                    render::error(&format!("{e:#}"));
                    1
                }
            }
        }
        ConfigCmd::SetBaseUrl { url } => {
            let mut cfg = config::load();
            cfg.api_base = Some(url.clone());
            match config::save(&cfg) {
                Ok(()) => {
                    render::success(&format!("API base URL set to `{url}`"));
                    0
                }
                Err(e) => {
                    render::error(&format!("{e:#}"));
                    1
                }
            }
        }
        ConfigCmd::Show => show_config(),
    }
}

fn show_config() -> i32 {
    let cfg = config::load();
    println!("{}", "rusthrc configuration".cyan().bold());
    println!("  config file: {}", config::config_path().display());
    println!(
        "  model:       {}",
        cfg.model
            .clone()
            .unwrap_or_else(|| format!("{} (default)", config::DEFAULT_MODEL))
    );
    println!(
        "  api base:    {}",
        cfg.api_base
            .clone()
            .unwrap_or_else(|| format!("{} (default)", config::DEFAULT_BASE_URL))
    );

    let (stored, warnings) = config::resolve_api_key();
    for w in &warnings {
        println!("  {}", w.yellow());
    }
    match stored {
        Some(key) => println!(
            "  api key:     {} (from {})",
            render::mask_key(&key.value).green(),
            key.source
        ),
        None => println!(
            "  api key:     {}",
            "not set — run `rusthrc login`".yellow()
        ),
    }

    let env = Environment::sniff();
    println!();
    println!("{}", "detected environment".cyan().bold());
    println!("  os:      {} ({}, {})", env.os_pretty, env.os, env.arch);
    println!(
        "  shell:   {} ({})",
        env.shell,
        if env.shell_path.is_empty() { "default" } else { &env.shell_path }
    );
    println!("  cwd:     {}", env.cwd);
    println!("  entries: {}", env.cwd_summary);
    0
}
