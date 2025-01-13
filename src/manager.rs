use crate::config::{ConfigManager, EmailEncryption};
use crate::printer::Printer;

use log::{debug, error, trace, warn};
use serde_json::json;
use std::collections::HashMap;
use std::fmt::Write;
use std::net::IpAddr;
use std::ops::Not;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use futures::executor::block_on;
use futures::future::join_all;
use futures::StreamExt;
use mail_send::mail_builder::MessageBuilder;
use mail_send::mail_builder::mime::BodyPart;
use reqwest::multipart::Part;
use rocket::http::hyper::body::HttpBody;
use tokio::sync::Mutex;
use tokio::task::{block_in_place, spawn_blocking};

static PROGRESS_CHECK_INTERVAL: Duration = Duration::from_secs(60);

pub type PrinterManager = Arc<Mutex<Printers>>;

#[derive(Debug, Clone, Copy)]
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
                write!(str, "File: {}\n", status.current_file.unwrap_or("(None)".to_string())).unwrap();
                write!(str, "IP: {}\n", printer.ip()).unwrap();
                // TODO: more data?
                str
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

    pub async fn start_watch_thread(manager: PrinterManager) {
        debug!("Starting watch thread at interval {:?}", PROGRESS_CHECK_INTERVAL);
        tokio::task::spawn(async move {
            tokio::time::sleep(PROGRESS_CHECK_INTERVAL).await;
            loop {
                // Grab list of printers
                trace!("Getting list of printers");
                let mut sent_notifications = {
                    let manager = manager.lock().await;
                    let (printers, mut sent_notifications) = {
                        let lock = &manager;
                        (lock.printers(), lock.notification_sent.clone())
                    };

                    trace!("Checking printers");
                    for printer in printers {
                        let mut printer = printer.lock().await;
                        if printer.refresh_status().is_ok() {
                            if printer.current_file().is_none() { continue; }
                            let prog = printer.get_progress().unwrap();
                            // Check if progress is 100%
                            trace!("printer {} layer={:?} byte={:?}", printer.name(), prog.layer, prog.byte);
                            if prog.layer.0 >= prog.layer.1 {
                                // Get current file from status
                                let status = printer.get_status().unwrap();
                                if status.current_file.is_none() {
                                    continue;
                                }
                                // Check if we have already sent a notification
                                let current_file = status.current_file.unwrap();
                                let has_notified = sent_notifications.get(printer.name()).unwrap_or(&"".to_string()) == &current_file;

                                if !has_notified {
                                    debug!("will notify for printer {}", printer.name());
                                    manager.send_notification(&mut printer, NotificationType::PrintComplete).await;
                                    // has_sent.insert(id.clone(), current_file);

                                    let current_file = printer.current_file().as_ref().unwrap().clone();
                                    sent_notifications.insert(printer.name().to_string(), current_file);
                                }
                            }
                        }
                    }
                    sent_notifications
                };
                {
                    let mut manager = manager.lock().await;
                    manager.notification_sent = sent_notifications;
                }
                tokio::time::sleep(PROGRESS_CHECK_INTERVAL).await;
            }
        });
    }

    fn has_notified(&self, printer_id: &str, file_name: &str) -> bool {
        !self.notification_sent.contains_key(printer_id) || self.notification_sent.get(printer_id).unwrap() != file_name
    }

    pub async fn send_notification(&self, printer: &mut Printer, notification_type: NotificationType) {
        if let Some(notification) = self.config.get_notification_destinations(&notification_type) {
            // Fetch latest image
            printer.get_camera_snapshot().await.ok();

            debug!("Sending notification: {:?}", notification_type);
            if let Some(emails) = &notification.emails {
                debug!("have emails, sending emails");
                self.send_email_notifications(printer, notification_type, emails.iter().map(|s| s.as_str()).collect()).await
            }
            if let Some(urls) = &notification.webhooks {
                debug!("have webhooks, sending webhooks");
                self.send_webhook_notifications(printer, notification_type, urls.iter().map(|s| s.as_str()).collect()).await
            }
        }
    }
    async fn send_email_notifications(&self, printer: &mut Printer, notification_type: NotificationType, emails: Vec<&str>) {
        let Some(mailer) = self.config.mailer() else { return; };
        let mut mailer = mailer.lock().await;

        let send_user = &self.config.smtp().unwrap().user;
        let subject = notification_type.get_subject(printer);
        let body = notification_type.get_message(printer);

        trace!("smtp configured, sending from {}", send_user);
        let mut builder = MessageBuilder::new()
            .from(send_user.as_str())
            .text_body(body)
            .subject(subject);
        if let Some(last_img) = printer.last_image() {
            builder = builder.attachment("image/jpeg", "printer_image.jpg", BodyPart::from(last_img));
        }
        for to_email in emails {
            builder = builder.bcc(to_email);
        }
        mailer.send(builder).await.expect("failed to send email");
        trace!("Sent notification {:?} for printer {}", notification_type, printer);
    }

    async fn send_webhook_notifications(&self, printer: &mut Printer, notification_type: NotificationType, urls: Vec<&str>) {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .user_agent(format!("jackzmc/{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION")))
            .build().expect("failed to create reqwest client for webhooks");
        // TODO: proper struct? probably going to make it templated so eh
        trace!("created webhook client");
        let body = json!({
            "username": printer.name(),
            "embeds": [
                {
                    "title": notification_type.get_subject(&*printer),
                    "description": notification_type.get_message(&*printer),
                    "image": {
                        "url": "attachment://printer_image.jpg"
                    }
                }
            ]
        });
        for url in urls {
            trace!("POST {}", url);
            let mut form_data = reqwest::multipart::Form::new()
                .text("payload_json", body.to_string());
            if let Some(image) = printer.last_image() {
                let part = Part::bytes(image)
                    .file_name("printer_image.jpg")
                    .mime_str("image/jpeg")
                    .unwrap();
                form_data = form_data.part("file1", part);
            }
            let request = client
                .post(url)
                .multipart(form_data);
            match request.send().await {
                Ok(response) => {
                    if let Err(err) = response.error_for_status() {
                        error!("Failed to send webhook: \n{}", err);
                    }
                },
                Err(err) => {
                    error!("Failed to send webhook to \"{}\":\n{}", url, err);
                }
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

