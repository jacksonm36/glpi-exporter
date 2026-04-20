use crate::i18n::T;
use crate::models::{AggregatedSoftware, HistoryViewSummary};
use eframe::egui;

pub fn show(ui: &mut egui::Ui, filtered_data: &[AggregatedSoftware], total_data: usize, t: &T) {
    ui.horizontal(|ui| {
        let total_installations: usize = filtered_data.iter().map(|s| s.total_host_count).sum();

        if total_data > 0 {
            ui.label(format!(
                "{} {} / {} software",
                t.showing_of,
                filtered_data.len(),
                total_data
            ));
            ui.separator();
            ui.label(format!("{}: {}", t.total_installations, format_number(total_installations)));
        } else {
            ui.label(t.no_data_loaded);
        }
    });
}

pub fn show_history(ui: &mut egui::Ui, summary: Option<&HistoryViewSummary>, t: &T) {
    ui.horizontal(|ui| {
        if let Some(summary) = summary {
            ui.label(format!(
                "{} {} / {}: {}",
                t.history_snapshot_label, summary.snapshot_captured_at, t.col_software_name, summary.software_count
            ));
            ui.separator();
            ui.label(format!("{}: {}", t.col_hosts, format_number(summary.host_count)));
        } else {
            ui.label(t.history_no_snapshot);
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
