use crate::aggregator;
use crate::date_util;
use crate::glpi_client::GlpiClient;
use crate::history_store;
use crate::models::*;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::mpsc::{Receiver, Sender};

pub enum WorkerRequest {
    FetchAll {
        url: String,
        user_token: String,
        app_token: String,
        accept_invalid_certs: bool,
    },
    FetchMachineDetails {
        url: String,
        user_token: String,
        app_token: String,
        accept_invalid_certs: bool,
        machine_id: u64,
    },
    DryRunCleanup {
        url: String,
        user_token: String,
        app_token: String,
        accept_invalid_certs: bool,
        older_than_days: i64,
    },
    PingComputers {
        targets: Vec<(u64, String)>,
    },
    FetchPcLog {
        url: String,
        user_token: String,
        app_token: String,
        accept_invalid_certs: bool,
        computer_id: u64,
        days_back: i64,
    },
    FetchPcSoftwareLog {
        url: String,
        user_token: String,
        app_token: String,
        accept_invalid_certs: bool,
        computer_id: u64,
        days_back: i64,
    },
    FetchGlobalAuditRemovals {
        url: String,
        user_token: String,
        app_token: String,
        accept_invalid_certs: bool,
        computer_ids: Vec<u64>,
        filters: FilterState,
    },
}

pub enum WorkerResponse {
    Status(FetchStatus),
    Data {
        software: Vec<AggregatedSoftware>,
        computers: HashMap<u64, ComputerInfo>,
        license_keys: Vec<LicenseKeyRecord>,
        agents: Vec<AgentInfo>,
        snapshot_warning: Option<String>,
    },
    MachineDetailsLoaded {
        machine_id: u64,
        info: ComputerInfo,
    },
    CleanupPreviewReady {
        items: Vec<SoftwareCleanupCandidate>,
        days: i64,
        skipped_no_date: usize,
    },
    PingResults {
        results: HashMap<u64, bool>,
    },
    PcLogReady {
        #[allow(dead_code)]
        computer_id: u64,
        entries: Vec<PcSoftwareLogEntry>,
    },
    PcLogError(String),
    PcSoftwareLogReady {
        computer_id: u64,
        entries: Vec<PcSoftwareLogEntry>,
    },
    PcSoftwareLogError {
        computer_id: u64,
        message: String,
    },
    GlobalAuditRemovalsProgress { done: usize, total: usize },
    GlobalAuditRemovalsReady {
        removals: Vec<GlobalAuditRemovalRow>,
        partial_errors: Vec<String>,
    },
    GlobalAuditRemovalsError(String),
    MachineDetailsError(String),
    Error(String),
}

pub fn spawn_worker(
    req_rx: Receiver<WorkerRequest>,
    resp_tx: Sender<WorkerResponse>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        while let Ok(request) = req_rx.recv() {
            match request {
                WorkerRequest::FetchAll {
                    url,
                    user_token,
                    app_token,
                    accept_invalid_certs,
                } => {
                    handle_fetch(&url, &user_token, &app_token, accept_invalid_certs, &resp_tx);
                }
                WorkerRequest::FetchMachineDetails {
                    url,
                    user_token,
                    app_token,
                    accept_invalid_certs,
                    machine_id,
                } => {
                    handle_machine_details_fetch(
                        &url,
                        &user_token,
                        &app_token,
                        accept_invalid_certs,
                        machine_id,
                        &resp_tx,
                    );
                }
                WorkerRequest::DryRunCleanup {
                    url,
                    user_token,
                    app_token,
                    accept_invalid_certs,
                    older_than_days,
                } => {
                    handle_cleanup_dry_run(
                        &url,
                        &user_token,
                        &app_token,
                        accept_invalid_certs,
                        older_than_days,
                        &resp_tx,
                    );
                }
                WorkerRequest::PingComputers { targets } => {
                    handle_ping_computers(targets, &resp_tx);
                }
                WorkerRequest::FetchPcLog {
                    url,
                    user_token,
                    app_token,
                    accept_invalid_certs,
                    computer_id,
                    days_back,
                } => {
                    handle_fetch_pc_log(
                        &url,
                        &user_token,
                        &app_token,
                        accept_invalid_certs,
                        computer_id,
                        days_back,
                        &resp_tx,
                    );
                }
                WorkerRequest::FetchPcSoftwareLog {
                    url,
                    user_token,
                    app_token,
                    accept_invalid_certs,
                    computer_id,
                    days_back,
                } => {
                    handle_fetch_pc_software_log(
                        &url,
                        &user_token,
                        &app_token,
                        accept_invalid_certs,
                        computer_id,
                        days_back,
                        &resp_tx,
                    );
                }
                WorkerRequest::FetchGlobalAuditRemovals {
                    url,
                    user_token,
                    app_token,
                    accept_invalid_certs,
                    computer_ids,
                    filters,
                } => {
                    handle_fetch_global_audit_removals(
                        &url,
                        &user_token,
                        &app_token,
                        accept_invalid_certs,
                        computer_ids,
                        filters,
                        &resp_tx,
                    );
                }
            }
        }
    })
}

