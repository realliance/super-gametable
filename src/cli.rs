use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Perform a health check and exit
    #[clap(long)]
    pub health_check: bool,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Run the gametable service
    Service,
    /// Access various tools
    Tools {
        #[command(subcommand)]
        tool: Tool,
    },
}

#[derive(Subcommand, Debug)]
pub enum Tool {
    /// Queue a match and wait for the result
    QueueMatch {
        /// The players to include in the match
        #[clap(required = true, num_args = 1..=4)]
        players: Vec<String>,
    },
}
