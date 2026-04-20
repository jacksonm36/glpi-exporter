use crate::app::{AgentStatusFilter, AppState};
use crate::date_util;
use crate::i18n::T;
use crate::models::{AgentInfo, AgentStatus, AggregatedSoftware, ComputerInfo, PingResult};
use eframe::egui;
use std::collections::HashMap;

/// `true` if last_contact is within the last `max_days` (inclusive), or missing/invalid counts as not fresh.
pub(crate) fn agent_has_recent_contact(agent: &AgentInfo, max_days: i64) -> bool {
    let Some(ref raw) = agent.last_contact else {
        return false;
    };
    let s = raw.trim();
    if s.is_empty() {
        return false;
    }
    let Some(dt) = date_util::parse_datetime(s) else {
        return false;
    };
    let now = chrono::Local::now().naive_local();
    now.signed_duration_since(dt) <= chrono::Duration::days(max_days)
}

/// PC Software panel PC list: keep a row unless the agent `last_contact` **parses** and is older
/// than `max_days` **and** the computer `last_inventory` date is also not within that window.
///
/// Missing / empty / unparseable `last_contact` does **not** hide the PC (cannot prove stale).
pub(crate) fn computer_passes_pc_software_stale_filter(
    computer: Option<&ComputerInfo>,
    agent: Option<&AgentInfo>,
    max_days: i64,
) -> bool {
    let Some(agent) = agent else {
        return true;
    };

    let now_dt = chrono::Local::now().naive_local();
    let now_date = now_dt.date();

    let agent_definitively_stale = match &agent.last_contact {
        Some(raw) if !raw.trim().is_empty() => {
            match date_util::parse_datetime(raw.trim()) {
                Some(dt) => now_dt.signed_duration_since(dt) > chrono::Duration::days(max_days),
                None => false,
            }
        }
        _ => false,
    };

    if !agent_definitively_stale {
        return true;
    }

    computer
        .and_then(|c| date_util::parse_date(c.last_inventory.trim()))
        .map(|d| {
            if d > now_date {
                return true;
            }
            (now_date - d).num_days() <= max_days
        })
        .unwrap_or(false)
}

