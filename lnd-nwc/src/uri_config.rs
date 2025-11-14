use confy;

use crate::config::Config;
use crate::uri::create_uri;

// Config is stored in
// Linux: $XDG_CONFIG_HOME/<project_path> or $HOME/.config/<project_path>
// OSX: $HOME/.config/<project_path>
// Windows: {FOLDERID_RoamingAppData}/<project_path>/config

pub fn load_and_display() -> Result<(), Box<dyn std::error::Error>> {
    let cfg: Config = confy::load("lnd-nwc", None)?;

    println!("Names and URIs:");
    if cfg.uris.is_empty() {
        println!("\tEmpty");
    } else {
        for uri in cfg.uris {
            println!("\t{}: {}", uri.0, uri.1);
        }
    }
    println!("");

    Ok(())
}

pub fn create_and_save(name: &str, relay: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut cfg: Config = confy::load("lnd-nwc", None)?;
    if cfg.uris.contains_key(name) {
        panic!("Uri name `{name}`already exists, remove it or use another one")
    }

    let new_uri = create_uri(relay);
    let _ = &cfg.uris.insert(name.into(), new_uri.clone());

    confy::store("lnd-nwc", None, cfg)?;

    println!("New URI created for {name}:\n{new_uri}");

    Ok(())
}

pub fn remove_and_save(name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut cfg: Config = confy::load("lnd-nwc", None)?;
    if cfg.uris.contains_key(name) == false {
        panic!("Uri name `{name}` does not exist")
    }

    let _ = &cfg.uris.remove(name);

    confy::store("lnd-nwc", None, cfg)?;

    println!("Removed URI for {name}");

    Ok(())
}
