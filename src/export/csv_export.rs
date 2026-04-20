use crate::models::{AggregatedSoftware, ComputerInfo};
use std::collections::HashMap;
use std::path::Path;

pub fn export_software_inventory_csv(data: &[AggregatedSoftware], path: &Path) -> Result<(), String> {
    let mut wtr =
        csv::Writer::from_path(path).map_err(|e| format!("Cannot create CSV file: {e}"))?;

    wtr.write_record([
        "Rank",
        "Software ID",
        "Software Name",
        "Publisher",
        "Host Count",
        "Latest Version",
        "Last install (any host)",
        "All-hosts install floor",
        "Last agent pull",
        "Last host inventory",
    ])
    .map_err(|e| format!("CSV write error: {e}"))?;

    for (i, sw) in data.iter().enumerate() {
        wtr.write_record([
            (i + 1).to_string(),
            sw.software_id.to_string(),
            sw.name.clone(),
            sw.publisher.clone(),
            sw.total_host_count.to_string(),
            sw.latest_version.clone(),
            sw.last_install_date.clone().unwrap_or_default(),
            sw.all_hosts_install_floor.clone().unwrap_or_default(),
            sw.last_agent_pull.clone().unwrap_or_default(),
            sw.last_host_inventory.clone().unwrap_or_default(),
        ])
        .map_err(|e| format!("CSV write error: {e}"))?;
    }

    wtr.flush().map_err(|e| format!("CSV flush error: {e}"))?;
    Ok(())
}

pub fn export_csv(computers: &HashMap<u64, ComputerInfo>, path: &Path) -> Result<(), String> {
    let mut wtr =
        csv::Writer::from_path(path).map_err(|e| format!("Cannot create CSV file: {e}"))?;

    wtr.write_record(["Hostname", "Serial Number", "Model"])
        .map_err(|e| format!("CSV write error: {e}"))?;

    let mut rows: Vec<&ComputerInfo> = computers.values().collect();
    rows.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    for info in rows {
        wtr.write_record([
            info.name.as_str(),
            info.serial_number.as_str(),
            info.model.as_str(),
        ])
        .map_err(|e| format!("CSV write error: {e}"))?;
    }

    wtr.flush().map_err(|e| format!("CSV flush error: {e}"))?;
    Ok(())
}
