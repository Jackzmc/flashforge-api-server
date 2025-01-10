use rocket::{get, State};
use rocket::serde::json::Json;
use crate::models::{CachedPrinterInfo, GenericError, PrinterHeadPosition, PrinterInfo, PrinterProgress, PrinterStatus, PrinterTemperature};
use crate::printer::Printer;
use crate::manager::{PrinterManager};

#[get("/names")]
pub fn list_printers_names(printers: &State<PrinterManager>) -> Json<Vec<String>> {
    let printers = printers.lock().unwrap();
    Json(printers.get_printer_names())
}

#[get("/")]
pub fn list_printers(manager: &State<PrinterManager>) -> Json<Vec<CachedPrinterInfo>> {
    let printers = {
        let lock = manager.lock().unwrap();
        lock.printers()
    };
    let printers = printers.iter().map(|printer| {
        let printer = printer.lock().unwrap();
        CachedPrinterInfo {
            name: printer.name().to_string(),
            is_online: printer.online(),
            current_file: printer.current_file().as_ref().map(|file| file.to_string()),
            firmware_version: None,
        }
    }).collect();
    Json(printers)
}

fn try_printer<T, F>(printers: &State<PrinterManager>, printer_id: &str, print_fn: F) -> Result<Json<T>, Json<GenericError>>
    where F: FnOnce(&Printer) -> Result<T, String> {
    // Acquire printer container
    let printer = {
        let lock = printers.lock().unwrap();
        let printer = lock.get_printer(printer_id).ok_or(Json(GenericError {
            error: "UNKNOWN_PRINTER".to_string(),
            message: Some(format!("unknown printer {}", printer_id)),
        }))?;
        drop(lock);
        printer.clone()
    };
    let printer = printer.lock().unwrap();
    print_fn(&*printer)
        .map(|r| Json(r))
        .map_err(|e| Json(GenericError {
            error: "PRINTER_ERROR".to_string(),
            message: Some(e)
        }))
}

#[get("/<printer_id>/info")]
pub fn get_printer_info(printers: &State<PrinterManager>, printer_id: &str)
    -> Result<Json<PrinterInfo>, Json<GenericError>>
{
    try_printer(printers, printer_id, |printer| printer.get_info())
}

#[get("/<printer_id>/status")]
pub fn get_printer_status(printers: &State<PrinterManager>, printer_id: &str)
                        -> Result<Json<PrinterStatus>, Json<GenericError>>
{
    try_printer(printers, printer_id, |printer| printer.get_status())
}

#[get("/<printer_id>/temperatures")]
pub fn get_printer_temps(printers: &State<PrinterManager>, printer_id: &str)
                          -> Result<Json<PrinterTemperature>, Json<GenericError>>
{
    try_printer(printers, printer_id, |printer| printer.get_temperatures())
}

#[get("/<printer_id>/progress")]
pub fn get_printer_progress(printers: &State<PrinterManager>, printer_id: &str)
                          -> Result<Json<PrinterProgress>, Json<GenericError>>
{
    try_printer(printers, printer_id, |printer| printer.get_progress())
}

#[get("/<printer_id>/head-position")]
pub fn get_printer_head_position(printers: &State<PrinterManager>, printer_id: &str)
                            -> Result<Json<PrinterHeadPosition>, Json<GenericError>>
{
    try_printer(printers, printer_id, |printer| printer.get_head_position())
}
