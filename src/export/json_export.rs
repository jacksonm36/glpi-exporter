use crate::models::{AggregatedSoftware, ComputerInfo};
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Serialize)]
struct SoftwareTableReport {
    generated_at: String,
    software_count: usize,
    software: Vec<AggregatedSoftware>,
}

#[derive(Serialize)]
struct JsonReport {
    generated_at: String,
    computer_count: usize,
    computers: Vec<JsonComputerEntry>,
}

#[derive(Serialize)]
struct JsonComputerEntry {
    hostname: String,
    serial_number: String,
    model: String,
}

pub fn export_software_inventory_json(data: &[AggregatedSoftware], path: &Path) -> Result<(), String> {
    let report = SoftwareTableReport {
        generated_at: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        software_count: data.len(),
        software: data.to_vec(),
    };

    let json = serde_json::to_string_pretty(&report)
        .map_err(|e| format!("JSON serialize error: {e}"))?;

    std::fs::write(path, json).map_err(|e| format!("Failed to write JSON: {e}"))?;

    Ok(())
}

pub fn export_json(computers: &HashMap<u64, ComputerInfo>, path: &Path) -> Result<(), String> {
    let mut rows: Vec<&ComputerInfo> = computers.values().collect();
    rows.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    let report = JsonReport {
        generated_at: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        computer_count: rows.len(),
        computers: rows
            .iter()
            .map(|info| JsonComputerEntry {
                hostname: info.name.clone(),
                serial_number: info.serial_number.clone(),
                model: info.model.clone(),
            })
            .collect(),
    };

    let json = serde_json::to_string_pretty(&report)
        .map_err(|e| format!("JSON serialize error: {e}"))?;

    std::fs::write(path, json).map_err(|e| format!("Failed to write JSON: {e}"))?;

    Ok(())
}
