use std::future::Future;
use std::io;
use std::io::{Cursor, Error, ErrorKind};
use std::pin::Pin;
use async_stream::__private::AsyncStream;
use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use futures::future::join_all;
use log::trace;
use multipart_stream::Part;
use reqwest::Url;
use rocket::{get, Either, Response, State};
use rocket::futures::{Stream, StreamExt, TryStreamExt};
use rocket::futures::stream::MapErr;
use rocket::http::{ContentType, Header, HeaderMap};
use rocket::http::hyper::body::Bytes;
use rocket::response::{Debug, Responder};
use rocket::response::stream::{stream, ByteStream, ReaderStream, TextStream};
use rocket::serde::json::Json;
use tokio_stream::wrappers::BroadcastStream;
use rocket_multipart::{MultipartSection, MultipartStream};
use tokio_util::bytes::Buf;
use tokio_util::bytes::buf::Writer;
use tokio_util::codec::{FramedRead, FramedWrite, LinesCodec};
use tokio_util::io::StreamReader;
use crate::models::{CachedPrinterInfo, GenericError, PrinterHeadPosition, PrinterInfo, PrinterProgress, PrinterStatus, PrinterTemperature};
use crate::printer::{Printer, PRINTER_CAM_PORT, PRINTER_CAM_STREAM_PATH};
use crate::manager::{PrinterManager};
use std::io::Write;


#[get("/names")]
pub async fn list_printers_names(printers: &State<PrinterManager>) -> Json<Vec<String>> {
    let printers = printers.lock().await;
    Json(printers.get_printer_names())
}

#[get("/")]
pub async fn list_printers(manager: &State<PrinterManager>) -> Json<Vec<CachedPrinterInfo>> {
    let printers = {
        let lock = manager.lock().await;
        lock.printers()
    };
    let mut printers_info = Vec::new();
    for printer in printers {
        let printer = printer.lock().await;
        let info = CachedPrinterInfo {
            name: printer.name().to_string(),
            is_online: printer.online(),
            current_file: printer.current_file().as_ref().map(|file| file.to_string()),
            firmware_version: None,
        };
        printers_info.push(info);
    }
    Json(printers_info)
}

async fn try_printer<T, F>(printers: &State<PrinterManager>, printer_id: &str, print_fn: F) -> Result<T, Json<GenericError>>
    where F: FnOnce(&Printer) -> Result<T, String> {
    // Acquire printer container
    let printer = {
        let lock = printers.lock().await;
        let printer = lock.get_printer(printer_id).ok_or(Json(GenericError {
            error: "UNKNOWN_PRINTER".to_string(),
            message: Some(format!("unknown printer {}", printer_id)),
        }))?;
        drop(lock);
        printer.clone()
    };
    let printer = printer.lock().await;
    print_fn(&*printer)
        .map(|r| r)
        .map_err(|e| Json(GenericError {
            error: "PRINTER_ERROR".to_string(),
            message: Some(e)
        }))
}
async fn try_printer_json<T, F>(printers: &State<PrinterManager>, printer_id: &str, print_fn: F) -> Result<Json<T>, Json<GenericError>>
where F: FnOnce(&Printer) -> Result<T, String> {
    try_printer(printers, printer_id, |printer| {
        print_fn(printer).map(|r| Json(r))
    }).await
}

#[get("/<printer_id>/info")]
pub async fn get_printer_info(printers: &State<PrinterManager>, printer_id: &str)
    -> Result<Json<PrinterInfo>, Json<GenericError>>
{
    try_printer_json(printers, printer_id, |printer| printer.get_info()).await
}

#[get("/<printer_id>/status")]
pub async fn get_printer_status(printers: &State<PrinterManager>, printer_id: &str)
                        -> Result<Json<PrinterStatus>, Json<GenericError>>
{
    try_printer_json(printers, printer_id, |printer| printer.get_status()).await
}