fn handle_fetch(
    url: &str,
    user_token: &str,
    app_token: &str,
    accept_invalid_certs: bool,
    resp_tx: &Sender<WorkerResponse>,
) {
    let _ = resp_tx.send(WorkerResponse::Status(FetchStatus::Connecting));

    let app_tok = if app_token.is_empty() {
        None
    } else {
        Some(app_token)
    };

    let mut client = match GlpiClient::new(url, app_tok, accept_invalid_certs) {
        Ok(c) => c,
        Err(e) => {
            let _ = resp_tx.send(WorkerResponse::Error(e));
            return;
        }
    };

    if let Err(e) = client.init_session(user_token) {
        let _ = resp_tx.send(WorkerResponse::Error(e));
        return;
    }

    let status_tx = {
        let resp_tx = resp_tx.clone();
        let (tx, rx) = std::sync::mpsc::channel::<FetchStatus>();
        std::thread::spawn(move || {
            while let Ok(status) = rx.recv() {
                let _ = resp_tx.send(WorkerResponse::Status(status));
            }
        });
        tx
    };

    let software = match client.fetch_software(&status_tx) {
        Ok(s) => s,
        Err(e) => {
            let _ = resp_tx.send(WorkerResponse::Error(e));
            return;
        }
    };

    let versions = match client.fetch_software_versions(&status_tx) {
        Ok(v) => v,
        Err(e) => {
            let _ = resp_tx.send(WorkerResponse::Error(e));
            return;
        }
    };

    let installations = match client.fetch_item_software_versions(&status_tx) {
        Ok(i) => i,
        Err(e) => {
            let _ = resp_tx.send(WorkerResponse::Error(e));
            return;
        }
    };

    let computers = match client.fetch_computers(&status_tx) {
        Ok(c) => c,
        Err(e) => {
            let _ = resp_tx.send(WorkerResponse::Error(e));
            return;
        }
    };

    let licenses = match client.fetch_software_licenses(&status_tx) {
        Ok(v) => v,
        Err(_) => Vec::new(),
    };

    let glpi_agents = match client.fetch_agents(&status_tx) {
        Ok(a) => a,
        Err(_) => Vec::new(),
    };

    let _ = resp_tx.send(WorkerResponse::Status(FetchStatus::Aggregating));

    let license_keys = aggregator::aggregate_license_keys(&software, &licenses, &computers);

    let computer_map: HashMap<u64, ComputerInfo> = computers
        .iter()
        .map(|c| (c.id, computer_to_info(c, None)))
        .collect();

    let agents = build_agent_info_list(&glpi_agents, &computer_map);

    let aggregated = aggregator::aggregate(&software, &versions, &installations, &computers);
    let snapshot = build_inventory_snapshot(&software, &versions, &installations, &computers);
    let snapshot_warning = history_store::save_snapshot(&snapshot).err();

    let total_hosts: usize = {
        let mut all_hosts = std::collections::HashSet::new();
        for sw in &aggregated {
            for h in &sw.host_ids {
                all_hosts.insert(*h);
            }
        }
        all_hosts.len()
    };

    let _ = resp_tx.send(WorkerResponse::Status(FetchStatus::Done {
        software_count: aggregated.len(),
        total_hosts,
    }));
    let _ = resp_tx.send(WorkerResponse::Data {
        software: aggregated,
        computers: computer_map,
        license_keys,
        agents,
        snapshot_warning,
    });
}

