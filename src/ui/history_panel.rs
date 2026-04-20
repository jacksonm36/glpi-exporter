use crate::app::AppState;
use crate::history_query;
use crate::history_store;
use crate::models::PcLogAction;
use eframe::egui;
use std::collections::HashSet;

pub fn show_controls(ui: &mut egui::Ui, state: &mut AppState) {
    let t = state.t();

    let is_pc_log = state.history_pc_log_tab;
    let is_bulk = state.history_bulk_delete_tab && !is_pc_log;
    let is_compare = state.history_compare_mode && !is_pc_log && !is_bulk;
    let is_browse = !is_compare && !is_pc_log && !is_bulk;

    ui.horizontal(|ui| {
        ui.strong(t.history_title);
        ui.add_space(12.0);

        if ui.selectable_label(is_browse, t.history_tab_browse).clicked() {
            state.history_compare_mode = false;
            state.history_pc_log_tab = false;
            state.history_bulk_delete_tab = false;
        }
        if ui.selectable_label(is_compare, t.history_tab_compare).clicked() {
            state.history_compare_mode = true;
            state.history_pc_log_tab = false;
            state.history_bulk_delete_tab = false;
            if state.history_compare_a.is_none() {
                state.history_compare_a = state
                    .history_snapshots
                    .get(1)
                    .map(|s| s.file_name.clone());
            }
            if state.history_compare_b.is_none() {
                state.history_compare_b = state
                    .history_snapshots
                    .first()
                    .map(|s| s.file_name.clone());
            }
        }
        if ui.selectable_label(is_bulk, t.history_tab_bulk_delete).clicked() {
            state.history_bulk_delete_tab = true;
            state.history_compare_mode = false;
            state.history_pc_log_tab = false;
        }
        if ui.selectable_label(is_pc_log, t.history_tab_pc_log).clicked() {
            state.history_pc_log_tab = true;
            state.history_compare_mode = false;
            state.history_bulk_delete_tab = false;
            if state.history_pc_log_computer.is_none() {
                state.history_pc_log_computer = state.computers.keys().next().copied();
            }
        }
    });

    if is_pc_log {
        show_pc_log_controls(ui, state);
        return;
    }

    if state.history_snapshots.is_empty() {
        ui.add_space(8.0);
        ui.label(egui::RichText::new(t.history_no_snapshots_yet).strong());
        ui.label(egui::RichText::new(t.history_no_snapshots_hint).weak());
        return;
    }

    if is_bulk {
        show_bulk_delete_controls(ui, state);
        return;
    }

    if is_compare {
        show_compare_controls(ui, state);
    } else {
        show_browse_controls(ui, state);
    }
}

fn show_browse_controls(ui: &mut egui::Ui, state: &mut AppState) {
    let t = state.t();

    ui.horizontal(|ui| {
        ui.label(t.history_days_ago);
        let response = ui.add(
            egui::TextEdit::singleline(&mut state.history_days_ago)
                .desired_width(48.0)
                .hint_text("5"),
        );
        if response.changed() {
            state.history_days_ago.retain(|c| c.is_ascii_digit());
        }
        if ui.button(t.history_resolve_snapshot).clicked() {
            state.resolve_history_snapshot();
        }
    });

    ui.horizontal(|ui| {
        ui.label(t.history_snapshot_label);
        let selected_text = state
            .history_selected_snapshot
            .as_ref()
            .and_then(|selected| {
                state
                    .history_snapshots
                    .iter()
                    .find(|snap| &snap.file_name == selected)
                    .map(|snap| format_snapshot_label(snap))
            })
            .unwrap_or_else(|| t.history_no_snapshot.to_string());

        egui::ComboBox::from_id_salt("history_snapshot_combo")
            .selected_text(selected_text)
            .width(420.0)
            .show_ui(ui, |ui| {
                let snapshots = state.history_snapshots.clone();
                for snap in snapshots {
                    let label = format_snapshot_label(&snap);
                    if ui
                        .selectable_label(
                            state.history_selected_snapshot.as_deref()
                                == Some(snap.file_name.as_str()),
                            label,
                        )
                        .clicked()
                    {
                        state.history_selected_snapshot = Some(snap.file_name);
                        state.refresh_history_view();
                    }
                }
            });
    });

    show_snapshot_info_card(ui, state);

    ui.horizontal(|ui| {
        ui.label(t.software_name);
        let r = ui.add(
            egui::TextEdit::singleline(&mut state.history_software_name)
                .desired_width(180.0)
                .hint_text(t.search_hint),
        );
        let mut changed = r.changed();

        ui.add_space(10.0);
        ui.label(t.publisher);
        let r = ui.add(
            egui::TextEdit::singleline(&mut state.history_publisher)
                .desired_width(180.0)
                .hint_text(t.search_hint),
        );
        changed |= r.changed();

        ui.add_space(10.0);
        ui.label(t.history_pc_filter);
        let r = ui.add(
            egui::TextEdit::singleline(&mut state.history_pc_filter)
                .desired_width(180.0)
                .hint_text(t.search_hint),
        );
        changed |= r.changed();

        if changed {
            state.refresh_history_view();
        }
    });

    ui.horizontal(|ui| {
        ui.checkbox(&mut state.history_show_selected_only, t.show_selected_only)
            .on_hover_text(t.show_selected_only_tip);

        ui.add_space(10.0);
        ui.label(format!("{} {}", state.history_selected.len(), t.n_selected));

        ui.add_space(10.0);
        if ui
            .button(t.select_all_visible)
            .on_hover_text(t.select_all_visible_tip)
            .clicked()
        {
            for row in &state.history_rows {
                state.history_selected.insert(row.software_key.clone());
            }
        }
        if ui.button(t.deselect_all).clicked() {
            state.history_selected.clear();
            if state.history_show_selected_only {
                state.history_show_selected_only = false;
            }
        }
    });

    show_snapshot_management(ui, state);
}

