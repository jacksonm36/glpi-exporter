use crate::app::AppState;
use crate::models::{FetchStatus, SoftwareCleanupCandidate};
use eframe::egui;
use std::fs::File;
use std::path::Path;

pub fn show(ui: &mut egui::Ui, state: &mut AppState) {
    let t = state.t();
    let is_busy = matches!(
        state.status,
        FetchStatus::Connecting
            | FetchStatus::FetchingSoftware { .. }
            | FetchStatus::FetchingVersions { .. }
            | FetchStatus::FetchingInstallations { .. }
            | FetchStatus::FetchingLicenses { .. }
            | FetchStatus::FetchingComputers { .. }
            | FetchStatus::Aggregating
    );

    ui.add_space(4.0);
    ui.collapsing(t.cleanup_title, |ui| {
        ui.label(t.cleanup_warning);
        ui.add_space(4.0);

        ui.horizontal(|ui| {
            ui.label(t.cleanup_older_than);
            let response = ui.add(
                egui::TextEdit::singleline(&mut state.cleanup_days)
                    .desired_width(55.0)
                    .hint_text("60"),
            );
            if response.changed() {
                state.cleanup_days.retain(|c| c.is_ascii_digit());
            }
            ui.label(t.days);

            ui.add_space(10.0);
            if ui
                .add_enabled(
                    !is_busy
                        && !state.config.glpi_url.is_empty()
                        && !state.config.user_token.is_empty(),
                    egui::Button::new(t.cleanup_dry_run),
                )
                .clicked()
            {
                state.request_cleanup_dry_run();
            }

            ui.add_space(6.0);
            if ui
                .add_enabled(
                    !is_busy,
                    egui::Button::new(t.cleanup_save_csv),
                )
                .clicked()
            {
                if let Some(path) = rfd::FileDialog::new()
                    .set_title(t.cleanup_save_csv)
                    .add_filter(t.csv_files, &["csv"])
                    .set_file_name("glpi_cleanup_preview.csv")
                    .save_file()
                {
                    match save_cleanup_csv(&state.cleanup_preview, &path) {
                        Ok(()) => {
                            state.cleanup_message =
                                Some(format!("{} {}", t.csv_saved, path.display()))
                        }
                        Err(e) => state.cleanup_message = Some(format!("{}: {e}", t.csv_error)),
                    }
                }
            }

            if ui
                .add_enabled(
                    !is_busy,
                    egui::Button::new(t.cleanup_save_json),
                )
                .clicked()
            {
                if let Some(path) = rfd::FileDialog::new()
                    .set_title(t.cleanup_save_json)
                    .add_filter(t.json_files, &["json"])
                    .set_file_name("glpi_cleanup_preview.json")
                    .save_file()
                {
                    match save_cleanup_json(&state.cleanup_preview, &path) {
                        Ok(()) => {
                            state.cleanup_message =
                                Some(format!("{} {}", t.json_saved, path.display()))
                        }
                        Err(e) => state.cleanup_message = Some(format!("{}: {e}", t.json_error)),
                    }
                }
            }

            if ui
                .add_enabled(!is_busy, egui::Button::new(t.cleanup_save_sql))
                .clicked()
            {
                if let Some(path) = rfd::FileDialog::new()
                    .set_title(t.cleanup_save_sql)
                    .add_filter(t.sql_files, &["sql"])
                    .set_file_name("glpi_cleanup_preview_mariadb.sql")
                    .save_file()
                {
                    match save_cleanup_sql(&state.cleanup_preview, &path) {
                        Ok(()) => {
                            state.cleanup_message =
                                Some(format!("{} {}", t.sql_saved, path.display()))
                        }
                        Err(e) => state.cleanup_message = Some(format!("{}: {e}", t.sql_error)),
                    }
                }
            }
        });

        ui.add_space(4.0);
        if !state.cleanup_preview.is_empty() {
            ui.label(format!(
                "{} {}",
                state.cleanup_preview.len(),
                t.cleanup_candidates_found
            ));

            egui::ScrollArea::vertical()
                .max_height(140.0)
                .show(ui, |ui| {
                    egui::Grid::new("cleanup_preview_grid")
                        .striped(true)
                        .num_columns(4)
                        .show(ui, |ui| {
                            ui.label("ID");
                            ui.label(t.col_software_name);
                            ui.label(t.col_publisher);
                            ui.label(t.col_last_updated);
                            ui.end_row();

                            for item in &state.cleanup_preview {
                                ui.label(item.software_id.to_string());
                                ui.label(&item.name);
                                ui.label(&item.publisher);
                                ui.label(&item.date_mod);
                                ui.end_row();
                            }
                        });
                });
        }

        if let Some(ref msg) = state.cleanup_message {
            ui.add_space(4.0);
            if msg.contains("error")
                || msg.contains("Error")
                || msg.contains("failed")
                || msg.contains("Hiba")
                || msg.contains("hiba")
            {
                ui.colored_label(egui::Color32::RED, msg);
            } else {
                ui.colored_label(egui::Color32::from_rgb(0, 150, 0), msg);
            }
        }
    });
}

