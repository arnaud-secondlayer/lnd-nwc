use clap::{Arg, Command};

mod config;
mod grpc;
mod uri;

use uri::{create_and_save, remove_and_save, load_and_display};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let matches = cli().get_matches();

    match matches.subcommand() {
        Some(("deamon", _)) => {
            println!("Starting deamon");
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

    // let uri = uri::create_uri(&"wss://relay.damus.io".to_string());
    // println!("{}", uri);
    // let info = grpc::get_info().await;
    // println!("{:?}", info);
    Ok(())
}

fn cli() -> Command {
    Command::new("git")
        .about("A fictional versioning CLI")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .allow_external_subcommands(true)
        .subcommand(Command::new("deamon").about("Starts the deamon"))
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
