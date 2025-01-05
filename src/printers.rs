use std::collections::HashMap;
use std::net::IpAddr;
use log::{debug, warn};
use crate::printer::Printer;

pub struct Printers {
    printers: HashMap<String, Printer>
}

impl Printers {
    pub fn new() -> Printers {
        Self {
            printers: HashMap::new()
        }
    }

    pub fn get_printer_names(&self) -> Vec<String> {
        self.printers.keys().map(|s| s.clone()).collect()
    }

    pub fn get_printer(&self, id: &str) -> Option<&Printer> {
        self.printers.get(id)
    }

    pub fn add_printer(&mut self, id: String, ip: IpAddr) {
        debug!("adding printer {} with ip {}", id, ip);
        let mut printer = Printer::new(ip);
        if printer.get_meta().is_none() {
            warn!("printer {} failed to get meta:", id);
        }
        self.printers.insert(id, printer);
    }
}