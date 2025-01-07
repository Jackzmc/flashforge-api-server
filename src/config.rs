use std::collections::HashMap;
use std::net::IpAddr;
use lettre::SmtpTransport;
use lettre::transport::smtp::authentication::Credentials;
use log::{debug, error};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    pub(crate) smtp: Option<EmailConfig>,
    pub(crate) notifications: Option<NotificationConfig>,
    pub(crate) general: Option<GeneralConfig>,
    pub(crate) printers: HashMap<String, PrinterConfig>
}

pub struct ConfigManager {
    config: Config,
    mailer: Option<SmtpTransport>,
}


impl ConfigManager {
    pub fn load() -> Self {
        let config = toml::from_str(&std::fs::read_to_string("config.toml").expect("could not read config.toml file")).map_err(|e| {
            error!("Failed to parse config.toml: {} span={:?}", e.message(), e.span());
            std::process::exit(1);
        }).unwrap();
        let mut s = ConfigManager {
            config,
            mailer: None
        };
        match s.setup_mailer() {
            Ok(m) => { s.mailer = m },
            Err(e) => {
                error!("Failed to setup mailer: {}", e);
            }
        }
        s
    }

    pub fn smtp(&self) -> Option<&EmailConfig> {
        self.config.smtp.as_ref()
    }

    pub fn notifications(&self) -> Option<&NotificationConfig> {
        self.config.notifications.as_ref()
    }

    pub fn general(&self) -> Option<&GeneralConfig> {
        self.config.general.as_ref()
    }

    pub fn printers(&self) -> &HashMap<String, PrinterConfig> {
        &self.config.printers
    }

    pub fn mailer(&self) -> Option<&SmtpTransport> {
        self.mailer.as_ref()
    }

    /// Sets up SMTP mailer, if configured. Ok(None) if not setup, Err if invalid configuration
    fn setup_mailer(&mut self) -> Result<Option<SmtpTransport>, &str> {
        if let Some(smtp) = &self.config.smtp {
            if smtp.port <= 0 {
               Err("SMTP: Smtp port is invalid, smtp support not enabled")
            } else if smtp.user == "" {
                Err("SMTP: Smtp user is empty, smtp support not enabled")
            } else if smtp.host == "" {
                Err("SMTP: Smtp host is empty, smtp support not enabled")
            } else {
                let builder = match smtp.encryption {
                    EmailEncryption::None => SmtpTransport::builder_dangerous(&smtp.host),
                    EmailEncryption::StartTLS => SmtpTransport::starttls_relay(&smtp.host).unwrap(),
                    EmailEncryption::TLS => SmtpTransport::relay(&smtp.host).unwrap()
                };
                Ok(Some(builder
                    .port(smtp.port)
                    .credentials(Credentials::new(smtp.user.to_string(), smtp.password.to_string()))
                    .build()
                ))
            }
        } else {
            Ok(None)
        }
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

