use crate::app::AppState;
use eframe::egui;

pub fn show(ui: &mut egui::Ui, state: &mut AppState) {
    ui.horizontal(|ui| {
        ui.label("GLPI URL:");
        ui.add(
            egui::TextEdit::singleline(&mut state.config.glpi_url)
                .desired_width(300.0)
                .hint_text("https://glpi.example.com"),
        );
    });

    ui.horizontal(|ui| {
        ui.label("User Token:");
        ui.add(
            egui::TextEdit::singleline(&mut state.config.user_token)
                .desired_width(250.0)
                .password(true)
                .hint_text("Your GLPI API user token"),
        );

        ui.add_space(10.0);
        ui.label("App Token:");
        ui.add(
            egui::TextEdit::singleline(&mut state.config.app_token)
                .desired_width(250.0)
                .password(true)
                .hint_text("Optional"),
        );
    });

    ui.horizontal(|ui| {
        ui.checkbox(
            &mut state.config.accept_invalid_certs,
            "Accept invalid TLS certificates",
        )
        .on_hover_text("Only enable this for self-signed certificates. Disables TLS verification.");

        ui.add_space(10.0);
        ui.colored_label(
            egui::Color32::from_rgb(180, 140, 0),
            "Tokens are saved in plaintext next to the .exe",
        );
    });

    ui.horizontal(|ui| {
        let is_fetching = matches!(
            state.status,
            crate::models::FetchStatus::Connecting
                | crate::models::FetchStatus::FetchingSoftware { .. }
                | crate::models::FetchStatus::FetchingVersions { .. }
                | crate::models::FetchStatus::FetchingInstallations { .. }
                | crate::models::FetchStatus::FetchingComputers { .. }
                | crate::models::FetchStatus::Aggregating
        );

        let can_connect = !state.config.glpi_url.is_empty()
            && !state.config.user_token.is_empty()
            && !is_fetching;

        if ui
            .add_enabled(can_connect, egui::Button::new("Connect & Fetch"))
            .clicked()
        {
            state.request_fetch();
        }

        ui.add_space(10.0);

        let status_text = format!("Status: {}", state.status);
        match &state.status {
            crate::models::FetchStatus::Error(_) => {
                ui.colored_label(egui::Color32::RED, &status_text);
            }
            crate::models::FetchStatus::Done { .. } => {
                ui.colored_label(egui::Color32::from_rgb(0, 150, 0), &status_text);
            }
            _ => {
                ui.label(&status_text);
            }
        }
    });
}
