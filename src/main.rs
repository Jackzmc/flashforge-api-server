mod models;
mod socket;
mod printer;
mod util;

use std::net::{AddrParseError, SocketAddr, ToSocketAddrs};
use log::{debug,info};
use rocket::{catch, catchers, get, launch, routes, serde::json::Json, Request};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use crate::models::{GenericError, Position};
use crate::printer::Printer;
use crate::socket::{PrinterRequest, PrinterResponse};

fn parse_address(server: String) -> Result<SocketAddr, GenericError> {
    let server_addr = server.to_socket_addrs().map_err(|e| {
        debug!("server: {:?} err: {:?}", server, e);
        GenericError {
            error: "INVALID_SERVER_ADDRESS".to_string(),
            message: Some(e.to_string()),
        }
    })?.next();
    if let Some(server_addr) = server_addr {
        Ok(server_addr)
    } else {
        Err(GenericError {
            error: "SERVER_ADDR_NOT_FOUND".to_string(),
            message: Some("Could not solve an address".to_string())
        })
    }
}

#[get("/<server>/info")]
fn get_printer_info(server: String) -> Result<Json<PrinterResponse>, Json<GenericError>> {
    // let server_addr = parse_address(server).map_err(|e| Json(e))?;
    let printer = Printer::new(server.parse().unwrap());
    Ok(Json(printer.send_request(PrinterRequest::GetInfo).map_err(|e| Json(GenericError {
        error: "SERVER_ERROR".to_string(),
        message: Some(e)
    }))?))

}

#[get("/<server>/temperature")]
fn get_printer_temperature(server: String) -> Result<Json<PrinterResponse>, Json<GenericError>> {
    // let server_addr = parse_address(server).map_err(|e| Json(e))?;
    let printer = Printer::new(server.parse().unwrap());
    Ok(Json(printer.send_request(PrinterRequest::GetTemperature).map_err(|e| Json(GenericError {
        error: "SERVER_ERROR".to_string(),
        message: Some(e)
    }))?))
}

#[get("/<server>/status")]
fn get_printer_status(server: String) -> Result<Json<PrinterResponse>, Json<GenericError>> {
    // let server_addr = parse_address(server).map_err(|e| Json(e))?;
    let printer = Printer::new(server.parse().unwrap());
    Ok(Json(printer.send_request(PrinterRequest::GetStatus).map_err(|e| Json(GenericError {
        error: "SERVER_ERROR".to_string(),
        message: Some(e)
    }))?))
}

#[get("/<server>/progress")]
fn get_printer_progress(server: String) -> Result<Json<PrinterResponse>, Json<GenericError>> {
    // let server_addr = parse_address(server).map_err(|e| Json(e))?;
    let printer = Printer::new(server.parse().unwrap());
    Ok(Json(printer.send_request(PrinterRequest::GetProgress).map_err(|e| Json(GenericError {
        error: "SERVER_ERROR".to_string(),
        message: Some(e)
    }))?))
}

#[get("/<server>/head-position")]
fn get_printer_head_position(server: String) -> Result<Json<PrinterResponse>, Json<GenericError>> {
    // let server_addr = parse_address(server).map_err(|e| Json(e))?;
    let printer = Printer::new(server.parse().unwrap());
    Ok(Json(printer.send_request(PrinterRequest::GetHeadPosition).map_err(|e| Json(GenericError {
        error: "SERVER_ERROR".to_string(),
        message: Some(e)
    }))?))
}

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

    let mut config = rocket::Config::default();
    config.port = 8080;
    let r = rocket::build()
        .configure(&config)
        .mount("/", routes![
            get_printer_info,
            get_printer_temperature,
            get_printer_progress,
            get_printer_status,
            get_printer_head_position
        ])
        .register("/", catchers![error_404]);
    info!("Server ready and listening on :{}", config.port);
    r
}