fn show_compare_controls(ui: &mut egui::Ui, state: &mut AppState) {
    let t = state.t();
    let snapshots = state.history_snapshots.clone();

    ui.horizontal(|ui| {
        ui.label(t.history_snapshot_a);
        let text_a = state
            .history_compare_a
            .as_ref()
            .and_then(|f| snapshots.iter().find(|s| &s.file_name == f))
            .map(|s| s.captured_at.clone())
            .unwrap_or_else(|| "—".to_string());

        egui::ComboBox::from_id_salt("compare_snap_a")
            .selected_text(text_a)
            .width(280.0)
            .show_ui(ui, |ui| {
                for snap in &snapshots {
                    let label = format_snapshot_label(snap);
                    if ui
                        .selectable_label(
                            state.history_compare_a.as_deref() == Some(&snap.file_name),
                            label,
                        )
                        .clicked()
                    {
                        state.history_compare_a = Some(snap.file_name.clone());
                    }
                }
            });

        ui.add_space(8.0);
        ui.label(t.history_snapshot_b);
        let text_b = state
            .history_compare_b
            .as_ref()
            .and_then(|f| snapshots.iter().find(|s| &s.file_name == f))
            .map(|s| s.captured_at.clone())
            .unwrap_or_else(|| "—".to_string());

        egui::ComboBox::from_id_salt("compare_snap_b")
            .selected_text(text_b)
            .width(280.0)
            .show_ui(ui, |ui| {
                for snap in &snapshots {
                    let label = format_snapshot_label(snap);
                    if ui
                        .selectable_label(
                            state.history_compare_b.as_deref() == Some(&snap.file_name),
                            label,
                        )
                        .clicked()
                    {
                        state.history_compare_b = Some(snap.file_name.clone());
                    }
                }
            });

        if ui.button(t.history_compare).clicked() {
            run_compare(state);
        }
    });
}

fn run_compare(state: &mut AppState) {
    state.history_diff = None;
    let (Some(file_a), Some(file_b)) = (&state.history_compare_a, &state.history_compare_b) else {
        return;
    };
    let snap_a = match history_store::load_snapshot(file_a) {
        Ok(s) => s,
        Err(e) => {
            state.history_message = Some(e);
            return;
        }
    };
    let snap_b = match history_store::load_snapshot(file_b) {
        Ok(s) => s,
        Err(e) => {
            state.history_message = Some(e);
            return;
        }
    };
    state.history_diff = Some(history_query::compare_snapshots(&snap_a, &snap_b));
    state.history_message = None;
}

