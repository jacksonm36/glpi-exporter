use crate::date_util;
use crate::models::{
    DiffEntry, HistoricalComputerEntry, HistoricalSoftwareEntry, InventorySnapshot, SnapshotDiff,
    SnapshotSummary,
};
use std::collections::{BTreeSet, HashMap, HashSet};

/// Latest snapshot whose capture **date** (day) falls in `[from_date, to_date]` inclusive.
pub fn pick_snapshot_in_date_range(
    summaries: &[SnapshotSummary],
    from_date: chrono::NaiveDate,
    to_date: chrono::NaiveDate,
) -> Option<SnapshotSummary> {
    if from_date > to_date {
        return None;
    }
    summaries
        .iter()
        .filter(|s| {
            date_util::parse_date(&s.captured_at)
                .map(|d| d >= from_date && d <= to_date)
                .unwrap_or(false)
        })
        .max_by(|a, b| a.captured_at.cmp(&b.captured_at))
        .cloned()
}

pub fn resolve_snapshot_for_days_ago(
    snapshots: &[SnapshotSummary],
    days_ago: i64,
) -> Option<SnapshotSummary> {
    let target_date = (chrono::Local::now().naive_local() - chrono::Duration::days(days_ago.max(0)))
        .date();

    snapshots
        .iter()
        .filter(|s| {
            date_util::parse_date(&s.captured_at)
                .map(|captured| captured <= target_date)
                .unwrap_or(false)
        })
        .max_by(|a, b| a.captured_at.cmp(&b.captured_at))
        .cloned()
}

pub fn build_historical_software_view(
    snapshot: &InventorySnapshot,
    software_filter: &str,
    publisher_filter: &str,
    pc_filter: &str,
) -> Vec<HistoricalSoftwareEntry> {
    let software_filter = software_filter.trim().to_lowercase();
    let publisher_filter = publisher_filter.trim().to_lowercase();
    let pc_filter = pc_filter.trim().to_lowercase();

    let computers_by_id: HashMap<u64, _> = snapshot.computers.iter().map(|c| (c.id, c)).collect();

    struct SoftwareAccum {
        name: String,
        publisher: String,
        versions: BTreeSet<String>,
        computers: HashMap<u64, HistoricalComputerEntry>,
    }

    let mut grouped: HashMap<String, SoftwareAccum> = HashMap::new();

    for inst in &snapshot.installations {
        let Some(comp) = computers_by_id.get(&inst.computer_id) else {
            continue;
        };

        if !software_filter.is_empty() && !inst.software_name.to_lowercase().contains(&software_filter)
        {
            continue;
        }
        if !publisher_filter.is_empty() && !inst.publisher.to_lowercase().contains(&publisher_filter) {
            continue;
        }
        if !pc_filter.is_empty() {
            let name_match = comp.name.to_lowercase().contains(&pc_filter);
            let contact_match = comp.contact.to_lowercase().contains(&pc_filter);
            let serial_match = comp.serial_number.to_lowercase().contains(&pc_filter);
            if !(name_match || contact_match || serial_match) {
                continue;
            }
        }

        let key = format!(
            "{}|{}",
            normalize_for_grouping(&inst.software_name),
            normalize_for_grouping(&inst.publisher)
        );

        let entry = grouped.entry(key.clone()).or_insert_with(|| SoftwareAccum {
            name: inst.software_name.clone(),
            publisher: inst.publisher.clone(),
            versions: BTreeSet::new(),
            computers: HashMap::new(),
        });

        if !inst.version_name.trim().is_empty() {
            entry.versions.insert(inst.version_name.clone());
        }

        let comp_entry = entry
            .computers
            .entry(comp.id)
            .or_insert_with(|| HistoricalComputerEntry {
                computer_name: comp.name.clone(),
                contact: comp.contact.clone(),
                serial_number: comp.serial_number.clone(),
                model: comp.model.clone(),
                last_inventory: comp.last_inventory.clone(),
                versions: Vec::new(),
            });

        if !inst.version_name.trim().is_empty()
            && !comp_entry.versions.iter().any(|v| v == &inst.version_name)
        {
            comp_entry.versions.push(inst.version_name.clone());
            comp_entry.versions.sort();
        }
    }

    let mut rows: Vec<HistoricalSoftwareEntry> = grouped
        .into_iter()
        .map(|(software_key, acc)| {
            let mut computers: Vec<_> = acc.computers.into_values().collect();
            computers.sort_by(|a, b| a.computer_name.to_lowercase().cmp(&b.computer_name.to_lowercase()));
            HistoricalSoftwareEntry {
                software_key,
                software_name: acc.name,
                publisher: if acc.publisher.trim().is_empty() {
                    "Unknown".to_string()
                } else {
                    acc.publisher
                },
                host_count: computers.len(),
                versions: acc.versions.into_iter().collect(),
                computers,
            }
        })
        .collect();

    rows.sort_by(|a, b| {
        b.host_count
            .cmp(&a.host_count)
            .then_with(|| a.software_name.to_lowercase().cmp(&b.software_name.to_lowercase()))
    });
    rows
}

struct SwSummary {
    name: String,
    publisher: String,
    hosts: HashSet<u64>,
    versions: BTreeSet<String>,
}

