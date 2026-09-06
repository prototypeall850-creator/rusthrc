use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser, Debug)]
#[command(
    name = "rusthrc",
    bin_name = "rusthrc",
    version,
    about = "AI terminal assistant — turns natural language into shell commands",
    long_about = "rusthrc is a locally executed AI coding assistant inspired by the OpenAI Codex CLI.\n\
                  Describe what you want in plain English (or Indonesian) and rusthrc drafts the\n\
                  shell command for you: review it, run it, copy it, or ask for a revision.",
    after_help = "Examples:\n  rusthrc \"list all hidden files\"\n  echo \"show disk usage\" | rusthrc --json\n  rusthrc --base-url http://localhost:11434/v1 -m llama3 \"compress this folder\""
)]
pub struct Cli {
    /// Natural-language description of the command you want
    pub prompt: Vec<String>,

    /// Print the generated command as JSON (headless mode)
    #[arg(long)]
    pub json: bool,

    /// Print the generated command only, without the interactive menu
    #[arg(short = 'p', long)]
    pub print: bool,

    /// Model used for generation (overrides `config set-model`)
    #[arg(short = 'm', long)]
    pub model: Option<String>,

    /// OpenAI-compatible API base URL (overrides `config set-base-url`)
    #[arg(long = "base-url")]
    pub base_url: Option<String>,

    #[command(subcommand)]
    pub command: Option<Subcommands>,
}

#[derive(Subcommand, Debug)]
pub enum Subcommands {
    /// Store the OpenAI API key for future runs
    Login {
        /// The API key (you will be prompted securely if omitted)
        key: Option<String>,
        /// Where to persist the key
        #[arg(long, value_enum, default_value_t = KeyStore::Keyring)]
        store: KeyStore,
    },
    /// Remove the stored API key
    Logout,
    /// Manage local configuration
    Config {
        #[command(subcommand)]
        cmd: ConfigCmd,
    },
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyStore {
    /// OS-native credential manager (GNOME Keyring, macOS Keychain, Windows Credential Manager)
    Keyring,
    /// Plain config file (chmod 600) — for headless machines without a keyring
    File,
}

#[derive(Subcommand, Clone, Debug)]
pub enum ConfigCmd {
    /// Save the API key (prompts securely if KEY is omitted)
    SetKey {
        key: Option<String>,
        #[arg(long, value_enum, default_value_t = KeyStore::Keyring)]
        store: KeyStore,
    },
    /// Set the default model
    SetModel { model: String },
    /// Set a custom OpenAI-compatible API base URL
    SetBaseUrl { url: String },
    /// Show configuration and detected environment context
    Show,
}
