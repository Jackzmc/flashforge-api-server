use futures::StreamExt;
use std::fmt::Display;
use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpStream};
use std::sync::{Arc, RwLock};
use std::time::Duration;
use log::{trace, warn};
use multipart_stream::Part;
use reqwest::Url;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use crate::models::{PrinterHeadPosition, PrinterInfo, PrinterProgress, PrinterStatus, PrinterTemperature};
use crate::socket::{PrinterRequest, PrinterResponse};

pub struct Printer {
    socket_addr: SocketAddr,
    info: Option<PrinterInfo>,
    name: String,
    is_online: bool,
    current_file: Option<String>,
    camera_channel: broadcast::Sender<Part>,
    camera_task: Option<JoinHandle<()>>,
    last_image: Arc<RwLock<Option<Vec<u8>>>>
    // camera_stream: Option<Receiver<>>
}

// The port the TCP API is on
pub const PRINTER_API_PORT: u16 = 8899;
pub const PRINTER_CAM_PORT: u16 = 8080;
pub const PRINTER_CAM_STREAM_PATH: &str = "/?action=stream";
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
            camera_task: None,
            last_image: Arc::new(RwLock::new(None)),
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

    pub fn current_file(&self) -> &Option<String> { &self.current_file }


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

    /// Returns the last received image, if any. Call [get_camera_snapshot] for a live
    pub fn last_image(&self) -> Option<Vec<u8>> {
        let read = self.last_image.read().expect("poisoned");
        read.clone()
    }

    /// Gets a fresh camera snapshot, by internally calling [subscribe_camera]()
    pub async fn get_camera_snapshot(&mut self) -> Result<Vec<u8>, String> {
        let mut rx = self.subscribe_camera().map_err(|e| e.to_string())?;
        trace!("subscribed, now waiting for image");
        let part = rx.recv().await.map_err(|e| e.to_string())?;
        trace!("returning image");
        Ok(part.body.to_vec())
    }

    /// Returns a receiver that returns Part (header and image body from multipart/x-mixed-replace)
    /// If there is not already a connection to printer's camera, a new one will be created.
    /// Image is JPEG, size is provided in header `Content-length`
    pub fn subscribe_camera(&mut self) -> Result<broadcast::Receiver<Part>, String> {
        let sub = self.camera_channel.subscribe();
        let image_store = self.last_image.clone();
        if self.camera_task.is_none() || self.camera_task.as_ref().unwrap().is_finished() {
            let stream_url = format!("http://{}:{}{}", self.ip(), PRINTER_CAM_PORT, PRINTER_CAM_STREAM_PATH);
            let stream_url = Url::parse(&stream_url).map_err(|e| e.to_string())?;
            trace!("starting new camera task. stream url = {:?}", stream_url);

            let tx = self.camera_channel.clone();
            let task = tokio::spawn(async move {
                trace!("starting reqwest");
                // TODO: better handling of offline printer
                let res = reqwest::get(stream_url).await.expect("failed to fetch stream");
                let bytes_stream = res.bytes_stream();
                trace!("starting read loop");
                let image_store = image_store;
                let mut chunk_stream = multipart_stream::parse(bytes_stream, "boundarydonotcross");
                while let Ok(part) = chunk_stream.next().await.unwrap() {
                    let mut write = image_store.write().unwrap();
                    *write = Some(part.body.to_vec());
                    if tx.send(part).is_err() {
                        trace!("no more subscribers, stopping task");
                        break;
                    }
                }
            });
            self.camera_task = Some(task);
        }
        Ok(sub)
    }
}