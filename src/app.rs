use crate::config::AppConfig;
use crate::history_query;
use crate::history_store;
use crate::i18n::{self, Lang, T};
use crate::models::*;
use crate::ui;
use crate::worker::{WorkerRequest, WorkerResponse};
use eframe::egui;
use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{Receiver, Sender};

pub struct AppState {
    pub config: AppConfig,
    pub status: FetchStatus,
    pub all_data: Vec<AggregatedSoftware>,
    pub filtered_data: Vec<AggregatedSoftware>,
    /// After filters and optional “agent-fresh only” tab; used for table, status, export.
    pub main_inventory_display: Vec<AggregatedSoftware>,
    pub filters: FilterState,
    pub expanded: HashSet<u64>,
    pub detail_tabs: HashMap<u64, usize>,
    pub selected: HashSet<u64>,
    pub computers: HashMap<u64, crate::models::ComputerInfo>,
    pub show_pc_panel: bool,
    pub export_message: Option<String>,
    pub all_license_keys: Vec<LicenseKeyRecord>,
    pub show_license_panel: bool,
    pub selected_machine_id: Option<u64>,
    pub machine_panel_tab: usize,
    pub machine_details_loading: bool,
    pub machine_details_error: Option<String>,
    pub last_machine_details_fetch_id: Option<u64>,
    pub cleanup_days: String,
    pub cleanup_preview: Vec<SoftwareCleanupCandidate>,
    pub cleanup_message: Option<String>,
    pub history_mode: bool,
    pub history_days_ago: String,
    pub history_software_name: String,
    pub history_publisher: String,
    pub history_pc_filter: String,
    pub history_selected_snapshot: Option<String>,
    pub history_snapshots: Vec<SnapshotSummary>,
    pub history_rows: Vec<HistoricalSoftwareEntry>,
    pub history_summary: Option<HistoryViewSummary>,
    pub history_message: Option<String>,
    pub history_selected: HashSet<String>,
    pub history_expanded: HashSet<String>,
    pub history_detail_tabs: HashMap<String, usize>,
    pub history_show_selected_only: bool,
    pub history_compare_mode: bool,
    pub history_compare_a: Option<String>,
    pub history_compare_b: Option<String>,
    pub history_diff: Option<SnapshotDiff>,
    pub history_delete_confirm: Option<String>,
    pub history_snapshot_bulk_selected: HashSet<String>,
    pub history_bulk_delete_pending: Option<Vec<String>>,
    pub history_bulk_delete_tab: bool,
    pub history_pc_log_tab: bool,
    pub history_pc_log_computer: Option<u64>,
    pub history_pc_log_days: String,
    pub history_pc_log_entries: Vec<PcSoftwareLogEntry>,
    pub history_pc_log_loading: bool,
    pub history_pc_log_fetched: bool,
    pub history_pc_log_error: Option<String>,
    pub warning_message: Option<String>,
    pub show_pc_software_panel: bool,
    pub pc_software_selected: Option<u64>,
    pub pc_software_filter: String,
    pub pc_software_hide_windows: bool,
    pub pc_software_hide_kb: bool,
    pub pc_software_time_filter: bool,
    pub pc_software_time_days: String,
    pub pc_software_show_deleted: bool,
    pub pc_software_log_entries: Vec<PcSoftwareLogEntry>,
    pub pc_software_log_loading: bool,
    pub pc_software_log_fetched_for: Option<u64>,
    pub pc_software_log_error: Option<String>,
    pub pc_software_hist_snapshot: bool,
    pub pc_software_hist_from: String,
    pub pc_software_hist_to: String,
    /// Set when live data / snapshot list may have changed; forces PC Software hist cache refresh.
    pub pc_software_hist_cache_dirty: bool,
    pub pc_software_hist_cache_key: Option<(String, String, usize, String)>,
    pub pc_software_hist_cache: Option<PcSoftwareHistCache>,
    pub agents: Vec<AgentInfo>,
    pub show_agent_panel: bool,
    pub agent_filter: String,
    pub agent_status_filter: AgentStatusFilter,
    pub agent_hide_stale_no_contact: bool,
    pub agent_stale_max_days: String,
    pub ping_in_progress: bool,
    pub main_inventory_tab: MainInventoryTab,
    pub main_table_show_audit_removals: bool,
    pub audit_removals_by_key: HashMap<String, AuditRemovalGroup>,
    pub audit_removals_loading: bool,
    pub audit_removals_error: Option<String>,
    pub audit_removals_progress: Option<(usize, usize)>,
    worker_tx: Sender<WorkerRequest>,
    worker_rx: Receiver<WorkerResponse>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MainInventoryTab {
    #[default]
    Full,
    /// Only software where every install PC has agent last_contact within `agent_stale_max_days`.
    AgentFreshOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentStatusFilter {
    All,
    Online,
    Offline,
}

impl AppState {
    pub fn lang(&self) -> Lang {
        self.config.language
    }

    pub fn t(&self) -> &'static T {
        i18n::t(self.config.language)
    }

    pub fn request_fetch(&mut self) {
        self.config.save();
        self.all_data.clear();
        self.filtered_data.clear();
        self.main_inventory_display.clear();
        self.expanded.clear();
        self.detail_tabs.clear();
        self.export_message = None;
        self.all_license_keys.clear();
        self.show_license_panel = false;
        self.selected_machine_id = None;
        self.machine_panel_tab = 0;
        self.machine_details_loading = false;
        self.machine_details_error = None;
        self.last_machine_details_fetch_id = None;
        self.cleanup_message = None;
        self.warning_message = None;

        let _ = self.worker_tx.send(WorkerRequest::FetchAll {
            url: self.config.glpi_url.clone(),
            user_token: self.config.user_token.clone(),
            app_token: self.config.app_token.clone(),
            accept_invalid_certs: self.config.accept_invalid_certs,
        });
    }

    pub fn request_cleanup_dry_run(&mut self) {
        self.cleanup_message = None;
        let days = self.cleanup_days.parse::<i64>().unwrap_or(60).max(1);
        let _ = self.worker_tx.send(WorkerRequest::DryRunCleanup {
            url: self.config.glpi_url.clone(),
            user_token: self.config.user_token.clone(),
            app_token: self.config.app_token.clone(),
            accept_invalid_certs: self.config.accept_invalid_certs,
            older_than_days: days,
        });
    }

    pub fn request_machine_details(&mut self, machine_id: u64) {
        self.machine_details_loading = true;
        self.machine_details_error = None;
        self.last_machine_details_fetch_id = Some(machine_id);
        let _ = self.worker_tx.send(WorkerRequest::FetchMachineDetails {
            url: self.config.glpi_url.clone(),
            user_token: self.config.user_token.clone(),
            app_token: self.config.app_token.clone(),
            accept_invalid_certs: self.config.accept_invalid_certs,
            machine_id,
        });
    }

    pub fn reload_history_snapshots(&mut self) {
        self.history_snapshots = history_store::list_snapshots();
        self.pc_software_hist_cache_dirty = true;
        let valid: HashSet<_> = self
            .history_snapshots
            .iter()
            .map(|s| s.file_name.clone())
            .collect();
        self.history_snapshot_bulk_selected
            .retain(|f| valid.contains(f));
    }

    pub fn request_ping_all(&mut self) {
        let targets: Vec<(u64, String)> = self
            .agents
            .iter()
            .map(|a| (a.computer_id, a.computer_name.clone()))
            .collect();
        if targets.is_empty() {
            return;
        }
        for a in &mut self.agents {
            a.ping = PingResult::Pending;
        }
        self.ping_in_progress = true;
        let _ = self.worker_tx.send(WorkerRequest::PingComputers { targets });
    }

    pub fn rebuild_main_inventory_display(&mut self) {
        self.main_inventory_display = self.filtered_data.clone();
        if matches!(self.main_inventory_tab, MainInventoryTab::AgentFreshOnly) {
            ui::agent_panel::retain_agent_fresh_all_hosts(
                &mut self.main_inventory_display,
                &self.agents,
                &self.computers,
                &self.agent_stale_max_days,
            );
        }
    }

    pub fn request_ping_single(&mut self, computer_id: u64) {
        if let Some(agent) = self.agents.iter_mut().find(|a| a.computer_id == computer_id) {
            agent.ping = PingResult::Pending;
            let hostname = agent.computer_name.clone();
            self.ping_in_progress = true;
            let _ = self.worker_tx.send(WorkerRequest::PingComputers {
                targets: vec![(computer_id, hostname)],
            });
        }
    }

    pub fn request_audit_removals_refresh(&mut self) {
        if self.computers.is_empty() {
            return;
        }
        self.audit_removals_error = None;
        self.audit_removals_loading = true;
        self.audit_removals_progress = None;
        let computer_ids: Vec<u64> = self.computers.keys().copied().collect();
        let _ = self.worker_tx.send(WorkerRequest::FetchGlobalAuditRemovals {
            url: self.config.glpi_url.clone(),
            user_token: self.config.user_token.clone(),
            app_token: self.config.app_token.clone(),
            accept_invalid_certs: self.config.accept_invalid_certs,
            computer_ids,
            filters: self.filters.clone(),
        });
    }

    fn merge_audit_removals(&mut self, rows: Vec<GlobalAuditRemovalRow>) {
        let mut by_key: HashMap<String, AuditRemovalGroup> = HashMap::new();
        let mut seen: HashSet<(u64, String)> = HashSet::new();
        for r in rows {
            let key = r.software_name.trim().to_lowercase();
            if key.is_empty() {
                continue;
            }
            if !seen.insert((r.computer_id, key.clone())) {
                continue;
            }
            let computer_name = self
                .computers
                .get(&r.computer_id)
                .map(|c| c.name.clone())
                .unwrap_or_default();
            let g = by_key.entry(key).or_default();
            if g.display_label.is_empty() && !r.software_name.trim().is_empty() {
                g.display_label = r.software_name.trim().to_string();
            }
            g.items.push(AuditRemovalItem {
                computer_id: r.computer_id,
                computer_name,
                removed_at: r.removed_at,
            });
        }
        for g in by_key.values_mut() {
            g.items
                .sort_by(|a, b| a.computer_name.to_lowercase().cmp(&b.computer_name.to_lowercase()));
        }
        self.audit_removals_by_key = by_key;
    }

    pub fn request_pc_software_log(&mut self) {
        let Some(computer_id) = self.pc_software_selected else {
            return;
        };
        let days_back = self.pc_software_time_days.parse::<i64>().unwrap_or(30).max(1);
        self.pc_software_log_loading = true;
        self.pc_software_log_error = None;
        self.pc_software_log_entries.clear();
        let _ = self.worker_tx.send(WorkerRequest::FetchPcSoftwareLog {
            url: self.config.glpi_url.clone(),
            user_token: self.config.user_token.clone(),
            app_token: self.config.app_token.clone(),
            accept_invalid_certs: self.config.accept_invalid_certs,
            computer_id,
            days_back,
        });
    }

    pub fn request_pc_log(&mut self) {
        let Some(computer_id) = self.history_pc_log_computer else {
            return;
        };
        let days_back = self.history_pc_log_days.parse::<i64>().unwrap_or(30).max(1);
        self.history_pc_log_loading = true;
        self.history_pc_log_fetched = true;
        self.history_pc_log_error = None;
        self.history_pc_log_entries.clear();
        let _ = self.worker_tx.send(WorkerRequest::FetchPcLog {
            url: self.config.glpi_url.clone(),
            user_token: self.config.user_token.clone(),
            app_token: self.config.app_token.clone(),
            accept_invalid_certs: self.config.accept_invalid_certs,
            computer_id,
            days_back,
        });
    }

    pub fn resolve_history_snapshot(&mut self) {
        let days = self.history_days_ago.parse::<i64>().unwrap_or(5).max(0);
        self.history_selected_snapshot = history_query::resolve_snapshot_for_days_ago(
            &self.history_snapshots,
            days,
        )
        .map(|s| s.file_name);
        self.refresh_history_view();
    }

    pub fn refresh_history_view(&mut self) {
        self.history_rows.clear();
        self.history_summary = None;
        self.history_message = None;

        let Some(file_name) = self.history_selected_snapshot.clone() else {
            self.history_message = Some(self.t().history_no_snapshot.to_string());
            return;
        };

        let snapshot = match history_store::load_snapshot(&file_name) {
            Ok(snapshot) => snapshot,
            Err(e) => {
                self.history_message = Some(format!("{}: {e}", self.t().status_error));
                return;
            }
        };

        let rows = history_query::build_historical_software_view(
            &snapshot,
            &self.history_software_name,
            &self.history_publisher,
            &self.history_pc_filter,
        );

        if rows.is_empty() {
            self.history_message = Some(self.t().history_no_matches.to_string());
        } else {
            self.history_summary = Some(HistoryViewSummary {
                snapshot_captured_at: snapshot.captured_at.clone(),
                software_count: rows.len(),
                host_count: rows.iter().map(|r| r.host_count).sum(),
            });
            self.history_message = Some(format!(
                "{}: {} | {}: {} | {}: {}",
                self.t().history_snapshot_label,
                snapshot.captured_at,
                self.t().col_software_name,
                rows.len(),
                self.t().col_hosts,
                rows.iter().map(|r| r.host_count).sum::<usize>()
            ));
        }
        self.history_rows = rows;
    }

    fn poll_worker(&mut self) {
        while let Ok(resp) = self.worker_rx.try_recv() {
            match resp {
                WorkerResponse::Status(s) => self.status = s,
                WorkerResponse::Data {
                    software,
                    computers,
                    license_keys,
                    agents,
                    snapshot_warning,
                } => {
                    self.all_data = software;
                    self.computers = computers;
                    self.all_license_keys = license_keys;
                    self.agents = agents;
                    self.warning_message = snapshot_warning;
                    self.audit_removals_by_key.clear();
                    self.audit_removals_error = None;
                    self.filtered_data =
                        ui::filter_panel::apply_filters(&self.all_data, &self.filters, &self.selected);
                    self.rebuild_main_inventory_display();
                    self.reload_history_snapshots();
                    if self.history_mode {
                        self.resolve_history_snapshot();
                    }
                }
                WorkerResponse::PingResults { results } => {
                    for agent in &mut self.agents {
                        if let Some(&reachable) = results.get(&agent.computer_id) {
                            agent.ping = if reachable {
                                PingResult::Reachable
                            } else {
                                PingResult::Unreachable
                            };
                        }
                    }
                    self.ping_in_progress = false;
                }
                WorkerResponse::MachineDetailsLoaded { machine_id, info } => {
                    self.computers.insert(machine_id, info);
                    self.machine_details_loading = false;
                    self.machine_details_error = None;
                }
                WorkerResponse::CleanupPreviewReady {
                    items,
                    days,
                    skipped_no_date,
                } => {
                    let t = self.t();
                    let count = items.len();
                    self.cleanup_preview = items;
                    self.cleanup_message = Some(if skipped_no_date > 0 {
                        format!(
                            "{} {} {} {} {} {} {} {} {}",
                            t.cleanup_dry_run_complete_prefix,
                            count,
                            t.cleanup_dry_run_complete_mid,
                            days,
                            t.days,
                            t.cleanup_dry_run_complete_suffix,
                            t.cleanup_dry_run_skipped_prefix,
                            skipped_no_date,
                            t.cleanup_dry_run_skipped_suffix
                        )
                    } else {
                        format!(
                            "{} {} {} {} {} {}",
                            t.cleanup_dry_run_complete_prefix,
                            count,
                            t.cleanup_dry_run_complete_mid,
                            days,
                            t.days,
                            t.cleanup_dry_run_complete_suffix
                        )
                    });
                }
                WorkerResponse::PcLogReady { computer_id: _, entries } => {
                    self.history_pc_log_entries = entries;
                    self.history_pc_log_loading = false;
                    self.history_pc_log_error = None;
                }
                WorkerResponse::PcLogError(e) => {
                    self.history_pc_log_loading = false;
                    self.history_pc_log_error = Some(e);
                }
                WorkerResponse::PcSoftwareLogReady { computer_id, entries } => {
                    self.pc_software_log_entries = entries;
                    self.pc_software_log_loading = false;
                    self.pc_software_log_fetched_for = Some(computer_id);
                    self.pc_software_log_error = None;
                }
                WorkerResponse::PcSoftwareLogError { computer_id, message } => {
                    self.pc_software_log_loading = false;
                    self.pc_software_log_fetched_for = Some(computer_id);
                    self.pc_software_log_error = Some(message);
                }
                WorkerResponse::GlobalAuditRemovalsProgress { done, total } => {
                    self.audit_removals_progress = Some((done, total));
                }
                WorkerResponse::GlobalAuditRemovalsReady {
                    removals,
                    partial_errors,
                } => {
                    self.merge_audit_removals(removals);
                    self.audit_removals_loading = false;
                    self.audit_removals_error = if partial_errors.is_empty() {
                        None
                    } else {
                        Some(partial_errors.join("; "))
                    };
                    self.audit_removals_progress = None;
                }
                WorkerResponse::GlobalAuditRemovalsError(e) => {
                    self.audit_removals_loading = false;
                    self.audit_removals_error = Some(e);
                    self.audit_removals_progress = None;
                }
                WorkerResponse::MachineDetailsError(e) => {
                    self.machine_details_loading = false;
                    self.machine_details_error = Some(e);
                }
                WorkerResponse::Error(e) => {
                    self.status = FetchStatus::Error(e);
                }
            }
        }
    }
}

pub struct GlpiApp {
    state: AppState,
    _worker_handle: std::thread::JoinHandle<()>,
}

impl GlpiApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let config = AppConfig::load();
        let selected = crate::config::load_selections();
        let history_snapshots = history_store::list_snapshots();

