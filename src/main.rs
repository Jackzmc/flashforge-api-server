mod models;
mod socket;
mod printer;
mod util;
mod config;
mod printers;
mod routes;

use std::net::{AddrParseError, SocketAddr, ToSocketAddrs};
use log::{debug,info};
use rocket::{catch, catchers, get, launch, routes, serde::json::Json, Request};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use crate::config::Config;
use crate::models::{GenericError, Position};
use crate::printer::Printer;
use crate::printers::Printers;
use crate::routes::{get_printer_head_position, get_printer_info, get_printer_progress, get_printer_status, get_printer_temps, list_printers};
use crate::socket::{PrinterRequest, PrinterResponse};

#[catch(404)]
fn error_404(req: &Request) -> Json<GenericError> {
    Json(GenericError {
        error: "NOT_FOUND".to_string(),
        message: Some("Route not found".to_string()),
    })
}

#[launch]
fn rocket() -> _ {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::filter::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("{}=trace", env!("CARGO_CRATE_NAME")).into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let mut printers = Printers::new();
    let config = Config::load();
    for (id, printer_config) in &config.printers {
        printers.add_printer(id.to_string(), printer_config.ip)
    }

    let mut rk_config = rocket::Config::default();
    rk_config.port = 8080;
    let r = rocket::build()
        .configure(&rk_config)
        .manage(printers)
        .manage(config)
        .mount("/api/printers", routes![
            list_printers,
            get_printer_info,
            get_printer_temps,
            get_printer_progress,
            get_printer_status,
            get_printer_head_position
        ])
        .register("/", catchers![error_404]);
    info!("Server ready and listening on :{}", rk_config.port);
    r
}