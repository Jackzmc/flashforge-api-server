use crate::models::{ControlSuccess, EndStopPosition, Position, PrinterHeadPosition, PrinterInfo, PrinterProgress, PrinterStatus, PrinterTemperature, TemperatureMeasurement};
use crate::util::parse_kv;
use log::{debug};
use regex::Regex;
use serde::Serialize;
use std::sync::LazyLock;

#[derive(Debug)]
pub enum PrinterRequest {
    ControlMessage,
    GetInfo,
    GetHeadPosition,
    GetTemperature,
    GetProgress,
    GetStatus,
    SetTemperature(u8, f32),
}

#[derive(Serialize)]
pub enum PrinterResponse {
    #[serde(rename = "success")]
    ControlSuccess(ControlSuccess),
    #[serde(rename = "info")]
    PrinterInfo(PrinterInfo),
    #[serde(rename = "position")]
    PrinterHeadPosition(PrinterHeadPosition),
    #[serde(rename = "temperatures")]
    PrinterTemperature(PrinterTemperature),
    #[serde(rename = "progress")]
    PrinterProgress(PrinterProgress),
    #[serde(rename = "status")]
    PrinterStatus(PrinterStatus),
}



static RE_PRINTER_PROGRESS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(\d+)/(\d+)").unwrap());
impl PrinterRequest {
    pub fn parse_response(&self, input: &str) -> Result<PrinterResponse, String> {
        match self {
            PrinterRequest::ControlMessage => Ok(PrinterResponse::ControlSuccess(ControlSuccess { success: true })),
            PrinterRequest::SetTemperature(_, _) => Ok(PrinterResponse::ControlSuccess(ControlSuccess { success: true})),
            PrinterRequest::GetInfo => {
                let kv = parse_kv(input)?;
                debug!("{:?}", &kv);
                Ok(PrinterResponse::PrinterInfo(PrinterInfo{
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
                }))
            },
            PrinterRequest::GetProgress => {
                let prog: Vec<(u32,u32)> = RE_PRINTER_PROGRESS.captures_iter(input)
                    .map(|c| (c[1].parse().unwrap(), c[2].parse().unwrap()))
                    .collect();
                if prog.is_empty() {
                    panic!("no matches found");
                }
                Ok(PrinterResponse::PrinterProgress(PrinterProgress {
                    byte: prog[0],
                    layer: prog[1],
                }))
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
                let pr = PrinterTemperature(temps);
                Ok(PrinterResponse::PrinterTemperature(pr))
            },
            PrinterRequest::GetStatus => {
                let kv = parse_kv(input)?;
                debug!("{:?}", kv);
                let current_file = kv.get("CurrentFile").filter(|s| !s.is_empty()).map(|s| s.to_string());
                Ok(PrinterResponse::PrinterStatus(PrinterStatus {
                    end_stop: EndStopPosition {
                        x_max: kv.get("X-max").unwrap().parse().unwrap(),
                        y_max: kv.get("Y-max").unwrap().parse().unwrap(),
                        z_min: kv.get("Z-min").unwrap().parse().unwrap(),
                    },
                    machine_status: kv.get("MachineStatus").unwrap().to_string(),
                    move_mode: kv.get("MoveMode").unwrap().to_string(),
                    led: kv.get("LED").unwrap() == "1",
                    current_file
                }))
            },
            PrinterRequest::GetHeadPosition => {
              let kv = parse_kv(input)?;
                Ok(PrinterResponse::PrinterHeadPosition(PrinterHeadPosition {
                    x: kv.get("X").unwrap().parse().unwrap(),
                    y: kv.get("Y").unwrap().parse().unwrap(),
                    z: kv.get("Z").unwrap().parse().unwrap(),
                    a: kv.get("A").unwrap().parse().unwrap(),
                    b: kv.get("B").unwrap().parse().unwrap(),
                }))
            },
            #[allow(unreachable_patterns)]
            _ => {
                debug!("unknown request {:?}. content: {:?}", self, input);
                Err("unknown request".to_string())
            }
        }
    }
}

// https://marlinfw.org/docs/gcode/M104.html
impl PrinterRequest {
    pub fn get_gcode(&self) -> String {
        match self {
            PrinterRequest::ControlMessage => "~M601 S1".to_string(),
            PrinterRequest::GetInfo => "~M115".to_string(),
            PrinterRequest::GetHeadPosition => "~M114".to_string(),
            PrinterRequest::GetTemperature => "~M105".to_string(),
            PrinterRequest::GetProgress => "~M27".to_string(),
            PrinterRequest::GetStatus => "~M119".to_string(),
            PrinterRequest::SetTemperature(index, temp) => format!("~M104 S{} T{}", temp, index)
        }
    }
    pub fn get_instruction(&self) -> String {
        format!("{}\r\n", self.get_gcode())
    }
}