fn show_snapshot_info_card(ui: &mut egui::Ui, state: &AppState) {
    if let Some(ref selected) = state.history_selected_snapshot {
        if let Some(snap) = state
            .history_snapshots
            .iter()
            .find(|s| &s.file_name == selected)
        {
            egui::Frame::group(ui.style())
                .inner_margin(egui::Margin::same(6))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!("📅 {}", snap.captured_at)).strong(),
                        );
                        ui.separator();
                        ui.label(format!("🖥 {} PCs", snap.computer_count));
                        ui.separator();
                        ui.label(format!("📦 {} installs", snap.installation_count));
                        ui.separator();
                        ui.label(format_file_size(snap.file_size_bytes));
                    });
                });
        }
    }
}

fn show_snapshot_management(ui: &mut egui::Ui, state: &mut AppState) {
    let t = state.t();

    ui.horizontal(|ui| {
        ui.label(format!(
            "{}: {}",
            t.history_available_snapshots,
            state.history_snapshots.len()
        ));

        if let Some(ref confirm_file) = state.history_delete_confirm.clone() {
            ui.add_space(12.0);
            ui.colored_label(
                egui::Color32::from_rgb(200, 60, 60),
                t.history_delete_confirm,
            );
            if ui
                .button(egui::RichText::new(t.history_delete_snapshot).color(egui::Color32::RED))
                .clicked()
            {
                let f = confirm_file.clone();
                let _ = history_store::delete_snapshot(&f);
                state.history_delete_confirm = None;
                apply_removed_snapshots(state, &[f]);
            }
            if ui.button(t.cancel).clicked() {
                state.history_delete_confirm = None;
            }
        } else if state.history_selected_snapshot.is_some() {
            ui.add_space(12.0);
            if ui.button(t.history_delete_snapshot).clicked() {
                state.history_bulk_delete_pending = None;
                state.history_delete_confirm = state.history_selected_snapshot.clone();
            }
        }
    });
}

fn show_bulk_delete_controls(ui: &mut egui::Ui, state: &mut AppState) {
    let t = state.t();
    ui.label(egui::RichText::new(t.history_bulk_delete_header).strong());
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label(format!(
            "{}: {}",
            t.history_available_snapshots,
            state.history_snapshots.len()
        ));
    });
    ui.add_space(4.0);
    show_snapshot_bulk_delete_body(ui, state);
}

fn show_snapshot_bulk_delete_body(ui: &mut egui::Ui, state: &mut AppState) {
    let t = state.t();
    let snapshots = state.history_snapshots.clone();
    if let Some(pending) = state.history_bulk_delete_pending.clone() {
        ui.horizontal(|ui| {
            ui.colored_label(
                egui::Color32::from_rgb(200, 60, 60),
                t.history_bulk_confirm_prompt
                    .replace("{}", &pending.len().to_string()),
            );
        });
        ui.horizontal(|ui| {
            if ui
                .button(
                    egui::RichText::new(t.history_bulk_delete_confirm_btn)
                        .color(egui::Color32::RED),
                )
                .clicked()
            {
                match history_store::delete_snapshots(&pending) {
                    Ok(_) => {}
                    Err(e) => state.history_message = Some(e),
                }
                state.history_bulk_delete_pending = None;
                apply_removed_snapshots(state, &pending);
            }
            if ui.button(t.cancel).clicked() {
                state.history_bulk_delete_pending = None;
            }
        });
    } else {
        ui.horizontal(|ui| {
            if ui.button(t.history_bulk_select_all).clicked() {
                state.history_delete_confirm = None;
                state.history_snapshot_bulk_selected =
                    snapshots.iter().map(|s| s.file_name.clone()).collect();
            }
            if ui.button(t.history_bulk_clear_selection).clicked() {
                state.history_snapshot_bulk_selected.clear();
            }
            let n = state.history_snapshot_bulk_selected.len();
            let label = format!("{} ({})", t.history_bulk_delete_selected, n);
            if ui
                .add_enabled(n > 0, egui::Button::new(label))
                .clicked()
            {
                state.history_delete_confirm = None;
                state.history_bulk_delete_pending = Some(
                    state.history_snapshot_bulk_selected.iter().cloned().collect(),
                );
            }
        });

        egui::ScrollArea::vertical()
            .max_height(220.0)
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                for snap in &snapshots {
                    let mut checked = state
                        .history_snapshot_bulk_selected
                        .contains(&snap.file_name);
                    let label = format_snapshot_label(snap);
                    if ui.checkbox(&mut checked, label).changed() {
                        if checked {
                            state.history_delete_confirm = None;
                            state
                                .history_snapshot_bulk_selected
                                .insert(snap.file_name.clone());
                        } else {
                            state
                                .history_snapshot_bulk_selected
                                .remove(&snap.file_name);
                        }
                    }
                }
            });
    }
}

