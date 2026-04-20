use crate::app::AppState;
use crate::date_util;
use crate::history_query;
use crate::history_store;
use crate::models::{
    AggregatedSoftware, InventorySnapshot, PcLogAction, PcSoftwareHistCache, PcSoftwareLogEntry,
    SnapshotSummary,
};
use crate::ui::agent_panel::computer_passes_pc_software_stale_filter;
use eframe::egui;
use egui_extras::{Column, TableBuilder};
use std::collections::{BTreeSet, HashMap};

struct PcSoftwareRow {
    name: String,
    publisher: String,
    version: String,
    last_install: String,
    last_updated: String,
    last_agent_pull: String,
    is_deleted: bool,
    deleted_date: String,
}

fn is_windows_component(name: &str, publisher: &str) -> bool {
    let pub_lower = publisher.to_lowercase();
    let name_lower = name.to_lowercase();

    if name.len() >= 30 && name.chars().all(|c| c.is_ascii_hexdigit() || c == '-') {
        return true;
    }

    let is_ms = pub_lower.contains("microsoft");
    if !is_ms {
        return false;
    }

    let keep_patterns = [
        "office", "visual studio", "sql server", "teams", ".net framework",
        "visual c++", "powershell", "azure", "onedrive", "edge",
        "xbox", "skype", "onenote", "outlook", "excel", "word",
        "powerpoint", "access", "publisher",
    ];
    if keep_patterns.iter().any(|p| name_lower.contains(p)) {
        return false;
    }

    let hide_patterns = [
        "windows", "microsoft.", "alkalmazásfeloldó",
        "asynctextservice", "asztali alkalmazás",
        "a microsoft", "a windows",
    ];
    if hide_patterns.iter().any(|p| name_lower.contains(p)) {
        return true;
    }

    if pub_lower.contains("platform extensions") {
        return true;
    }

    false
}

fn is_kb_update(name: &str) -> bool {
    let n = name.trim();
    // Match patterns like "KB1234567", "Update for KB...", "Security Update KB..."
    // Also match "Hotfix for ..." and "(KB1234567)"
    let lower = n.to_lowercase();
    if lower.len() > 2
        && lower.starts_with("kb")
        && lower[2..]
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_digit())
    {
        return true;
    }
    if lower.contains("(kb") && lower.contains(')') {
        return true;
    }
    if (lower.contains("update for") || lower.contains("hotfix for") || lower.contains("security update"))
        && lower.contains("kb")
    {
        return true;
    }
    false
}

fn gather_software_for_pc(
    all_data: &[AggregatedSoftware],
    computer_id: u64,
    filter: &str,
    hide_windows: bool,
    hide_kb: bool,
    time_filter_cutoff: Option<chrono::NaiveDate>,
    fallback_agent_date: &str,
) -> Vec<PcSoftwareRow> {
    let filter_lower = filter.trim().to_lowercase();

    let mut rows: Vec<PcSoftwareRow> = all_data
        .iter()
        .filter(|sw| sw.host_ids.contains(&computer_id))
        .filter(|sw| {
            if hide_windows && is_windows_component(&sw.name, &sw.publisher) {
                return false;
            }
            if hide_kb && is_kb_update(&sw.name) {
                return false;
            }
            if filter_lower.is_empty() {
                return true;
            }
            sw.name_lower.contains(&filter_lower) || sw.publisher_lower.contains(&filter_lower)
        })
        .filter_map(|sw| {
            let version = sw
                .versions
                .iter()
                .filter(|v| v.host_ids.contains(&computer_id))
                .map(|v| v.version_name.clone())
                .collect::<Vec<_>>()
                .join(", ");

            let last_install = sw
                .versions
                .iter()
                .filter(|v| v.host_ids.contains(&computer_id))
                .filter_map(|v| v.last_install_date.clone())
                .max()
                .unwrap_or_default();

            let last_updated_str = sw.last_updated.clone().unwrap_or_default();

            // Time filter: check if any relevant date falls within the cutoff
            if let Some(cutoff) = time_filter_cutoff {
                let dominated = [&last_install, &last_updated_str];
                let any_recent = dominated.iter().any(|d| {
                    if d.is_empty() {
                        return false;
                    }
                    date_util::parse_date(d).map_or(false, |nd| nd >= cutoff)
                });
                if !any_recent {
                    return None;
                }
            }

            let agent_pull = sw
                .last_agent_pull
                .as_deref()
                .filter(|s| !s.is_empty())
                .unwrap_or(fallback_agent_date)
                .to_string();

            Some(PcSoftwareRow {
                name: sw.name.clone(),
                publisher: sw.publisher.clone(),
                version: if version.is_empty() {
                    sw.latest_version.clone()
                } else {
                    version
                },
                last_install,
                last_updated: last_updated_str,
                last_agent_pull: agent_pull,
                is_deleted: false,
                deleted_date: String::new(),
            })
        })
        .collect();

    rows.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    rows
}