pub fn show(ctx: &egui::Context, state: &mut AppState) {
    if !state.show_agent_panel {
        return;
    }

    let t = state.t();
    let mut open = state.show_agent_panel;

    egui::Window::new(t.agent_panel_title)
        .id(egui::Id::new("agent_panel_window"))
        .default_size([700.0, 450.0])
        .resizable(true)
        .collapsible(true)
        .open(&mut open)
        .show(ctx, |ui| {
            let t = state.t();

            ui.horizontal(|ui| {
                ui.label(t.license_search_label);
                ui.add(
                    egui::TextEdit::singleline(&mut state.agent_filter)
                        .desired_width(200.0)
                        .hint_text(t.search_hint),
                );

                ui.add_space(12.0);
                if ui
                    .selectable_label(
                        state.agent_status_filter == AgentStatusFilter::All,
                        t.agent_filter_all,
                    )
                    .clicked()
                {
                    state.agent_status_filter = AgentStatusFilter::All;
                }
                if ui
                    .selectable_label(
                        state.agent_status_filter == AgentStatusFilter::Online,
                        t.agent_filter_online,
                    )
                    .clicked()
                {
                    state.agent_status_filter = AgentStatusFilter::Online;
                }
                if ui
                    .selectable_label(
                        state.agent_status_filter == AgentStatusFilter::Offline,
                        t.agent_filter_offline,
                    )
                    .clicked()
                {
                    state.agent_status_filter = AgentStatusFilter::Offline;
                }
            });

            ui.horizontal(|ui| {
                if ui
                    .add_enabled(!state.ping_in_progress, egui::Button::new(t.agent_ping_all))
                    .clicked()
                {
                    state.request_ping_all();
                }

                ui.add_space(16.0);
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

            ui.separator();

            if state.agents.is_empty() {
                ui.label(t.agent_no_data);
                return;
            }

            let filter_lower = state.agent_filter.to_lowercase();
            let stale_max_days = state
                .agent_stale_max_days
                .parse::<i64>()
                .unwrap_or(60)
                .max(1);
            let filtered_agents: Vec<usize> = state
                .agents
                .iter()
                .enumerate()
                .filter(|(_, a)| {
                    if state.agent_hide_stale_no_contact
                        && !agent_has_recent_contact(a, stale_max_days)
                    {
                        return false;
                    }
                    if !filter_lower.is_empty()
                        && !a.computer_name.to_lowercase().contains(&filter_lower)
                        && !a.agent_name.to_lowercase().contains(&filter_lower)
                    {
                        return false;
                    }
                    match state.agent_status_filter {
                        AgentStatusFilter::All => true,
                        AgentStatusFilter::Online => {
                            a.status == AgentStatus::Online || a.ping == PingResult::Reachable
                        }
                        AgentStatusFilter::Offline => {
                            a.status == AgentStatus::Offline || a.ping == PingResult::Unreachable
                        }
                    }
                })
                .map(|(i, _)| i)
                .collect();

            ui.label(format!("{} / {}", filtered_agents.len(), state.agents.len()));
            ui.add_space(4.0);

            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    egui::Grid::new("agent_grid")
                        .num_columns(6)
                        .striped(true)
                        .spacing([10.0, 4.0])
                        .min_col_width(60.0)
                        .show(ui, |ui| {
                            ui.strong(t.agent_col_pc_name);
                            ui.strong(t.agent_col_version);
                            ui.strong(t.agent_col_last_contact);
                            ui.strong(t.agent_col_status);
                            ui.strong(t.agent_col_ping);
                            ui.strong("");
                            ui.end_row();

                            for &idx in &filtered_agents {
                                let agent = &state.agents[idx];
                                ui.label(&agent.computer_name);
                                ui.label(if agent.version.is_empty() {
                                    "—"
                                } else {
                                    &agent.version
                                });
                                ui.label(
                                    agent
                                        .last_contact
                                        .as_deref()
                                        .unwrap_or("—"),
                                );

                                let (status_text, status_color) = match agent.status {
                                    AgentStatus::Online => {
                                        (t.agent_status_online, egui::Color32::from_rgb(0, 160, 0))
                                    }
                                    AgentStatus::Stale => {
                                        (t.agent_status_stale, egui::Color32::from_rgb(200, 160, 0))
                                    }
                                    AgentStatus::Offline => {
                                        (t.agent_status_offline, egui::Color32::from_rgb(200, 50, 50))
                                    }
                                    AgentStatus::Unknown => match agent.ping {
                                        PingResult::Reachable => {
                                            (t.agent_status_online, egui::Color32::from_rgb(0, 160, 0))
                                        }
                                        PingResult::Unreachable => {
                                            (t.agent_status_offline, egui::Color32::from_rgb(200, 50, 50))
                                        }
                                        _ => (t.agent_status_unknown, egui::Color32::GRAY),
                                    },
                                };
                                ui.colored_label(status_color, status_text);

                                let (ping_text, ping_color) = match agent.ping {
                                    PingResult::Reachable => {
                                        (t.agent_ping_ok, egui::Color32::from_rgb(0, 160, 0))
                                    }
                                    PingResult::Unreachable => {
                                        (t.agent_ping_fail, egui::Color32::from_rgb(200, 50, 50))
                                    }
                                    PingResult::Pending => {
                                        (t.agent_ping_pending, egui::Color32::GRAY)
                                    }
                                    PingResult::NotChecked => ("—", egui::Color32::GRAY),
                                };
                                ui.colored_label(ping_color, ping_text);

                                let computer_id = agent.computer_id;
                                if ui
                                    .add_enabled(
                                        !state.ping_in_progress,
                                        egui::Button::new("Ping"),
                                    )
                                    .clicked()
                                {
                                    state.request_ping_single(computer_id);
                                }
                                ui.end_row();
                            }
                        });
                });
        });

    state.show_agent_panel = open;
}

