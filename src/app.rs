use crate::config::AppConfig;
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
    pub filters: FilterState,
    pub expanded: HashSet<u64>,
    pub detail_tabs: HashMap<u64, usize>,
    pub selected: HashSet<u64>,
    pub computers: HashMap<u64, crate::models::ComputerInfo>,
    pub show_pc_panel: bool,
    pub export_message: Option<String>,
    worker_tx: Sender<WorkerRequest>,
    worker_rx: Receiver<WorkerResponse>,
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
        self.expanded.clear();
        self.detail_tabs.clear();
        self.export_message = None;

        let _ = self.worker_tx.send(WorkerRequest::FetchAll {
            url: self.config.glpi_url.clone(),
            user_token: self.config.user_token.clone(),
            app_token: self.config.app_token.clone(),
            accept_invalid_certs: self.config.accept_invalid_certs,
        });
    }

    fn poll_worker(&mut self) {
        while let Ok(resp) = self.worker_rx.try_recv() {
            match resp {
                WorkerResponse::Status(s) => self.status = s,
                WorkerResponse::Data { software, computers } => {
                    self.all_data = software;
                    self.computers = computers;
                    self.filtered_data =
                        ui::filter_panel::apply_filters(&self.all_data, &self.filters, &self.selected);
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

        let (req_tx, req_rx) = std::sync::mpsc::channel();
        let (resp_tx, resp_rx) = std::sync::mpsc::channel();
        let handle = crate::worker::spawn_worker(req_rx, resp_tx);

        Self {
            state: AppState {
                config,
                status: FetchStatus::Idle,
                all_data: Vec::new(),
                filtered_data: Vec::new(),
                filters: FilterState::default(),
                expanded: HashSet::new(),
                detail_tabs: HashMap::new(),
                selected,
                computers: HashMap::new(),
                show_pc_panel: false,
                export_message: None,
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
                | FetchStatus::FetchingComputers { .. }
                | FetchStatus::Aggregating
        );

        if is_fetching {
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
            ui.add_space(4.0);
        });

        egui::TopBottomPanel::bottom("status_panel").show(ctx, |ui| {
            ui.add_space(2.0);
            ui::status_bar::show(ui, &self.state.filtered_data, self.state.all_data.len(), t);
            ui.add_space(2.0);
        });

        let recent_days: i64 = self.state.filters.days.parse().unwrap_or(30);
        let mut selection_dirty = false;

        egui::TopBottomPanel::top("filter_export_panel").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.separator();
            let changed = ui::filter_panel::show(ui, &mut self.state.filters, &self.state.filtered_data, &mut self.state.selected, &mut self.state.show_pc_panel, t);
            if changed {
                self.state.filtered_data =
                    ui::filter_panel::apply_filters(&self.state.all_data, &self.state.filters, &self.state.selected);
            }
            ui.add_space(4.0);
            ui::export_panel::show(
                ui,
                &self.state.filtered_data,
                &mut self.state.export_message,
                recent_days,
                t,
            );
            ui.add_space(4.0);
            ui.separator();
        });

        ui::pc_panel::show(
            ctx,
            &self.state.all_data,
            &self.state.selected,
            &self.state.computers,
            &mut self.state.show_pc_panel,
            t,
        );

        egui::CentralPanel::default().show(ctx, |ui| {
            let sel_changed = ui::software_table::show(ui, &self.state.filtered_data, &mut self.state.expanded, &mut self.state.detail_tabs, &mut self.state.selected, recent_days, &self.state.computers, t);
            if sel_changed {
                selection_dirty = true;
                if self.state.filters.show_selected_only {
                    self.state.filtered_data =
                        ui::filter_panel::apply_filters(&self.state.all_data, &self.state.filters, &self.state.selected);
                }
            }
        });

        if selection_dirty || self.state.selected.len() != sel_snapshot {
            crate::config::save_selections(&self.state.selected);
        }
    }
}