fn snapshot_head_fingerprint(snapshots: &[SnapshotSummary]) -> String {
    snapshots
        .first()
        .map(|s| s.file_name.clone())
        .unwrap_or_default()
}

fn refresh_pc_software_hist_cache(state: &mut AppState) {
    if !state.pc_software_hist_snapshot {
        state.pc_software_hist_cache = None;
        state.pc_software_hist_cache_key = None;
        state.pc_software_hist_cache_dirty = false;
        return;
    }

    let fp = snapshot_head_fingerprint(&state.history_snapshots);
    let key = (
        state.pc_software_hist_from.clone(),
        state.pc_software_hist_to.clone(),
        state.history_snapshots.len(),
        fp,
    );

    if !state.pc_software_hist_cache_dirty {
        if let Some(ref k) = state.pc_software_hist_cache_key {
            if k == &key && state.pc_software_hist_cache.is_some() {
                return;
            }
        }
    }

    state.pc_software_hist_cache_dirty = false;
    state.pc_software_hist_cache_key = Some(key);

    let today = chrono::Local::now().naive_local().date();
    match resolve_hist_date_range(
        &state.pc_software_hist_from,
        &state.pc_software_hist_to,
        today,
    ) {
        Err(()) => {
            state.pc_software_hist_cache = Some(PcSoftwareHistCache::InvalidDates);
        }
        Ok((from_d, to_d)) => {
            if let Some(sum) = history_query::pick_snapshot_in_date_range(
                &state.history_snapshots,
                from_d,
                to_d,
            ) {
                match history_store::load_snapshot(&sum.file_name) {
                    Ok(inv) => {
                        state.pc_software_hist_cache = Some(PcSoftwareHistCache::Ready {
                            summary: sum,
                            inventory: inv,
                        });
                    }
                    Err(_) => {
                        state.pc_software_hist_cache = Some(PcSoftwareHistCache::LoadError);
                    }
                }
            } else {
                state.pc_software_hist_cache = Some(PcSoftwareHistCache::NoSnapshotInRange);
            }
        }
    }
}

fn resolve_hist_date_range(
    from_s: &str,
    to_s: &str,
    today: chrono::NaiveDate,
) -> Result<(chrono::NaiveDate, chrono::NaiveDate), ()> {
    let from_d = if from_s.trim().is_empty() {
        today - chrono::Duration::days(120)
    } else {
        date_util::parse_date(from_s.trim()).ok_or(())?
    };
    let to_d = if to_s.trim().is_empty() {
        today - chrono::Duration::days(60)
    } else {
        date_util::parse_date(to_s.trim()).ok_or(())?
    };
    if from_d > to_d {
        return Err(());
    }
    Ok((from_d, to_d))
}

