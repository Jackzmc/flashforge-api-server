use crate::config::ConfigManager;
use crate::printer::Printer;
use lettre::message::header::ContentType;
use lettre::message::Mailbox;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Address, Message, SmtpTransport, Transport};
use log::{debug, error, trace, warn};
use serde_json::json;
use std::collections::HashMap;
use std::fmt::Write;
use std::net::IpAddr;
use std::ops::Not;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

static PROGRESS_CHECK_INTERVAL: Duration = Duration::from_secs(60);

pub type PrinterManager = Arc<Mutex<Printers>>;

#[derive(Debug)]
pub enum NotificationType {
    PrintComplete
}

impl NotificationType {
    pub fn get_subject(&self, printer: &Printer) -> String {
        match self {
            NotificationType::PrintComplete => format!("Print complete on {}", printer.name()),
            _ => printer.name().to_string()
        }
    }

    pub fn get_message(&self, printer: &Printer) -> String {
        match self {
            NotificationType::PrintComplete => {
                let status = printer.get_status().unwrap();
                let mut str = String::new();
                write!(str, "File: {}\n", status.current_file.unwrap_or("unknown".to_string())).unwrap();
                write!(str, "IP: {}\n", printer.ip()).unwrap();
                str
                // TODO: send an image?
            }
            _ => "".to_string()
        }
    }
}

type PrinterContainer = Arc<Mutex<Printer>>;

pub struct Printers {
    printers: HashMap<String, PrinterContainer>,
    config: Arc<ConfigManager>,
    notification_sent: HashMap<String, String>, // If printer (key) has value, then a print done notification has been submitted for file (value
}

impl Printers {
    pub fn new(config: Arc<ConfigManager>) -> Printers {
        Self {
            printers: HashMap::new(),
            config,
            notification_sent: HashMap::new()
        }
    }

    pub fn start_watch_thread(manager: PrinterManager) {
        debug!("Starting watch thread at interval {:?}", PROGRESS_CHECK_INTERVAL);
        std::thread::spawn(move || {
            std::thread::sleep(PROGRESS_CHECK_INTERVAL);
            loop {
                // Grab list of printers
                trace!("Getting list of printers");
                let (printers, mut sent_notifications) = {
                    let mut lock = manager.lock().unwrap();
                    (lock.printers(), lock.notification_sent.clone())
                };

                trace!("Checking printers");
                // Update status of all printers, and check if there is a notification we need to send
                let notify_queue: Vec<&PrinterContainer> = printers.iter().filter(|printer| {
                    let mut printer = printer.lock().unwrap();
                    if printer.refresh_status().is_ok() {
                        if printer.current_file().is_none() { return false; }
                        let prog = printer.get_progress().unwrap();
                        // Check if progress is 100%
                        trace!("printer {} layer={:?} byte={:?}", printer.name(), prog.layer, prog.byte);
                        if prog.layer.0 >= prog.layer.1 {
                            // Get current file from status
                            let status = printer.get_status().unwrap();
                            if status.current_file.is_none() {
                                return false;
                            }
                            // Check if we have already sent a notification
                            let current_file = status.current_file.unwrap();
                            let has_notified = sent_notifications.get(printer.name()).unwrap_or(&"".to_string()) == &current_file;

                            if !has_notified {
                                debug!("will notify for printer {}", printer.name());
                                return true;
                                // lock.send_notification(printer, NotificationType::PrintComplete);
                                // has_sent.insert(id.clone(), current_file);
                            }
                        }
                    }
                    false
                }).collect();

                // trace!("Sending any notifications");
                // Send notifications to any as needed
                {
                    let mut lock = manager.lock().unwrap();
                    for printer in notify_queue {
                        let printer = printer.lock().unwrap();
                        lock.send_notification(&*printer, NotificationType::PrintComplete);

                        let current_file = printer.current_file().as_ref().unwrap().clone();
                        sent_notifications.insert(printer.name().to_string(), current_file);
                    }
                }
                std::thread::sleep(PROGRESS_CHECK_INTERVAL);
            }
        });
    }

    fn has_notified(&self, printer_id: &str, file_name: &str) -> bool {
        !self.notification_sent.contains_key(printer_id) || self.notification_sent.get(printer_id).unwrap() != file_name
    }

    fn
    send_notification(&self, printer: &Printer, notification_type: NotificationType) {
        if let Some(notification) = self.config.get_notification_destinations(&notification_type) {
            debug!("Sending notification: {:?}", notification_type);
            if let Some(emails) = &notification.emails {
                self.send_email_notifications(printer, &notification_type, emails.iter().map(|s| s.as_str()).collect())
            }
            if let Some(urls) = &notification.webhooks {
                self.send_webhook_notifications(printer, &notification_type, urls.iter().map(|s| s.as_str()).collect())
            }
        }
    }

    fn send_email_notifications(&self, printer: &Printer, notification_type: &NotificationType, emails: Vec<&str>) {
        if let Some(mailer) = &self.config.mailer() {
            let user = &self.config.smtp().unwrap().user;
            trace!("smtp configured, sending from {}", user);
            match user.parse() {
                Ok(from_addr) => {
                    let mut builder = Message::builder()
                        .from(Mailbox::new(None, from_addr))
                        .subject(notification_type.get_subject(printer))
                        .header(ContentType::TEXT_PLAIN);
                    for email in emails {
                        builder = builder.bcc(email.parse().unwrap())
                    }
                    let email = builder.body(notification_type.get_message(printer)).unwrap();

                    mailer.send(&email).unwrap();
                    trace!("Sent notification {:?} for printer {}", notification_type, printer);
                },
                Err(e) => {
                    error!("Could not parse from address \"{}\": {}", user, e);
                }
            }
        }
    }

    fn send_webhook_notifications(&self, printer: &Printer, notification_type: &NotificationType, urls: Vec<&str>) {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(5))
            .user_agent(format!("jackzmc/{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION")))
            .build().expect("failed to create reqwest client for webhooks");
        // TODO: proper struct? probably going to make it templated so eh
        let body = json!({
            "username": printer.name(),
            "embeds": [
                {
                    "title": notification_type.get_subject(printer),
                    "description": notification_type.get_message(printer),
                }
            ]
        });
        for url in urls {
            let request = client
                .post(url)
                .body(body.to_string());
            if let Err(e) = request.send() {
                error!("Failed to send webhook to \"{}\":\n{}", url, e);
            }
        }
    }

    pub fn get_printer_names(&self) -> Vec<String> {
        self.printers.keys().map(|s| s.clone()).collect()
    }

    pub fn printers(&self) -> Vec<PrinterContainer> {
        self.printers.values().map(|v| v.clone()).collect()
    }

    pub fn get_printer(&self, id: &str) -> Option<PrinterContainer> {
        self.printers.get(id).map(|printer| printer.clone())
    }

    pub fn add_printer(&mut self, id: String, ip: IpAddr) {
        debug!("adding printer {} with ip {}", id, ip);
        let mut printer = Printer::new(id.clone(), ip);
        printer.get_meta();
        let container = Arc::new(Mutex::new(printer));
        self.printers.insert(id, container);
    }
}