        let (req_tx, req_rx) = std::sync::mpsc::channel();
        let (resp_tx, resp_rx) = std::sync::mpsc::channel();
        let handle = crate::worker::spawn_worker(req_rx, resp_tx);

        Self {
            state: AppState {
                config,
                status: FetchStatus::Idle,
                all_data: Vec::new(),
                filtered_data: Vec::new(),
                main_inventory_display: Vec::new(),
                filters: FilterState::default(),
                expanded: HashSet::new(),
                detail_tabs: HashMap::new(),
                selected,
                computers: HashMap::new(),
                show_pc_panel: false,
                export_message: None,
                all_license_keys: Vec::new(),
                show_license_panel: false,
                selected_machine_id: None,
                machine_panel_tab: 0,
                machine_details_loading: false,
                machine_details_error: None,
                last_machine_details_fetch_id: None,
                cleanup_days: "60".to_string(),
                cleanup_preview: Vec::new(),
                cleanup_message: None,
                history_mode: false,
                history_days_ago: "5".to_string(),
                history_software_name: String::new(),
                history_publisher: String::new(),
                history_pc_filter: String::new(),
                history_selected_snapshot: None,
                history_snapshots,
                history_rows: Vec::new(),
                history_summary: None,
                history_message: None,
                history_selected: HashSet::new(),
                history_expanded: HashSet::new(),
                history_detail_tabs: HashMap::new(),
                history_show_selected_only: false,
                history_compare_mode: false,
                history_compare_a: None,
                history_compare_b: None,
                history_diff: None,
                history_delete_confirm: None,
                history_snapshot_bulk_selected: HashSet::new(),
                history_bulk_delete_pending: None,
                history_bulk_delete_tab: false,
                history_pc_log_tab: false,
                history_pc_log_computer: None,
                history_pc_log_days: "30".to_string(),
                history_pc_log_entries: Vec::new(),
                history_pc_log_loading: false,
                history_pc_log_fetched: false,
                history_pc_log_error: None,
                warning_message: None,
                show_pc_software_panel: false,
                pc_software_selected: None,
                pc_software_filter: String::new(),
                pc_software_hide_windows: false,
                pc_software_hide_kb: false,
                pc_software_time_filter: false,
                pc_software_time_days: "30".to_string(),
                pc_software_show_deleted: false,
                pc_software_log_entries: Vec::new(),
                pc_software_log_loading: false,
                pc_software_log_fetched_for: None,
                pc_software_log_error: None,
                pc_software_hist_snapshot: false,
                pc_software_hist_from: String::new(),
                pc_software_hist_to: String::new(),
                pc_software_hist_cache_dirty: false,
                pc_software_hist_cache_key: None,
                pc_software_hist_cache: None,
                agents: Vec::new(),
                show_agent_panel: false,
                agent_filter: String::new(),
                agent_status_filter: AgentStatusFilter::All,
                agent_hide_stale_no_contact: false,
                agent_stale_max_days: "60".to_string(),
                ping_in_progress: false,
                main_inventory_tab: MainInventoryTab::default(),
                main_table_show_audit_removals: false,
                audit_removals_by_key: HashMap::new(),
                audit_removals_loading: false,
                audit_removals_error: None,
                audit_removals_progress: None,
                worker_tx: req_tx,
                worker_rx: resp_rx,
            },
            _worker_handle: handle,
        }
    }
}

