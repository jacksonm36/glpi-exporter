use crate::date_util;
use crate::i18n::T;
use crate::models::{AggregatedSoftware, ComputerInfo};
use eframe::egui;
use std::collections::{HashMap, HashSet};

/// Returns `true` if the selection changed (a checkbox was toggled).
pub fn show(
    ui: &mut egui::Ui,
    data: &[AggregatedSoftware],
    expanded: &mut HashSet<u64>,
    detail_tabs: &mut HashMap<u64, usize>,
    selected: &mut HashSet<u64>,
    recent_days: i64,
    computers: &HashMap<u64, ComputerInfo>,
    t: &T,
) -> bool {
    let mut selection_changed = false;
    if data.is_empty() {
        ui.centered_and_justified(|ui| {
            ui.label(t.no_data_msg);
        });
        return false;
    }

    egui::ScrollArea::both()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            egui::Grid::new("software_table_grid")
                .num_columns(8)
                .spacing([8.0, 2.0])
                .striped(true)
                .min_col_width(0.0)
                .show(ui, |ui| {
                    let all_visible_selected =
                        data.iter().all(|sw| selected.contains(&sw.software_id));
                    let mut header_checked = all_visible_selected && !data.is_empty();
                    if ui.checkbox(&mut header_checked, "").clicked() {
                        if header_checked {
                            for sw in data {
                                selected.insert(sw.software_id);
                            }
                        } else {
                            for sw in data {
                                selected.remove(&sw.software_id);
                            }
                        }
                        selection_changed = true;
                    }
                    ui.strong(t.col_rank);
                    ui.strong(t.col_software_name);
                    ui.strong(t.col_publisher);
                    ui.strong(t.col_hosts);
                    ui.strong(t.col_latest_version);
                    ui.strong(t.col_last_updated);
                    ui.strong(t.col_recent);
                    ui.end_row();

                    let now = chrono::Local::now().naive_local().date();

                    for (i, sw) in data.iter().enumerate() {
                        let is_expanded = expanded.contains(&sw.software_id);
                        let toggle_text = if is_expanded { "▼" } else { "▶" };
                        let recent =
                            date_util::is_recent(&sw.last_updated, now, recent_days);

                        let mut is_selected = selected.contains(&sw.software_id);
                        if ui.checkbox(&mut is_selected, "").clicked() {
                            if is_selected {
                                selected.insert(sw.software_id);
                            } else {
                                selected.remove(&sw.software_id);
                            }
                            selection_changed = true;
                        }

                        ui.label(format!("{}", i + 1));
                        let label_text = format!("{} {}", toggle_text, &sw.name);
                        let resp = ui.add(
                            egui::Label::new(egui::RichText::new(&label_text).strong())
                                .sense(egui::Sense::click()),
                        );
                        if resp.clicked() {
                            if is_expanded {
                                expanded.remove(&sw.software_id);
                            } else {
                                expanded.insert(sw.software_id);
                            }
                        }
                        ui.label(&sw.publisher);
                        ui.label(
                            egui::RichText::new(sw.total_host_count.to_string()).strong(),
                        );
                        ui.label(&sw.latest_version);
                        ui.label(sw.last_updated.as_deref().unwrap_or("-"));
                        {
                            let date_tip =
                                sw.last_updated.as_deref().unwrap_or(t.no_date);
                            if recent {
                                ui.colored_label(
                                    egui::Color32::from_rgb(0, 150, 0),
                                    t.yes,
                                )
                                .on_hover_text(date_tip);
                            } else {
                                ui.colored_label(egui::Color32::GRAY, t.no)
                                    .on_hover_text(date_tip);
                            }
                        }
                        ui.end_row();

                        if is_expanded {
                            ui.label("");
                            ui.label("");
                            ui.vertical(|ui| {
                                let active_tab =
                                    detail_tabs.entry(sw.software_id).or_insert(0);

                                egui::Frame::new()
                                    .inner_margin(10.0)
                                    .corner_radius(6.0)
                                    .fill(ui.visuals().extreme_bg_color)
                                    .stroke(egui::Stroke::new(
                                        1.0,
                                        egui::Color32::from_rgb(80, 120, 180),
                                    ))
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            let ver_label = format!(
                                                "{} ({})",
                                                t.versions_tab,
                                                sw.versions.len()
                                            );
                                            let pc_label = format!(
                                                "{} ({})",
                                                t.pcs_tab,
                                                sw.host_ids.len()
                                            );

                                            if tab_button(ui, &ver_label, *active_tab == 0)
                                            {
                                                *active_tab = 0;
                                            }
                                            ui.add_space(4.0);
                                            if tab_button(ui, &pc_label, *active_tab == 1)
                                            {
                                                *active_tab = 1;
                                            }
                                        });

                                        ui.add_space(4.0);
                                        ui.separator();
                                        ui.add_space(4.0);

                                        match *active_tab {
                                            0 => render_versions_tab(
                                                ui,
                                                sw,
                                                now,
                                                recent_days,
                                                t,
                                            ),
                                            1 => {
                                                render_pcs_tab(ui, sw, computers, t)
                                            }
                                            _ => {}
                                        }
                                    });
                            });
                            ui.label("");
                            ui.label("");
                            ui.label("");
                            ui.label("");
                            ui.label("");
                            ui.end_row();
                        }
                    }
                });
        });

    selection_changed
}

