use confy;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct Config {
    pub secret_key: String,
    pub uris: HashMap<String, String>,
}

// Config is stored in
// Linux: $XDG_CONFIG_HOME/<project_path> or $HOME/.config/<project_path>
// OSX: $HOME/.config/<project_path>
// Windows: {FOLDERID_RoamingAppData}/<project_path>/config

pub fn load_config() -> Config {
    confy::load("lnd-nwc", None).unwrap_or_default()
}

pub fn store_config(config: &Config) {
    confy::store("lnd-nwc", None, config).expect("Could not save the config")
}
