use clap::{Arg, Command};

mod config;
mod lnd;
mod lnd_config;
mod nostr;
mod nostr_config;
mod uri;
mod uri_config;

use lnd::lnd_display_info;
use lnd_config::store_lnd_config;
use nostr::start_deamon;
use nostr_config::load_or_generate_keys;
use uri_config::{create_and_save, load_and_display, remove_and_save};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().init();

    let matches = cli().get_matches();

    match matches.subcommand() {
        Some(("deamon", _)) => {
            let keys = load_or_generate_keys().expect("Could not retrieve keys");
            start_deamon(keys).await;
        }
        Some(("lnd", sub_matches)) => {
            let command = sub_matches.subcommand().unwrap_or(("", sub_matches));
            match command {
                ("info", _) => {
                    lnd_display_info().await;
                }
                ("set", sub_matches) => {
                    store_lnd_config(
                        sub_matches.get_one::<String>("cert").unwrap(),
                        sub_matches.get_one::<String>("macaroon").unwrap(),
                        sub_matches.get_one::<String>("uri").unwrap(),
                    );
                }
                (name, _) => {
                    unreachable!("Unsupported subcommand `{name}`")
                }
            }
        }
        Some(("uri", sub_matches)) => {
            let command = sub_matches.subcommand().unwrap_or(("", sub_matches));
            match command {
                ("create", sub_matches) => {
                    let default_relay = "wss://relay.damus.io".to_string();
                    let name = sub_matches.get_one::<String>("name").unwrap();
                    let relay = sub_matches
                        .get_one::<String>("relay")
                        .unwrap_or(&default_relay);
                    let _ = create_and_save(name, relay);
                }
                ("remove", sub_matches) => {
                    let name = sub_matches.get_one::<String>("name").unwrap();
                    let _ = remove_and_save(name);
                }
                ("list", _) => {
                    let _ = load_and_display();
                }
                (name, _) => {
                    unreachable!("Unsupported subcommand `{name}`")
                }
            }
        }
        Some((ext, sub_matches)) => {
            let args = sub_matches
                .get_many::<String>("")
                .into_iter()
                .flatten()
                .collect::<Vec<_>>();
            println!("Calling out to {ext:?} with {args:?}");
        }
        _ => unreachable!(), // If all subcommands are defined above, anything else is unreachable!()
    };

    Ok(())
}

fn cli() -> Command {
    Command::new("lnd-nwc")
        .about("A nostr waller service")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .allow_external_subcommands(true)
        .subcommand(Command::new("deamon").about("Starts the deamon"))
        .subcommand(
            Command::new("lnd")
                .args_conflicts_with_subcommands(true)
                .flatten_help(true)
                .subcommand(Command::new("info"))
                .subcommand(
                    Command::new("set")
                        .arg(
                            Arg::new("cert")
                                .short('c')
                                .help("path to the certificate file")
                                .required(true),
                        )
                        .arg(
                            Arg::new("macaroon")
                                .short('m')
                                .help("path to the macaroon file")
                                .required(true),
                        )
                        .arg(
                            Arg::new("uri")
                                .short('u')
                                .help("lnd server uri")
                                .required(true),
                        ),
                ),
        )
        .subcommand(
            Command::new("uri")
                .args_conflicts_with_subcommands(true)
                .flatten_help(true)
                .subcommand(
                    Command::new("create")
                        .arg(
                            Arg::new("relay")
                                .short('r')
                                .help("the nost relay URI")
                                .required(false),
                        )
                        .arg(
                            Arg::new("name")
                                .short('n')
                                .help("the uri name")
                                .required(true),
                        ),
                )
                .subcommand(
                    Command::new("remove").arg(
                        Arg::new("name")
                            .short('n')
                            .help("the uri name")
                            .required(true),
                    ),
                )
                .subcommand(Command::new("list")),
        )
}
