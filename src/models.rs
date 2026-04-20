use chrono::NaiveDate;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::{HashMap, HashSet};

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
    #[serde(default)]
    #[allow(dead_code)]
    pub date_creation: Option<String>,
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
    #[serde(default, deserialize_with = "deserialize_string_or_number")]
    pub serial: String,
    #[serde(default, deserialize_with = "deserialize_string_or_number")]
    pub computermodels_id: String,
    #[serde(default)]
    pub date_mod: Option<String>,
    #[serde(default)]
    pub date_creation: Option<String>,
    #[serde(flatten)]
    pub extra_fields: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GlpiSoftwareLicense {
    #[allow(dead_code)]
    pub id: u64,
    #[serde(default)]
    pub name: String,
    #[serde(default, deserialize_with = "deserialize_string_or_number")]
    pub softwares_id: String,
    #[serde(default, deserialize_with = "deserialize_string_or_number")]
    pub serial: String,
    #[serde(default)]
    pub comment: Option<String>,
    #[serde(flatten)]
    pub extra_fields: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct ComputerInfo {
    pub name: String,
    pub contact: String,
    pub serial_number: String,
    pub model: String,
    /// Latest known inventory timestamp from GLPI (Computer `date_mod` / `date_creation`).
    pub last_inventory: String,
    pub windows_product_key: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SoftwareCleanupCandidate {
    pub software_id: u64,
    pub name: String,
    pub publisher: String,
    pub date_mod: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum LicenseSource {
    Glpi,
    ComputerInventory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum LicenseCategory {
    Windows,
    Office,
    ThirdParty,
}

#[derive(Debug, Clone, Serialize)]
pub struct LicenseKeyRecord {
    pub source: LicenseSource,
    pub category: LicenseCategory,
    pub product_name: String,
    pub license_key: String,
    pub computer_id: Option<u64>,
    pub computer_name: Option<String>,
    pub user_contact: Option<String>,
    pub notes: Option<String>,
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
    /// Lowercase cache for filter matching (not serialized).
    #[serde(skip)]
    pub name_lower: String,
    #[serde(skip)]
    pub publisher_lower: String,
    pub total_host_count: usize,
    pub latest_version: String,
    pub last_install_date: Option<String>,
    pub last_updated: Option<String>,
    pub last_agent_pull: Option<String>,
    /// Newest `Computer` inventory date among hosts that have this software installed.
    pub last_host_inventory: Option<String>,
    /// Oldest among each host’s newest `date_install` (per host); hosts with only `date_mod` use that.
    /// Used for “every PC installed within window”. `None` if any host lacks usable dates.
    pub all_hosts_install_floor: Option<String>,
    #[serde(skip)]
    pub host_install_best_per_host: HashMap<u64, Option<String>>,
    #[serde(skip)]
    pub host_mod_best_per_host: HashMap<u64, Option<String>>,
    pub versions: Vec<VersionDetail>,
    #[serde(skip)]
    pub host_ids: HashSet<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventorySnapshot {
    pub captured_at: String,
    pub computers: Vec<SnapshotComputer>,
    pub installations: Vec<SnapshotInstallation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotComputer {
    pub id: u64,
    pub name: String,
    pub contact: String,
    pub serial_number: String,
    pub model: String,
    pub last_inventory: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotInstallation {
    pub computer_id: u64,
    pub software_id: u64,
    pub software_name: String,
    pub publisher: String,
    pub version_id: u64,
    pub version_name: String,
}

#[derive(Debug, Clone)]
pub struct SnapshotSummary {
    pub file_name: String,
    pub captured_at: String,
    pub computer_count: usize,
    pub installation_count: usize,
    pub file_size_bytes: u64,
}

/// Cached resolution of PC Software panel historical snapshot (from/to range → file load).
#[derive(Debug, Clone)]
pub enum PcSoftwareHistCache {
    InvalidDates,
    NoSnapshotInRange,
    LoadError,
    Ready {
        summary: SnapshotSummary,
        inventory: InventorySnapshot,
    },
}

#[derive(Debug, Clone)]
pub struct HistoricalComputerEntry {
    pub computer_name: String,
    pub contact: String,
    pub serial_number: String,
    #[allow(dead_code)]
    pub model: String,
    #[allow(dead_code)]
    pub last_inventory: String,
    pub versions: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct HistoricalSoftwareEntry {
    pub software_key: String,
    pub software_name: String,
    pub publisher: String,
    pub host_count: usize,
    pub versions: Vec<String>,
    pub computers: Vec<HistoricalComputerEntry>,
}

#[derive(Debug, Clone)]
pub struct HistoryViewSummary {
    pub snapshot_captured_at: String,
    pub software_count: usize,
    pub host_count: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GlpiAgent {
    #[allow(dead_code)]
    pub id: u64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    #[allow(dead_code)]
    pub deviceid: Option<String>,
    #[serde(default)]
    pub last_contact: Option<String>,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub items_id: u64,
    #[serde(default)]
    pub itemtype: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentStatus {
    Online,
    Stale,
    Offline,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PingResult {
    Reachable,
    Unreachable,
    Pending,
    NotChecked,
}

#[derive(Debug, Clone)]
pub struct AgentInfo {
    pub computer_id: u64,
    pub computer_name: String,
    pub agent_name: String,
    pub last_contact: Option<String>,
    #[allow(dead_code)]
    pub port: u16,
    pub version: String,
    pub status: AgentStatus,
    pub ping: PingResult,
}

#[derive(Debug, Clone)]
pub struct SnapshotDiff {
    pub added: Vec<DiffEntry>,
    pub removed: Vec<DiffEntry>,
    pub changed: Vec<DiffEntry>,
}

#[derive(Debug, Clone)]
pub struct DiffEntry {
    pub software_name: String,
    pub publisher: String,
    pub hosts_a: usize,
    pub hosts_b: usize,
    #[allow(dead_code)]
    pub versions_a: Vec<String>,
    #[allow(dead_code)]
    pub versions_b: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GlpiLogEntry {
    #[allow(dead_code)]
    pub id: u64,
    #[serde(default)]
    pub date_mod: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    pub user_name: Option<String>,
    #[serde(default)]
    pub linked_action: Option<i64>,
    #[serde(default)]
    pub itemtype_link: Option<String>,
    #[serde(default)]
    pub old_value: Option<String>,
    #[serde(default)]
    pub new_value: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PcLogAction {
    Installed,
    Updated,
    Removed,
}

#[derive(Debug, Clone)]
pub struct PcSoftwareLogEntry {
    pub date: String,
    pub action: PcLogAction,
    pub software_name: String,
    pub old_value: String,
    pub new_value: String,
}

/// One uninstall event from GLPI history (aggregated across PCs for the main table).
#[derive(Debug, Clone)]
pub struct GlobalAuditRemovalRow {
    pub computer_id: u64,
    pub software_name: String,
    pub removed_at: String,
}

#[derive(Debug, Clone)]
pub struct AuditRemovalItem {
    pub computer_id: u64,
    pub computer_name: String,
    pub removed_at: String,
}

#[derive(Debug, Clone, Default)]
pub struct AuditRemovalGroup {
    pub display_label: String,
    pub items: Vec<AuditRemovalItem>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RecentTimeMode {
    #[default]
    RollingDays,
    CutoffFrom,
    Between,
}

#[derive(Debug, Clone)]
pub struct FilterState {
    pub software_name: String,
    pub publisher: String,
    pub min_hosts: String,
    pub recently_updated: bool,
    pub recent_install_only: bool,
    /// Every host’s install evidence (see `all_hosts_install_floor`) must fall in the recency window.
    pub every_host_install_in_window: bool,
    /// When `recently_updated` is on, use host PC last inventory instead of install-row `date_mod`.
    pub recent_use_host_inventory: bool,
    pub days: String,
    /// How `recently_updated` / `recent_install_only` / Recent column interpret dates.
    pub recent_time_mode: RecentTimeMode,
    pub recent_cutoff_from: NaiveDate,
    pub recent_range_from: NaiveDate,
    pub recent_range_to: NaiveDate,
    pub top_n: String,
    pub hide_os_defaults: bool,
    pub show_selected_only: bool,
}

impl Default for FilterState {
    fn default() -> Self {
        let today = chrono::Local::now().date_naive();
        let month_ago = today
            .checked_sub_signed(chrono::Duration::days(30))
            .unwrap_or(today);
        Self {
            software_name: String::new(),
            publisher: String::new(),
            min_hosts: String::new(),
            recently_updated: false,
            recent_install_only: false,
            every_host_install_in_window: false,
            recent_use_host_inventory: false,
            days: "30".to_string(),
            recent_time_mode: RecentTimeMode::default(),
            recent_cutoff_from: month_ago,
            recent_range_from: month_ago,
            recent_range_to: today,
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
    FetchingLicenses { done: usize, total: Option<usize> },
    FetchingComputers { done: usize, total: Option<usize> },
    FetchingAgents { done: usize, total: Option<usize> },
    CleanupPreview { count: usize, days: i64 },
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
            FetchStatus::FetchingLicenses { done, total } => {
                if let Some(t) = total {
                    write!(f, "Fetching licenses: {done}/{t}")
                } else {
                    write!(f, "Fetching licenses: {done}...")
                }
            }
            FetchStatus::FetchingComputers { done, total } => {
                if let Some(t) = total {
                    write!(f, "Fetching computers: {done}/{t}")
                } else {
                    write!(f, "Fetching computers: {done}...")
                }
            }
            FetchStatus::FetchingAgents { done, total } => {
                if let Some(t) = total {
                    write!(f, "Fetching agents: {done}/{t}")
                } else {
                    write!(f, "Fetching agents: {done}...")
                }
            }
            FetchStatus::CleanupPreview { count, days } => {
                write!(f, "Dry-run found {count} software older than {days} days")
            }
            FetchStatus::Aggregating => write!(f, "Aggregating data..."),
            FetchStatus::Done { software_count, total_hosts } => {
                write!(f, "Loaded {software_count} software across {total_hosts} hosts")
            }
            FetchStatus::Error(e) => write!(f, "Error: {e}"),
        }
    }
}

impl FetchStatus {
    /// Main inventory load is not in progress (side requests like audit log are OK).
    pub fn allows_side_queries(&self) -> bool {
        matches!(
            self,
            FetchStatus::Idle | FetchStatus::Done { .. } | FetchStatus::Error(_)
        )
    }
}