fn handle_machine_details_fetch(
    url: &str,
    user_token: &str,
    app_token: &str,
    accept_invalid_certs: bool,
    machine_id: u64,
    resp_tx: &Sender<WorkerResponse>,
) {
    let app_tok = if app_token.is_empty() {
        None
    } else {
        Some(app_token)
    };

    let mut client = match GlpiClient::new(url, app_tok, accept_invalid_certs) {
        Ok(c) => c,
        Err(e) => {
            let _ = resp_tx.send(WorkerResponse::MachineDetailsError(e));
            return;
        }
    };

    if let Err(e) = client.init_session(user_token) {
        let _ = resp_tx.send(WorkerResponse::MachineDetailsError(e));
        return;
    }

    match client.fetch_computer_by_id(machine_id) {
        Ok(machine) => {
            let windows_key = client
                .fetch_windows_product_key_by_machine(machine_id)
                .ok()
                .flatten()
                .unwrap_or_else(|| pick_windows_product_key(&machine));
            let _ = resp_tx.send(WorkerResponse::MachineDetailsLoaded {
                machine_id,
                info: computer_to_info(&machine, Some(windows_key)),
            });
        }
        Err(e) => {
            let _ = resp_tx.send(WorkerResponse::MachineDetailsError(e));
        }
    }
}

