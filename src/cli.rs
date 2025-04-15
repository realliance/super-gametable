use clap::{Parser, Subcommand};

#[derive(Subcommand)]
pub enum TableModes {
    /// Single match execution mode.
    /// The first four bots to connect are entered into a match together.
    SingleMatch,
}

/// CLI Options
#[derive(Parser)]
pub struct Cli {
    #[clap(subcommand)]
    pub command: TableModes,
}
