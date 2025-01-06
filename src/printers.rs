use std::collections::HashMap;
use std::net::IpAddr;
use std::ops::Not;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;
use lettre::{Address, Message, SmtpTransport, Transport};
use lettre::message::header::ContentType;
use lettre::message::Mailbox;
use lettre::transport::smtp::authentication::Credentials;
use log::{debug, error, trace, warn};
use crate::config::{Config, EmailEncryption};
use crate::printer::Printer;
use std::fmt::Write;



static PROGRESS_CHECK_INTERVAL: Duration = Duration::from_secs(60);

pub type PrinterManager = Arc<Mutex<Printers>>;

#[derive(Debug)]
enum NotificationType {
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

pub struct Printers {
    printers: HashMap<String, Printer>,
    config: Arc<Config>,
    mailer: Option<SmtpTransport>,

    notification_sent: HashMap<String, String> // If printer (key) has value, then a print done notification has been submitted for file (value)
}

impl Printers {
    pub fn new(config: Arc<Config>) -> Printers {
        let mut s = Self {
            printers: HashMap::new(),
            config,
            mailer: None,
            notification_sent: HashMap::new()
        };
        if let Some(smtp) = &s.config.smtp {
            if smtp.port <= 0 {
                error!("SMTP: Smtp port is invalid, smtp support not enabled");
            } else if smtp.user == "" {
                error!("SMTP: Smtp user is empty, smtp support not enabled");
            } else if smtp.host == "" {
                error!("SMTP: Smtp host is empty, smtp support not enabled");
            } else {
                let builder = match smtp.encryption {
                    EmailEncryption::None => SmtpTransport::builder_dangerous(&smtp.host),
                    EmailEncryption::StartTLS => SmtpTransport::starttls_relay(&smtp.host).unwrap(),
                    EmailEncryption::TLS => SmtpTransport::relay(&smtp.host).unwrap()
                };
                s.mailer = Some(builder
                    .port(smtp.port)
                    .credentials(Credentials::new(smtp.user.to_string(), smtp.password.to_string()))
                    .build()
                )
            }
        }
        s
    }

    pub fn start_watch_thread(manager: PrinterManager) {
        debug!("Starting watch thread at interval {:?}", PROGRESS_CHECK_INTERVAL);
        std::thread::spawn(move || {
            std::thread::sleep(PROGRESS_CHECK_INTERVAL);
            loop {
                trace!("Checking printers");
                let mut lock = manager.lock().unwrap();
                let mut has_sent = lock.notification_sent.clone();
                for (id, printer) in &lock.printers {
                    if let Ok(prog) = printer.get_progress() {
                        // Check if progress is 100%
                        if prog.layer.0 >= prog.layer.1 {
                            // Get current file from status
                            let status = printer.get_status().unwrap();
                            if status.current_file.is_none() {
                                continue;
                            }
                            // Check if we have already sent a notification
                            let current_file = status.current_file.unwrap();
                            let has_notified = lock.has_notified(id, &current_file);

                            if !has_notified {
                                lock.send_notification(printer, NotificationType::PrintComplete);
                                has_sent.insert(id.clone(), current_file);
                            }
                        }
                    }
                }
                lock.notification_sent = has_sent;
                drop(lock);
                std::thread::sleep(PROGRESS_CHECK_INTERVAL);
            }
        });
    }

    fn has_notified(&self, printer_id: &str, file_name: &str) -> bool {
        !self.notification_sent.contains_key(printer_id) || self.notification_sent.get(printer_id).unwrap() != file_name
    }

    fn send_notification(&self, printer: &Printer, notification_type: NotificationType) {
        debug!("Sending notification: {:?}", notification_type);
        let Some(notifications) = &self.config.notifications else { return; };
        if let Some(mailer) = &self.mailer {
            let user = &self.config.smtp.as_ref().unwrap().user;
            trace!("smtp configured, sending from {}", user);
            match user.parse() {
                Ok(from_addr) => {
                    let mut builder = Message::builder()
                        .from(Mailbox::new(None, from_addr))
                        .subject(notification_type.get_subject(printer))
                        .header(ContentType::TEXT_PLAIN);
                    for email in notifications.emails.iter().flatten() {
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

    pub fn get_printer_names(&self) -> Vec<String> {
        self.printers.keys().map(|s| s.clone()).collect()
    }

    pub fn get_printer(&self, id: &str) -> Option<&Printer> {
        self.printers.get(id)
    }

    pub fn add_printer(&mut self, id: String, ip: IpAddr) {
        debug!("adding printer {} with ip {}", id, ip);
        let mut printer = Printer::new(id.clone(), ip);
        printer.get_meta();
        self.printers.insert(id, printer);
    }
}