fn apply_removed_snapshots(state: &mut AppState, removed: &[String]) {
    let removed_set: HashSet<_> = removed.iter().cloned().collect();
    let compare_touched = state
        .history_compare_a
        .as_ref()
        .is_some_and(|f| removed_set.contains(f))
        || state
            .history_compare_b
            .as_ref()
            .is_some_and(|f| removed_set.contains(f));
    if compare_touched {
        if state
            .history_compare_a
            .as_ref()
            .is_some_and(|f| removed_set.contains(f))
        {
            state.history_compare_a = None;
        }
        if state
            .history_compare_b
            .as_ref()
            .is_some_and(|f| removed_set.contains(f))
        {
            state.history_compare_b = None;
        }
        state.history_diff = None;
    }
    let selected_removed = state
        .history_selected_snapshot
        .as_ref()
        .is_some_and(|f| removed_set.contains(f));
    if selected_removed {
        state.history_selected_snapshot = None;
        state.history_rows.clear();
        state.history_summary = None;
    }
    state.reload_history_snapshots();
    state.history_snapshot_bulk_selected.clear();
    if selected_removed && state.history_mode {
        state.resolve_history_snapshot();
    }
}

pub fn show_results(ui: &mut egui::Ui, state: &mut AppState) {
    if state.history_pc_log_tab {
        show_pc_log_results(ui, state);
    } else if state.history_bulk_delete_tab {
        if let Some(message) = &state.history_message {
            ui.label(message);
        }
    } else if state.history_compare_mode {
        show_compare_results(ui, state);
    } else {
        show_browse_results(ui, state);
    }
}

fn show_browse_results(ui: &mut egui::Ui, state: &mut AppState) {
    let t = state.t();

    if let Some(message) = &state.history_message {
        ui.label(message);
        ui.add_space(8.0);
    }

    if state.history_rows.is_empty() {
        return;
    }

    let visible_rows: Vec<usize> = state
        .history_rows
        .iter()
        .enumerate()
        .filter(|(_, row)| {
            if state.history_show_selected_only {
                state.history_selected.contains(&row.software_key)
            } else {
                true
            }
        })
        .map(|(i, _)| i)
        .collect();

    if visible_rows.is_empty() {
        ui.label(t.history_no_matches);
        return;
    }

    egui::ScrollArea::both()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            egui::Grid::new("history_grid")
                .num_columns(6)
                .spacing([8.0, 2.0])
                .striped(true)
                .min_col_width(0.0)
                .show(ui, |ui| {
                    let all_visible_selected = visible_rows.iter().all(|&i| {
                        state
                            .history_selected
                            .contains(&state.history_rows[i].software_key)
                    });
                    let mut header_checked = all_visible_selected && !visible_rows.is_empty();
                    if ui.checkbox(&mut header_checked, "").clicked() {
                        if header_checked {
                            for &i in &visible_rows {
                                state
                                    .history_selected
                                    .insert(state.history_rows[i].software_key.clone());
                            }
                        } else {
                            for &i in &visible_rows {
                                state
                                    .history_selected
                                    .remove(&state.history_rows[i].software_key);
                            }
                        }
                    }
                    ui.strong(t.col_rank);
                    ui.strong(t.col_software_name);
                    ui.strong(t.col_publisher);
                    ui.strong(t.col_hosts);
                    ui.strong(t.history_versions);
                    ui.end_row();

                    for (display_rank, &row_idx) in visible_rows.iter().enumerate() {
                        let row = &state.history_rows[row_idx];
                        let sw_key = row.software_key.clone();
                        let is_expanded = state.history_expanded.contains(&sw_key);

                        let mut is_checked =
                            state.history_selected.contains(&sw_key);
                        if ui.checkbox(&mut is_checked, "").clicked() {
                            if is_checked {
                                state.history_selected.insert(sw_key.clone());
                            } else {
                                state.history_selected.remove(&sw_key);
                            }
                        }

                        let toggle_text = if is_expanded { "▼" } else { "▶" };
                        let rank_label =
                            format!("{} {}", display_rank + 1, toggle_text);
                        let resp = ui.add(
                            egui::Label::new(egui::RichText::new(&rank_label))
                                .sense(egui::Sense::click()),
                        );
                        if resp.clicked() {
                            if is_expanded {
                                state.history_expanded.remove(&sw_key);
                            } else {
                                state.history_expanded.insert(sw_key.clone());
                            }
                        }

                        let name_resp = ui.add(
                            egui::Label::new(
                                egui::RichText::new(&row.software_name).strong(),
                            )
                            .sense(egui::Sense::click()),
                        );
                        if name_resp.clicked() {
                            if is_expanded {
                                state.history_expanded.remove(&sw_key);
                            } else {
                                state.history_expanded.insert(sw_key.clone());
                            }
                        }

                        ui.label(&row.publisher);
                        ui.label(
                            egui::RichText::new(row.host_count.to_string()).strong(),
                        );
                        ui.label(row.versions.join(", "));
                        ui.end_row();

                        if is_expanded {
                            let row = &state.history_rows[row_idx];
                            let versions = row.versions.clone();
                            let computers = row.computers.clone();
                            let host_count = row.host_count;

                            ui.label("");
                            ui.label("");
                            ui.vertical(|ui| {
                                let active_tab = state
                                    .history_detail_tabs
                                    .entry(sw_key.clone())
                                    .or_insert(0);

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
                                                versions.len()
                                            );
                                            let pc_label = format!(
                                                "{} ({})",
                                                t.pcs_tab, host_count
                                            );

                                            if tab_button(ui, &ver_label, *active_tab == 0) {
                                                *active_tab = 0;
                                            }
                                            ui.add_space(4.0);
                                            if tab_button(ui, &pc_label, *active_tab == 1) {
                                                *active_tab = 1;
                                            }
                                        });

                                        ui.add_space(4.0);
                                        ui.separator();
                                        ui.add_space(4.0);

                                        match *active_tab {
                                            0 => {
                                                render_history_versions_tab(
                                                    ui, &versions, &sw_key, t,
                                                );
                                            }
                                            1 => {
                                                render_history_pcs_tab(
                                                    ui, &computers, &sw_key, t,
                                                );
                                            }
                                            _ => {}
                                        }
                                    });
                            });
                            ui.label("");
                            ui.label("");
                            ui.label("");
                            ui.end_row();
                        }
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

