use confy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct NostrConfig {
    pub secret: String,
}

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct LndConfig {
    pub uri: String,
    pub cert_file: PathBuf,
    pub macaroon_file: PathBuf,
}

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct Config {
    pub nostr: NostrConfig,
    pub uris: HashMap<String, String>,
    pub lnd: LndConfig,
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
