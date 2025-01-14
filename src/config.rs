use std::cmp::PartialEq;
use std::collections::HashMap;
use std::net::{IpAddr};
use std::sync::Arc;
use log::{error};
use mail_send::{Credentials, SmtpClient, SmtpClientBuilder};
use serde::{Deserialize, Serialize};
use tokio::net::{TcpStream};
use tokio::sync::Mutex;
use tokio_rustls::client::TlsStream;

use crate::manager::NotificationType;

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    pub(crate) smtp: Option<EmailConfig>,
    pub(crate) notifications: Option<HashMap<String, NotificationDestinations>>,
    pub(crate) auth: Option<AuthConfig>,
    pub(crate) printers: HashMap<String, PrinterConfig>
}

pub struct ConfigManager {
    config: Config,
    mailer: Option<Arc<Mutex<Mailer>>>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct NotificationDestinations {
    pub(crate) emails: Option<Vec<String>>,
    pub(crate) webhooks: Option<Vec<String>>
}

pub type Mailer = SmtpClient<TlsStream<TcpStream>>;

#[allow(unused)]
impl ConfigManager {
    pub async fn load() -> Self {
        let config = toml::from_str(&std::fs::read_to_string("config.toml").expect("could not read config.toml file")).map_err(|e| {
            error!("Failed to parse config.toml: {} span={:?}", e.message(), e.span());
            std::process::exit(1);
        }).unwrap();
        let mut s = ConfigManager {
            config,
            mailer: None
        };
        match s.setup_mailer().await {
            Ok(Some(m)) => { s.mailer = Some(Arc::new(Mutex::new(m))); },
            Err(e) => {
                error!("Failed to setup mailer: {}", e);
            }
            _ => {}
        }
        s
    }

    pub fn smtp(&self) -> Option<&EmailConfig> {
        self.config.smtp.as_ref()
    }

    pub fn get_notification_destinations(&self, notification_type: &NotificationType) -> Option<&NotificationDestinations> {
        if let Some(notifications) = &self.config.notifications {
            let key = match notification_type {
                NotificationType::PrintComplete => { "on_done" },
                _ => return None
            };
            return notifications.get(key)
        }
        None
    }

    pub fn auth(&self) -> Option<&AuthConfig> {
        self.config.auth.as_ref()
    }

    pub fn printers(&self) -> &HashMap<String, PrinterConfig> {
        &self.config.printers
    }

    pub fn mailer(&self) -> Option<Arc<Mutex<Mailer>>> {
        self.mailer.as_ref().map(|m| m.clone())
    }

    /// Sets up SMTP mailer, if configured. Ok(None) if not setup, Err if invalid configuration
    async fn setup_mailer(&mut self) -> Result<Option<Mailer>, String> {
        if let Some(smtp) = &self.config.smtp {
            if smtp.port == 0 {
               Err("SMTP: Smtp port is invalid, smtp support not enabled".to_string())
            } else if smtp.user.is_empty() {
                Err("SMTP: Smtp user is empty, smtp support not enabled".to_string())
            } else if smtp.host.is_empty() {
                Err("SMTP: Smtp host is empty, smtp support not enabled".to_string())
            } else {
                let client = SmtpClientBuilder::new(&smtp.host, smtp.port)
                    .implicit_tls(smtp.encryption == EmailEncryption::Tls)
                    .credentials(Credentials::new(&smtp.user, &smtp.password))
                    .connect()
                    .await
                    .unwrap();
                Ok(Some(client))
            }
        } else {
            Ok(None)
        }
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum EmailEncryption {
    None,
    StartTls,
    Tls
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AuthConfig {
    pub(crate)password_for_write: bool,
    pub(crate)password_for_read: bool,
    pub(crate)password: String
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PrinterConfig {
    pub(crate) ip: IpAddr
}

