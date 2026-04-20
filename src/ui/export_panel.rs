use crate::export::{csv_export, excel_export, json_export};
use crate::i18n::T;
use crate::models::{AggregatedSoftware, ComputerInfo};
use eframe::egui;
use std::collections::HashMap;

pub fn show(
    ui: &mut egui::Ui,
    data: &[AggregatedSoftware],
    computers: &HashMap<u64, ComputerInfo>,
    export_message: &mut Option<String>,
    show_audit_export_note: bool,
    t: &T,
) {
    let has_computers = !computers.is_empty();
    let has_software_table = !data.is_empty();

    ui.horizontal(|ui| {
        ui.label(t.export);

        if ui
            .add_enabled(has_computers, egui::Button::new("CSV"))
            .clicked()
        {
            if let Some(path) = rfd::FileDialog::new()
                .set_title(t.save_csv)
                .add_filter(t.csv_files, &["csv"])
                .set_file_name("glpi_computer_inventory.csv")
                .save_file()
            {
                match csv_export::export_csv(computers, &path) {
                    Ok(()) => *export_message = Some(format!("{} {}", t.csv_saved, path.display())),
                    Err(e) => *export_message = Some(format!("{}: {e}", t.csv_error)),
                }
            }
        }

        if ui
            .add_enabled(has_computers, egui::Button::new("Excel"))
            .clicked()
        {
            if let Some(path) = rfd::FileDialog::new()
                .set_title(t.save_excel)
                .add_filter(t.excel_files, &["xlsx"])
                .set_file_name("glpi_computer_inventory.xlsx")
                .save_file()
            {
                match excel_export::export_excel(computers, &path) {
                    Ok(()) => *export_message = Some(format!("{} {}", t.excel_saved, path.display())),
                    Err(e) => *export_message = Some(format!("{}: {e}", t.excel_error)),
                }
            }
        }

        if ui
            .add_enabled(has_computers, egui::Button::new("JSON"))
            .clicked()
        {
            if let Some(path) = rfd::FileDialog::new()
                .set_title(t.save_json)
                .add_filter(t.json_files, &["json"])
                .set_file_name("glpi_computer_inventory.json")
                .save_file()
            {
                match json_export::export_json(computers, &path) {
                    Ok(()) => *export_message = Some(format!("{} {}", t.json_saved, path.display())),
                    Err(e) => *export_message = Some(format!("{}: {e}", t.json_error)),
                }
            }
        }

        ui.add_space(12.0);
        if ui
            .add_enabled(has_software_table, egui::Button::new(t.export_software_csv))
            .clicked()
        {
            if let Some(path) = rfd::FileDialog::new()
                .set_title(t.save_software_csv)
                .add_filter(t.csv_files, &["csv"])
                .set_file_name("glpi_software_table.csv")
                .save_file()
            {
                match csv_export::export_software_inventory_csv(data, &path) {
                    Ok(()) => {
                        *export_message = Some(format!(
                            "{} {}",
                            t.software_table_export_saved,
                            path.display()
                        ))
                    }
                    Err(e) => *export_message = Some(format!("{}: {e}", t.software_export_error)),
                }
            }
        }
        if ui
            .add_enabled(has_software_table, egui::Button::new(t.export_software_excel))
            .clicked()
        {
            if let Some(path) = rfd::FileDialog::new()
                .set_title(t.save_software_excel)
                .add_filter(t.excel_files, &["xlsx"])
                .set_file_name("glpi_software_table.xlsx")
                .save_file()
            {
                match excel_export::export_software_inventory_excel(data, &path) {
                    Ok(()) => {
                        *export_message = Some(format!(
                            "{} {}",
                            t.software_table_export_saved,
                            path.display()
                        ))
                    }
                    Err(e) => *export_message = Some(format!("{}: {e}", t.software_export_error)),
                }
            }
        }
        if ui
            .add_enabled(has_software_table, egui::Button::new(t.export_software_json))
            .clicked()
        {
            if let Some(path) = rfd::FileDialog::new()
                .set_title(t.save_software_json)
                .add_filter(t.json_files, &["json"])
                .set_file_name("glpi_software_table.json")
                .save_file()
            {
                match json_export::export_software_inventory_json(data, &path) {
                    Ok(()) => {
                        *export_message = Some(format!(
                            "{} {}",
                            t.software_table_export_saved,
                            path.display()
                        ))
                    }
                    Err(e) => *export_message = Some(format!("{}: {e}", t.software_export_error)),
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

        if show_audit_export_note {
            ui.add_space(8.0);
            ui.label(egui::RichText::new(t.export_note_audit_rows).weak().small());
        }
    });
}
