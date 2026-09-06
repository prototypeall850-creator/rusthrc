use std::{env, fs, path::Path};

#[derive(Debug, Clone)]
pub struct Environment {
    pub os: &'static str,
    pub arch: &'static str,
    pub os_pretty: String,
    pub shell: String,
    pub shell_path: String,
    pub cwd: String,
    /// Comma-separated snapshot of top-level directory entries, capped.
    pub cwd_summary: String,
}

impl Environment {
    pub fn sniff() -> Self {
        let os = env::consts::OS;
        let arch = env::consts::ARCH;
        let os_pretty = pretty_os();
        let (shell, shell_path) = detect_shell();
        let cwd = env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| "?".to_string());
        let cwd_summary = list_dir_summary(&cwd);
        Self {
            os,
            arch,
            os_pretty,
            shell,
            shell_path,
            cwd,
            cwd_summary,
        }
    }

    pub fn summary_line(&self) -> String {
        format!("{} · {} · {}", self.os_pretty, self.arch, self.shell)
    }
}

#[cfg(target_os = "linux")]
fn pretty_os() -> String {
    fs::read_to_string("/etc/os-release")
        .ok()
        .and_then(|body| {
            body.lines().find_map(|line| {
                line.strip_prefix("PRETTY_NAME=")
                    .map(|v| v.trim_matches('"').to_string())
            })
        })
        .unwrap_or_else(|| "Linux".to_string())
}

#[cfg(target_os = "macos")]
fn pretty_os() -> String {
    "macOS".to_string()
}

#[cfg(target_os = "windows")]
fn pretty_os() -> String {
    "Windows".to_string()
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn pretty_os() -> String {
    env::consts::OS.to_string()
}

#[cfg(unix)]
fn detect_shell() -> (String, String) {
    let shell_path = env::var("SHELL").unwrap_or_default();
    let shell = Path::new(&shell_path)
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    if shell.is_empty() {
        ("sh".to_string(), String::new())
    } else {
        (shell, shell_path)
    }
}

#[cfg(windows)]
fn detect_shell() -> (String, String) {
    let comspec = env::var("COMSPEC").unwrap_or_default();
    let name = Path::new(&comspec)
        .file_name()
        .map(|s| s.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if name.contains("powershell") || name.contains("pwsh") {
        (name, comspec)
    } else {
        // PowerShell is the default target for generated commands on Windows.
        ("powershell".to_string(), "powershell".to_string())
    }
}

#[cfg(not(any(unix, windows)))]
fn detect_shell() -> (String, String) {
    ("sh".to_string(), String::new())
}

fn list_dir_summary(cwd: &str) -> String {
    const CAP: usize = 40;
    let mut names: Vec<String> = Vec::new();
    if let Ok(entries) = fs::read_dir(cwd) {
        for entry in entries.flatten().take(CAP) {
            let mut name = entry.file_name().to_string_lossy().to_string();
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                name.push('/');
            }
            names.push(name);
        }
    }
    names.sort();
    if names.is_empty() {
        "(empty directory)".to_string()
    } else if names.len() >= CAP {
        format!("{}, … (truncated)", names[..CAP - 1].join(", "))
    } else {
        names.join(", ")
    }
}
