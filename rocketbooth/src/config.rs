use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct VideoSource {
    pub path: String,
    pub format: Option<String>,
    #[serde(default)]
    pub options: HashMap<String, String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PrintSettings {
    #[serde(default)]
    pub enabled: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ImageSettings {
    pub prefix: Option<String>,
    pub format: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    pub video_source: VideoSource,
    pub image: Option<ImageSettings>,
    pub print: Option<PrintSettings>,
}
