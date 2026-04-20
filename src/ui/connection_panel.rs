use crate::app::{AppState, MainInventoryTab};
use crate::models::FetchStatus;
use eframe::egui;

pub fn show(ui: &mut egui::Ui, state: &mut AppState) {
    let t = state.t();

    ui.horizontal(|ui| {
        ui.label(t.glpi_url);
        ui.add(
            egui::TextEdit::singleline(&mut state.config.glpi_url)
                .desired_width(300.0)
                .hint_text(t.glpi_url_hint),
        );
    });

    ui.horizontal(|ui| {
        ui.label(t.user_token);
        ui.add(
            egui::TextEdit::singleline(&mut state.config.user_token)
                .desired_width(250.0)
                .password(true)
                .hint_text(t.user_token_hint),
        );

        ui.add_space(10.0);
        ui.label(t.app_token);
        ui.add(
            egui::TextEdit::singleline(&mut state.config.app_token)
                .desired_width(250.0)
                .password(true)
                .hint_text(t.app_token_hint),
        );
    });

    ui.horizontal(|ui| {
        ui.checkbox(&mut state.config.accept_invalid_certs, t.accept_invalid_tls)
            .on_hover_text(t.accept_invalid_tls_tip);

        ui.add_space(10.0);
        ui.colored_label(
            egui::Color32::from_rgb(180, 140, 0),
            t.tokens_warning,
        );
    });

    if let Some(warning) = &state.warning_message {
        ui.horizontal(|ui| {
            ui.colored_label(
                egui::Color32::from_rgb(180, 140, 0),
                format!("{}: {warning}", t.status_warning),
            );
        });
    }

    ui.horizontal(|ui| {
        let is_fetching = matches!(
            state.status,
            FetchStatus::Connecting
                | FetchStatus::FetchingSoftware { .. }
                | FetchStatus::FetchingVersions { .. }
                | FetchStatus::FetchingInstallations { .. }
                | FetchStatus::FetchingLicenses { .. }
                | FetchStatus::FetchingComputers { .. }
                | FetchStatus::FetchingAgents { .. }
                | FetchStatus::Aggregating
        );

        let can_connect = !state.config.glpi_url.is_empty()
            && !state.config.user_token.is_empty()
            && !is_fetching;

        if ui
            .add_enabled(can_connect, egui::Button::new(t.connect_fetch))
            .clicked()
        {
            state.request_fetch();
        }

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);

        let current_btn = egui::Button::new(
            egui::RichText::new(t.mode_current)
                .strong()
                .color(if !state.history_mode {
                    egui::Color32::WHITE
                } else {
                    ui.style().visuals.text_color()
                }),
        );
        let current_btn = if !state.history_mode {
            current_btn.fill(egui::Color32::from_rgb(50, 110, 180))
        } else {
            current_btn
        };
        if ui.add(current_btn).clicked() {
            state.history_mode = false;
        }

        let history_btn = egui::Button::new(
            egui::RichText::new(t.mode_history)
                .strong()
                .color(if state.history_mode {
                    egui::Color32::WHITE
                } else {
                    ui.style().visuals.text_color()
                }),
        );
        let history_btn = if state.history_mode {
            history_btn.fill(egui::Color32::from_rgb(50, 110, 180))
        } else {
            history_btn
        };
        if ui.add(history_btn).clicked() {
            state.history_mode = true;
            state.main_inventory_tab = MainInventoryTab::Full;
            state.reload_history_snapshots();
            if state.history_selected_snapshot.is_none() {
                state.resolve_history_snapshot();
            } else {
                state.refresh_history_view();
            }
        }

        if !state.history_mode {
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);

            let full_sel = matches!(state.main_inventory_tab, MainInventoryTab::Full);
            let fresh_sel = matches!(state.main_inventory_tab, MainInventoryTab::AgentFreshOnly);

            let full_btn = egui::Button::new(
                egui::RichText::new(t.main_inv_tab_full)
                    .strong()
                    .color(if full_sel {
                        egui::Color32::WHITE
                    } else {
                        ui.style().visuals.text_color()
                    }),
            );
            let full_btn = if full_sel {
                full_btn.fill(egui::Color32::from_rgb(40, 120, 90))
            } else {
                full_btn
            };
            if ui.add(full_btn).clicked() {
                state.main_inventory_tab = MainInventoryTab::Full;
            }

            let fresh_btn = egui::Button::new(
                egui::RichText::new(t.main_inv_tab_agent_fresh)
                    .strong()
                    .color(if fresh_sel {
                        egui::Color32::WHITE
                    } else {
                        ui.style().visuals.text_color()
                    }),
            );
            let fresh_btn = if fresh_sel {
                fresh_btn.fill(egui::Color32::from_rgb(40, 120, 90))
            } else {
                fresh_btn
            };
            if ui
                .add(fresh_btn)
                .on_hover_text(t.main_inv_agent_fresh_tip)
                .clicked()
            {
                state.main_inventory_tab = MainInventoryTab::AgentFreshOnly;
            }
        }

        ui.add_space(4.0);
        ui.separator();
        ui.add_space(4.0);

        let can_open_agent_panel =
            (!state.agents.is_empty() || !state.computers.is_empty()) && !is_fetching;
        if ui
            .add_enabled(can_open_agent_panel, egui::Button::new(t.agent_panel_button))
            .clicked()
        {
            state.show_agent_panel = true;
        }

        ui.add_space(8.0);
        let can_open_pc_software = !state.computers.is_empty() && !is_fetching;
        if ui
            .add_enabled(can_open_pc_software, egui::Button::new(t.pc_software_panel_button))
            .clicked()
        {
            state.show_pc_software_panel = true;
            if state.pc_software_selected.is_none() {
                state.pc_software_selected = state.computers.keys().next().copied();
            }
        }

        ui.add_space(8.0);
        let can_open_license_panel = !state.computers.is_empty() && !is_fetching;
        if ui
            .add_enabled(
                can_open_license_panel,
                egui::Button::new(t.show_machine_serial_button),
            )
            .clicked()
        {
            state.show_license_panel = true;
            state.show_pc_panel = false;
            state.machine_panel_tab = 0;
            state.last_machine_details_fetch_id = None;
            if state.selected_machine_id.is_none() {
                state.selected_machine_id = state.computers.keys().next().copied();
            }
        }

        ui.add_space(10.0);

        let status_text = format!("{}: {}", t.status_prefix, format_status(&state.status, t));
        match &state.status {
            FetchStatus::Error(_) => {
                ui.colored_label(egui::Color32::RED, &status_text);
            }
            FetchStatus::Done { .. } => {
                ui.colored_label(egui::Color32::from_rgb(0, 150, 0), &status_text);
            }
            _ => {
                ui.label(&status_text);
            }
        }
    });
}

