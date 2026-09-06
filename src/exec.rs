use anyhow::{anyhow, Result};
use std::process::{Command, ExitStatus};

use crate::env::Environment;

/// (pattern, reason) pairs matched case-insensitively against the command.
const RISK_RULES: &[(&str, &str)] = &[
    ("sudo", "runs with elevated privileges"),
    ("rm -rf", "recursively force-deletes files"),
    ("rm -fr", "recursively force-deletes files"),
    ("mkfs", "formats a filesystem"),
    ("dd if=", "writes raw blocks to a device"),
    ("> /dev/", "writes directly to a device"),
    ("chmod -r 777", "makes files world-writable"),
    (":(){:|:&};:", "is a fork bomb"),
    ("shutdown", "shuts the machine down"),
    ("reboot", "reboots the machine"),
    ("git push --force", "force-overwrites remote history"),
    ("git reset --hard", "discards uncommitted changes"),
];

const PIPED_SHELL_PATTERNS: &[&str] = &["| sh", "|sh", "| bash", "|bash", "| sudo sh", "| sudo bash"];

pub fn risk_warning(command: &str) -> Option<&'static str> {
    let lower = command.to_lowercase();
    for (pattern, reason) in RISK_RULES {
        if lower.contains(pattern) {
            return Some(reason);
        }
    }
    if (lower.contains("curl") || lower.contains("wget"))
        && PIPED_SHELL_PATTERNS.iter().any(|p| lower.contains(p))
    {
        return Some("pipes a remote script straight into the shell");
    }
    None
}

/// Run `command` through the user's shell with inherited stdio, so output
/// streams live and interactive programs still work.
pub fn run(env: &Environment, command: &str) -> Result<ExitStatus> {
    let (program, args) = invocation(env, command);
    match Command::new(&program).args(&args).status() {
        Ok(status) => Ok(status),
        Err(e) => fallback_or_err(&program, command, e),
    }
}

#[cfg(unix)]
fn invocation(env: &Environment, command: &str) -> (String, Vec<String>) {
    let program = if env.shell_path.trim().is_empty() {
        "/bin/sh".to_string()
    } else {
        env.shell_path.clone()
    };
    (program, vec!["-c".to_string(), command.to_string()])
}

#[cfg(windows)]
fn invocation(env: &Environment, command: &str) -> (String, Vec<String>) {
    if env.shell.to_lowercase().contains("powershell") || env.shell.to_lowercase().contains("pwsh") {
        (
            env.shell_path.clone(),
            vec![
                "-NoProfile".to_string(),
                "-Command".to_string(),
                command.to_string(),
            ],
        )
    } else {
        (
            "cmd".to_string(),
            vec!["/C".to_string(), command.to_string()],
        )
    }
}

#[cfg(unix)]
fn fallback_or_err(program: &str, command: &str, e: std::io::Error) -> Result<ExitStatus> {
    if program != "/bin/sh" {
        if let Ok(status) = Command::new("/bin/sh").arg("-c").arg(command).status() {
            return Ok(status);
        }
    }
    Err(anyhow!("failed to spawn `{program}`: {e}"))
}

#[cfg(not(unix))]
fn fallback_or_err(program: &str, _command: &str, e: std::io::Error) -> Result<ExitStatus> {
    Err(anyhow!("failed to spawn `{program}`: {e}"))
}

pub fn exit_code(status: &ExitStatus) -> i32 {
    status.code().or_else(|| signal_code(status)).unwrap_or(1)
}

#[cfg(unix)]
fn signal_code(status: &ExitStatus) -> Option<i32> {
    use std::os::unix::process::ExitStatusExt;
    status.signal().map(|s| 128 + s)
}

#[cfg(not(unix))]
fn signal_code(_status: &ExitStatus) -> Option<i32> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_dangerous_commands() {
        assert!(risk_warning("sudo rm -rf /").is_some());
        assert!(risk_warning("dd if=/dev/zero of=/dev/sda").is_some());
        assert!(risk_warning("curl -fsSL https://example.com/x.sh | sh").is_some());
        assert!(risk_warning("wget -qO- x.sh|bash").is_some());
    }

    #[test]
    fn benign_commands_pass() {
        assert!(risk_warning("ls -la").is_none());
        assert!(risk_warning("git status && cargo build").is_none());
        assert!(risk_warning("curl https://example.com").is_none());
    }
}
