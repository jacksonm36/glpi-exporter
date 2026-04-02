use crate::i18n::T;
use crate::models::{AggregatedSoftware, ComputerInfo};
use eframe::egui;
use std::collections::{HashMap, HashSet};

pub fn show(
    ctx: &egui::Context,
    all_data: &[AggregatedSoftware],
    selected: &HashSet<u64>,
    computers: &HashMap<u64, ComputerInfo>,
    show: &mut bool,
    t: &T,
) {
    if !*show || selected.is_empty() {
        return;
    }

    egui::SidePanel::right("pc_panel")
        .default_width(350.0)
        .min_width(250.0)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.strong(t.pcs_with_selected);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("X").clicked() {
                        *show = false;
                    }
                });
            });
            ui.separator();

            let mut pc_to_software: HashMap<u64, Vec<&str>> = HashMap::new();
            for sw in all_data {
                if selected.contains(&sw.software_id) {
                    for &host_id in &sw.host_ids {
                        pc_to_software
                            .entry(host_id)
                            .or_default()
                            .push(&sw.name);
                    }
                }
            }

            if pc_to_software.is_empty() {
                ui.label(t.no_install_data_selected);
                return;
            }

            struct PcEntry<'a> {
                id: u64,
                name: String,
                contact: String,
                sw_names: Vec<&'a str>,
            }

            let mut pc_list: Vec<PcEntry> = pc_to_software
                .into_iter()
                .map(|(id, sw_names)| {
                    let (name, contact) = match computers.get(&id) {
                        Some(info) => (info.name.clone(), info.contact.clone()),
                        None => (format!("#{id}"), String::new()),
                    };
                    PcEntry { id, name, contact, sw_names }
                })
                .collect();
            pc_list.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

            ui.label(format!(
                "{} {} {} {} software",
                pc_list.len(),
                t.pcs_found_across,
                selected.len(),
                t.n_selected
            ));
            ui.add_space(4.0);

            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for pc in &pc_list {
                        let header_text = if pc.contact.is_empty() {
                            pc.name.clone()
                        } else {
                            format!("{} — {}", pc.name, pc.contact)
                        };

                        egui::CollapsingHeader::new(
                            egui::RichText::new(&header_text).strong(),
                        )
                        .id_salt(pc.id)
                        .show(ui, |ui| {
                            if !pc.contact.is_empty() {
                                ui.label(
                                    egui::RichText::new(format!("{}: {}", t.user_prefix, pc.contact)).weak(),
                                );
                            }
                            for sw in &pc.sw_names {
                                ui.label(format!("  • {sw}"));
                            }
                        });
                    }
                });
        });
}
