use crate::models::AggregatedSoftware;
use serde::Serialize;
use std::path::Path;

#[derive(Serialize)]
struct JsonReport {
    generated_at: String,
    software_count: usize,
    software: Vec<JsonSoftwareEntry>,
}

#[derive(Serialize)]
struct JsonSoftwareEntry {
    rank: usize,
    name: String,
    publisher: String,
    host_count: usize,
    latest_version: String,
    last_updated: Option<String>,
    versions: Vec<JsonVersionEntry>,
}

#[derive(Serialize)]
struct JsonVersionEntry {
    version_name: String,
    host_count: usize,
    last_install_date: Option<String>,
}

pub fn export_json(data: &[AggregatedSoftware], path: &Path) -> Result<(), String> {
    let report = JsonReport {
        generated_at: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        software_count: data.len(),
        software: data
            .iter()
            .enumerate()
            .map(|(i, sw)| JsonSoftwareEntry {
                rank: i + 1,
                name: sw.name.clone(),
                publisher: sw.publisher.clone(),
                host_count: sw.total_host_count,
                latest_version: sw.latest_version.clone(),
                last_updated: sw.last_updated.clone(),
                versions: sw
                    .versions
                    .iter()
                    .map(|v| JsonVersionEntry {
                        version_name: v.version_name.clone(),
                        host_count: v.host_count,
                        last_install_date: v.last_install_date.clone(),
                    })
                    .collect(),
            })
            .collect(),
    };

    let json = serde_json::to_string_pretty(&report)
        .map_err(|e| format!("JSON serialize error: {e}"))?;

    std::fs::write(path, json).map_err(|e| format!("Failed to write JSON: {e}"))?;

    Ok(())
}
