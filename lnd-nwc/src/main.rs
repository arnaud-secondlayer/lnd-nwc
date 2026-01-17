use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod config;
mod lnd;
mod lnd_config;
mod nostr;
mod nostr_config;
mod nwc_types;
mod uri;
mod uri_config;

#[derive(Parser)]
#[command(name = "lnd-nwc")]
#[command(about = "A nostr wallet service")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Daemon {
        #[command(subcommand)]
        action: DaemonAction,
    },
    Lnd {
        #[command(subcommand)]
        action: LndAction,
    },
    Uri {
        #[command(subcommand)]
        action: UriAction,
    },
}

#[derive(Subcommand)]
enum DaemonAction {
    Start {
        #[arg(short = 'p', long)]
        pid_file: String,
    },
    Stop {
        #[arg(short = 'p', long)]
        pid_file: String,
    },
    Status {
        #[arg(short = 'p', long)]
        pid_file: String,
    },
}

#[derive(Subcommand)]
enum LndAction {
    Set {
        #[arg(short = 'c', long)]
        cert: String,
        #[arg(short = 'm', long)]
        macaroon: String,
        #[arg(short = 'u', long)]
        uri: String,
    },
    Info,
}

#[derive(Subcommand)]
enum UriAction {
    Create {
        #[arg(short = 'n', long)]
        name: String,
        #[arg(short = 'r', long)]
        relay: String,
    },
    Remove {
        #[arg(short = 'n', long)]
        name: String,
    },
    List,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Uri { action } => match action {
            UriAction::Create { name, relay } => {
                let _ = nostr_config::load_or_generate_keys().expect("Could not retrieve keys");
                let _ = uri_config::create_and_save(&name, &relay);
            }
            UriAction::Remove { name } => {
                let _ = uri_config::remove_and_save(&name);
            }
            UriAction::List => {
                let _ = uri_config::load_and_display();
            }
        },
        Commands::Lnd { action } => match action {
            LndAction::Set {
                cert,
                macaroon,
                uri,
            } => lnd_config::store(&cert, &macaroon, &uri),
            LndAction::Info => lnd::display_info().await,
        },
        Commands::Daemon { action } => match action {
            DaemonAction::Start { pid_file } => {
                let _ = nostr::start_deamon(
                    nostr_config::load_or_generate_keys().expect("Could not retrieve keys"),
                    &PathBuf::from(pid_file),
                )
                .await;
            }
            DaemonAction::Stop { pid_file } => {
                let _ = nostr::stop_deamon(&PathBuf::from(pid_file));
            }
            DaemonAction::Status { pid_file } => {
                let pb = &PathBuf::from(pid_file);
                let _ = nostr::status_deamon(pb);
            }
        },
    }

    Ok(())
}
