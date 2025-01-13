mod models;
mod socket;
mod printer;
mod util;
mod config;
mod manager;
mod routes;

use std::sync::{Arc};
use log::{info};
use rocket::{catch, catchers, launch, routes, serde::json::Json};
use tokio::sync::Mutex;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use crate::config::{ConfigManager};
use crate::models::{GenericError};
use crate::manager::Printers;
use crate::routes::api;

#[catch(404)]
fn error_404() -> Json<GenericError> {
    Json(GenericError {
        error: "NOT_FOUND".to_string(),
        message: Some("Route not found".to_string()),
    })
}

#[launch]
async fn rocket() -> _ {
    tokio_rustls::rustls::crypto::ring::default_provider().install_default().unwrap();
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::filter::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("{}=warn", env!("CARGO_CRATE_NAME")).into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Arc::new(ConfigManager::load().await);
    let mut printers = Printers::new(config.clone());
    for (id, printer_config) in config.printers() {
        printers.add_printer(id.to_string(), printer_config.ip)
    }
    let printers = Arc::new(Mutex::new(printers));
    Printers::start_watch_thread(printers.clone()).await;

    let mut rk_config = rocket::Config::default();
    rk_config.port = 8080;

    let r = rocket::build()
        .configure(&rk_config)
        .manage(config)
        .manage(printers)
        .mount("/api/printers", routes![
            api::list_printers_names,
            api::list_printers,
            api::get_printer_info,
            api::get_printer_temps,
            api::get_printer_progress,
            api::get_printer_status,
            api::get_printer_head_position,
            api::get_printer_snapshot,
            api::get_printer_camera
        ])
        // .mount("/", routes![
        //     routes::ui::index
        // ])
        .register("/", catchers![error_404]);
    info!("Server ready and listening on :{}", rk_config.port);
    r
}