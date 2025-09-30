//! Flight Hub CLI

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "flightctl")]
#[command(about = "Flight Hub command line interface")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List connected devices
    Devices,
    /// Show system status
    Status,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Devices => {
            println!("Listing devices...");
            // TODO: Implement device listing
        }
        Commands::Status => {
            println!("System status: OK");
            // TODO: Implement status check
        }
    }

    Ok(())
}