fn handle_cleanup_dry_run(
    url: &str,
    user_token: &str,
    app_token: &str,
    accept_invalid_certs: bool,
    older_than_days: i64,
    resp_tx: &Sender<WorkerResponse>,
) {
    let _ = resp_tx.send(WorkerResponse::Status(FetchStatus::Connecting));
    let app_tok = if app_token.is_empty() {
        None
    } else {
        Some(app_token)
    };

    let mut client = match GlpiClient::new(url, app_tok, accept_invalid_certs) {
        Ok(c) => c,
        Err(e) => {
            let _ = resp_tx.send(WorkerResponse::Error(e));
            return;
        }
    };

    if let Err(e) = client.init_session(user_token) {
        let _ = resp_tx.send(WorkerResponse::Error(e));
        return;
    }

    let status_tx = {
        let resp_tx = resp_tx.clone();
        let (tx, rx) = std::sync::mpsc::channel::<FetchStatus>();
        std::thread::spawn(move || {
            while let Ok(status) = rx.recv() {
                let _ = resp_tx.send(WorkerResponse::Status(status));
            }
        });
        tx
    };

    let software = match client.fetch_software(&status_tx) {
        Ok(s) => s,
        Err(e) => {
            let _ = resp_tx.send(WorkerResponse::Error(e));
            return;
        }
    };

    let versions = match client.fetch_software_versions(&status_tx) {
        Ok(v) => v,
        Err(e) => {
            let _ = resp_tx.send(WorkerResponse::Error(e));
            return;
        }
    };

    let installations = match client.fetch_item_software_versions(&status_tx) {
        Ok(i) => i,
        Err(e) => {
            let _ = resp_tx.send(WorkerResponse::Error(e));
            return;
        }
    };

    let days = older_than_days.max(1);
    let now = chrono::Local::now().naive_local().date();
    let cutoff = now - chrono::Duration::days(days);

    let mut latest_by_sw: HashMap<u64, String> = HashMap::new();
    for sw in &software {
        if let Some(d) = sw.date_mod.as_deref() {
            update_latest_date(&mut latest_by_sw, sw.id, d);
        }
        if let Some(d) = sw.date_creation.as_deref() {
            update_latest_date(&mut latest_by_sw, sw.id, d);
        }
    }

    let mut version_to_sw: HashMap<u64, u64> = HashMap::new();
    for v in &versions {
        version_to_sw.entry(v.id).or_insert(v.softwares_id);
        if let Some(d) = v.date_mod.as_deref() {
            update_latest_date(&mut latest_by_sw, v.softwares_id, d);
        }
        if let Some(d) = v.date_creation.as_deref() {
            update_latest_date(&mut latest_by_sw, v.softwares_id, d);
        }
    }

    for inst in &installations {
        let Some(sw_id) = version_to_sw.get(&inst.softwareversions_id).copied() else {
            continue;
        };
        if let Some(d) = inst.date_install.as_deref() {
            update_latest_date(&mut latest_by_sw, sw_id, d);
        }
        if let Some(d) = inst.date_mod.as_deref() {
            update_latest_date(&mut latest_by_sw, sw_id, d);
        }
    }

    let mut skipped_no_date = 0usize;
    let mut items: Vec<SoftwareCleanupCandidate> = Vec::new();
    for s in software {
        let Some(latest) = latest_by_sw.get(&s.id) else {
            skipped_no_date += 1;
            continue;
        };
        let Some(modified) = date_util::parse_date(latest) else {
            skipped_no_date += 1;
            continue;
        };
        if modified >= cutoff {
            continue;
        }

        items.push(SoftwareCleanupCandidate {
            software_id: s.id,
            name: s.name,
            publisher: if s.manufacturers_id.trim().is_empty()
                || s.manufacturers_id == "0"
                || s.manufacturers_id == "&nbsp;"
            {
                "Unknown".to_string()
            } else {
                s.manufacturers_id
            },
            date_mod: modified.to_string(),
        });
    }

    items.sort_by(|a, b| a.date_mod.cmp(&b.date_mod).then_with(|| a.name.cmp(&b.name)));

    let _ = resp_tx.send(WorkerResponse::Status(FetchStatus::CleanupPreview {
        count: items.len(),
        days,
    }));
    let _ = resp_tx.send(WorkerResponse::CleanupPreviewReady {
        items,
        days,
        skipped_no_date,
    });
}

fn update_latest_date(latest_by_sw: &mut HashMap<u64, String>, software_id: u64, candidate: &str) {
    if candidate.trim().is_empty() {
        return;
    }
    let entry = latest_by_sw.entry(software_id).or_default();
    if entry.is_empty() || date_util::date_is_newer(candidate, entry) {
        *entry = candidate.to_string();
    }
}

fn build_inventory_snapshot(
    software: &[GlpiSoftware],
    versions: &[GlpiSoftwareVersion],
    installations: &[GlpiItemSoftwareVersion],
    computers: &[GlpiComputer],
) -> InventorySnapshot {
    let captured_at = chrono::Local::now()
        .naive_local()
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();

    let software_by_id: HashMap<u64, &GlpiSoftware> = software.iter().map(|s| (s.id, s)).collect();
    let version_by_id: HashMap<u64, &GlpiSoftwareVersion> = versions.iter().map(|v| (v.id, v)).collect();

    let computers = computers
        .iter()
        .map(|c| {
            let info = computer_to_info(c, None);
            SnapshotComputer {
                id: c.id,
                name: info.name,
                contact: info.contact,
                serial_number: info.serial_number,
                model: info.model,
                last_inventory: info.last_inventory,
            }
        })
        .collect();

    let installations = installations
        .iter()
        .filter_map(|inst| {
            let version = version_by_id.get(&inst.softwareversions_id)?;
            let software = software_by_id.get(&version.softwares_id)?;
            Some(SnapshotInstallation {
                computer_id: inst.items_id,
                software_id: software.id,
                software_name: software.name.clone(),
                publisher: if software.manufacturers_id.trim().is_empty()
                    || software.manufacturers_id == "0"
                    || software.manufacturers_id == "&nbsp;"
                {
                    "Unknown".to_string()
                } else {
                    software.manufacturers_id.clone()
                },
                version_id: version.id,
                version_name: version.name.clone(),
            })
        })
        .collect();

    InventorySnapshot {
        captured_at,
        computers,
        installations,
    }
}

