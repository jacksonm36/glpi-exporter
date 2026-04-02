use crate::export::{csv_export, excel_export, json_export};
use crate::models::AggregatedSoftware;
use eframe::egui;

pub fn show(ui: &mut egui::Ui, data: &[AggregatedSoftware], export_message: &mut Option<String>, recent_days: i64) {
    let has_data = !data.is_empty();

    ui.horizontal(|ui| {
        ui.label("Export:");

        if ui
            .add_enabled(has_data, egui::Button::new("CSV"))
            .clicked()
        {
            if let Some(path) = rfd::FileDialog::new()
                .set_title("Save CSV")
                .add_filter("CSV Files", &["csv"])
                .set_file_name("glpi_software_report.csv")
                .save_file()
            {
                match csv_export::export_csv(data, &path, recent_days) {
                    Ok(()) => *export_message = Some(format!("CSV saved to {}", path.display())),
                    Err(e) => *export_message = Some(format!("CSV error: {e}")),
                }
            }
        }

        if ui
            .add_enabled(has_data, egui::Button::new("Excel"))
            .clicked()
        {
            if let Some(path) = rfd::FileDialog::new()
                .set_title("Save Excel")
                .add_filter("Excel Files", &["xlsx"])
                .set_file_name("glpi_software_report.xlsx")
                .save_file()
            {
                match excel_export::export_excel(data, &path, recent_days) {
                    Ok(()) => {
                        *export_message = Some(format!("Excel saved to {}", path.display()))
                    }
                    Err(e) => *export_message = Some(format!("Excel error: {e}")),
                }
            }
        }

        if ui
            .add_enabled(has_data, egui::Button::new("JSON"))
            .clicked()
        {
            if let Some(path) = rfd::FileDialog::new()
                .set_title("Save JSON")
                .add_filter("JSON Files", &["json"])
                .set_file_name("glpi_software_report.json")
                .save_file()
            {
                match json_export::export_json(data, &path) {
                    Ok(()) => *export_message = Some(format!("JSON saved to {}", path.display())),
                    Err(e) => *export_message = Some(format!("JSON error: {e}")),
                }
            }
        }

        if let Some(ref msg) = export_message {
            ui.add_space(10.0);
            if msg.contains("error") || msg.contains("Error") {
                ui.colored_label(egui::Color32::RED, msg);
            } else {
                ui.colored_label(egui::Color32::from_rgb(0, 150, 0), msg);
            }
        }
    });
}