fn render_history_versions_tab(
    ui: &mut egui::Ui,
    versions: &[String],
    id_salt: &str,
    t: &crate::i18n::T,
) {
    if versions.is_empty() {
        ui.label(t.no_version_data);
        return;
    }

    egui::ScrollArea::vertical()
        .id_salt(("hist_ver", id_salt))
        .max_height(280.0)
        .show(ui, |ui| {
            egui::Grid::new(("hist_ver_grid", id_salt))
                .num_columns(1)
                .spacing([12.0, 3.0])
                .striped(true)
                .show(ui, |ui| {
                    ui.label(egui::RichText::new(t.col_version).strong().size(12.0));
                    ui.end_row();

                    for ver in versions {
                        ui.label(egui::RichText::new(ver).size(12.0));
                        ui.end_row();
                    }
                });
        });
}

fn render_history_pcs_tab(
    ui: &mut egui::Ui,
    computers: &[crate::models::HistoricalComputerEntry],
    id_salt: &str,
    t: &crate::i18n::T,
) {
    if computers.is_empty() {
        ui.label(t.no_install_data);
        return;
    }

    ui.label(
        egui::RichText::new(format!("{} {} :", computers.len(), t.pcs_with_software))
            .strong()
            .size(12.5),
    );
    ui.add_space(4.0);

    egui::ScrollArea::vertical()
        .id_salt(("hist_pcs", id_salt))
        .max_height(280.0)
        .show(ui, |ui| {
            egui::Grid::new(("hist_pc_grid", id_salt))
                .num_columns(4)
                .spacing([20.0, 3.0])
                .striped(true)
                .show(ui, |ui| {
                    ui.label(egui::RichText::new(t.col_pc_name).strong().size(12.0));
                    ui.label(
                        egui::RichText::new(t.col_user_contact).strong().size(12.0),
                    );
                    ui.label(egui::RichText::new("SN").strong().size(12.0));
                    ui.label(
                        egui::RichText::new(t.history_versions).strong().size(12.0),
                    );
                    ui.end_row();

                    for comp in computers {
                        ui.label(
                            egui::RichText::new(&comp.computer_name)
                                .size(12.0)
                                .color(egui::Color32::from_rgb(160, 195, 235)),
                        );
                        ui.label(
                            egui::RichText::new(if comp.contact.is_empty() {
                                "—"
                            } else {
                                &comp.contact
                            })
                            .size(12.0)
                            .color(egui::Color32::from_rgb(180, 180, 180)),
                        );
                        ui.label(
                            egui::RichText::new(if comp.serial_number.is_empty() {
                                "—"
                            } else {
                                &comp.serial_number
                            })
                            .size(12.0)
                            .color(egui::Color32::from_rgb(180, 180, 180)),
                        );
                        ui.label(
                            egui::RichText::new(if comp.versions.is_empty() {
                                "—".to_string()
                            } else {
                                comp.versions.join(", ")
                            })
                            .size(12.0),
                        );
                        ui.end_row();
                    }
                });
        });
}