fn computer_to_info(c: &GlpiComputer, windows_product_key: Option<String>) -> ComputerInfo {
    ComputerInfo {
        name: c.name.clone(),
        contact: c.contact.clone().unwrap_or_default(),
        serial_number: pick_serial_number(c),
        model: pick_model(c),
        last_inventory: aggregator::computer_inventory_timestamp(c).unwrap_or_default(),
        windows_product_key: windows_product_key.unwrap_or_else(|| pick_windows_product_key(c)),
    }
}

fn pick_serial_number(c: &GlpiComputer) -> String {
    if !is_blank_value(&c.serial) {
        return c.serial.trim().to_string();
    }
    for (field, value) in &c.extra_fields {
        let key = field.to_lowercase();
        if key.contains("serial") {
            let candidate = value_to_string(value);
            if !is_blank_value(&candidate) {
                return candidate;
            }
        }
    }
    String::new()
}

fn pick_model(c: &GlpiComputer) -> String {
    if !is_blank_value(&c.computermodels_id) {
        return c.computermodels_id.trim().to_string();
    }
    for (field, value) in &c.extra_fields {
        let key = field.to_lowercase();
        if key.contains("model") {
            let candidate = value_to_string(value);
            if !is_blank_value(&candidate) {
                return candidate;
            }
        }
    }
    String::new()
}

fn pick_windows_product_key(c: &GlpiComputer) -> String {
    let preferred = [
        "operatingsystems_serial",
        "operatingsystem_serial",
        "operatingsystems_productid",
        "operatingsystem_productid",
        "windows_product_key",
        "windows_key",
        "productkey",
        "product_key",
        "productid",
    ];
    for (field, value) in &c.extra_fields {
        let key = field.to_lowercase();
        if preferred.iter().any(|needle| key.contains(needle)) {
            let candidate = value_to_string(value);
            if is_windows_key_like(&candidate) {
                return candidate;
            }
        }
    }
    String::new()
}

fn value_to_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.trim().to_string(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        _ => String::new(),
    }
}

fn is_blank_value(value: &str) -> bool {
    let v = value.trim();
    v.is_empty() || v == "0" || v == "&nbsp;"
}

fn is_windows_key_like(value: &str) -> bool {
    let v = value.trim();
    if v.len() < 15 || v.len() > 80 {
        return false;
    }
    let valid_chars = v.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
    let hyphen_count = v.chars().filter(|c| *c == '-').count();
    valid_chars && hyphen_count >= 3
}

