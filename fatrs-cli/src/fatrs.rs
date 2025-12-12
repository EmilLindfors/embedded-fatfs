//! FAT Filesystem CLI Tool - Main Entry Point

use anyhow::Result;
use clap::Parser;

mod cli;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    let cli = cli::Cli::parse();
    cli::run(cli).await
}