fn show_pc_log_controls(ui: &mut egui::Ui, state: &mut AppState) {
    let t = state.t();

    if state.computers.is_empty() {
        ui.add_space(8.0);
        ui.label(egui::RichText::new(t.no_data_msg).weak());
        return;
    }

    let mut sorted_computers: Vec<(u64, String)> = state
        .computers
        .iter()
        .map(|(&id, info)| (id, info.name.clone()))
        .collect();
    sorted_computers.sort_by(|a, b| a.1.to_lowercase().cmp(&b.1.to_lowercase()));

    ui.horizontal(|ui| {
        ui.label(t.history_pc_log_select);

        let selected_name = state
            .history_pc_log_computer
            .and_then(|id| state.computers.get(&id))
            .map(|c| c.name.clone())
            .unwrap_or_else(|| "—".to_string());

        egui::ComboBox::from_id_salt("pc_log_computer_combo")
            .selected_text(&selected_name)
            .width(300.0)
            .show_ui(ui, |ui| {
                for (id, name) in &sorted_computers {
                    if ui
                        .selectable_label(
                            state.history_pc_log_computer == Some(*id),
                            name,
                        )
                        .clicked()
                    {
                        state.history_pc_log_computer = Some(*id);
                        state.history_pc_log_entries.clear();
                        state.history_pc_log_fetched = false;
                        state.history_pc_log_error = None;
                    }
                }
            });

        ui.add_space(12.0);
        ui.label(t.history_pc_log_days);
        let r = ui.add(
            egui::TextEdit::singleline(&mut state.history_pc_log_days)
                .desired_width(48.0)
                .hint_text("30"),
        );
        if r.changed() {
            state.history_pc_log_days.retain(|c| c.is_ascii_digit());
        }

        ui.add_space(12.0);
        let can_fetch =
            state.history_pc_log_computer.is_some() && !state.history_pc_log_loading;
        if ui
            .add_enabled(can_fetch, egui::Button::new(t.history_pc_log_fetch))
            .clicked()
        {
            state.request_pc_log();
        }
    });

    if state.history_pc_log_loading {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(t.history_pc_log_loading);
        });
    }

    if let Some(ref err) = state.history_pc_log_error {
        ui.add_space(4.0);
        ui.colored_label(egui::Color32::RED, format!("{}: {err}", t.status_error));
    }
}