fn build_agent_info_list(
    glpi_agents: &[GlpiAgent],
    computer_map: &HashMap<u64, ComputerInfo>,
) -> Vec<AgentInfo> {
    let now = chrono::Local::now().naive_local();
    let stale_threshold = chrono::Duration::days(7);

    let mut seen_computer_ids = std::collections::HashSet::new();

    let mut result: Vec<AgentInfo> = glpi_agents
        .iter()
        .filter(|a| {
            a.itemtype.as_deref().unwrap_or_default() == "Computer" && a.items_id > 0
        })
        .map(|a| {
            seen_computer_ids.insert(a.items_id);

            let computer_name = computer_map
                .get(&a.items_id)
                .map(|c| c.name.clone())
                .unwrap_or_else(|| format!("Computer #{}", a.items_id));

            let status = match &a.last_contact {
                Some(lc) if !lc.is_empty() => {
                    if let Some(dt) = date_util::parse_datetime(lc) {
                        if now.signed_duration_since(dt) < chrono::Duration::hours(1) {
                            AgentStatus::Online
                        } else if now.signed_duration_since(dt) < stale_threshold {
                            AgentStatus::Stale
                        } else {
                            AgentStatus::Offline
                        }
                    } else {
                        AgentStatus::Unknown
                    }
                }
                _ => AgentStatus::Unknown,
            };

            AgentInfo {
                computer_id: a.items_id,
                computer_name,
                agent_name: a.name.clone(),
                last_contact: a.last_contact.clone(),
                port: a.port.unwrap_or(62354),
                version: a.version.clone().unwrap_or_default(),
                status,
                ping: PingResult::NotChecked,
            }
        })
        .collect();

    for (&computer_id, info) in computer_map {
        if seen_computer_ids.contains(&computer_id) {
            continue;
        }
        result.push(AgentInfo {
            computer_id,
            computer_name: info.name.clone(),
            agent_name: String::new(),
            last_contact: None,
            port: 0,
            version: String::new(),
            status: AgentStatus::Unknown,
            ping: PingResult::NotChecked,
        });
    }

    result.sort_by(|a, b| a.computer_name.to_lowercase().cmp(&b.computer_name.to_lowercase()));
    result
}

fn handle_ping_computers(targets: Vec<(u64, String)>, resp_tx: &Sender<WorkerResponse>) {
    use std::sync::{Arc, Mutex};

    const MAX_CONCURRENT_PINGS: usize = 20;

    let results = Arc::new(Mutex::new(HashMap::new()));

    for chunk in targets.chunks(MAX_CONCURRENT_PINGS) {
        let mut handles = Vec::new();
        for (computer_id, hostname) in chunk {
            let results = Arc::clone(&results);
            let computer_id = *computer_id;
            let hostname = hostname.clone();
            let handle = std::thread::spawn(move || {
                let reachable = ping_host(&hostname);
                let mut guard = results
                    .lock()
                    .unwrap_or_else(|e| e.into_inner());
                guard.insert(computer_id, reachable);
            });
            handles.push(handle);
        }
        for h in handles {
            let _ = h.join();
        }
    }

    let results = match Arc::try_unwrap(results) {
        Ok(mutex) => mutex.into_inner().unwrap_or_else(|e| e.into_inner()),
        Err(arc) => arc
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone(),
    };
    let _ = resp_tx.send(WorkerResponse::PingResults { results });
}

fn ping_host(hostname: &str) -> bool {
    use std::process::Command;

    let sanitized = hostname.trim();
    if sanitized.is_empty() || !is_safe_hostname(sanitized) {
        return false;
    }

    #[cfg(target_os = "windows")]
    let output = {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        Command::new("ping")
            .args(["-n", "1", "-w", "2000", sanitized])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
    };

    #[cfg(not(target_os = "windows"))]
    let output = Command::new("ping")
        .args(["-c", "1", "-W", "2", sanitized])
        .output();

    match output {
        Ok(o) => o.status.success(),
        Err(_) => false,
    }
}

fn is_safe_hostname(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 253
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_')
        && !s.starts_with('-')
        && !s.starts_with('.')
}

