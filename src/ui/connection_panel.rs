use crate::app::AppState;
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

    ui.horizontal(|ui| {
        let is_fetching = matches!(
            state.status,
            FetchStatus::Connecting
                | FetchStatus::FetchingSoftware { .. }
                | FetchStatus::FetchingVersions { .. }
                | FetchStatus::FetchingInstallations { .. }
                | FetchStatus::FetchingComputers { .. }
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
        FetchStatus::FetchingComputers { done, total } => {
            if let Some(tot) = total {
                format!("{}: {done}/{tot}", t.status_fetching_computers)
            } else {
                format!("{}: {done}...", t.status_fetching_computers)
            }
        }
        FetchStatus::Aggregating => t.status_aggregating.to_string(),
        FetchStatus::Done { software_count, total_hosts } => {
            format!("{} {software_count} software / {total_hosts} hosts", t.status_loaded)
        }
        FetchStatus::Error(e) => format!("{}: {e}", t.status_error),
    }
}
