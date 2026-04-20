use crate::app::{AppState, MainInventoryTab};
use crate::date_util;
use crate::i18n::T;
use crate::models::{AgentInfo, AggregatedSoftware, FilterState};
use crate::ui::agent_panel::{agent_status_presentation, ping_presentation};
use eframe::egui;
use egui_extras::{Column, TableBuilder};
use std::collections::{HashMap, HashSet};

/// Newest agent `last_contact` among PCs in `host_ids` (by parsed datetime).
fn latest_agent_contact_among_hosts(
    host_ids: &HashSet<u64>,
    agent_by_id: &HashMap<u64, &AgentInfo>,
) -> String {
    let mut best: Option<(chrono::NaiveDateTime, String)> = None;
    let mut fallback: Option<String> = None;

    for &hid in host_ids {
        let Some(agent) = agent_by_id.get(&hid) else {
            continue;
        };
        let Some(ref raw) = agent.last_contact else {
            continue;
        };
        let s = raw.trim();
        if s.is_empty() {
            continue;
        }
        if let Some(dt) = date_util::parse_datetime(s) {
            if best.as_ref().map_or(true, |(bdt, _)| dt > *bdt) {
                best = Some((dt, s.to_string()));
            }
        } else if fallback.is_none() {
            fallback = Some(s.to_string());
        }
    }

    best.map(|(_, s)| s)
        .or(fallback)
        .unwrap_or_else(|| "—".to_string())
}

fn versions_for_host(sw: &AggregatedSoftware, host_id: u64) -> String {
    let mut v: Vec<&str> = sw
        .versions
        .iter()
        .filter(|ver| ver.host_ids.contains(&host_id))
        .map(|ver| ver.version_name.as_str())
        .collect();
    v.sort();
    if v.is_empty() {
        "—".to_string()
    } else {
        v.join(", ")
    }
}

