use crate::app::AppState;
use crate::models::{LicenseCategory, LicenseSource};
use eframe::egui;

pub fn show(ctx: &egui::Context, state: &mut AppState) {
    if !state.show_license_panel {
        return;
    }

    let t = state.t();
    let mut machine_rows: Vec<(u64, String)> = state
        .computers
        .iter()
        .map(|(id, info)| (*id, info.name.clone()))
        .collect();
    machine_rows.sort_by(|a, b| a.1.to_lowercase().cmp(&b.1.to_lowercase()));
    if state.selected_machine_id.is_none() {
        state.selected_machine_id = machine_rows.first().map(|(id, _)| *id);
    }
    ensure_machine_details_loaded(state);

    let mut open = true;
    egui::Window::new(t.machine_serial_window_title)
        .id(egui::Id::new("license_serial_window"))
        .resizable(true)
        .default_size([780.0, 320.0])
        .min_width(680.0)
        .open(&mut open)
        .show(ctx, |ui| {
            if machine_rows.is_empty() {
                ui.label(t.no_machine_data);
                return;
            }

            ui.label(t.machine_serial_window_help);
            ui.add_space(8.0);

            ui.horizontal(|ui| {
                if ui
                    .selectable_label(state.machine_panel_tab == 0, t.machine_tab_key_details)
                    .clicked()
                {
                    state.machine_panel_tab = 0;
                }
                if ui
                    .selectable_label(state.machine_panel_tab == 1, t.machine_tab_host_user_map)
                    .clicked()
                {
                    state.machine_panel_tab = 1;
                }
            });
            ui.add_space(8.0);

            let selected_before = state.selected_machine_id;
            ui.horizontal(|ui| {
                ui.label(t.select_machine_label);
                egui::ComboBox::from_id_salt("selected_machine_combo")
                    .selected_text(selected_machine_name(state, &machine_rows))
                    .width(460.0)
                    .show_ui(ui, |ui| {
                        for (id, name) in &machine_rows {
                            ui.selectable_value(&mut state.selected_machine_id, Some(*id), name);
                        }
                    });
            });
            if state.selected_machine_id != selected_before {
                ensure_machine_details_loaded(state);
            }

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(10.0);

            if state.machine_panel_tab == 1 {
                if let Some(machine_id) = state.selected_machine_id {
                    if let Some(info) = state.computers.get(&machine_id) {
                        render_selected_host_user_tab(ui, info, t);
                    }
                }
                return;
            }

            if state.machine_details_loading {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(t.loading_machine_details);
                });
                return;
            }

            if let Some(err) = &state.machine_details_error {
                ui.colored_label(egui::Color32::RED, format!("{}: {err}", t.machine_details_error));
                if let Some(machine_id) = state.selected_machine_id {
                    if ui.small_button(t.retry_label).clicked() {
                        state.request_machine_details(machine_id);
                    }
                }
                ui.add_space(8.0);
            }

            if let Some(machine_id) = state.selected_machine_id {
                if let Some(info) = state.computers.get(&machine_id) {
                    let windows_key = selected_machine_windows_key(state, machine_id);
                    render_machine_details(ui, info, &windows_key, t);
                }
            }
        });

    state.show_license_panel = open;
}

fn ensure_machine_details_loaded(state: &mut AppState) {
    if state.machine_details_loading {
        return;
    }
    if let Some(machine_id) = state.selected_machine_id {
        if state.last_machine_details_fetch_id != Some(machine_id) {
            state.request_machine_details(machine_id);
        }
    }
}

fn selected_machine_name(state: &AppState, machine_rows: &[(u64, String)]) -> String {
    state
        .selected_machine_id
        .and_then(|id| machine_rows.iter().find(|(m_id, _)| *m_id == id))
        .map(|(_, name)| name.clone())
        .unwrap_or_else(|| "—".to_string())
}

fn selected_machine_windows_key(state: &AppState, machine_id: u64) -> String {
    if let Some(info) = state.computers.get(&machine_id) {
        let direct_key = info.windows_product_key.trim();
        if !direct_key.is_empty() {
            return direct_key.to_string();
        }
    }

    let mut best: Option<(u8, String)> = None;

    for rec in &state.all_license_keys {
        if rec.category != LicenseCategory::Windows {
            continue;
        }
        let Some(rec_machine_id) = rec.computer_id else {
            continue;
        };
        if rec_machine_id != machine_id {
            continue;
        }
        let priority = match rec.source {
            // Prefer per-computer inventory key first, then GLPI license entry.
            LicenseSource::ComputerInventory => 0,
            LicenseSource::Glpi => 1,
        };
        match &best {
            Some((current_priority, _)) if *current_priority <= priority => {}
            _ => best = Some((priority, rec.license_key.clone())),
        }
    }

    best.map(|(_, key)| key).unwrap_or_default()
}

fn render_machine_details(
    ui: &mut egui::Ui,
    info: &crate::models::ComputerInfo,
    windows_key: &str,
    t: &crate::i18n::T,
) {
    let mut hostname = info.name.clone();
    let mut serial = printable_value(windows_key, t);
    let mut model = printable_value(&info.model, t);
    ui.horizontal(|ui| {
        ui.label(t.hostname_label);
        ui.add(
            egui::TextEdit::singleline(&mut hostname)
                .desired_width(520.0)
                .interactive(false),
        );
        if ui.small_button(t.copy_label).clicked() {
            ui.ctx().copy_text(info.name.clone());
        }
    });

    ui.horizontal(|ui| {
        ui.label(t.windows_product_key_label);
        ui.add(
            egui::TextEdit::singleline(&mut serial)
                .desired_width(520.0)
                .interactive(false),
        );
        if ui.small_button(t.copy_label).clicked() {
            ui.ctx().copy_text(serial.clone());
        }
    });

    ui.horizontal(|ui| {
        ui.label(t.model_label);
        ui.add(
            egui::TextEdit::singleline(&mut model)
                .desired_width(520.0)
                .interactive(false),
        );
        if ui.small_button(t.copy_label).clicked() {
            ui.ctx().copy_text(model.clone());
        }
    });
}

fn render_selected_host_user_tab(
    ui: &mut egui::Ui,
    info: &crate::models::ComputerInfo,
    t: &crate::i18n::T,
) {
    ui.label(t.host_user_map_help);
    ui.add_space(8.0);
    ui.separator();
    ui.add_space(8.0);

    let mut hostname = info.name.clone();
    let mut username = printable_value(&info.contact, t);
    ui.horizontal(|ui| {
        ui.label(t.hostname_label);
        ui.add(
            egui::TextEdit::singleline(&mut hostname)
                .desired_width(520.0)
                .interactive(false),
        );
        if ui.small_button(t.copy_label).clicked() {
            ui.ctx().copy_text(info.name.clone());
        }
    });
    ui.horizontal(|ui| {
        ui.label(t.col_user_contact);
        ui.add(
            egui::TextEdit::singleline(&mut username)
                .desired_width(520.0)
                .interactive(false),
        );
        if ui.small_button(t.copy_label).clicked() {
            ui.ctx().copy_text(username.clone());
        }
    });
}

fn printable_value(value: &str, t: &crate::i18n::T) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        t.not_available.to_string()
    } else {
        trimmed.to_string()
    }
}