fn gather_snapshot_software_for_pc(
    snapshot: &InventorySnapshot,
    computer_id: u64,
    filter: &str,
    hide_windows: bool,
    hide_kb: bool,
) -> Vec<PcSoftwareRow> {
    let filter_lower = filter.trim().to_lowercase();
    let mut grouped: HashMap<(String, String), BTreeSet<String>> = HashMap::new();

    for inst in &snapshot.installations {
        if inst.computer_id != computer_id {
            continue;
        }
        if hide_windows && is_windows_component(&inst.software_name, &inst.publisher) {
            continue;
        }
        if hide_kb && is_kb_update(&inst.software_name) {
            continue;
        }
        if !filter_lower.is_empty()
            && !inst.software_name.to_lowercase().contains(&filter_lower)
            && !inst.publisher.to_lowercase().contains(&filter_lower)
        {
            continue;
        }
        let publisher = if inst.publisher.trim().is_empty() {
            "Unknown".to_string()
        } else {
            inst.publisher.clone()
        };
        let key = (inst.software_name.clone(), publisher);
        let entry = grouped.entry(key).or_default();
        if !inst.version_name.trim().is_empty() {
            entry.insert(inst.version_name.clone());
        }
    }

    let mut rows: Vec<PcSoftwareRow> = grouped
        .into_iter()
        .map(|((name, publisher), vers)| {
            let version = vers.into_iter().collect::<Vec<_>>().join(", ");
            PcSoftwareRow {
                name,
                publisher,
                version,
                last_install: String::new(),
                last_updated: String::new(),
                last_agent_pull: String::new(),
                is_deleted: false,
                deleted_date: String::new(),
            }
        })
        .collect();
    rows.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    rows
}

/// Build rows from PC log entries that represent deleted software not currently installed.
fn gather_deleted_rows(
    log_entries: &[PcSoftwareLogEntry],
    current_names: &std::collections::HashSet<String>,
    filter: &str,
    hide_windows: bool,
    hide_kb: bool,
    time_filter_cutoff: Option<chrono::NaiveDate>,
) -> Vec<PcSoftwareRow> {
    let filter_lower = filter.trim().to_lowercase();
    let mut seen = std::collections::HashSet::new();
    let mut deleted: Vec<PcSoftwareRow> = Vec::new();

    for entry in log_entries {
        if entry.action != PcLogAction::Removed {
            continue;
        }
        let name = entry.software_name.trim();
        if name.is_empty() {
            continue;
        }
        let name_lower = name.to_lowercase();
        if current_names.contains(&name_lower) {
            continue;
        }
        if !seen.insert(name_lower.clone()) {
            continue;
        }
        if hide_windows && is_windows_component(name, "") {
            continue;
        }
        if hide_kb && is_kb_update(name) {
            continue;
        }
        if !filter_lower.is_empty() && !name_lower.contains(&filter_lower) {
            continue;
        }
        if let Some(cutoff) = time_filter_cutoff {
            if let Some(nd) = date_util::parse_date(&entry.date) {
                if nd < cutoff {
                    continue;
                }
            }
        }

        // Extract version from old_value if present (format: "SoftwareName (version)")
        let version = entry
            .old_value
            .rfind(" (")
            .map(|i| {
                let s = &entry.old_value[i + 2..];
                s.trim_end_matches(')').to_string()
            })
            .unwrap_or_default();

        deleted.push(PcSoftwareRow {
            name: name.to_string(),
            publisher: String::new(),
            version,
            last_install: String::new(),
            last_updated: String::new(),
            last_agent_pull: String::new(),
            is_deleted: true,
            deleted_date: entry.date.get(..10).unwrap_or(&entry.date).to_string(),
        });
    }

    deleted.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    deleted
}