fn tab_button(ui: &mut egui::Ui, label: &str, active: bool) -> bool {
    let text = if active {
        egui::RichText::new(label).strong().size(13.0)
    } else {
        egui::RichText::new(label).weak().size(13.0)
    };

    let btn = if active {
        egui::Button::new(text)
            .fill(egui::Color32::from_rgb(60, 90, 140))
            .corner_radius(4.0)
    } else {
        egui::Button::new(text)
            .fill(egui::Color32::TRANSPARENT)
            .corner_radius(4.0)
    };

    ui.add(btn).clicked()
}

fn render_versions_tab(
    ui: &mut egui::Ui,
    sw: &AggregatedSoftware,
    now: chrono::NaiveDate,
    recent_days: i64,
    t: &T,
) {
    if sw.versions.is_empty() {
        ui.label(t.no_version_data);
        return;
    }

    egui::ScrollArea::vertical()
        .id_salt(("ver", sw.software_id))
        .max_height(280.0)
        .show(ui, |ui| {
            egui::Grid::new(("ver_grid", sw.software_id))
                .num_columns(4)
                .spacing([12.0, 3.0])
                .striped(true)
                .show(ui, |ui| {
                    ui.label(egui::RichText::new(t.col_version).strong().size(12.0));
                    ui.label(egui::RichText::new(t.col_hosts).strong().size(12.0));
                    ui.label(
                        egui::RichText::new(t.col_last_install).strong().size(12.0),
                    );
                    ui.label(egui::RichText::new(t.col_recent).strong().size(12.0));
                    ui.end_row();

                    for ver in &sw.versions {
                        let ver_recent =
                            date_util::is_recent(&ver.last_install_date, now, recent_days);

                        ui.label(egui::RichText::new(&ver.version_name).size(12.0));
                        ui.label(
                            egui::RichText::new(ver.host_count.to_string()).size(12.0),
                        );
                        ui.label(
                            egui::RichText::new(
                                ver.last_install_date.as_deref().unwrap_or("-"),
                            )
                            .size(12.0),
                        );
                        if ver_recent {
                            ui.colored_label(
                                egui::Color32::from_rgb(0, 150, 0),
                                egui::RichText::new(t.yes).size(12.0),
                            );
                        } else {
                            ui.colored_label(
                                egui::Color32::GRAY,
                                egui::RichText::new(t.no).size(12.0),
                            );
                        }
                        ui.end_row();
                    }
                });
        });
}

fn render_pcs_tab(
    ui: &mut egui::Ui,
    sw: &AggregatedSoftware,
    computers: &HashMap<u64, ComputerInfo>,
    t: &T,
) {
    let mut pc_list: Vec<(&str, &str)> = sw
        .host_ids
        .iter()
        .map(|id| match computers.get(id) {
            Some(info) => (info.name.as_str(), info.contact.as_str()),
            None => (t.unknown, ""),
        })
        .collect();
    pc_list.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));

    if pc_list.is_empty() {
        ui.label(t.no_install_data);
        return;
    }

    ui.label(
        egui::RichText::new(format!("{} {} :", pc_list.len(), t.pcs_with_software))
            .strong()
            .size(12.5),
    );
    ui.add_space(4.0);

    egui::ScrollArea::vertical()
        .id_salt(("pcs", sw.software_id))
        .max_height(280.0)
        .show(ui, |ui| {
            egui::Grid::new(("pc_grid", sw.software_id))
                .num_columns(2)
                .spacing([20.0, 3.0])
                .striped(true)
                .show(ui, |ui| {
                    ui.label(egui::RichText::new(t.col_pc_name).strong().size(12.0));
                    ui.label(
                        egui::RichText::new(t.col_user_contact).strong().size(12.0),
                    );
                    ui.end_row();

                    for (pc_name, contact) in &pc_list {
                        ui.label(
                            egui::RichText::new(*pc_name)
                                .size(12.0)
                                .color(egui::Color32::from_rgb(160, 195, 235)),
                        );
                        ui.label(
                            egui::RichText::new(if contact.is_empty() {
                                "—"
                            } else {
                                contact
                            })
                            .size(12.0)
                            .color(egui::Color32::from_rgb(180, 180, 180)),
                        );
                        ui.end_row();
                    }
                });
        });
}