impl eframe::App for GlpiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.state.poll_worker();

        let sel_snapshot = self.state.selected.len();
        let t = self.state.t();

        let is_fetching = matches!(
            self.state.status,
            FetchStatus::Connecting
                | FetchStatus::FetchingSoftware { .. }
                | FetchStatus::FetchingVersions { .. }
                | FetchStatus::FetchingInstallations { .. }
                | FetchStatus::FetchingLicenses { .. }
                | FetchStatus::FetchingComputers { .. }
                | FetchStatus::FetchingAgents { .. }
                | FetchStatus::Aggregating
        );

        if is_fetching
            || self.state.ping_in_progress
            || self.state.history_pc_log_loading
            || self.state.pc_software_log_loading
            || self.state.audit_removals_loading
        {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }

        egui::TopBottomPanel::top("connection_panel").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.heading(t.app_title);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let lang = self.state.lang();
                    let next = lang.toggle();
                    if ui.button(next.label()).on_hover_text(
                        if lang == Lang::En { "Váltás magyarra" } else { "Switch to English" }
                    ).clicked() {
                        self.state.config.language = next;
                        self.state.config.save();
                    }
                });
            });
            ui.add_space(4.0);
            ui.separator();
            ui.add_space(4.0);
            ui::connection_panel::show(ui, &mut self.state);
            ui::cleanup_panel::show(ui, &mut self.state);
            ui.add_space(4.0);
        });

        let mut selection_dirty = false;

        egui::TopBottomPanel::top("filter_export_panel").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.separator();
            if self.state.history_mode {
                ui::history_panel::show_controls(ui, &mut self.state);
            } else {
                let filter_visible = self.state.filtered_data.clone();
                let changed =
                    ui::filter_panel::show(ui, &mut self.state, &filter_visible, t);
                if changed {
                    self.state.filtered_data =
                        ui::filter_panel::apply_filters(&self.state.all_data, &self.state.filters, &self.state.selected);
                }
                self.state.rebuild_main_inventory_display();
            }
            ui.add_space(4.0);
            if !self.state.history_mode {
                ui::export_panel::show(
                    ui,
                    &self.state.main_inventory_display,
                    &self.state.computers,
                    &mut self.state.export_message,
                    !self.state.audit_removals_by_key.is_empty(),
                    t,
                );
            }
            ui.add_space(4.0);
            ui.separator();
        });

        if !self.state.history_mode {
            ui::pc_panel::show(
                ctx,
                &self.state.all_data,
                &self.state.selected,
                &self.state.computers,
                &self.state.filters,
                &mut self.state.show_pc_panel,
                t,
            );
        }
        ui::license_panel::show(ctx, &mut self.state);
        ui::agent_panel::show(ctx, &mut self.state);
        ui::pc_software_panel::show(ctx, &mut self.state);

        egui::CentralPanel::default().show(ctx, |ui| {
            if self.state.history_mode {
                ui::history_panel::show_results(ui, &mut self.state);
            } else {
                let total_loaded = self.state.all_data.len();
                let filtered_count = self.state.filtered_data.len();
                let table_data = self.state.main_inventory_display.clone();
                let (sel_changed, table_filters_changed) = ui::software_table::show(
                    ui,
                    &table_data,
                    total_loaded,
                    filtered_count,
                    &mut self.state,
                    t,
                );
                if table_filters_changed {
                    self.state.filtered_data = ui::filter_panel::apply_filters(
                        &self.state.all_data,
                        &self.state.filters,
                        &self.state.selected,
                    );
                }
                if sel_changed {
                    selection_dirty = true;
                    if self.state.filters.show_selected_only {
                        self.state.filtered_data =
                            ui::filter_panel::apply_filters(&self.state.all_data, &self.state.filters, &self.state.selected);
                    }
                }
                self.state.rebuild_main_inventory_display();
            }
        });

        egui::TopBottomPanel::bottom("status_panel").show(ctx, |ui| {
            ui.add_space(2.0);
            if self.state.history_mode {
                ui::status_bar::show_history(ui, self.state.history_summary.as_ref(), t);
            } else {
                ui::status_bar::show(
                    ui,
                    &self.state.main_inventory_display,
                    self.state.all_data.len(),
                    t,
                );
            }
            ui.add_space(2.0);
        });

        if selection_dirty || self.state.selected.len() != sel_snapshot {
            crate::config::save_selections(&self.state.selected);
        }
    }
}
