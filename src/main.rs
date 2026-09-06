mod app;
mod cli;
mod config;
mod env;
mod exec;
mod llm;
mod render;

use clap::Parser;

#[tokio::main]
async fn main() -> std::process::ExitCode {
    let args = cli::Cli::parse();
    let code = app::dispatch(args).await;
    std::process::ExitCode::from(code.clamp(0, 255) as u8)
}