pub fn show(ctx: &egui::Context, state: &mut AppState) {
    if !state.show_pc_software_panel {
        return;
    }

    let t = state.t();
    let mut open = state.show_pc_software_panel;

    egui::Window::new(t.pc_software_panel_title)
        .id(egui::Id::new("pc_software_panel_window"))
        .default_size([960.0, 540.0])
        .resizable(true)
        .collapsible(true)
        .open(&mut open)
        .show(ctx, |ui| {
            let t = state.t();

            if state.computers.is_empty() {
                ui.label(t.pc_software_no_data);
                return;
            }

            let mut sorted_computers: Vec<(u64, String)> = state
                .computers
                .iter()
                .map(|(&id, info)| (id, info.name.clone()))
                .collect();
            sorted_computers.sort_by(|a, b| a.1.to_lowercase().cmp(&b.1.to_lowercase()));

            if state.agent_hide_stale_no_contact {
                let stale_max_days = state
                    .agent_stale_max_days
                    .parse::<i64>()
                    .unwrap_or(60)
                    .max(1);
                sorted_computers.retain(|(id, _)| {
                    let agent = state.agents.iter().find(|a| a.computer_id == *id);
                    let computer = state.computers.get(id);
                    computer_passes_pc_software_stale_filter(computer, agent, stale_max_days)
                });
            }

            if let Some(sel) = state.pc_software_selected {
                if !sorted_computers.iter().any(|(id, _)| *id == sel) {
                    state.pc_software_selected = sorted_computers.first().map(|(id, _)| *id);
                    state.pc_software_log_entries.clear();
                    state.pc_software_log_fetched_for = None;
                    state.pc_software_log_error = None;
                }
            }

            if state.pc_software_selected.is_none() {
                state.pc_software_selected = sorted_computers.first().map(|(id, _)| *id);
            }

            // Row 1: PC selector + text filter
            ui.horizontal(|ui| {
                ui.label(t.pc_software_select_pc);

                let selected_name = state
                    .pc_software_selected
                    .and_then(|id| state.computers.get(&id))
                    .map(|c| c.name.clone())
                    .unwrap_or_else(|| "—".to_string());

                egui::ComboBox::from_id_salt("pc_software_combo")
                    .selected_text(&selected_name)
                    .width(300.0)
                    .show_ui(ui, |ui| {
                        for (id, name) in &sorted_computers {
                            if ui
                                .selectable_label(
                                    state.pc_software_selected == Some(*id),
                                    name,
                                )
                                .clicked()
                            {
                                let prev = state.pc_software_selected;
                                state.pc_software_selected = Some(*id);
                                if prev != Some(*id) {
                                    state.pc_software_log_entries.clear();
                                    state.pc_software_log_fetched_for = None;
                                    state.pc_software_log_error = None;
                                }
                            }
                        }
                    });

                ui.add_space(12.0);
                ui.label(t.pc_software_search);
                ui.add(
                    egui::TextEdit::singleline(&mut state.pc_software_filter)
                        .desired_width(200.0)
                        .hint_text(t.search_hint),
                );
            });

            // Row 2: filter checkboxes
            ui.horizontal(|ui| {
                ui.checkbox(&mut state.pc_software_hide_windows, t.pc_software_hide_windows);
                ui.add_space(8.0);
                ui.checkbox(&mut state.pc_software_hide_kb, t.pc_software_hide_kb);
                ui.add_space(8.0);
                let show_del_prev = state.pc_software_show_deleted;
                ui.add_enabled(
                    !state.pc_software_hist_snapshot && !state.pc_software_recent30_combined,
                    egui::Checkbox::new(&mut state.pc_software_show_deleted, t.pc_software_show_deleted),
                );
                if state.pc_software_show_deleted != show_del_prev {
                    state.pc_software_log_fetched_for = None;
                    state.pc_software_log_error = None;
                }
                ui.add_space(12.0);
                ui.add_enabled_ui(!state.pc_software_recent30_combined, |ui| {
                    ui.checkbox(&mut state.pc_software_time_filter, t.pc_software_time_filter);
                    // Keep days visible so the "(days):" label does not look like a missing control.
                    ui.add_enabled_ui(state.pc_software_time_filter, |ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut state.pc_software_time_days)
                                .desired_width(48.0)
                                .hint_text("30"),
                        );
                    });
                });
                ui.add_space(16.0);
                // "Last 30d + Current" combined-view button
                let btn_active = state.pc_software_recent30_combined;
                let btn_label = egui::RichText::new(t.pc_software_recent30_btn)
                    .color(if btn_active {
                        egui::Color32::from_rgb(90, 200, 120)
                    } else {
                        ui.visuals().text_color()
                    });
                if ui
                    .button(btn_label)
                    .on_hover_text(t.pc_software_recent30_tip)
                    .clicked()
                {
                    state.pc_software_recent30_combined = !state.pc_software_recent30_combined;
                    if state.pc_software_recent30_combined {
                        // entering combined mode: disable hist snapshot, reset log cache
                        state.pc_software_hist_snapshot = false;
                        state.pc_software_log_fetched_for = None;
                        state.pc_software_log_error = None;
                    }
                }
            });

            // Row 3: hide stale agents in PC list (same setting as Agent panel)
            ui.horizontal(|ui| {
                ui.checkbox(
                    &mut state.agent_hide_stale_no_contact,
                    t.agent_hide_stale_no_contact,
                );
                if state.agent_hide_stale_no_contact {
                    ui.label(t.agent_stale_older_than_days);
                    let r = ui.add(
                        egui::TextEdit::singleline(&mut state.agent_stale_max_days)
                            .desired_width(48.0)
                            .hint_text("60"),
                    );
                    if r.changed() {
                        state.agent_stale_max_days.retain(|c| c.is_ascii_digit());
                    }
                }
            });

            if state.agent_hide_stale_no_contact
                && sorted_computers.is_empty()
                && !state.computers.is_empty()
            {
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new(t.pc_software_stale_filter_hides_all)
                        .color(egui::Color32::from_rgb(220, 160, 80)),
                );
            }

            // Row 4: historical snapshot range
            ui.horizontal(|ui| {
                let hist_prev = state.pc_software_hist_snapshot;
                ui.checkbox(&mut state.pc_software_hist_snapshot, t.pc_software_hist_snapshot_label);
                if state.pc_software_hist_snapshot && !hist_prev {
                    state.pc_software_show_deleted = false;
                    if state.pc_software_hist_from.is_empty() && state.pc_software_hist_to.is_empty() {
                        let today = chrono::Local::now().naive_local().date();
                        let to_d = today - chrono::Duration::days(60);
                        let from_d = today - chrono::Duration::days(120);
                        state.pc_software_hist_from = from_d.format("%Y-%m-%d").to_string();
                        state.pc_software_hist_to = to_d.format("%Y-%m-%d").to_string();
                    }
                }
                if !state.pc_software_hist_snapshot && hist_prev {
                    state.pc_software_show_deleted = false;
                }
                if state.pc_software_hist_snapshot {
                    ui.label(t.pc_software_hist_from_date);
                    ui.add(
                        egui::TextEdit::singleline(&mut state.pc_software_hist_from)
                            .desired_width(110.0),
                    );
                    ui.label(t.pc_software_hist_to_date);
                    ui.add(
                        egui::TextEdit::singleline(&mut state.pc_software_hist_to)
                            .desired_width(110.0),
                    );
                }
            });

            if state.pc_software_show_deleted {
                if let Some(err) = state.pc_software_log_error.clone() {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!("{}: {err}", t.status_error))
                                .color(egui::Color32::from_rgb(220, 80, 80)),
                        );
                        if ui.button(t.retry_label).clicked() {
                            state.pc_software_log_fetched_for = None;
                            state.pc_software_log_error = None;
                        }
                    });
                }
            }

            if let Some(pc_id) = state.pc_software_selected {
                refresh_pc_software_hist_cache(state);

                let hist_date_invalid = state.pc_software_hist_snapshot
                    && matches!(
                        state.pc_software_hist_cache,
                        Some(PcSoftwareHistCache::InvalidDates)
                    );
                let hist_no_snapshot_in_range = state.pc_software_hist_snapshot
                    && matches!(
                        state.pc_software_hist_cache,
                        Some(PcSoftwareHistCache::NoSnapshotInRange)
                    );
                let hist_load_error = state.pc_software_hist_snapshot
                    && matches!(
                        state.pc_software_hist_cache,
                        Some(PcSoftwareHistCache::LoadError)
                    );

                // Auto-fetch log when "show deleted" is on (live only), or when combined mode is on
                if !state.pc_software_hist_snapshot
                    && (state.pc_software_show_deleted || state.pc_software_recent30_combined)
                    && !state.pc_software_log_loading
                    && state.pc_software_log_fetched_for != Some(pc_id)
                {
                    state.request_pc_software_log();
                }

                let hist_ready: Option<(&SnapshotSummary, &InventorySnapshot)> =
                    match state.pc_software_hist_cache.as_ref() {
                        Some(PcSoftwareHistCache::Ready {
                            summary,
                            inventory,
                        }) if state.pc_software_hist_snapshot => Some((summary, inventory)),
                        _ => None,
                    };

                // Resolve agent last-contact for this PC (live header)
                let agent_last_contact: String = state
                    .agents
                    .iter()
                    .find(|a| a.computer_id == pc_id)
                    .and_then(|a| a.last_contact.clone())
                    .unwrap_or_default();

                let computer_last_inv = state
                    .computers
                    .get(&pc_id)
                    .map(|c| c.last_inventory.clone())
                    .unwrap_or_default();

                let fallback_agent_date = if !agent_last_contact.is_empty() {
                    agent_last_contact.clone()
                } else {
                    computer_last_inv.clone()
                };

                // Snapshot / history messages
                if state.pc_software_hist_snapshot {
                    ui.add_space(4.0);
                    if hist_date_invalid {
                        ui.label(
                            egui::RichText::new(t.pc_software_hist_invalid_dates)
                                .color(egui::Color32::from_rgb(220, 160, 80)),
                        );
                    } else if hist_load_error {
                        ui.label(
                            egui::RichText::new(t.pc_software_hist_load_error)
                                .color(egui::Color32::from_rgb(220, 80, 80)),
                        );
                    } else if let Some((sum, _)) = hist_ready {
                        ui.label(
                            egui::RichText::new(format!(
                                "{} {} | {} {}",
                                t.pc_software_hist_at,
                                sum.captured_at,
                                t.pc_software_hist_file,
                                sum.file_name
                            ))
                            .strong()
                            .color(egui::Color32::from_rgb(160, 200, 255)),
                        );
                    } else if hist_no_snapshot_in_range {
                        ui.label(
                            egui::RichText::new(t.pc_software_hist_no_snapshot)
                                .color(egui::Color32::from_rgb(220, 160, 80)),
                        );
                    }
                }

                // Header: snapshot PC row or live ComputerInfo
                if let Some((_, snap)) = hist_ready {
                    if let Some(sc) = snap.computers.iter().find(|c| c.id == pc_id) {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(format!("🖥 {}", sc.name))
                                    .strong()
                                    .size(13.0),
                            );
                            if !sc.contact.is_empty() {
                                ui.separator();
                                ui.label(
                                    egui::RichText::new(format!("👤 {}", sc.contact))
                                        .size(12.0)
                                        .color(egui::Color32::from_rgb(180, 180, 180)),
                                );
                            }
                            if !sc.serial_number.is_empty() {
                                ui.separator();
                                ui.label(
                                    egui::RichText::new(format!("SN: {}", sc.serial_number))
                                        .size(12.0)
                                        .color(egui::Color32::from_rgb(180, 180, 180)),
                                );
                            }
                            if !sc.model.is_empty() {
                                ui.separator();
                                ui.label(
                                    egui::RichText::new(&sc.model)
                                        .size(12.0)
                                        .color(egui::Color32::from_rgb(180, 180, 180)),
                                );
                            }
                            if !sc.last_inventory.is_empty() {
                                ui.separator();
                                ui.label(
                                    egui::RichText::new(format!(
                                        "{}: {}",
                                        t.pc_software_col_last_updated,
                                        sc.last_inventory
                                    ))
                                    .size(12.0)
                                    .color(egui::Color32::from_rgb(160, 190, 220)),
                                );
                            }
                        });
                    } else if let Some(info) = state.computers.get(&pc_id) {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(format!(
                                    "🖥 {} — {}",
                                    info.name, t.pc_software_hist_pc_missing
                                ))
                                    .weak(),
                            );
                        });
                    }
                } else if let Some(info) = state.computers.get(&pc_id) {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!("🖥 {}", info.name))
                                .strong()
                                .size(13.0),
                        );
                        if !info.contact.is_empty() {
                            ui.separator();
                            ui.label(
                                egui::RichText::new(format!("👤 {}", info.contact))
                                    .size(12.0)
                                    .color(egui::Color32::from_rgb(180, 180, 180)),
                            );
                        }
                        if !info.serial_number.is_empty() {
                            ui.separator();
                            ui.label(
                                egui::RichText::new(format!("SN: {}", info.serial_number))
                                    .size(12.0)
                                    .color(egui::Color32::from_rgb(180, 180, 180)),
                            );
                        }
                        if !info.model.is_empty() {
                            ui.separator();
                            ui.label(
                                egui::RichText::new(&info.model)
                                    .size(12.0)
                                    .color(egui::Color32::from_rgb(180, 180, 180)),
                            );
                        }
                        if !fallback_agent_date.is_empty() {
                            ui.separator();
                            ui.label(
                                egui::RichText::new(format!(
                                    "🕐 {}: {}",
                                    t.pc_software_agent_contact, fallback_agent_date
                                ))
                                .size(12.0)
                                .color(egui::Color32::from_rgb(130, 190, 130)),
                            );
                        }
                    });
                }

                // Compute time filter cutoff (live list only, not used in combined mode)
                let time_cutoff = if state.pc_software_time_filter
                    && !state.pc_software_hist_snapshot
                    && !state.pc_software_recent30_combined
                {
                    let days = state.pc_software_time_days.parse::<i64>().unwrap_or(30).max(1);
                    let now = chrono::Local::now().naive_local().date();
                    Some(now - chrono::Duration::days(days))
                } else {
                    None
                };

                let mut rows: Vec<PcSoftwareRow> = if let Some((_, snap)) = hist_ready {
                    gather_snapshot_software_for_pc(
                        snap,
                        pc_id,
                        &state.pc_software_filter,
                        state.pc_software_hide_windows,
                        state.pc_software_hide_kb,
                    )
                } else if state.pc_software_hist_snapshot {
                    Vec::new()
                } else {
                    gather_software_for_pc(
                        &state.all_data,
                        pc_id,
                        &state.pc_software_filter,
                        state.pc_software_hide_windows,
                        state.pc_software_hide_kb,
                        time_cutoff,
                        &fallback_agent_date,
                    )
                };

                // Merge deleted rows — either from "show deleted" checkbox or combined mode
                let need_deleted = !state.pc_software_hist_snapshot
                    && (state.pc_software_show_deleted || state.pc_software_recent30_combined);
                if need_deleted {
                    // In combined mode: deleted rows use a fixed 30-day cutoff regardless of
                    // other filters; in show_deleted mode: use the regular time_cutoff.
                    let deleted_cutoff = if state.pc_software_recent30_combined {
                        let now = chrono::Local::now().naive_local().date();
                        Some(now - chrono::Duration::days(30))
                    } else {
                        time_cutoff
                    };

                    if state.pc_software_log_loading {
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(t.pc_software_loading_log);
                        });
                    } else if state.pc_software_log_error.is_none()
                        && state.pc_software_log_fetched_for == Some(pc_id)
                    {
                        let current_names: std::collections::HashSet<String> = rows
                            .iter()
                            .map(|r| r.name.to_lowercase())
                            .collect();
                        let deleted_rows = gather_deleted_rows(
                            &state.pc_software_log_entries,
                            &current_names,
                            &state.pc_software_filter,
                            state.pc_software_hide_windows,
                            state.pc_software_hide_kb,
                            deleted_cutoff,
                        );
                        rows.extend(deleted_rows);
                    }
                }

                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(format!("{} {}", rows.len(), t.pc_software_count))
                        .strong(),
                );
                ui.separator();

                if rows.is_empty() {
                    ui.add_space(8.0);
                    ui.label(t.pc_software_no_matches);
                    return;
                }

                let row_count = rows.len();
                let dim = egui::Color32::from_rgb(160, 160, 160);
                let deleted_color = egui::Color32::from_rgb(220, 60, 60);

                let normal_text = ui.visuals().text_color();

                TableBuilder::new(ui)
                    .id_salt("pc_software_table")
                    .striped(true)
                    .resizable(true)
                    .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                    .column(Column::auto().at_least(28.0))
                    .column(Column::initial(200.0).at_least(80.0).resizable(true))
                    .column(Column::initial(140.0).at_least(60.0).resizable(true))
                    .column(Column::initial(110.0).at_least(50.0).resizable(true))
                    .column(Column::initial(90.0).at_least(50.0).resizable(true))
                    .column(Column::initial(90.0).at_least(50.0).resizable(true))
                    .column(Column::remainder().at_least(70.0))
                    .header(18.0, |mut header| {
                        header.col(|ui| { ui.strong("#"); });
                        header.col(|ui| { ui.strong(t.col_software_name); });
                        header.col(|ui| { ui.strong(t.col_publisher); });
                        header.col(|ui| { ui.strong(t.pc_software_col_version); });
                        header.col(|ui| { ui.strong(t.pc_software_col_install_date); });
                        header.col(|ui| { ui.strong(t.pc_software_col_last_updated); });
                        header.col(|ui| { ui.strong(t.pc_software_col_agent_run); });
                    })
                    .body(|body| {
                        body.rows(18.0, row_count, |mut row| {
                            let idx = row.index();
                            let r = &rows[idx];

                            let name_color = if r.is_deleted {
                                deleted_color
                            } else {
                                normal_text
                            };
                            let text_color = if r.is_deleted { deleted_color } else { dim };

                            row.col(|ui| { ui.label(format!("{}", idx + 1)); });
                            row.col(|ui| {
                                if r.is_deleted {
                                    ui.label(
                                        egui::RichText::new(format!("{} {}", t.pc_software_deleted_tag, &r.name))
                                            .strong()
                                            .size(12.0)
                                            .color(deleted_color),
                                    );
                                } else {
                                    ui.label(
                                        egui::RichText::new(&r.name)
                                            .strong()
                                            .size(12.0)
                                            .color(name_color),
                                    );
                                }
                            });
                            row.col(|ui| {
                                ui.label(
                                    egui::RichText::new(&r.publisher)
                                        .size(12.0)
                                        .color(if r.is_deleted {
                                            deleted_color
                                        } else {
                                            egui::Color32::from_rgb(180, 180, 180)
                                        }),
                                );
                            });
                            row.col(|ui| {
                                ui.label(
                                    egui::RichText::new(&r.version)
                                        .size(12.0)
                                        .color(text_color),
                                );
                            });
                            row.col(|ui| {
                                let txt = if r.is_deleted {
                                    &r.deleted_date
                                } else if r.last_install.is_empty() {
                                    "—"
                                } else {
                                    &r.last_install
                                };
                                ui.label(egui::RichText::new(txt).size(12.0).color(text_color));
                            });
                            row.col(|ui| {
                                ui.label(
                                    egui::RichText::new(if r.last_updated.is_empty() { "—" } else { &r.last_updated })
                                        .size(12.0).color(text_color),
                                );
                            });
                            row.col(|ui| {
                                ui.label(
                                    egui::RichText::new(if r.last_agent_pull.is_empty() { "—" } else { &r.last_agent_pull })
                                        .size(12.0).color(text_color),
                                );
                            });
                        });
                    });
            }
        });

    state.show_pc_software_panel = open;
}
