use crate::models::AggregatedSoftware;
use eframe::egui;

pub fn show(ui: &mut egui::Ui, filtered_data: &[AggregatedSoftware], total_data: usize) {
    ui.horizontal(|ui| {
        let total_installations: usize = filtered_data.iter().map(|s| s.total_host_count).sum();

        if total_data > 0 {
            ui.label(format!(
                "Showing {} of {} software",
                filtered_data.len(),
                total_data
            ));
            ui.separator();
            ui.label(format!("Total installations: {}", format_number(total_installations)));
        } else {
            ui.label("No data loaded");
        }
    });
}

fn format_number(n: usize) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result.chars().rev().collect()
}