pub fn agent_status_presentation(agent: &AgentInfo, t: &T) -> (&'static str, egui::Color32) {
    match agent.status {
        AgentStatus::Online => (t.agent_status_online, egui::Color32::from_rgb(0, 160, 0)),
        AgentStatus::Stale => (t.agent_status_stale, egui::Color32::from_rgb(200, 160, 0)),
        AgentStatus::Offline => (t.agent_status_offline, egui::Color32::from_rgb(200, 50, 50)),
        AgentStatus::Unknown => match agent.ping {
            PingResult::Reachable => (t.agent_status_online, egui::Color32::from_rgb(0, 160, 0)),
            PingResult::Unreachable => (t.agent_status_offline, egui::Color32::from_rgb(200, 50, 50)),
            _ => (t.agent_status_unknown, egui::Color32::GRAY),
        },
    }
}

/// When GLPI returns multiple agent rows for one computer, keep the best candidate so we do not
/// drop a real `last_contact` in favor of a placeholder row.
fn agent_entry_preference_rank(a: &AgentInfo) -> u8 {
    match &a.last_contact {
        Some(lc) if !lc.trim().is_empty() => {
            if date_util::parse_datetime(lc.trim()).is_some() {
                4
            } else {
                2
            }
        }
        _ => {
            if !a.agent_name.trim().is_empty() {
                1
            } else {
                0
            }
        }
    }
}

/// One entry per `computer_id`, merging duplicates by [`agent_entry_preference_rank`].
pub fn agent_by_computer_id<'a>(agents: &'a [AgentInfo]) -> HashMap<u64, &'a AgentInfo> {
    let mut map: HashMap<u64, &'a AgentInfo> = HashMap::new();
    for a in agents {
        match map.get(&a.computer_id).copied() {
            None => {
                map.insert(a.computer_id, a);
            }
            Some(existing) => {
                if agent_entry_preference_rank(a) > agent_entry_preference_rank(existing) {
                    map.insert(a.computer_id, a);
                }
            }
        }
    }
    map
}

pub fn agent_info_by_computer_id_merged(agents: &[AgentInfo]) -> HashMap<u64, AgentInfo> {
    agent_by_computer_id(agents)
        .into_iter()
        .map(|(k, v)| (k, v.clone()))
        .collect()
}

pub fn ping_presentation(agent: &AgentInfo, t: &T) -> (&'static str, egui::Color32) {
    match agent.ping {
        PingResult::Reachable => (t.agent_ping_ok, egui::Color32::from_rgb(0, 160, 0)),
        PingResult::Unreachable => (t.agent_ping_fail, egui::Color32::from_rgb(200, 50, 50)),
        PingResult::Pending => (t.agent_ping_pending, egui::Color32::GRAY),
        PingResult::NotChecked => ("—", egui::Color32::GRAY),
    }
}

/// Every PC with this software must pass the same “not provably stale” rule as the PC Software panel:
/// see [`computer_passes_pc_software_stale_filter`].
pub fn software_all_hosts_have_fresh_agent(
    sw: &AggregatedSoftware,
    agent_by_id: &HashMap<u64, &AgentInfo>,
    computers: &HashMap<u64, ComputerInfo>,
    max_days: i64,
) -> bool {
    if sw.host_ids.is_empty() {
        return false;
    }
    for &hid in &sw.host_ids {
        let agent = agent_by_id.get(&hid).copied();
        let computer = computers.get(&hid);
        if !computer_passes_pc_software_stale_filter(computer, agent, max_days) {
            return false;
        }
    }
    true
}

pub fn retain_agent_fresh_all_hosts(
    rows: &mut Vec<AggregatedSoftware>,
    agents: &[AgentInfo],
    computers: &HashMap<u64, ComputerInfo>,
    max_days_str: &str,
) {
    let max_days = max_days_str.parse::<i64>().unwrap_or(60).max(1);
    if agents.is_empty() {
        rows.clear();
        return;
    }
    let map = agent_by_computer_id(agents);
    rows.retain(|sw| software_all_hosts_have_fresh_agent(sw, &map, computers, max_days));
}
