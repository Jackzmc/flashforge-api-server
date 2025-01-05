use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpStream};
use std::sync::LazyLock;
use std::time::Duration;
use log::{debug, trace, warn};
use regex::Regex;
use serde::Serialize;
use crate::models::{EndStopPosition, Position, TemperatureMeasurement};
use crate::socket::PrinterResponse::{PrinterHeadPosition, PrinterInfo, PrinterProgress, PrinterStatus, PrinterTemperature};
use crate::util::parse_kv;

pub fn send_printer_requests(ip: SocketAddr, requests: &[PrinterRequest]) -> Result<Vec<String>, String> {
    trace!("connecting to {:?}", ip);
    let mut conn = TcpStream::connect(ip).map_err(|e| e.to_string())?;
    conn.set_write_timeout(Some(Duration::from_secs(3))).unwrap();
    conn.set_read_timeout(Some(Duration::from_secs(10))).unwrap();

    let mut results: Vec<String> = Vec::with_capacity(requests.len());
    for request in requests {
        let mut buf = [0; 1024];
        send_request(&mut conn, request)?;
        conn.read(&mut buf).map_err(|e| e.to_string())?;
        results.push(String::from_utf8_lossy(&buf).to_string());
    }
    debug!("return");
    Ok(results)
}
fn send_request(conn: &mut TcpStream, req: &PrinterRequest) -> Result<(), String> {
    trace!("sending gcode {:?}", req.get_gcode());
    let req_str = req.get_instruction();
    conn.write_all(req_str.as_bytes()).map_err(|e| e.to_string())?;
    trace!("sent, now read");
    Ok(())
}

#[derive(Debug)]
pub enum PrinterRequest {
    ControlMessage,
    GetInfo,
    GetHeadPosition,
    GetTemperature,
    GetProgress,
    GetStatus
}

#[derive(Serialize)]
pub enum PrinterResponse {
    ControlSuccess,
    #[serde(rename = "info")]
    PrinterInfo {
        name: String,
        firmware_version: String,
        sn: String,
        tool_count: u8,
        model_name: String,
        mac_addr: String,
        position: Position
    },
    #[serde(rename = "position")]
    PrinterHeadPosition {
        x: f32,
        y: f32,
        z: f32,
        a: f32,
        b: u32
    },
    #[serde(rename = "temperatures")]
    PrinterTemperature(HashMap<String, TemperatureMeasurement>),
    #[serde(rename = "progress")]
    PrinterProgress {
        layer: (u32, u32),
        byte: (u32, u32)
    },
    #[serde(rename = "status")]
    PrinterStatus {
        end_stop: EndStopPosition,
        machine_status: String, // "READY",
        move_mode: String, // "READY"
        // status: Option<>, // S:1, L:0, J:0, F:0
        led: bool,
        current_file: Option<String>
    }
}



static RE_PRINTER_PROGRESS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(\d+)/(\d+)").unwrap());
impl PrinterRequest {
    pub fn parse_response(&self, input: &str) -> Result<PrinterResponse, String> {
        match self {
            PrinterRequest::ControlMessage => Ok(PrinterResponse::ControlSuccess),
            PrinterRequest::GetInfo => {
                let kv = parse_kv(&input)?;
                debug!("{:?}", &kv);
                Ok(PrinterInfo {
                    name: kv.get("Machine Name").unwrap().to_string(),
                    firmware_version: kv.get("Firmware").unwrap().to_string(),
                    sn: kv.get("SN").unwrap().to_string(),
                    tool_count: kv.get("Tool Count").unwrap().parse().unwrap(),
                    model_name: kv.get("Machine Type").unwrap().to_string(),
                    mac_addr: kv.get("Mac Address").unwrap().to_string(),
                    position: Position {
                        x: kv.get("X").unwrap().parse().unwrap(),
                        y: kv.get("Y").unwrap().parse().unwrap(),
                        z: kv.get("Z").unwrap().parse().unwrap(),
                    }
                })
            },
            PrinterRequest::GetProgress => {
                let prog: Vec<(u32,u32)> = RE_PRINTER_PROGRESS.captures_iter(input)
                    .map(|c| (c[1].parse().unwrap(), c[2].parse().unwrap()))
                    .collect();
                if prog.is_empty() {
                    panic!("no matches found");
                }
                Ok(PrinterResponse::PrinterProgress {
                    byte: prog[0],
                    layer: prog[1],
                })
            },
            PrinterRequest::GetTemperature => {
                let kv = parse_kv(input)?;
                debug!("{:?}", kv);
                let temps = kv.into_iter().map(|(key, val)| {
                    let temp: Vec<f32> = val.split("/").map(|s|s.parse().unwrap()).collect();
                    (key.to_string(), TemperatureMeasurement {
                        target: temp[1],
                        current: temp[0],
                    })
                }).collect();
                Ok(PrinterTemperature(temps))
            },
            PrinterRequest::GetStatus => {
                let kv = parse_kv(input)?;
                debug!("{:?}", kv);
                Ok(PrinterStatus {
                    end_stop: EndStopPosition {
                        x_max: kv.get("X-max").unwrap().parse().unwrap(),
                        y_max: kv.get("Y-max").unwrap().parse().unwrap(),
                        z_min: kv.get("Z-min").unwrap().parse().unwrap(),
                    },
                    machine_status: kv.get("MachineStatus").unwrap().to_string(),
                    move_mode: kv.get("MoveMode").unwrap().to_string(),
                    led: kv.get("LED").unwrap() == "1",
                    current_file: kv.get("CurrentFile").map(|s| s.to_string()),
                })
            },
            PrinterRequest::GetHeadPosition => {
              let kv = parse_kv(input)?;
                Ok(PrinterHeadPosition {
                    x: kv.get("X").unwrap().parse().unwrap(),
                    y: kv.get("Y").unwrap().parse().unwrap(),
                    z: kv.get("Z").unwrap().parse().unwrap(),
                    a: kv.get("A").unwrap().parse().unwrap(),
                    b: kv.get("B").unwrap().parse().unwrap(),
                })
            },
            _ => {
                debug!("unknown request {:?}. content: {:?}", self, input);
                Err("unknown request".to_string())
            }
        }
    }
}

impl PrinterRequest {
    pub fn get_gcode(&self) -> &'static str {
        match self {
            PrinterRequest::ControlMessage => "~M601 S1",
            PrinterRequest::GetInfo => "~M115",
            PrinterRequest::GetHeadPosition => "~M114",
            PrinterRequest::GetTemperature => "~M105",
            PrinterRequest::GetProgress => "~M27",
            PrinterRequest::GetStatus => "~M119",
        }
    }
    pub fn get_instruction(&self) -> String {
        format!("{}\r\n", self.get_gcode())
    }
}