fn save_cleanup_csv(items: &[SoftwareCleanupCandidate], path: &Path) -> Result<(), String> {
    let mut wtr =
        csv::Writer::from_path(path).map_err(|e| format!("Cannot create CSV file: {e}"))?;
    wtr.write_record(["software_id", "name", "publisher", "date_mod"])
        .map_err(|e| format!("CSV write error: {e}"))?;

    for item in items {
        wtr.write_record([
            item.software_id.to_string(),
            item.name.clone(),
            item.publisher.clone(),
            item.date_mod.clone(),
        ])
        .map_err(|e| format!("CSV write error: {e}"))?;
    }
    wtr.flush().map_err(|e| format!("CSV flush error: {e}"))?;
    Ok(())
}

fn save_cleanup_json(items: &[SoftwareCleanupCandidate], path: &Path) -> Result<(), String> {
    let file = File::create(path).map_err(|e| format!("Cannot create JSON file: {e}"))?;
    serde_json::to_writer_pretty(file, items).map_err(|e| format!("JSON write error: {e}"))?;
    Ok(())
}

fn save_cleanup_sql(items: &[SoftwareCleanupCandidate], path: &Path) -> Result<(), String> {
    let mut sql = String::new();
    sql.push_str("-- GLPI cleanup preview export for MariaDB\n");
    sql.push_str(
        "-- Update table names if your GLPI schema differs from defaults.\n\n",
    );
    sql.push_str("START TRANSACTION;\n\n");
    sql.push_str(
        "CREATE TABLE IF NOT EXISTS glpi_cleanup_preview (\n\
         \tid BIGINT NOT NULL,\n\
         \tname VARCHAR(255) NOT NULL,\n\
         \tpublisher VARCHAR(255) NOT NULL,\n\
         \tlast_activity DATE NULL,\n\
         \tPRIMARY KEY (id)\n\
         ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;\n\n",
    );
    sql.push_str("TRUNCATE TABLE glpi_cleanup_preview;\n\n");

    for item in items {
        let name = escape_sql(&item.name);
        let publisher = escape_sql(&item.publisher);
        let date_mod = escape_sql(&item.date_mod);
        sql.push_str(&format!(
            "INSERT INTO glpi_cleanup_preview (id, name, publisher, last_activity) VALUES ({}, '{}', '{}', '{}');\n",
            item.software_id, name, publisher, date_mod
        ));
    }

    sql.push_str("\nCOMMIT;\n");

    std::fs::write(path, sql).map_err(|e| format!("SQL write error: {e}"))?;
    Ok(())
}

fn escape_sql(value: &str) -> String {
    value.replace('\\', "\\\\").replace('\'', "''")
}