/// Returns `(selection_changed, filters_changed)`.
pub fn show(
    ui: &mut egui::Ui,
    data: &[AggregatedSoftware],
    total_loaded: usize,
    filtered_row_count: usize,
    state: &mut AppState,
    t: &T,
) -> (bool, bool) {
    let mut selection_changed = false;
    let mut filters_changed = false;
    let mut filters_local = state.filters.clone();
    if data.is_empty() {
        ui.centered_and_justified(|ui| {
            if total_loaded == 0 {
                ui.label(t.no_data_msg);
            } else if filtered_row_count == 0 {
                ui.label(t.no_filter_matches_msg);
                ui.add_space(6.0);
                ui.label(egui::RichText::new(t.no_filter_matches_hint).weak());
                if matches!(state.main_inventory_tab, MainInventoryTab::AgentFreshOnly) {
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new(t.agent_fresh_after_filters_hint).weak());
                }
                if filters_local.recently_updated
                    || filters_local.recent_install_only
                    || filters_local.every_host_install_in_window
                {
                    ui.add_space(10.0);
                    if ui.button(t.show_full_inventory).clicked() {
                        filters_local.recently_updated = false;
                        filters_local.recent_install_only = false;
                        filters_local.every_host_install_in_window = false;
                        filters_local.recent_use_host_inventory = false;
                        filters_changed = true;
                    }
                }
            } else if matches!(state.main_inventory_tab, MainInventoryTab::AgentFreshOnly) {
                if state.agents.is_empty() {
                    ui.label(t.agent_fresh_need_agents);
                } else {
                    ui.label(t.agent_fresh_no_matches);
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new(t.main_inv_agent_fresh_tip).weak());
                }
                ui.add_space(10.0);
                if ui.button(t.agent_fresh_switch_to_full).clicked() {
                    state.main_inventory_tab = MainInventoryTab::Full;
                }
            } else {
                ui.label(t.no_filter_matches_msg);
                ui.add_space(6.0);
                ui.label(egui::RichText::new(t.no_filter_matches_hint).weak());
                if filters_local.recently_updated
                    || filters_local.recent_install_only
                    || filters_local.every_host_install_in_window
                {
                    ui.add_space(10.0);
                    if ui.button(t.show_full_inventory).clicked() {
                        filters_local.recently_updated = false;
                        filters_local.recent_install_only = false;
                        filters_local.every_host_install_in_window = false;
                        filters_local.recent_use_host_inventory = false;
                        filters_changed = true;
                    }
                }
            }
        });
        if filters_changed {
            state.filters = filters_local;
        }
        show_removal_only_audit_section(ui, state, t);
        return (false, filters_changed);
    }

    let now = chrono::Local::now().date_naive();

    let agents_for_table = state.agents.clone();
    let agent_by_id: HashMap<u64, &AgentInfo> =
        crate::ui::agent_panel::agent_by_computer_id(&agents_for_table);

    let all_visible_selected = data
        .iter()
        .all(|sw| state.selected.contains(&sw.software_id));

    TableBuilder::new(ui)
        .id_salt("software_table_main")
        .striped(true)
        .resizable(true)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::auto().at_least(24.0))
        .column(Column::auto().at_least(24.0))
        .column(Column::initial(240.0).at_least(100.0).resizable(true).clip(true))
        .column(Column::initial(160.0).at_least(60.0).resizable(true).clip(true))
        .column(Column::auto().at_least(40.0))
        .column(Column::initial(120.0).at_least(50.0).resizable(true).clip(true))
        .column(Column::initial(130.0).at_least(56.0).resizable(true).clip(true))
        .column(Column::initial(100.0).at_least(50.0).resizable(true).clip(true))
        .column(Column::remainder().at_least(40.0))
        .header(20.0, |mut header| {
            header.col(|ui| {
                let mut hdr = all_visible_selected && !data.is_empty();
                if ui.checkbox(&mut hdr, "").clicked() {
                    if hdr {
                        for sw in data {
                            state.selected.insert(sw.software_id);
                        }
                    } else {
                        for sw in data {
                            state.selected.remove(&sw.software_id);
                        }
                    }
                    selection_changed = true;
                }
            });
            header.col(|ui| {
                ui.strong(t.col_rank);
            });
            header.col(|ui| {
                ui.strong(t.col_software_name);
            });
            header.col(|ui| {
                ui.strong(t.col_publisher);
            });
            header.col(|ui| {
                ui.strong(t.col_hosts);
            });
            header.col(|ui| {
                ui.strong(t.col_latest_version);
            });
            header.col(|ui| {
                ui.strong(t.col_last_updated);
            });
            header.col(|ui| {
                ui.strong(t.col_software_latest_agent_contact);
            });
            header.col(|ui| {
                ui.strong(t.col_recent);
            });
        })
        .body(|mut body| {
            for (i, sw) in data.iter().enumerate() {
                let is_expanded = state.expanded.contains(&sw.software_id);
                let toggle_text = if is_expanded { "▼" } else { "▶" };
                let recent_date = if filters_local.every_host_install_in_window {
                    &sw.all_hosts_install_floor
                } else if filters_local.recent_install_only {
                    &sw.last_install_date
                } else if filters_local.recently_updated {
                    if filters_local.recent_use_host_inventory {
                        &sw.last_host_inventory
                    } else {
                        &sw.last_agent_pull
                    }
                } else {
                    &sw.last_updated
                };
                let recent = date_util::date_in_recency_window(recent_date, now, &filters_local);
                let sw_agent_contact =
                    latest_agent_contact_among_hosts(&sw.host_ids, &agent_by_id);

                body.row(20.0, |mut row| {
                    row.col(|ui| {
                        let mut is_sel = state.selected.contains(&sw.software_id);
                        if ui.checkbox(&mut is_sel, "").clicked() {
                            if is_sel {
                                state.selected.insert(sw.software_id);
                            } else {
                                state.selected.remove(&sw.software_id);
                            }
                            selection_changed = true;
                        }
                    });
                    row.col(|ui| {
                        ui.label(format!("{}", i + 1));
                    });
                    row.col(|ui| {
                        let label_text = format!("{} {}", toggle_text, &sw.name);
                        let resp = ui.add(
                            egui::Label::new(egui::RichText::new(&label_text).strong())
                                .sense(egui::Sense::click()),
                        );
                        if resp.clicked() {
                            if is_expanded {
                                state.expanded.remove(&sw.software_id);
                            } else {
                                state.expanded.insert(sw.software_id);
                            }
                        }
                    });
                    row.col(|ui| {
                        ui.label(&sw.publisher);
                    });
                    row.col(|ui| {
                        ui.label(egui::RichText::new(sw.total_host_count.to_string()).strong());
                    });
                    row.col(|ui| {
                        ui.label(&sw.latest_version);
                    });
                    row.col(|ui| {
                        ui.label(recent_date.as_deref().unwrap_or_else(|| {
                            if filters_local.recently_updated
                                || filters_local.recent_install_only
                                || filters_local.every_host_install_in_window
                            {
                                "-"
                            } else {
                                sw.last_updated.as_deref().unwrap_or("-")
                            }
                        }));
                    });
                    row.col(|ui| {
                        ui.label(&sw_agent_contact).on_hover_text(t.col_agent_contact);
                    });
                    row.col(|ui| {
                        let date_tip = recent_date.as_deref().unwrap_or(t.no_date);
                        if recent {
                            ui.colored_label(egui::Color32::from_rgb(0, 150, 0), t.yes)
                                .on_hover_text(date_tip);
                        } else {
                            ui.colored_label(egui::Color32::GRAY, t.no)
                                .on_hover_text(date_tip);
                        }
                    });
                });

                if state.expanded.contains(&sw.software_id) {
                    body.row(380.0, |mut row| {
                        row.col(|_| {});
                        row.col(|_| {});
                        row.col(|ui| {
                            let sw_id = sw.software_id;
                            let mut active_tab = state
                                .detail_tabs
                                .get(&sw_id)
                                .copied()
                                .unwrap_or(0);

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
                                        let ver_label =
                                            format!("{} ({})", t.versions_tab, sw.versions.len());
                                        let pc_label =
                                            format!("{} ({})", t.pcs_tab, sw.host_ids.len());

                                        if tab_button(ui, &ver_label, active_tab == 0) {
                                            active_tab = 0;
                                        }
                                        ui.add_space(4.0);
                                        if tab_button(ui, &pc_label, active_tab == 1) {
                                            active_tab = 1;
                                        }
                                    });

                                    ui.add_space(4.0);
                                    ui.separator();
                                    ui.add_space(4.0);

                                    match active_tab {
                                        0 => render_versions_tab(ui, sw, now, &filters_local, t),
                                        1 => render_pcs_tab(ui, sw, &filters_local, state, t),
                                        _ => {}
                                    }
                                });
                            state.detail_tabs.insert(sw_id, active_tab);
                        });
                        row.col(|_| {});
                        row.col(|_| {});
                        row.col(|_| {});
                        row.col(|_| {});
                        row.col(|_| {});
                        row.col(|_| {});
                    });
                }
            }
        });

    show_removal_only_audit_section(ui, state, t);

    (selection_changed, filters_changed)
}

