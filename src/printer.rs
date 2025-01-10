use std::fmt::Display;
use std::io::{Bytes, Read, Write};
use std::net::{IpAddr, SocketAddr, TcpStream};
use std::thread::{sleep, spawn, Thread};
use std::time::Duration;
use futures::{StreamExt, TryStreamExt};
use log::{debug, trace, warn};
use reqwest::Url;
use rocket::response::stream::ByteStream;
use rocket::serde::json::Json;
use rocket_multipart::MultipartSection;
use tokio::sync::broadcast;
use tokio::sync::mpsc::Receiver;
use tokio::task::JoinHandle;
use tokio_util::bytes::Buf;
use tokio_util::codec::{AnyDelimiterCodec, BytesCodec, FramedRead, FramedWrite};
use crate::models::{GenericError, PrinterHeadPosition, PrinterInfo, PrinterProgress, PrinterStatus, PrinterTemperature};
use crate::socket::{PrinterRequest, PrinterResponse};

pub struct Printer {
    socket_addr: SocketAddr,
    info: Option<PrinterInfo>,
    name: String,
    is_online: bool,
    current_file: Option<String>,
    camera_channel: broadcast::Sender<u8>,
    camera_thread: Option<JoinHandle<()>>
    // camera_stream: Option<Receiver<>>
}

// The port the TCP API is on
pub const PRINTER_API_PORT: u16 = 8899;
pub const PRINTER_CAM_PORT: u16 = 8080;
pub const PRINTER_CAM_STREAM_PATH: &'static str = "/?action=stream";
pub const PRINTER_CAM_SNAPSHOT_PATH: &'static str = "/?action=snapshot";

impl Display for Printer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}
impl Printer {
    pub fn new(name: String, ip_addr: IpAddr) -> Self {
        let (tx, _) = broadcast::channel(1024);
        Printer {
            socket_addr: SocketAddr::new(ip_addr, PRINTER_API_PORT),
            info: None,
            name,
            is_online: false,
            current_file: None,
            camera_channel: tx,
            camera_thread: None
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn ip(&self) -> IpAddr {
        self.socket_addr.ip()
    }

    // Only updated by watcher thread
    pub fn online(&self) -> bool { self.is_online }

    pub fn set_online(&mut self, on: bool) { self.is_online = on; }

    pub fn current_file(&self) -> &Option<String> { &self.current_file }

    fn set_current_file(&mut self, file: Option<String>) { self.current_file = file }

    pub fn get_meta(&mut self) -> Option<PrinterInfo> {
        if self.info.is_none() {
            match self.get_info() {
                Ok(info) => self.info = Some(info),
                Err(e) => {
                    warn!("printer/{} get_meta error: {}", self.name, e);
                }
            }
        }
        self.info.clone()
    }

    fn process_requests(&self, requests: &[PrinterRequest]) -> Result<PrinterResponse, String> {
        trace!("connecting to {:?}", self.socket_addr);
        let mut conn = TcpStream::connect(self.socket_addr).map_err(|e| e.to_string())?;
        conn.set_write_timeout(Some(Duration::from_secs(3))).unwrap();
        conn.set_read_timeout(Some(Duration::from_secs(10))).unwrap();

        // let mut results: Vec<String> = Vec::with_capacity(requests.len());
        let mut buf = [0; 1024];
        let mut result: Option<PrinterResponse> = None;
        if requests.is_empty() {
            panic!("No requests given")
        }
        for request in requests {
            let req_str = request.get_instruction();
            conn.write_all(req_str.as_bytes()).map_err(|e| e.to_string())?;
            let n = conn.read(&mut buf).map_err(|e| e.to_string())?;
            let str = String::from_utf8_lossy(&buf[..n]);
            result = Some(request.parse_response(&str)?);
        }
        Ok(result.unwrap())
    }

    pub fn send_request(&self, printer_request: PrinterRequest) -> Result<PrinterResponse, String> {
        let requests = vec![
            PrinterRequest::ControlMessage,
            printer_request
        ];
        self.process_requests(&requests)
    }

    pub fn refresh_status(&mut self) -> Result<(), String> {
        if let Ok(status) = self.get_status() {
            self.current_file = status.current_file;
            self.is_online = true;
        } else {
            self.is_online = false;
            // TODO: own error enum
            return Err("Printer unreachable or offline".to_string());
        }
        Ok(())
    }

    pub fn get_info(&self) -> Result<PrinterInfo, String> {
        match self.send_request(PrinterRequest::GetInfo) {
            Ok(PrinterResponse::PrinterInfo(info)) => Ok(info),
            Ok(_) => panic!("got wrong response from request"),
            Err(e) => Err(e)
        }
    }

    pub fn get_status(&self) -> Result<PrinterStatus, String> {
        match self.send_request(PrinterRequest::GetStatus) {
            Ok(PrinterResponse::PrinterStatus(v)) => Ok(v),
            Ok(_) => panic!("got wrong response from request"),
            Err(e) => Err(e)
        }
    }

    pub fn get_temperatures(&self) -> Result<PrinterTemperature, String> {
        match self.send_request(PrinterRequest::GetTemperature) {
            Ok(PrinterResponse::PrinterTemperature(t)) => Ok(t),
            Ok(_) => panic!("got wrong response from request"),
            Err(e) => Err(e)
        }
    }

    pub fn get_progress(&self) -> Result<PrinterProgress, String> {
        match self.send_request(PrinterRequest::GetProgress) {
            Ok(PrinterResponse::PrinterProgress(t)) => Ok(t),
            Ok(_) => panic!("got wrong response from request"),
            Err(e) => Err(e)
        }
    }

    pub fn get_head_position(&self) -> Result<PrinterHeadPosition, String> {
        match self.send_request(PrinterRequest::GetHeadPosition) {
            Ok(PrinterResponse::PrinterHeadPosition(t)) => Ok(t),
            Ok(_) => panic!("got wrong response from request"),
            Err(e) => Err(e)
        }
    }

    pub fn subscribe_camera(&mut self) -> Result<broadcast::Receiver<u8>, String> {
        if self.camera_channel.receiver_count() == 1 {
            // thread needs access to transmitter, rweference maybe to make sure its alive?
            let stream_url = format!("http://{}:{}{}", self.ip(), PRINTER_CAM_PORT, PRINTER_CAM_STREAM_PATH);
            let stream_url = Url::parse(&stream_url).map_err(|e| e.to_string())?;
            let tx = self.camera_channel.clone();
            let task = tokio::spawn(async move {
                let res = reqwest::get(stream_url).await.unwrap(); //.map_err(|e| Json(GenericError {error: "PRINTER_INTERNAL_ERROR".to_string(), message: Some(e.to_string())})).unwrap();
                let mut bytes_stream = res.bytes_stream(); //.map_err(std::io::Error::other);
                // let mut fw = FramedRead::new(bytes_stream, BytesCodec::new());
                while let Ok(chunk) = bytes_stream.next().await.unwrap() {
                    for byte in chunk {
                        tx.send(byte).unwrap();
                    }
                }
                while tx.receiver_count() > 0 {

                    sleep(Duration::from_secs(1))
                }
            });
            self.camera_thread = Some(task);
            // std::thread::spawn(|| async {
            //
            //     // No more subscribers, shut ourselves down
            // });
        }
        Ok(self.camera_channel.subscribe())
    }
}