use crate::export::{csv_export, excel_export, json_export};
use crate::i18n::T;
use crate::models::AggregatedSoftware;
use eframe::egui;

pub fn show(ui: &mut egui::Ui, data: &[AggregatedSoftware], export_message: &mut Option<String>, recent_days: i64, t: &T) {
    let has_data = !data.is_empty();

    ui.horizontal(|ui| {
        ui.label(t.export);

        if ui
            .add_enabled(has_data, egui::Button::new("CSV"))
            .clicked()
        {
            if let Some(path) = rfd::FileDialog::new()
                .set_title(t.save_csv)
                .add_filter(t.csv_files, &["csv"])
                .set_file_name("glpi_software_report.csv")
                .save_file()
            {
                match csv_export::export_csv(data, &path, recent_days) {
                    Ok(()) => *export_message = Some(format!("{} {}", t.csv_saved, path.display())),
                    Err(e) => *export_message = Some(format!("{}: {e}", t.csv_error)),
                }
            }
        }

        if ui
            .add_enabled(has_data, egui::Button::new("Excel"))
            .clicked()
        {
            if let Some(path) = rfd::FileDialog::new()
                .set_title(t.save_excel)
                .add_filter(t.excel_files, &["xlsx"])
                .set_file_name("glpi_software_report.xlsx")
                .save_file()
            {
                match excel_export::export_excel(data, &path, recent_days) {
                    Ok(()) => *export_message = Some(format!("{} {}", t.excel_saved, path.display())),
                    Err(e) => *export_message = Some(format!("{}: {e}", t.excel_error)),
                }
            }
        }

        if ui
            .add_enabled(has_data, egui::Button::new("JSON"))
            .clicked()
        {
            if let Some(path) = rfd::FileDialog::new()
                .set_title(t.save_json)
                .add_filter(t.json_files, &["json"])
                .set_file_name("glpi_software_report.json")
                .save_file()
            {
                match json_export::export_json(data, &path) {
                    Ok(()) => *export_message = Some(format!("{} {}", t.json_saved, path.display())),
                    Err(e) => *export_message = Some(format!("{}: {e}", t.json_error)),
                }
            }
        }

        if let Some(ref msg) = export_message {
            ui.add_space(10.0);
            if msg.contains("error") || msg.contains("Error") || msg.contains("hiba") || msg.contains("Hiba") {
                ui.colored_label(egui::Color32::RED, msg);
            } else {
                ui.colored_label(egui::Color32::from_rgb(0, 150, 0), msg);
            }
        }
    });
}
