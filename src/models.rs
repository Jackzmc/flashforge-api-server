use serde::Serialize;

#[derive(Serialize)]
pub struct GenericError {
    pub error: String,
    pub message: Option<String>
}

#[derive(Serialize)]
pub struct Position {
    pub x: i32,
    pub y: i32,
    pub z: i32
}

#[derive(Serialize)]
pub struct EndStopPosition {
    pub x_max: i32,
    pub y_max: i32,
    pub z_min: i32
}

#[derive(Serialize)]
pub struct TemperatureMeasurement {
    pub target: f32,
    pub current: f32
}

// #[derive(Serialize)]
// pub struct PrinterInfo {
//     pub name: String,
//     pub firmware_version: String,
//     pub sn: String,
//     pub tool_count: u8,
//     pub model_name: String,
//     pub position: Position
// }