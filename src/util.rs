use std::collections::HashMap;
use std::sync::{Arc, LazyLock};
use log::{debug, trace, warn};
use regex::Regex;
use rocket::http::Status;
use rocket::outcome::try_outcome;
use rocket::{Request, State};
use rocket::request::{FromRequest, Outcome};
use rocket::serde::json::Json;
use crate::config::{AuthConfig, ConfigManager};
use crate::manager::PrinterManager;
use crate::models::GenericError;
use crate::printer::Printer;

static RE_KV: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"([a-zA-Z0-9\-\s]+):\s*([^:\s]+)").unwrap());

pub async fn try_printer<T, F>(printers: &State<PrinterManager>, printer_id: &str, print_fn: F) -> Result<T, (Status, Json<GenericError>)>
where F: FnOnce(&Printer) -> Result<T, String> {
    // Acquire printer container
    let printer = {
        let lock = printers.lock().await;
        let printer = lock.get_printer(printer_id).ok_or((Status::NotFound, Json(GenericError {
            error: "UNKNOWN_PRINTER".to_string(),
            message: Some(format!("unknown printer {}", printer_id)),
        })))?;
        drop(lock);
        printer.clone()
    };
    let printer = printer.lock().await;
    print_fn(&printer)
        .map_err(|e| (Status::InternalServerError, Json(GenericError {
            error: "PRINTER_ERROR".to_string(),
            message: Some(e)
        })))
}



pub async fn try_printer_json<T, F>(printers: &State<PrinterManager>, printer_id: &str, print_fn: F) -> Result<Json<T>, (Status, Json<GenericError>)>
where F: FnOnce(&Printer) -> Result<T, String> {
    try_printer(printers, printer_id, |printer| {
        print_fn(printer).map(|r| Json(r))
    }).await
}

#[derive(PartialEq)]
pub(crate) enum AccessType {
    Read,
    Write
}

pub struct AuthGuard {
    input_password: Option<String>,
    auth_config: Option<AuthConfig>,
}
impl AuthGuard {
    pub(crate) fn check_auth(self, access_type: AccessType) -> Result<(), (Status, Json<GenericError>)> {
        if let Some(cfg) = self.auth_config {
            if (access_type == AccessType::Read && cfg.password_for_read) || (access_type == AccessType::Write && cfg.password_for_write) {
                if let Some(inp_pass) = self.input_password {
                    if cfg.password == inp_pass {
                        return Ok(())
                    }
                }
            }
        }
        Err((Status::Unauthorized, Json(GenericError {
            error: "PASSWORD_REQUIRED".to_string(),
            message: Some("The configured password is required to perform this action".to_string()),
        })))
    }
}
#[rocket::async_trait]
impl<'r> FromRequest<'r> for AuthGuard {
    type Error = ();

    async fn from_request(request: &'r Request<'_>) -> rocket::request::Outcome<AuthGuard, ()> {
        let config = try_outcome!(request.guard::<&State<Arc<ConfigManager>>>().await);
        let config = (*config).clone();
        let mut auth_guard = AuthGuard {
            input_password: None,
            auth_config: None
        };
        // If no auth config, then pass
        auth_guard.auth_config = config.auth().cloned();

        if let Some(secret) = request.headers().get("x-secret").next() {
            auth_guard.input_password = Some(secret.to_string());
        };
        Outcome::Success(auth_guard)
    }
}

/// Reads an input stream, with key: value per line or key1: val1 key2: val2
pub fn parse_multi_line(input: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();

    for cap in RE_KV.captures_iter(input) {
        // let val = Some(cap[2].to_string()).filter(|s| !s.is_empty());
        map.insert(cap[1].trim_start().to_string(), cap[2].to_string());
    }

    map
}

pub fn parse_kv(content: &str) -> Result<HashMap<String, String>, String> {
    trace!("parsing: {:?}", content);
    let mut kv = HashMap::new();
    // Skip first line ("CMD <GCODE> Received\r\n"), rest should be kv
    for line in content.lines().skip(1) {
        if line == "ok" {
            debug!("kv: {:?}", kv);
            return Ok(kv);
        }
        // Default will parse it as only key: value, but some cases we need to parse differently
        if let Ok((key, val)) = line.split_once(":").ok_or("invalid line") {
            if key == "X" {
                let p = parse_multi_line(line);
                kv.extend(p);
                // let pieces: Vec<&str> = line.split(" ").collect();
                // kv.insert("X", pieces[1]);
                // kv.insert("Y", pieces[3]);
                // kv.insert("Z", pieces[5]);
            } else if key == "Endstop" {
                let p = parse_multi_line(val);
                kv.extend(p);
            } else if key == "T0" {
                let p = parse_multi_line(line);
                kv.extend(p);
            } else {
                // kv.insert(key, val.trim_start());
                kv.insert(key.to_string(), val.trim_start().to_string());
            }
        } else {
            warn!("Invalid line: {}", line);
            continue;
        }
    }
    warn!("end of data, but did not see \"ok\"");
    Ok(kv)
}