#[get("/<printer_id>/temperatures")]
pub async fn get_printer_temps(printers: &State<PrinterManager>, printer_id: &str)
                          -> Result<Json<PrinterTemperature>, Json<GenericError>>
{
    try_printer_json(printers, printer_id, |printer| printer.get_temperatures()).await
}

#[get("/<printer_id>/progress")]
pub async fn get_printer_progress(printers: &State<PrinterManager>, printer_id: &str)
                          -> Result<Json<PrinterProgress>, Json<GenericError>>
{
    try_printer_json(printers, printer_id, |printer| printer.get_progress()).await
}

#[get("/<printer_id>/head-position")]
pub async fn get_printer_head_position(printers: &State<PrinterManager>, printer_id: &str)
                            -> Result<Json<PrinterHeadPosition>, Json<GenericError>>
{
    try_printer_json(printers, printer_id, |printer| printer.get_head_position()).await
}

#[derive(Responder)]
#[response(content_type = "image/jpeg")]
struct JpegImage(Vec<u8>);

// This is just a "No image" fallback embeded in base64
const NO_IMAGE_BASE64: &[u8] = b"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";

#[derive(Responder)]
#[response(content_type = "multipart/x-mixed-replace;boundary=boundarydonotcross")]
//header = "Cache-Control': 'no-store, no-cache, must-revalidate, pre-check=0, post-check=0, max-age=0'", header = "Pragma: 'no-cache'", header = "Connection: 'close'"
struct MjpegStream<T>(T);

#[get("/<printer_id>/snapshot")]
pub async fn get_printer_snapshot(printers: & State<PrinterManager>, printer_id: String) -> Result<JpegImage, Either<JpegImage, String>> {
    let mut snapshot = {
        trace!("acquiring printer");
        let printer = {
            let lock = printers.lock().await;
            let printer = lock.get_printer(&printer_id).ok_or(Either::Right("Unknown printer".to_string()))?;
            printer.clone()
        };
        let mut printer = printer.lock().await;
        trace!("requesting snapshot {}", printer_id);
        printer.get_camera_snapshot().await

    };
    trace!("returning snapshot");
    snapshot
        .map(|img| JpegImage(img))
        .map_err(|e| Either::Left(JpegImage(BASE64_STANDARD.decode(NO_IMAGE_BASE64).unwrap())))
}

// TODO: add headers (Connection: close) and (Cache-Control: no-cache ...)
// Cannot get it to work with rocket, needs Response to set headers but it will not compile
#[get("/<printer_id>/camera")]
pub async fn get_printer_camera(printers: & State<PrinterManager>, printer_id: String) -> Result<MjpegStream<ByteStream<Pin<Box<dyn Stream<Item = Vec<u8>> + Send + 'static>>>>, Either<JpegImage, String>> {
    let mut camera_rx = {
        trace!("acquiring printer");
        let printer = {
            let lock = printers.lock().await;
            let printer = lock.get_printer(&printer_id).ok_or(Either::Right("Unknown printer".to_string()))?;
            printer.clone()
        };
        let mut printer = printer.lock().await;
        trace!("requesting snapshot {}", printer_id);
        printer.subscribe_camera().map_err(|e| Either::Right(format!("Failed to setup camera stream: {}", e)))?
    };

    let stream = stream! {
        while let Ok(mut part) = camera_rx.recv().await {
            let len: usize = part.headers.get("content-length").unwrap().to_str().unwrap().parse().unwrap();
            let mut s = Vec::with_capacity(len+512);
            write!(s, "--boundarydonotcross\r\n").ok();
            for header in part.headers.iter() {
                write!(s, "{}: {}\r\n", header.0, header.1.to_str().unwrap()).ok();
            }
            write!(s, "\r\n").ok();
            // s.append()
            s.extend_from_slice(part.body.iter().as_slice());
            write!(s, "\r\n").ok();
            yield s;
        }
    };
    let text_stream = ByteStream::from(Box::pin(stream) as Pin<Box<dyn Stream<Item = Vec<u8>> + Send + 'static>>);
    Ok(MjpegStream(text_stream))
}
