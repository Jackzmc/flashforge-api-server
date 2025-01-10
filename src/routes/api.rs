use std::future::Future;
use std::io::{Error, ErrorKind};
use async_stream::__private::AsyncStream;
use log::trace;
use reqwest::Url;
use rocket::{get, Response, State};
use rocket::futures::{Stream, StreamExt, TryStreamExt};
use rocket::futures::stream::MapErr;
use rocket::http::{ContentType, Header};
use rocket::http::hyper::body::Bytes;
use rocket::response::Debug;
use rocket::response::stream::{stream, ByteStream, ReaderStream};
use rocket::serde::json::Json;
use tokio_stream::wrappers::BroadcastStream;
use rocket_multipart::{MultipartSection, MultipartStream};
use tokio_util::codec::{FramedRead, FramedWrite, LinesCodec};
use tokio_util::io::StreamReader;
use crate::models::{CachedPrinterInfo, GenericError, PrinterHeadPosition, PrinterInfo, PrinterProgress, PrinterStatus, PrinterTemperature};
use crate::printer::{Printer, PRINTER_CAM_PORT, PRINTER_CAM_STREAM_PATH};
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

fn try_printer<T, F>(printers: &State<PrinterManager>, printer_id: &str, print_fn: F) -> Result<T, Json<GenericError>>
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
        .map(|r| r)
        .map_err(|e| Json(GenericError {
            error: "PRINTER_ERROR".to_string(),
            message: Some(e)
        }))
}
fn try_printer_json<T, F>(printers: &State<PrinterManager>, printer_id: &str, print_fn: F) -> Result<Json<T>, Json<GenericError>>
where F: FnOnce(&Printer) -> Result<T, String> {
    try_printer(printers, printer_id, |printer| {
        print_fn(printer).map(|r| Json(r))
    })
}

#[get("/<printer_id>/info")]
pub fn get_printer_info(printers: &State<PrinterManager>, printer_id: &str)
    -> Result<Json<PrinterInfo>, Json<GenericError>>
{
    try_printer_json(printers, printer_id, |printer| printer.get_info())
}

#[get("/<printer_id>/status")]
pub fn get_printer_status(printers: &State<PrinterManager>, printer_id: &str)
                        -> Result<Json<PrinterStatus>, Json<GenericError>>
{
    try_printer_json(printers, printer_id, |printer| printer.get_status())
}

#[get("/<printer_id>/temperatures")]
pub fn get_printer_temps(printers: &State<PrinterManager>, printer_id: &str)
                          -> Result<Json<PrinterTemperature>, Json<GenericError>>
{
    try_printer_json(printers, printer_id, |printer| printer.get_temperatures())
}

#[get("/<printer_id>/progress")]
pub fn get_printer_progress(printers: &State<PrinterManager>, printer_id: &str)
                          -> Result<Json<PrinterProgress>, Json<GenericError>>
{
    try_printer_json(printers, printer_id, |printer| printer.get_progress())
}

#[get("/<printer_id>/head-position")]
pub fn get_printer_head_position(printers: &State<PrinterManager>, printer_id: &str)
                            -> Result<Json<PrinterHeadPosition>, Json<GenericError>>
{
    try_printer_json(printers, printer_id, |printer| printer.get_head_position())
}
//Content-Type
// 	multipart/x-mixed-replace;boundary=boundarydonotcross
#[get("/<printer_id>/camera")]
pub async fn get_printer_camera(printers: &State<PrinterManager>, printer_id: &str) -> Result<MultipartStream<impl Stream<Item = MultipartSection<'static>>>, Json<GenericError>> {
    let mut camera_rx = {
        let printer = {
            let lock = printers.lock().unwrap();
            let printer = lock.get_printer(printer_id).ok_or(Json(GenericError {
                error: "UNKNOWN_PRINTER".to_string(),
                message: Some(format!("unknown printer {}", printer_id)),
            }))?;
            drop(lock);
            printer.clone()
        };
        let mut printer = printer.lock().unwrap();
        printer.subscribe_camera().unwrap()
    };
    // TODO: somehow store the stream in Printer, so many clients -> one reqwest of camera.
    // As it stands this is a 1:1 proxy, which the printer only processes 1 client at a time.
    // trace!("starting reqwest for {}", stream_url);
    // let res = reqwest::get(stream_url).await.map_err(|e| Json(GenericError {error: "PRINTER_INTERNAL_ERROR".to_string(), message: Some(e.to_string())}))?;
    // let mut bytes_stream = res.bytes_stream().map_err(std::io::Error::other);
    // let f = FramedWrite::new(bytes_stream, LinesCodec::new());
    let s = tokio_util::io::StreamReader::new(BroadcastStream::new(camera_rx));
    // camera_rx.unwrap().recv().await.unwrap()
    let response_stream = MultipartStream::new(
        "boundarydonotcross",
        stream! {
            yield MultipartSection::new(s)
        },
    )
        .with_subtype("x-mixed-replace");
        // .add_header(Header::new("Cache-Control", "no-store, no-cache, must-revalidate, pre-check=0, post-check=0, max-age=0"))
        // .add_header(Header::new("Access-Control-Allow-Origin", "*"));
    Ok(response_stream)

    // URL of the MJPEG stream you want to proxy

    // trace!("streaming response");
    // Ok(MultipartStream::new("boundarydonotcross", bytes_stream))

    // Ok(ByteStream! {
    //     while let Some(Ok(bytes)) = bytes_stream.next().await {
    //         yield bytes;
    //     }
    // })
    //
    // // Create a blocking client to fetch the stream (reqwest is blocking here)
    // let client = reqwest::Client::new();
    // let mut response = client.get(stream_url)
    //     .send()
    //     .await
    //     .unwrap();
    //
    // // Create a buffer to hold the stream data
    //
    // // Make sure the response is of type multipart
    // let boundary = "boundary"; // Make sure to match the boundary from the MJPEG stream header
    //
    // use rocket::futures::TryStreamExt; // for map_err() call below:
    // let reader =
    //     StreamReader::new(response.bytes_stream().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e)));
    // ReaderStream::one(reader)
    // //
    // // // Stream the MJPEG frames and send them back to the client
    // // let stream = async_stream::stream! {
    // //     while let Some(chunk) = response.chunk().await.expect("Failed to read stream") {
    // //         // Check if it's a JPEG frame and send it
    // //         if chunk.starts_with(b"\xff\xd8") {  // JPEG start marker
    // //             yield chunk;
    // //         }
    // //     }
    // // };
    // //
    // // // Serve the stream as a response with the correct MIME type for MJPEG streaming
    // // Response::build()
    // //     .header(ContentType::new("multipart", "x-mixed-replace; boundary=boundary"))
    // //     .streamed_body(stream)
    // //     .finalize()
}

