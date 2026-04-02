use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashSet;

/// With `expand_dropdowns=true`, GLPI returns names (strings) instead of IDs (numbers).
/// This deserializer handles both cases gracefully.
fn deserialize_string_or_number<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let val: serde_json::Value = Deserialize::deserialize(deserializer)?;
    match val {
        serde_json::Value::String(s) => Ok(s),
        serde_json::Value::Number(n) => Ok(n.to_string()),
        serde_json::Value::Null => Ok(String::new()),
        other => Ok(other.to_string()),
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct GlpiSoftware {
    pub id: u64,
    pub name: String,
    #[serde(default, deserialize_with = "deserialize_string_or_number")]
    pub manufacturers_id: String,
    #[serde(default)]
    #[allow(dead_code)]
    pub date_mod: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GlpiSoftwareVersion {
    pub id: u64,
    pub name: String,
    pub softwares_id: u64,
    #[serde(default)]
    #[allow(dead_code)]
    pub date_mod: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    pub date_creation: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GlpiItemSoftwareVersion {
    #[allow(dead_code)]
    pub id: u64,
    pub items_id: u64,
    pub softwareversions_id: u64,
    #[serde(default)]
    #[allow(dead_code)]
    pub itemtype: Option<String>,
    #[serde(default)]
    pub date_install: Option<String>,
    #[serde(default)]
    pub date_mod: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GlpiComputer {
    pub id: u64,
    pub name: String,
    #[serde(default)]
    pub contact: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ComputerInfo {
    pub name: String,
    pub contact: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct VersionDetail {
    pub version_id: u64,
    pub version_name: String,
    pub host_count: usize,
    pub last_install_date: Option<String>,
    #[serde(skip)]
    #[allow(dead_code)]
    pub host_ids: HashSet<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AggregatedSoftware {
    pub software_id: u64,
    pub name: String,
    pub publisher: String,
    pub total_host_count: usize,
    pub latest_version: String,
    pub last_updated: Option<String>,
    pub versions: Vec<VersionDetail>,
    #[serde(skip)]
    pub host_ids: HashSet<u64>,
}

#[derive(Debug, Clone)]
pub struct FilterState {
    pub software_name: String,
    pub publisher: String,
    pub min_hosts: String,
    pub recently_updated: bool,
    pub days: String,
    pub top_n: String,
    pub hide_os_defaults: bool,
    pub show_selected_only: bool,
}

impl Default for FilterState {
    fn default() -> Self {
        Self {
            software_name: String::new(),
            publisher: String::new(),
            min_hosts: String::new(),
            recently_updated: false,
            days: "30".to_string(),
            top_n: String::new(),
            hide_os_defaults: false,
            show_selected_only: false,
        }
    }
}

#[derive(Debug, Clone)]
pub enum FetchStatus {
    Idle,
    Connecting,
    FetchingSoftware { done: usize, total: Option<usize> },
    FetchingVersions { done: usize, total: Option<usize> },
    FetchingInstallations { done: usize, total: Option<usize> },
    FetchingComputers { done: usize, total: Option<usize> },
    Aggregating,
    Done { software_count: usize, total_hosts: usize },
    Error(String),
}

impl std::fmt::Display for FetchStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FetchStatus::Idle => write!(f, "Not connected"),
            FetchStatus::Connecting => write!(f, "Connecting..."),
            FetchStatus::FetchingSoftware { done, total } => {
                if let Some(t) = total {
                    write!(f, "Fetching software: {done}/{t}")
                } else {
                    write!(f, "Fetching software: {done}...")
                }
            }
            FetchStatus::FetchingVersions { done, total } => {
                if let Some(t) = total {
                    write!(f, "Fetching versions: {done}/{t}")
                } else {
                    write!(f, "Fetching versions: {done}...")
                }
            }
            FetchStatus::FetchingInstallations { done, total } => {
                if let Some(t) = total {
                    write!(f, "Fetching installations: {done}/{t}")
                } else {
                    write!(f, "Fetching installations: {done}...")
                }
            }
            FetchStatus::FetchingComputers { done, total } => {
                if let Some(t) = total {
                    write!(f, "Fetching computers: {done}/{t}")
                } else {
                    write!(f, "Fetching computers: {done}...")
                }
            }
            FetchStatus::Aggregating => write!(f, "Aggregating data..."),
            FetchStatus::Done { software_count, total_hosts } => {
                write!(f, "Loaded {software_count} software across {total_hosts} hosts")
            }
            FetchStatus::Error(e) => write!(f, "Error: {e}"),
        }
    }
}
