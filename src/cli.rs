use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(author, version, about)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    #[arg(index = 1, help = "URI to open")]
    pub uri: Option<String>,
}

#[derive(Subcommand)]
pub enum Commands {
    Daemon {
        /// Set the default application index on fallback
        #[arg(long)]
        set_default: Option<u64>,

        /// Unset the default application on fallback, will open the UI instead
        #[arg(long, required = false)]
        unset_default: bool,

        /// Print status of the daemon
        #[arg(long, required = false)]
        status: bool,

        /// Kill the daemon
        #[arg(long, required = false)]
        kill: bool,

        /// Set the next default application index
        /// If no default application is set yet, this will set the first application as default
        /// If this is the last application, it will unset the default application
        #[arg(long, required = false)]
        set_default_next: bool,

        /// Waybar helper
        #[arg(long, required = false)]
        waybar: bool,
    },
}

pub fn parse() -> Cli {
    Cli::parse()
}
