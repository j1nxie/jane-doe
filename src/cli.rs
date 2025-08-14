use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,

    /// Should we log debugging things?
    #[arg(global = true, long)]
    pub debug: bool,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Start the bot.
    Start,
    /// Run the scrape task.
    Scrape,
}
