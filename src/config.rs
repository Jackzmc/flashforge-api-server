use std::collections::HashMap;
use std::net::IpAddr;
use log::{debug, error};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    pub(crate) smtp: Option<EmailConfig>,
    pub(crate) notifications: Option<NotificationConfig>,
    pub(crate) general: Option<GeneralConfig>,
    pub(crate) printers: HashMap<String, PrinterConfig>
}

impl Config {
    pub fn load() -> Self {
        let config = toml::from_str(&std::fs::read_to_string("config.toml").expect("could not read config.toml file")).map_err(|e| {
            error!("Failed to parse config.toml: {} span={:?}", e.message(), e.span());
            std::process::exit(1);
        }).unwrap();
        config
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "lowercase")]
pub enum EmailEncryption {
    None,
    StartTLS,
    TLS
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EmailConfig {
    pub(crate) host: String,
    pub(crate) port: u16,
    pub(crate) encryption: EmailEncryption,
    pub(crate) user: String,
    pub(crate) password: String
}
#[derive(Debug, Serialize, Deserialize)]
pub struct NotificationConfig {
    pub(crate) emails: Option<Vec<String>>,

    pub(crate) on_done: Option<Vec<String>>
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GeneralConfig {
    write_password: Option<String>
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PrinterConfig {
    pub(crate) ip: IpAddr
}

