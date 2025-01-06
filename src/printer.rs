use std::fmt::Display;
use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpStream};
use std::time::Duration;
use log::{debug, trace, warn};
use crate::models::{PrinterHeadPosition, PrinterInfo, PrinterProgress, PrinterStatus, PrinterTemperature};
use crate::socket::{PrinterRequest, PrinterResponse};

pub struct Printer {
    socket_addr: SocketAddr,
    info: Option<PrinterInfo>,
    name: String,
}

// The port the TCP API is on
const PRINTER_API_PORT: u16 = 8899;

impl Display for Printer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}
impl Printer {
    pub fn new(name: String, ip_addr: IpAddr) -> Self {
        Printer {
            socket_addr: SocketAddr::new(ip_addr, PRINTER_API_PORT),
            info: None,
            name
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn ip(&self) -> IpAddr {
        self.socket_addr.ip()
    }

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
}