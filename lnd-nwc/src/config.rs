use std::collections::HashMap;
use serde::{Serialize, Deserialize};

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct Config {
    pub uris: HashMap<String, String>
}