/// GLPI `Log` history constants (see `glpi/src/Log.php`, e.g. 10.0.x).
fn classify_glpi_software_history(linked_action: i64, old_val: &str, new_val: &str) -> PcLogAction {
    const HISTORY_INSTALL_SOFTWARE: i64 = 4;
    const HISTORY_UNINSTALL_SOFTWARE: i64 = 5;
    const HISTORY_DEL_RELATION: i64 = 16;
    const HISTORY_DELETE_SUBITEM: i64 = 19;

    let old_t = old_val.trim();
    let new_t = new_val.trim();

    // Explicit GLPI action codes (uninstall used to be misclassified as "Installed"
    // because the old code treated any linked_action > 0 as install).
    if linked_action == HISTORY_UNINSTALL_SOFTWARE {
        return PcLogAction::Removed;
    }
    if linked_action == HISTORY_INSTALL_SOFTWARE {
        return PcLogAction::Installed;
    }
    if linked_action == HISTORY_DEL_RELATION || linked_action == HISTORY_DELETE_SUBITEM {
        return PcLogAction::Removed;
    }

    if !old_t.is_empty() && !new_t.is_empty() {
        return PcLogAction::Updated;
    }
    if !new_t.is_empty() && old_t.is_empty() {
        return PcLogAction::Installed;
    }
    if !old_t.is_empty() && new_t.is_empty() {
        return PcLogAction::Removed;
    }

    if linked_action > 0 {
        PcLogAction::Installed
    } else {
        PcLogAction::Removed
    }
}

fn filter_state_rolling_days(days_back: i64) -> FilterState {
    let mut f = FilterState::default();
    f.recent_time_mode = RecentTimeMode::RollingDays;
    f.days = days_back.max(1).to_string();
    f
}

fn fetch_pc_software_history_entries(
    url: &str,
    user_token: &str,
    app_token: &str,
    accept_invalid_certs: bool,
    computer_id: u64,
    days_back: i64,
) -> Result<Vec<PcSoftwareLogEntry>, String> {
    fetch_pc_software_history_entries_filtered(
        url,
        user_token,
        app_token,
        accept_invalid_certs,
        computer_id,
        &filter_state_rolling_days(days_back),
    )
}

fn fetch_pc_software_history_entries_filtered(
    url: &str,
    user_token: &str,
    app_token: &str,
    accept_invalid_certs: bool,
    computer_id: u64,
    filters: &FilterState,
) -> Result<Vec<PcSoftwareLogEntry>, String> {
    let app_tok = if app_token.is_empty() {
        None
    } else {
        Some(app_token)
    };

    let mut client = GlpiClient::new(url, app_tok, accept_invalid_certs)?;
    client.init_session(user_token)?;
    fetch_pc_software_history_entries_with_client(&mut client, computer_id, filters)
}

fn fetch_pc_software_history_entries_with_client(
    client: &mut GlpiClient,
    computer_id: u64,
    filters: &FilterState,
) -> Result<Vec<PcSoftwareLogEntry>, String> {
    let today = chrono::Local::now().date_naive();
    let log_entries = client.fetch_computer_logs(computer_id)?;

    let mut results: Vec<PcSoftwareLogEntry> = log_entries
        .into_iter()
        .filter(|entry| {
            let is_software = entry
                .itemtype_link
                .as_deref()
                .map(|s| {
                    let low = s.to_lowercase();
                    low.contains("software")
                        || low.contains("item_softwareversion")
                        || low.contains("softwareversion")
                })
                .unwrap_or(false);
            if !is_software {
                return false;
            }
            if let Some(ref d) = entry.date_mod {
                if let Some(dt) = date_util::parse_datetime(d) {
                    return date_util::event_in_recency_window(dt, today, filters);
                }
            }
            false
        })
        .map(|entry| {
            let action_code = entry.linked_action.unwrap_or(0);
            let old_val = entry.old_value.clone().unwrap_or_default();
            let new_val = entry.new_value.clone().unwrap_or_default();

            let action = classify_glpi_software_history(action_code, &old_val, &new_val);

            let software_name = if !new_val.is_empty() {
                extract_software_name(&new_val)
            } else {
                extract_software_name(&old_val)
            };

            PcSoftwareLogEntry {
                date: entry.date_mod.unwrap_or_default(),
                action,
                software_name,
                old_value: old_val,
                new_value: new_val,
            }
        })
        .collect();

    results.sort_by(|a, b| b.date.cmp(&a.date));
    Ok(results)
}