fn summarize_snapshot(snapshot: &InventorySnapshot) -> HashMap<String, SwSummary> {
    let mut map: HashMap<String, SwSummary> = HashMap::new();
    for inst in &snapshot.installations {
        let key = format!(
            "{}|{}",
            normalize_for_grouping(&inst.software_name),
            normalize_for_grouping(&inst.publisher)
        );
        let entry = map.entry(key).or_insert_with(|| SwSummary {
            name: inst.software_name.clone(),
            publisher: inst.publisher.clone(),
            hosts: HashSet::new(),
            versions: BTreeSet::new(),
        });
        entry.hosts.insert(inst.computer_id);
        if !inst.version_name.trim().is_empty() {
            entry.versions.insert(inst.version_name.clone());
        }
    }
    map
}

pub fn compare_snapshots(a: &InventorySnapshot, b: &InventorySnapshot) -> SnapshotDiff {
    let sum_a = summarize_snapshot(a);
    let sum_b = summarize_snapshot(b);

    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut changed = Vec::new();

    let all_keys: BTreeSet<&String> = sum_a.keys().chain(sum_b.keys()).collect();
    for key in all_keys {
        match (sum_a.get(key), sum_b.get(key)) {
            (None, Some(sb)) => {
                added.push(DiffEntry {
                    software_name: sb.name.clone(),
                    publisher: sb.publisher.clone(),
                    hosts_a: 0,
                    hosts_b: sb.hosts.len(),
                    versions_a: Vec::new(),
                    versions_b: sb.versions.iter().cloned().collect(),
                });
            }
            (Some(sa), None) => {
                removed.push(DiffEntry {
                    software_name: sa.name.clone(),
                    publisher: sa.publisher.clone(),
                    hosts_a: sa.hosts.len(),
                    hosts_b: 0,
                    versions_a: sa.versions.iter().cloned().collect(),
                    versions_b: Vec::new(),
                });
            }
            (Some(sa), Some(sb)) => {
                if sa.hosts.len() != sb.hosts.len() || sa.versions != sb.versions {
                    changed.push(DiffEntry {
                        software_name: sa.name.clone(),
                        publisher: sa.publisher.clone(),
                        hosts_a: sa.hosts.len(),
                        hosts_b: sb.hosts.len(),
                        versions_a: sa.versions.iter().cloned().collect(),
                        versions_b: sb.versions.iter().cloned().collect(),
                    });
                }
            }
            (None, None) => continue,
        }
    }

    added.sort_by(|a, b| b.hosts_b.cmp(&a.hosts_b));
    removed.sort_by(|a, b| b.hosts_a.cmp(&a.hosts_a));
    changed.sort_by(|a, b| {
        let delta_a = (a.hosts_b as isize - a.hosts_a as isize).unsigned_abs();
        let delta_b = (b.hosts_b as isize - b.hosts_a as isize).unsigned_abs();
        delta_b.cmp(&delta_a)
    });

    SnapshotDiff { added, removed, changed }
}

fn normalize_for_grouping(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{InventorySnapshot, SnapshotComputer, SnapshotInstallation};

    #[test]
    fn groups_software_by_name_and_publisher() {
        let snapshot = InventorySnapshot {
            captured_at: "2026-04-16 10:00:00".to_string(),
            computers: vec![
                SnapshotComputer {
                    id: 1,
                    name: "PC-01".to_string(),
                    contact: "Alice".to_string(),
                    serial_number: "SN1".to_string(),
                    model: "Dell".to_string(),
                    last_inventory: "2026-04-15".to_string(),
                },
                SnapshotComputer {
                    id: 2,
                    name: "PC-02".to_string(),
                    contact: "Bob".to_string(),
                    serial_number: "SN2".to_string(),
                    model: "HP".to_string(),
                    last_inventory: "2026-04-15".to_string(),
                },
            ],
            installations: vec![
                SnapshotInstallation {
                    computer_id: 1,
                    software_id: 10,
                    software_name: "Office".to_string(),
                    publisher: "Microsoft".to_string(),
                    version_id: 100,
                    version_name: "2021".to_string(),
                },
                SnapshotInstallation {
                    computer_id: 2,
                    software_id: 11,
                    software_name: "Office".to_string(),
                    publisher: "Microsoft".to_string(),
                    version_id: 101,
                    version_name: "365".to_string(),
                },
            ],
        };

        let rows = build_historical_software_view(&snapshot, "", "", "");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].host_count, 2);
        assert_eq!(rows[0].versions, vec!["2021".to_string(), "365".to_string()]);
    }

    #[test]
    fn filters_by_pc_name() {
        let snapshot = InventorySnapshot {
            captured_at: "2026-04-16 10:00:00".to_string(),
            computers: vec![SnapshotComputer {
                id: 1,
                name: "FIN-PC-01".to_string(),
                contact: String::new(),
                serial_number: "SN1".to_string(),
                model: String::new(),
                last_inventory: String::new(),
            }],
            installations: vec![SnapshotInstallation {
                computer_id: 1,
                software_id: 10,
                software_name: "Office".to_string(),
                publisher: "Microsoft".to_string(),
                version_id: 100,
                version_name: "2021".to_string(),
            }],
        };

        assert_eq!(
            build_historical_software_view(&snapshot, "", "", "fin-pc").len(),
            1
        );
        assert!(build_historical_software_view(&snapshot, "", "", "hr-pc").is_empty());
    }
}
