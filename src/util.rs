use std::collections::HashMap;
use std::sync::LazyLock;
use log::{debug, trace, warn};
use regex::Regex;

static RE_KV: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"([a-zA-Z0-9\-\s]+):\s*([^:\s]+)").unwrap());


/// Reads an input stream, with key: value per line or key1: val1 key2: val2
pub fn parse_multi_line(input: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();

    for cap in RE_KV.captures_iter(input) {
        // let val = Some(cap[2].to_string()).filter(|s| !s.is_empty());
        map.insert(cap[1].trim_start().to_string(), cap[2].to_string());
    }

    map
}

pub fn parse_kv(content: &str) -> Result<HashMap<String, String>, String> {
    trace!("parsing: {:?}", content);
    let mut kv = HashMap::new();
    // Skip first line ("CMD <GCODE> Received\r\n"), rest should be kv
    for line in content.lines().skip(1) {
        if line == "ok" {
            debug!("kv: {:?}", kv);
            return Ok(kv);
        }
        // Default will parse it as only key: value, but some cases we need to parse differently
        if let Ok((key, val)) = line.split_once(":").ok_or("invalid line") {
            if key == "X" {
                let p = parse_multi_line(line);
                kv.extend(p);
                // let pieces: Vec<&str> = line.split(" ").collect();
                // kv.insert("X", pieces[1]);
                // kv.insert("Y", pieces[3]);
                // kv.insert("Z", pieces[5]);
            } else if key == "Endstop" {
                let p = parse_multi_line(val);
                kv.extend(p);
            } else if key == "T0" {
                let p = parse_multi_line(line);
                kv.extend(p);
            } else {
                // kv.insert(key, val.trim_start());
                kv.insert(key.to_string(), val.trim_start().to_string());
            }
        } else {
            warn!("Invalid line: {}", line);
            continue;
        }
    }
    warn!("end of data, but did not see \"ok\"");
    Ok(kv)
}