use std::collections::HashMap;
use serde::Serialize;

#[derive(Serialize)]
pub struct GenericError {
    pub error: String,
    pub message: Option<String>
}

#[derive(Serialize, Clone)]
pub struct Position {
    pub x: i32,
    pub y: i32,
    pub z: i32
}

#[derive(Serialize, Clone)]
pub struct EndStopPosition {
    pub x_max: i32,
    pub y_max: i32,
    pub z_min: i32
}

#[derive(Serialize, Clone)]
pub struct TemperatureMeasurement {
    pub target: f32,
    pub current: f32
}

#[derive(Serialize, Clone)]
pub struct PrinterInfo {
    pub name: String,
    pub firmware_version: String,
    pub sn: String,
    pub tool_count: u8,
    pub model_name: String,
    pub mac_addr: String,
    pub position: Position
}

#[derive(Serialize, Clone)]
pub struct CachedPrinterInfo {
    pub name: String,
    pub is_online: bool,
    pub current_file: Option<String>,
    pub firmware_version: Option<String>
}

#[derive(Serialize, Clone)]
pub struct PrinterHeadPosition {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub a: f32,
    pub b: u32
}

#[derive(Serialize, Clone)]
pub struct PrinterTemperature(pub HashMap<String, TemperatureMeasurement>);

#[derive(Serialize, Clone, Debug)]
pub struct PrinterProgress {
    pub layer: (u32, u32),
    pub byte: (u32, u32)
}
#[derive(Serialize, Clone)]
pub struct PrinterStatus {
    pub end_stop: EndStopPosition,
    pub machine_status: String, // "READY",
    pub move_mode: String, // "READY"
    // status: Option<>, // S:1, L:0, J:0, F:0
    pub led: bool,
    pub current_file: Option<String>
}