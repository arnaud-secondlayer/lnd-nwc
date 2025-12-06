use clap::{Parser, Subcommand};
use std::fmt;

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
    Daemon,
    Test {
        #[command(subcommand)]
        action: TestAction,
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

#[derive(Subcommand)]
enum TestAction {
    Info,
    Balance,
}

impl fmt::Display for TestAction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            TestAction::Info => write!(f, "info"),
            TestAction::Balance => write!(f, "balance"),
        }
    }
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
        Commands::Daemon => {
            let keys = nostr_config::load_or_generate_keys().expect("Could not retrieve keys");
            let _ = nostr::start_deamon(keys).await;
        }
        Commands::Test { action } => {
            let _ = nostr::test(&action.to_string()).await;
        }
    }

    Ok(())
}
