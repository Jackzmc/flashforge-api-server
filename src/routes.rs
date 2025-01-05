use rocket::{get, State};
use rocket::serde::json::Json;
use crate::models::{GenericError, PrinterHeadPosition, PrinterInfo, PrinterProgress, PrinterStatus, PrinterTemperature};
use crate::printer::Printer;
use crate::printers::Printers;
use crate::socket::PrinterRequest;

#[get("/")]
pub fn list_printers(printers: &State<Printers>) -> Json<Vec<String>> {
    Json(printers.get_printer_names())
}

fn try_printer<T, F>(printers: &State<Printers>, printer_id: &str, print_fn: F) -> Result<Json<T>, Json<GenericError>>
    where F: FnOnce(&Printer) -> Result<T, String> {
    let printer = printers.get_printer(printer_id).ok_or(Json(GenericError {
        error: "UNKNOWN_PRINTER".to_string(),
        message: Some(format!("unknown printer {}", printer_id)),
    }))?;
    print_fn(printer)
        .map(|r| Json(r))
        .map_err(|e| Json(GenericError {
            error: "PRINTER_ERROR".to_string(),
            message: Some(e)
        }))
}

#[get("/<printer_id>/info")]
pub fn get_printer_info(printers: &State<Printers>, printer_id: &str)
    -> Result<Json<PrinterInfo>, Json<GenericError>>
{
    try_printer(printers, printer_id, |printer| printer.get_info())
}

#[get("/<printer_id>/status")]
pub fn get_printer_status(printers: &State<Printers>, printer_id: &str)
                        -> Result<Json<PrinterStatus>, Json<GenericError>>
{
    try_printer(printers, printer_id, |printer| printer.get_status())
}

#[get("/<printer_id>/temperatures")]
pub fn get_printer_temps(printers: &State<Printers>, printer_id: &str)
                          -> Result<Json<PrinterTemperature>, Json<GenericError>>
{
    try_printer(printers, printer_id, |printer| printer.get_temperatures())
}

#[get("/<printer_id>/progress")]
pub fn get_printer_progress(printers: &State<Printers>, printer_id: &str)
                          -> Result<Json<PrinterProgress>, Json<GenericError>>
{
    try_printer(printers, printer_id, |printer| printer.get_progress())
}

#[get("/<printer_id>/head-position")]
pub fn get_printer_head_position(printers: &State<Printers>, printer_id: &str)
                            -> Result<Json<PrinterHeadPosition>, Json<GenericError>>
{
    try_printer(printers, printer_id, |printer| printer.get_head_position())
}
