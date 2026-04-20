use crate::config;
use crate::models::{InventorySnapshot, SnapshotSummary};
use std::fs;
use std::path::PathBuf;

fn history_dir() -> PathBuf {
    config::app_dir().join("history")
}

pub fn save_snapshot(snapshot: &InventorySnapshot) -> Result<PathBuf, String> {
    let dir = history_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("Could not create history directory: {e}"))?;

    let stamp = sanitize_timestamp(&snapshot.captured_at);
    let mut path = dir.join(format!("snapshot-{stamp}.json"));
    let mut suffix = 1usize;
    while path.exists() {
        path = dir.join(format!("snapshot-{stamp}-{suffix}.json"));
        suffix += 1;
    }
    let json = serde_json::to_string_pretty(snapshot)
        .map_err(|e| format!("Could not serialize snapshot: {e}"))?;
    fs::write(&path, json).map_err(|e| format!("Could not save snapshot: {e}"))?;
    Ok(path)
}

pub fn load_snapshot(file_name: &str) -> Result<InventorySnapshot, String> {
    let path = safe_snapshot_path(file_name)?;
    let json = fs::read_to_string(&path).map_err(|e| format!("Could not read snapshot: {e}"))?;
    serde_json::from_str(&json).map_err(|e| format!("Could not parse snapshot: {e}"))
}

pub fn list_snapshots() -> Vec<SnapshotSummary> {
    let dir = history_dir();
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };

    let mut snapshots = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let Some(file_name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        let file_size_bytes = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        let Ok(json) = fs::read_to_string(&path) else {
            continue;
        };
        let Ok(snapshot) = serde_json::from_str::<InventorySnapshot>(&json) else {
            continue;
        };
        snapshots.push(SnapshotSummary {
            file_name: file_name.to_string(),
            captured_at: snapshot.captured_at,
            computer_count: snapshot.computers.len(),
            installation_count: snapshot.installations.len(),
            file_size_bytes,
        });
    }

    snapshots.sort_by(|a, b| b.captured_at.cmp(&a.captured_at));
    snapshots
}

pub fn delete_snapshot(file_name: &str) -> Result<(), String> {
    let path = safe_snapshot_path(file_name)?;
    fs::remove_file(&path).map_err(|e| format!("Could not delete snapshot: {e}"))
}

/// Deletes multiple snapshots. Returns the number removed, or an error if none were removed.
pub fn delete_snapshots(file_names: &[String]) -> Result<usize, String> {
    if file_names.is_empty() {
        return Ok(0);
    }
    let mut deleted = 0usize;
    let mut errors = Vec::new();
    for f in file_names {
        match delete_snapshot(f) {
            Ok(()) => deleted += 1,
            Err(e) => errors.push(format!("{f}: {e}")),
        }
    }
    if deleted == 0 {
        return Err(errors.join("; "));
    }
    if !errors.is_empty() {
        return Err(format!(
            "Removed {deleted} snapshot(s); some failed: {}",
            errors.join("; ")
        ));
    }
    Ok(deleted)
}

fn safe_snapshot_path(file_name: &str) -> Result<PathBuf, String> {
    if file_name.contains("..") || file_name.contains('/') || file_name.contains('\\') {
        return Err("Invalid snapshot file name".to_string());
    }
    if !file_name.ends_with(".json") {
        return Err("Invalid snapshot file name".to_string());
    }
    let dir = history_dir();
    let path = dir.join(file_name);
    if path.parent() != Some(dir.as_path()) {
        return Err("Invalid snapshot path".to_string());
    }
    Ok(path)
}

fn sanitize_timestamp(timestamp: &str) -> String {
    timestamp
        .chars()
        .map(|c| match c {
            '0'..='9' | 'a'..='z' | 'A'..='Z' | '-' | '_' => c,
            _ => '-',
        })
        .collect()
}
