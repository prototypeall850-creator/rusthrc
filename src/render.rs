use colored::Colorize;
use termimad::MadSkin;

/// Diagnostics go to stderr so stdout stays clean for commands and JSON.
pub fn status(msg: &str) {
    eprintln!("{}", format!("· {msg}").dimmed());
}

pub fn clear_status() {
    use std::io::Write;
    eprint!("\r\x1b[2K");
    let _ = std::io::stderr().flush();
}

pub fn success(msg: &str) {
    eprintln!("{}", format!("✓ {msg}").green().bold());
}

pub fn error(msg: &str) {
    eprintln!("{}", format!("✗ {msg}").red().bold());
}

pub fn warning(msg: &str) {
    eprintln!("{}", format!("⚠  caution: this command {msg}").yellow().bold());
}

pub fn print_command_panel(command: &str, explanation: Option<&str>, model: &str, context: &str) {
    println!(
        "{}",
        format!("◆ suggested command · {model} · {context}").cyan().bold()
    );
    println!();
    for line in command.lines() {
        println!("  {} {}", "$".dimmed(), line.white().bold());
    }
    println!();
    print_explanation(explanation);
}

pub fn print_explanation_panel(explanation: &str, model: &str, context: &str) {
    println!(
        "{}",
        format!("◆ model reply · {model} · {context}").cyan().bold()
    );
    println!();
    print_explanation(Some(explanation));
}

fn print_explanation(explanation: Option<&str>) {
    if let Some(text) = explanation {
        if !text.trim().is_empty() {
            MadSkin::default().print_text(text.trim());
            println!();
        }
    }
}

pub fn mask_key(key: &str) -> String {
    let chars: Vec<char> = key.trim().chars().collect();
    if chars.len() <= 10 {
        "•".repeat(chars.len())
    } else {
        let head: String = chars[..7].iter().collect();
        let tail: String = chars[chars.len() - 4..].iter().collect();
        format!("{head}…{tail}")
    }
}