fn format_status(status: &FetchStatus, t: &crate::i18n::T) -> String {
    match status {
        FetchStatus::Idle => t.status_idle.to_string(),
        FetchStatus::Connecting => t.status_connecting.to_string(),
        FetchStatus::FetchingSoftware { done, total } => {
            if let Some(tot) = total {
                format!("{}: {done}/{tot}", t.status_fetching_software)
            } else {
                format!("{}: {done}...", t.status_fetching_software)
            }
        }
        FetchStatus::FetchingVersions { done, total } => {
            if let Some(tot) = total {
                format!("{}: {done}/{tot}", t.status_fetching_versions)
            } else {
                format!("{}: {done}...", t.status_fetching_versions)
            }
        }
        FetchStatus::FetchingInstallations { done, total } => {
            if let Some(tot) = total {
                format!("{}: {done}/{tot}", t.status_fetching_installations)
            } else {
                format!("{}: {done}...", t.status_fetching_installations)
            }
        }
        FetchStatus::FetchingLicenses { done, total } => {
            if let Some(tot) = total {
                format!("{}: {done}/{tot}", t.status_fetching_licenses)
            } else {
                format!("{}: {done}...", t.status_fetching_licenses)
            }
        }
        FetchStatus::FetchingComputers { done, total } => {
            if let Some(tot) = total {
                format!("{}: {done}/{tot}", t.status_fetching_computers)
            } else {
                format!("{}: {done}...", t.status_fetching_computers)
            }
        }
        FetchStatus::FetchingAgents { done, total } => {
            if let Some(tot) = total {
                format!("{}: {done}/{tot}", t.status_fetching_agents)
            } else {
                format!("{}: {done}...", t.status_fetching_agents)
            }
        }
        FetchStatus::CleanupPreview { count, days } => {
            format!(
                "{} {count} {} {days} {}",
                t.status_cleanup_preview,
                t.cleanup_dry_run_complete_mid,
                t.days
            )
        }
        FetchStatus::Aggregating => t.status_aggregating.to_string(),
        FetchStatus::Done { software_count, total_hosts } => {
            format!("{} {software_count} software / {total_hosts} hosts", t.status_loaded)
        }
        FetchStatus::Error(e) => format!("{}: {e}", t.status_error),
    }
}