fn show_removal_only_audit_section(ui: &mut egui::Ui, state: &AppState, t: &T) {
    if !state.main_table_show_audit_removals || state.audit_removals_by_key.is_empty() {
        return;
    }

    let inventory: HashSet<String> = state
        .all_data
        .iter()
        .map(|s| s.name_lower.clone())
        .collect();

    let mut only_audit: Vec<(&String, &crate::models::AuditRemovalGroup)> = state
        .audit_removals_by_key
        .iter()
        .filter(|(k, _)| !inventory.contains(*k))
        .collect();
    only_audit.sort_by(|a, b| a.0.cmp(b.0));

    if only_audit.is_empty() {
        return;
    }

    ui.add_space(12.0);
    ui.separator();
    ui.add_space(6.0);
    ui.label(
        egui::RichText::new(t.main_table_audit_section)
            .strong()
            .color(egui::Color32::from_rgb(200, 80, 80)),
    );
    ui.add_space(4.0);

    egui::ScrollArea::vertical()
        .max_height(200.0)
        .show(ui, |ui| {
            egui::Grid::new("removal_only_audit_grid")
                .num_columns(3)
                .spacing([12.0, 4.0])
                .striped(true)
                .show(ui, |ui| {
                    ui.strong(t.col_software_name);
                    ui.strong(t.main_table_audit_hosts);
                    ui.strong(t.col_last_updated);
                    ui.end_row();

                    for (key, group) in only_audit {
                        let title = if group.display_label.is_empty() {
                            key.as_str()
                        } else {
                            group.display_label.as_str()
                        };
                        ui.label(
                            egui::RichText::new(format!("{} {}", t.pc_software_deleted_tag, title))
                                .color(egui::Color32::from_rgb(220, 60, 60)),
                        );
                        ui.label(group.items.len().to_string());
                        ui.label(
                            group
                                .items
                                .iter()
                                .map(|i| i.removed_at.get(..10).unwrap_or(&i.removed_at))
                                .collect::<std::collections::BTreeSet<_>>()
                                .into_iter()
                                .collect::<Vec<_>>()
                                .join(", "),
                        );
                        ui.end_row();
                    }
                });
        });
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
    filters: &FilterState,
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
                    ui.label(egui::RichText::new(t.col_last_install).strong().size(12.0));
                    ui.label(egui::RichText::new(t.col_recent).strong().size(12.0));
                    ui.end_row();

                    for ver in &sw.versions {
                        let ver_recent = if filters.recently_updated && !filters.recent_install_only
                        {
                            false
                        } else {
                            date_util::date_in_recency_window(&ver.last_install_date, now, filters)
                        };

                        ui.label(egui::RichText::new(&ver.version_name).size(12.0));
                        ui.label(egui::RichText::new(ver.host_count.to_string()).size(12.0));
                        ui.label(
                            egui::RichText::new(ver.last_install_date.as_deref().unwrap_or("-"))
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
    filters: &FilterState,
    state: &mut AppState,
    t: &T,
) {
    let computers = state.computers.clone();
    let now = chrono::Local::now().date_naive();
    let agent_by_id: HashMap<u64, crate::models::AgentInfo> =
        crate::ui::agent_panel::agent_info_by_computer_id_merged(&state.agents);

    let host_visible = |host_id: u64| {
        if !(filters.recently_updated && filters.recent_use_host_inventory) {
            return true;
        }
        computers
            .get(&host_id)
            .and_then(|info| {
                let s = info.last_inventory.trim();
                if s.is_empty() {
                    return None;
                }
                Some(date_util::date_in_recency_window(
                    &Some(s.to_string()),
                    now,
                    filters,
                ))
            })
            .unwrap_or(false)
    };

    let deleted_color = egui::Color32::from_rgb(220, 60, 60);

    #[derive(Clone)]
    enum PcRow {
        Installed {
            id: u64,
        },
        RemovedAudit {
            id: u64,
            removed_at: String,
        },
    }

    let mut rows: Vec<PcRow> = sw
        .host_ids
        .iter()
        .filter(|id| host_visible(**id))
        .map(|id| PcRow::Installed { id: *id })
        .collect();

    if state.main_table_show_audit_removals {
        if let Some(group) = state.audit_removals_by_key.get(&sw.name_lower) {
            for it in &group.items {
                if sw.host_ids.contains(&it.computer_id) {
                    continue;
                }
                if !host_visible(it.computer_id) {
                    continue;
                }
                rows.push(PcRow::RemovedAudit {
                    id: it.computer_id,
                    removed_at: it.removed_at.clone(),
                });
            }
        }
    }

    rows.sort_by(|a, b| {
        let name_a = match a {
            PcRow::Installed { id } => computers.get(id).map(|c| c.name.as_str()).unwrap_or(""),
            PcRow::RemovedAudit { id, .. } => computers.get(id).map(|c| c.name.as_str()).unwrap_or(""),
        };
        let name_b = match b {
            PcRow::Installed { id } => computers.get(id).map(|c| c.name.as_str()).unwrap_or(""),
            PcRow::RemovedAudit { id, .. } => computers.get(id).map(|c| c.name.as_str()).unwrap_or(""),
        };
        name_a.to_lowercase().cmp(&name_b.to_lowercase())
    });

    if rows.is_empty() {
        ui.label(t.no_install_data);
        return;
    }

    ui.label(
        egui::RichText::new(format!("{} {} :", rows.len(), t.pcs_with_software))
            .strong()
            .size(12.5),
    );
    ui.add_space(4.0);

    egui::ScrollArea::vertical()
        .id_salt(("pcs", sw.software_id))
        .max_height(280.0)
        .show(ui, |ui| {
            egui::Grid::new(("pc_grid", sw.software_id))
                .num_columns(9)
                .spacing([10.0, 3.0])
                .striped(true)
                .show(ui, |ui| {
                    ui.label(egui::RichText::new(t.col_pc_state).strong().size(11.0));
                    ui.label(egui::RichText::new(t.col_pc_name).strong().size(11.0));
                    ui.label(egui::RichText::new(t.col_user_contact).strong().size(11.0));
                    ui.label(egui::RichText::new(t.col_installed_version).strong().size(11.0));
                    ui.label(egui::RichText::new(t.col_agent_version).strong().size(11.0));
                    ui.label(egui::RichText::new(t.col_agent_contact).strong().size(11.0));
                    ui.label(egui::RichText::new(t.col_agent_status).strong().size(11.0));
                    ui.label(egui::RichText::new(t.col_agent_ping).strong().size(11.0));
                    ui.label("");
                    ui.end_row();

                    for row in rows {
                        match row {
                            PcRow::Installed { id } => {
                                let (pc_name, contact) = match computers.get(&id) {
                                    Some(info) => (info.name.as_str(), info.contact.as_str()),
                                    None => (t.unknown, ""),
                                };
                                let ver_s = versions_for_host(sw, id);
                                ui.label(
                                    egui::RichText::new(t.pc_state_installed)
                                        .size(11.0)
                                        .color(egui::Color32::from_rgb(0, 150, 0)),
                                );
                                ui.label(
                                    egui::RichText::new(pc_name)
                                        .size(11.0)
                                        .color(egui::Color32::from_rgb(160, 195, 235)),
                                );
                                ui.label(
                                    egui::RichText::new(if contact.is_empty() {
                                        "—"
                                    } else {
                                        contact
                                    })
                                    .size(11.0)
                                    .color(egui::Color32::from_rgb(180, 180, 180)),
                                );
                                ui.label(egui::RichText::new(ver_s).size(11.0));
                                if let Some(agent) = agent_by_id.get(&id) {
                                    ui.label(
                                        egui::RichText::new(if agent.version.is_empty() {
                                            "—"
                                        } else {
                                            &agent.version
                                        })
                                        .size(11.0),
                                    );
                                    ui.label(
                                        egui::RichText::new(
                                            agent.last_contact.as_deref().unwrap_or("—"),
                                        )
                                        .size(11.0),
                                    );
                                    let (st, col) = agent_status_presentation(agent, t);
                                    ui.colored_label(col, egui::RichText::new(st).size(11.0));
                                    let (pt, pc) = ping_presentation(agent, t);
                                    ui.colored_label(pc, egui::RichText::new(pt).size(11.0));
                                    if ui
                                        .add_enabled(
                                            !state.ping_in_progress,
                                            egui::Button::new("Ping"),
                                        )
                                        .clicked()
                                    {
                                        state.request_ping_single(id);
                                    }
                                } else {
                                    ui.label("—");
                                    ui.label("—");
                                    ui.label("—");
                                    ui.label("—");
                                    ui.label("");
                                }
                                ui.end_row();
                            }
                            PcRow::RemovedAudit { id, removed_at } => {
                                let (pc_name, contact) = match computers.get(&id) {
                                    Some(info) => (info.name.as_str(), info.contact.as_str()),
                                    None => (t.unknown, ""),
                                };
                                ui.label(
                                    egui::RichText::new(t.pc_state_removed_audit)
                                        .size(11.0)
                                        .color(deleted_color),
                                );
                                ui.label(
                                    egui::RichText::new(pc_name)
                                        .size(11.0)
                                        .color(deleted_color),
                                );
                                ui.label(
                                    egui::RichText::new(if contact.is_empty() {
                                        "—"
                                    } else {
                                        contact
                                    })
                                    .size(11.0)
                                    .color(deleted_color),
                                );
                                ui.label(
                                    egui::RichText::new(removed_at.get(..10).unwrap_or(&removed_at))
                                        .size(11.0)
                                        .color(deleted_color),
                                );
                                if let Some(agent) = agent_by_id.get(&id) {
                                    ui.label(
                                        egui::RichText::new(if agent.version.is_empty() {
                                            "—"
                                        } else {
                                            &agent.version
                                        })
                                        .size(11.0),
                                    );
                                    ui.label(
                                        egui::RichText::new(
                                            agent.last_contact.as_deref().unwrap_or("—"),
                                        )
                                        .size(11.0),
                                    );
                                    let (st, col) = agent_status_presentation(agent, t);
                                    ui.colored_label(col, egui::RichText::new(st).size(11.0));
                                    let (pt, pc) = ping_presentation(agent, t);
                                    ui.colored_label(pc, egui::RichText::new(pt).size(11.0));
                                    if ui
                                        .add_enabled(
                                            !state.ping_in_progress,
                                            egui::Button::new("Ping"),
                                        )
                                        .clicked()
                                    {
                                        state.request_ping_single(id);
                                    }
                                } else {
                                    ui.label("—");
                                    ui.label("—");
                                    ui.label("—");
                                    ui.label("—");
                                    ui.label("");
                                }
                                ui.end_row();
                            }
                        }
                    }
                });
        });
}