fn handle_fetch_global_audit_removals(
    url: &str,
    user_token: &str,
    app_token: &str,
    accept_invalid_certs: bool,
    computer_ids: Vec<u64>,
    filters: FilterState,
    resp_tx: &Sender<WorkerResponse>,
) {
    let app_tok = if app_token.is_empty() {
        None
    } else {
        Some(app_token)
    };
    let mut client = match GlpiClient::new(url, app_tok, accept_invalid_certs) {
        Ok(c) => c,
        Err(e) => {
            let _ = resp_tx.send(WorkerResponse::GlobalAuditRemovalsError(e));
            return;
        }
    };
    if let Err(e) = client.init_session(user_token) {
        let _ = resp_tx.send(WorkerResponse::GlobalAuditRemovalsError(e));
        return;
    }

    let total = computer_ids.len().max(1);
    let mut removals = Vec::new();
    let mut errors: Vec<String> = Vec::new();

    for (i, computer_id) in computer_ids.iter().enumerate() {
        let _ = resp_tx.send(WorkerResponse::GlobalAuditRemovalsProgress {
            done: i + 1,
            total,
        });
        match fetch_pc_software_history_entries_with_client(&mut client, *computer_id, &filters) {
            Ok(entries) => {
                for e in entries {
                    if e.action == PcLogAction::Removed {
                        removals.push(GlobalAuditRemovalRow {
                            computer_id: *computer_id,
                            software_name: e.software_name,
                            removed_at: e.date,
                        });
                    }
                }
            }
            Err(msg) => errors.push(format!("{}: {msg}", computer_id)),
        }
    }

    if removals.is_empty() && !errors.is_empty() {
        let _ = resp_tx.send(WorkerResponse::GlobalAuditRemovalsError(errors.join("; ")));
        return;
    }

    let _ = resp_tx.send(WorkerResponse::GlobalAuditRemovalsReady {
        removals,
        partial_errors: errors,
    });
}

fn handle_fetch_pc_log(
    url: &str,
    user_token: &str,
    app_token: &str,
    accept_invalid_certs: bool,
    computer_id: u64,
    days_back: i64,
    resp_tx: &Sender<WorkerResponse>,
) {
    match fetch_pc_software_history_entries(
        url,
        user_token,
        app_token,
        accept_invalid_certs,
        computer_id,
        days_back,
    ) {
        Ok(entries) => {
            let _ = resp_tx.send(WorkerResponse::PcLogReady {
                computer_id,
                entries,
            });
        }
        Err(e) => {
            let _ = resp_tx.send(WorkerResponse::PcLogError(e));
        }
    }
}

fn handle_fetch_pc_software_log(
    url: &str,
    user_token: &str,
    app_token: &str,
    accept_invalid_certs: bool,
    computer_id: u64,
    days_back: i64,
    resp_tx: &Sender<WorkerResponse>,
) {
    match fetch_pc_software_history_entries(
        url,
        user_token,
        app_token,
        accept_invalid_certs,
        computer_id,
        days_back,
    ) {
        Ok(entries) => {
            let _ = resp_tx.send(WorkerResponse::PcSoftwareLogReady {
                computer_id,
                entries,
            });
        }
        Err(message) => {
            let _ = resp_tx.send(WorkerResponse::PcSoftwareLogError {
                computer_id,
                message,
            });
        }
    }
}

fn extract_software_name(value: &str) -> String {
    // GLPI log values often look like "Software Name (1.2.3)" or just "Software Name"
    let trimmed = value.trim();
    if let Some(paren) = trimmed.rfind(" (") {
        trimmed[..paren].to_string()
    } else {
        trimmed.to_string()
    }
}
