use clap::{Parser, Subcommand};

mod config;

use nostr_sdk::prelude::*;
use nwc::prelude::*;

use crate::config::load_config;

#[derive(Parser)]
#[command(name = "test")]
#[command(about = "A test executable")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Info,
    Balance,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().init();

    let cli = Cli::parse();
    let _ = test(cli.command).await;

    Ok(())
}

async fn test(command: Commands) -> Result<()> {
    let cfg = load_config();
    let uri = NostrWalletConnectURI::parse(cfg.uris.get("test").unwrap()).unwrap();
    let nwc = NWC::new(uri);

    tracing::info!("Test for {nwc:?}");

    match command {
        Commands::Info => {
            let response = nwc.get_info().await;
            tracing::info!("Supported methods: {:?}", response);
        }
        Commands::Balance => {
            let response = nwc.get_balance().await.expect("Could not get balance");
            tracing::info!("Supported methods: {:?}", response);
        }
    }

    Ok(())
}