fn show_pc_log_results(ui: &mut egui::Ui, state: &mut AppState) {
    let t = state.t();

    if state.history_pc_log_loading {
        return;
    }

    if state.history_pc_log_entries.is_empty() {
        if state.history_pc_log_fetched && state.history_pc_log_error.is_none() {
            ui.label(t.history_pc_log_empty);
        }
        return;
    }

    let green = egui::Color32::from_rgb(0, 160, 0);
    let yellow = egui::Color32::from_rgb(200, 160, 0);
    let red = egui::Color32::from_rgb(200, 50, 50);

    ui.label(
        egui::RichText::new(format!(
            "{} {}",
            state.history_pc_log_entries.len(),
            t.history_pc_log_col_action
        ))
        .strong(),
    );
    ui.add_space(4.0);

    egui::ScrollArea::both()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            egui::Grid::new("pc_log_grid")
                .num_columns(4)
                .spacing([12.0, 3.0])
                .striped(true)
                .min_col_width(60.0)
                .show(ui, |ui| {
                    ui.strong(t.history_pc_log_col_date);
                    ui.strong(t.history_pc_log_col_action);
                    ui.strong(t.history_pc_log_col_software);
                    ui.strong(t.history_pc_log_col_details);
                    ui.end_row();

                    for entry in &state.history_pc_log_entries {
                        ui.label(&entry.date);

                        let (action_label, action_color) = match entry.action {
                            PcLogAction::Installed => (t.history_pc_log_installed, green),
                            PcLogAction::Updated => (t.history_pc_log_updated, yellow),
                            PcLogAction::Removed => (t.history_pc_log_removed, red),
                        };
                        ui.colored_label(action_color, action_label);
                        ui.label(&entry.software_name);

                        let details = match entry.action {
                            PcLogAction::Updated => {
                                format!("{} → {}", &entry.old_value, &entry.new_value)
                            }
                            PcLogAction::Installed => entry.new_value.clone(),
                            PcLogAction::Removed => entry.old_value.clone(),
                        };
                        ui.label(
                            egui::RichText::new(&details)
                                .weak()
                                .size(12.0),
                        );
                        ui.end_row();
                    }
                });
        });
}

fn show_compare_results(ui: &mut egui::Ui, state: &mut AppState) {
    let t = state.t();

    if let Some(ref msg) = state.history_message {
        ui.label(msg);
    }

    let Some(ref diff) = state.history_diff else {
        ui.label(t.history_compare);
        return;
    };

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let green = egui::Color32::from_rgb(0, 160, 0);
            let red = egui::Color32::from_rgb(200, 50, 50);
            let yellow = egui::Color32::from_rgb(200, 160, 0);

            if !diff.added.is_empty() {
                ui.colored_label(
                    green,
                    egui::RichText::new(format!(
                        "▸ {} ({})",
                        t.history_diff_added,
                        diff.added.len()
                    ))
                    .strong(),
                );
                show_diff_table(ui, &diff.added, green, t);
                ui.add_space(8.0);
            }

            if !diff.removed.is_empty() {
                ui.colored_label(
                    red,
                    egui::RichText::new(format!(
                        "▸ {} ({})",
                        t.history_diff_removed,
                        diff.removed.len()
                    ))
                    .strong(),
                );
                show_diff_table(ui, &diff.removed, red, t);
                ui.add_space(8.0);
            }

            if !diff.changed.is_empty() {
                ui.colored_label(
                    yellow,
                    egui::RichText::new(format!(
                        "▸ {} ({})",
                        t.history_diff_changed,
                        diff.changed.len()
                    ))
                    .strong(),
                );
                show_diff_table(ui, &diff.changed, yellow, t);
            }

            if diff.added.is_empty() && diff.removed.is_empty() && diff.changed.is_empty() {
                ui.label(t.history_no_matches);
            }
        });
}

fn show_diff_table(
    ui: &mut egui::Ui,
    entries: &[crate::models::DiffEntry],
    color: egui::Color32,
    t: &crate::i18n::T,
) {
    egui::Grid::new(ui.next_auto_id())
        .num_columns(5)
        .striped(true)
        .spacing([8.0, 3.0])
        .show(ui, |ui| {
            ui.strong(t.col_software_name);
            ui.strong(t.col_publisher);
            ui.strong("Hosts A");
            ui.strong("Hosts B");
            ui.strong(t.history_host_delta);
            ui.end_row();

            for entry in entries {
                ui.colored_label(color, &entry.software_name);
                ui.label(&entry.publisher);
                ui.label(entry.hosts_a.to_string());
                ui.label(entry.hosts_b.to_string());
                let delta = entry.hosts_b as isize - entry.hosts_a as isize;
                let delta_str = if delta > 0 {
                    format!("+{delta}")
                } else {
                    format!("{delta}")
                };
                ui.label(delta_str);
                ui.end_row();
            }
        });
}

fn format_snapshot_label(snap: &crate::models::SnapshotSummary) -> String {
    format!(
        "{} | {} PCs | {} installs | {}",
        snap.captured_at,
        snap.computer_count,
        snap.installation_count,
        format_file_size(snap.file_size_bytes),
    )
}

fn format_file_size(bytes: u64) -> String {
    if bytes >= 1_048_576 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} B")
    }
}
