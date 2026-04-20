use crate::date_util;
use crate::models::*;
use serde_json::Value;
use std::collections::{HashMap, HashSet};

pub fn aggregate(
    software_list: &[GlpiSoftware],
    versions: &[GlpiSoftwareVersion],
    installations: &[GlpiItemSoftwareVersion],
    computers: &[GlpiComputer],
) -> Vec<AggregatedSoftware> {
    let host_inventory: HashMap<u64, String> = computers
        .iter()
        .filter_map(|c| {
            computer_inventory_timestamp(c).map(|d| (c.id, d))
        })
        .collect();

    let sw_map: HashMap<u64, &GlpiSoftware> = software_list
        .iter()
        .map(|s| (s.id, s))
        .collect();

    let mut version_to_sw: HashMap<u64, u64> = HashMap::new();
    for v in versions {
        version_to_sw.entry(v.id).or_insert(v.softwares_id);
    }

    let version_name_map: HashMap<u64, &str> = versions
        .iter()
        .map(|v| (v.id, v.name.as_str()))
        .collect();

    struct SwAccum {
        host_ids: HashSet<u64>,
        version_hosts: HashMap<u64, HashSet<u64>>,
        version_dates: HashMap<u64, Option<String>>,
        latest_install_date: Option<String>,
        latest_date: Option<String>,
        latest_agent_pull: Option<String>,
        latest_host_inventory: Option<String>,
        host_install_best: HashMap<u64, Option<String>>,
        host_mod_best: HashMap<u64, Option<String>>,
    }

    let mut accum: HashMap<u64, SwAccum> = HashMap::new();

    for inst in installations {
        let sw_id = match version_to_sw.get(&inst.softwareversions_id) {
            Some(id) => *id,
            None => continue,
        };

        let entry = accum.entry(sw_id).or_insert_with(|| SwAccum {
            host_ids: HashSet::new(),
            version_hosts: HashMap::new(),
            version_dates: HashMap::new(),
            latest_install_date: None,
            latest_date: None,
            latest_agent_pull: None,
            latest_host_inventory: None,
            host_install_best: HashMap::new(),
            host_mod_best: HashMap::new(),
        });

        entry.host_ids.insert(inst.items_id);

        if let Some(d) = inst.date_install.as_deref() {
            let v = entry
                .host_install_best
                .entry(inst.items_id)
                .or_insert(None);
            merge_date_best(v, d);
        }
        if let Some(d) = inst.date_mod.as_deref() {
            let v = entry.host_mod_best.entry(inst.items_id).or_insert(None);
            merge_date_best(v, d);
        }

        entry
            .version_hosts
            .entry(inst.softwareversions_id)
            .or_default()
            .insert(inst.items_id);

        if let Some(inv) = host_inventory.get(&inst.items_id) {
            merge_date_best(&mut entry.latest_host_inventory, inv);
        }

        if let Some(d) = inst.date_install.as_deref() {
            let ver_date = entry
                .version_dates
                .entry(inst.softwareversions_id)
                .or_insert(None);
            if ver_date
                .as_ref()
                .map_or(true, |existing| date_util::date_is_newer(d, existing))
            {
                *ver_date = Some(d.to_string());
            }
            if entry
                .latest_date
                .as_ref()
                .map_or(true, |existing| date_util::date_is_newer(d, existing))
            {
                entry.latest_date = Some(d.to_string());
            }
            if entry
                .latest_install_date
                .as_ref()
                .map_or(true, |existing| date_util::date_is_newer(d, existing))
            {
                entry.latest_install_date = Some(d.to_string());
            }
        }

        if let Some(d) = inst.date_mod.as_deref() {
            // Strict agent-only timestamp for "Fresh list".
            if entry
                .latest_agent_pull
                .as_ref()
                .map_or(true, |existing| date_util::date_is_newer(d, existing))
            {
                entry.latest_agent_pull = Some(d.to_string());
            }
        }

        if let Some(d) = inst.date_mod.as_deref() {
            let ver_date = entry
                .version_dates
                .entry(inst.softwareversions_id)
                .or_insert(None);
            if ver_date
                .as_ref()
                .map_or(true, |existing| date_util::date_is_newer(d, existing))
            {
                *ver_date = Some(d.to_string());
            }
            if entry
                .latest_date
                .as_ref()
                .map_or(true, |existing| date_util::date_is_newer(d, existing))
            {
                entry.latest_date = Some(d.to_string());
            }
        }
    }

    let per_id: Vec<AggregatedSoftware> = accum
        .into_iter()
        .filter_map(|(sw_id, acc)| {
            let sw = sw_map.get(&sw_id)?;

            let publisher = if sw.manufacturers_id.is_empty()
                || sw.manufacturers_id == "0"
                || sw.manufacturers_id == "&nbsp;"
            {
                "Unknown".to_string()
            } else {
                sw.manufacturers_id.clone()
            };

            let mut version_details: Vec<VersionDetail> = acc
                .version_hosts
                .iter()
                .map(|(vid, hosts)| VersionDetail {
                    version_id: *vid,
                    version_name: version_name_map
                        .get(vid)
                        .unwrap_or(&"Unknown")
                        .to_string(),
                    host_count: hosts.len(),
                    last_install_date: acc.version_dates.get(vid).cloned().flatten(),
                    host_ids: hosts.clone(),
                })
                .collect();

            version_details.sort_by(|a, b| b.host_count.cmp(&a.host_count));

            let latest_version = version_details
                .iter()
                .filter(|v| v.last_install_date.is_some())
                .max_by(|a, b| {
                    let da = a.last_install_date.as_deref().unwrap_or("");
                    let db = b.last_install_date.as_deref().unwrap_or("");
                    let pa = date_util::parse_date(da);
                    let pb = date_util::parse_date(db);
                    pa.cmp(&pb)
                })
                .or_else(|| version_details.first())
                .map(|v| v.version_name.clone())
                .unwrap_or_default();

            let name = sw.name.clone();
            let name_lower = name.to_lowercase();
            let publisher_lower = publisher.to_lowercase();
            let host_install_best_per_host = acc.host_install_best;
            let host_mod_best_per_host = acc.host_mod_best;
            let all_hosts_install_floor = all_hosts_install_floor_from_maps(
                &acc.host_ids,
                &host_install_best_per_host,
                &host_mod_best_per_host,
            );
            Some(AggregatedSoftware {
                software_id: sw_id,
                name,
                publisher,
                name_lower,
                publisher_lower,
                total_host_count: acc.host_ids.len(),
                latest_version,
                last_install_date: acc.latest_install_date,
                last_updated: acc.latest_date,
                last_agent_pull: acc.latest_agent_pull,
                last_host_inventory: acc.latest_host_inventory,
                all_hosts_install_floor,
                host_install_best_per_host,
                host_mod_best_per_host,
                versions: version_details,
                host_ids: acc.host_ids,
            })
        })
        .collect();

    // Merge duplicate entries with the same software name + publisher.
    // Some GLPI instances store the same product under multiple software IDs.
    let mut merged_map: HashMap<(String, String), AggregatedSoftware> = HashMap::new();
    for mut sw in per_id {
        let key = (normalize_for_grouping(&sw.name), normalize_for_grouping(&sw.publisher));
        if let Some(existing) = merged_map.get_mut(&key) {
            if sw.software_id < existing.software_id {
                existing.software_id = sw.software_id;
            }

            existing.host_ids.extend(sw.host_ids.drain());
            existing.total_host_count = existing.host_ids.len();

            if let Some(incoming) = sw.last_updated.as_ref() {
                if existing
                    .last_updated
                    .as_ref()
                    .map_or(true, |current| date_util::date_is_newer(incoming, current))
                {
                    existing.last_updated = Some(incoming.clone());
                }
            }
            if let Some(incoming) = sw.last_install_date.as_ref() {
                if existing
                    .last_install_date
                    .as_ref()
                    .map_or(true, |current| date_util::date_is_newer(incoming, current))
                {
                    existing.last_install_date = Some(incoming.clone());
                }
            }
            if let Some(incoming) = sw.last_agent_pull.as_ref() {
                if existing
                    .last_agent_pull
                    .as_ref()
                    .map_or(true, |current| date_util::date_is_newer(incoming, current))
                {
                    existing.last_agent_pull = Some(incoming.clone());
                }
            }
            if let Some(incoming) = sw.last_host_inventory.as_ref() {
                if existing
                    .last_host_inventory
                    .as_ref()
                    .map_or(true, |current| date_util::date_is_newer(incoming, current))
                {
                    existing.last_host_inventory = Some(incoming.clone());
                }
            }

            let mut by_version_id: HashMap<u64, usize> = existing
                .versions
                .iter()
                .enumerate()
                .map(|(idx, v)| (v.version_id, idx))
                .collect();

            for incoming_version in sw.versions {
                if let Some(idx) = by_version_id.get(&incoming_version.version_id).copied() {
                    let current = &mut existing.versions[idx];
                    current.host_ids.extend(incoming_version.host_ids.iter().copied());
                    current.host_count = current.host_ids.len();
                    if let Some(ref inc_date) = incoming_version.last_install_date {
                        if current
                            .last_install_date
                            .as_ref()
                            .map_or(true, |cur_date| date_util::date_is_newer(inc_date, cur_date))
                        {
                            current.last_install_date = Some(inc_date.clone());
                        }
                    }
                } else {
                    by_version_id.insert(incoming_version.version_id, existing.versions.len());
                    existing.versions.push(incoming_version);
                }
            }

            for (host_id, opt) in sw.host_install_best_per_host {
                if let Some(inc) = opt {
                    let e = existing
                        .host_install_best_per_host
                        .entry(host_id)
                        .or_insert(None);
                    merge_date_best(e, &inc);
                }
            }
            for (host_id, opt) in sw.host_mod_best_per_host {
                if let Some(inc) = opt {
                    let e = existing
                        .host_mod_best_per_host
                        .entry(host_id)
                        .or_insert(None);
                    merge_date_best(e, &inc);
                }
            }
            existing.all_hosts_install_floor = all_hosts_install_floor_from_maps(
                &existing.host_ids,
                &existing.host_install_best_per_host,
                &existing.host_mod_best_per_host,
            );
        } else {
            merged_map.insert(key, sw);
        }
    }

    let mut result: Vec<AggregatedSoftware> = merged_map
        .into_values()
        .map(|mut sw| {
            sw.versions
                .sort_by(|a, b| b.host_count.cmp(&a.host_count).then_with(|| a.version_name.cmp(&b.version_name)));

            sw.latest_version = sw
                .versions
                .iter()
                .filter(|v| v.last_install_date.is_some())
                .max_by(|a, b| {
                    let da = a.last_install_date.as_deref().unwrap_or("");
                    let db = b.last_install_date.as_deref().unwrap_or("");
                    let pa = date_util::parse_date(da);
                    let pb = date_util::parse_date(db);
                    pa.cmp(&pb)
                })
                .or_else(|| sw.versions.first())
                .map(|v| v.version_name.clone())
                .unwrap_or_default();
            sw.name_lower = sw.name.to_lowercase();
            sw.publisher_lower = sw.publisher.to_lowercase();
            sw
        })
        .collect();

    result.sort_by(|a, b| b.total_host_count.cmp(&a.total_host_count));
    result
}

fn all_hosts_install_floor_from_maps(
    host_ids: &HashSet<u64>,
    host_install_best: &HashMap<u64, Option<String>>,
    host_mod_best: &HashMap<u64, Option<String>>,
) -> Option<String> {
    let mut floor: Option<String> = None;
    for &hid in host_ids {
        let ev = host_install_best
            .get(&hid)
            .cloned()
            .flatten()
            .or_else(|| host_mod_best.get(&hid).cloned().flatten());
        let Some(d) = ev else {
            return None;
        };
        date_util::merge_date_earliest(&mut floor, &d);
    }
    floor
}

fn merge_date_best(best: &mut Option<String>, candidate: &str) {
    let c = candidate.trim();
    if c.is_empty() {
        return;
    }
    match best {
        None => *best = Some(c.to_string()),
        Some(b) if date_util::date_is_newer(c, b) => *best = Some(c.to_string()),
        _ => {}
    }
}

pub fn computer_inventory_timestamp(c: &GlpiComputer) -> Option<String> {
    let mut best: Option<String> = None;
    if let Some(ref d) = c.date_mod {
        merge_date_best(&mut best, d);
    }
    if let Some(ref d) = c.date_creation {
        merge_date_best(&mut best, d);
    }
    best
}

fn normalize_for_grouping(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

pub fn aggregate_license_keys(
    software_list: &[GlpiSoftware],
    licenses: &[GlpiSoftwareLicense],
    computers: &[GlpiComputer],
) -> Vec<LicenseKeyRecord> {
    let software_name_by_id: HashMap<u64, &str> = software_list
        .iter()
        .map(|s| (s.id, s.name.as_str()))
        .collect();
    let computer_info_by_id: HashMap<u64, (&str, &str)> = computers
        .iter()
        .map(|c| (c.id, (c.name.as_str(), c.contact.as_deref().unwrap_or_default())))
        .collect();

    let mut records = Vec::new();
    let mut dedup = HashSet::new();

    for lic in licenses {
        let mut key_candidates = HashSet::new();
        if !lic.serial.trim().is_empty() {
            key_candidates.insert(lic.serial.trim().to_string());
        }
        for (field, value) in &lic.extra_fields {
            if is_key_field_name(field) {
                for candidate in extract_string_candidates(value) {
                    if is_license_key_like(&candidate) {
                        key_candidates.insert(candidate);
                    }
                }
            }
        }

        let software_name = lic
            .softwares_id
            .parse::<u64>()
            .ok()
            .and_then(|id| software_name_by_id.get(&id).copied())
            .map(ToString::to_string)
            .unwrap_or_else(|| {
                if lic.name.trim().is_empty() {
                    "Unknown software".to_string()
                } else {
                    lic.name.clone()
                }
            });

        let category = classify_category(&software_name);
        if category == LicenseCategory::ThirdParty {
            continue;
        }
        let notes = lic.comment.clone().filter(|s| !s.trim().is_empty());
        let computer_id = read_u64_field(&lic.extra_fields, &["computers_id", "computer"]);
        let computer_name = read_string_field(&lic.extra_fields, &["computers_id", "computer"]);
        let user_contact =
            read_string_field(&lic.extra_fields, &["users_id", "contact", "user"]);

        for key in key_candidates {
            let dedup_key = format!("glpi|{}|{}", software_name.to_lowercase(), key.to_lowercase());
            if dedup.insert(dedup_key) {
                records.push(LicenseKeyRecord {
                    source: LicenseSource::Glpi,
                    category,
                    product_name: software_name.clone(),
                    license_key: key,
                    computer_id,
                    computer_name: computer_name.clone(),
                    user_contact: user_contact.clone(),
                    notes: notes.clone(),
                });
            }
        }
    }

    for comp in computers {
        for (field, value) in &comp.extra_fields {
            if !is_key_field_name(field) {
                continue;
            }
            for candidate in extract_string_candidates(value) {
                if !is_license_key_like(&candidate) {
                    continue;
                }
                let product_name = field_to_product(field);
                let category = classify_category(&product_name);
                if category == LicenseCategory::ThirdParty {
                    continue;
                }
                let contact = comp.contact.clone().filter(|s| !s.trim().is_empty());
                let dedup_key = format!(
                    "comp|{}|{}|{}",
                    comp.id,
                    product_name.to_lowercase(),
                    candidate.to_lowercase()
                );
                if dedup.insert(dedup_key) {
                    records.push(LicenseKeyRecord {
                        source: LicenseSource::ComputerInventory,
                        category,
                        product_name,
                        license_key: candidate,
                        computer_id: Some(comp.id),
                        computer_name: Some(comp.name.clone()),
                        user_contact: contact,
                        notes: Some(format!("Inventory field: {field}")),
                    });
                }
            }
        }
    }

    // Try to resolve numeric computer IDs returned from license records.
    for rec in &mut records {
        if rec.source == LicenseSource::Glpi {
            if rec.computer_id.is_none() {
                if let Some(name_or_id) = rec.computer_name.clone() {
                    if let Ok(id) = name_or_id.parse::<u64>() {
                        rec.computer_id = Some(id);
                    }
                }
            }
            if let Some(id) = rec.computer_id {
                if let Some((computer_name, contact)) = computer_info_by_id.get(&id) {
                    rec.computer_name = Some((*computer_name).to_string());
                    if rec.user_contact.is_none() && !contact.is_empty() {
                        rec.user_contact = Some((*contact).to_string());
                    }
                }
            }
        }
    }

    records.sort_by(|a, b| {
        a.product_name
            .to_lowercase()
            .cmp(&b.product_name.to_lowercase())
            .then_with(|| a.license_key.to_lowercase().cmp(&b.license_key.to_lowercase()))
    });
    records
}

fn read_string_field(fields: &HashMap<String, Value>, keys: &[&str]) -> Option<String> {
    for (field, value) in fields {
        let field_lc = field.to_lowercase();
        if keys.iter().any(|k| field_lc.contains(k)) {
            for candidate in extract_string_candidates(value) {
                let trimmed = candidate.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }
    }
    None
}

fn read_u64_field(fields: &HashMap<String, Value>, keys: &[&str]) -> Option<u64> {
    for (field, value) in fields {
        let field_lc = field.to_lowercase();
        if !keys.iter().any(|k| field_lc.contains(k)) {
            continue;
        }
        for candidate in extract_string_candidates(value) {
            let trimmed = candidate.trim();
            if let Ok(id) = trimmed.parse::<u64>() {
                return Some(id);
            }
        }
    }
    None
}

fn extract_string_candidates(value: &Value) -> Vec<String> {
    match value {
        Value::String(s) => vec![s.trim().to_string()],
        Value::Number(n) => vec![n.to_string()],
        Value::Array(arr) => arr.iter().flat_map(extract_string_candidates).collect(),
        Value::Object(map) => map.values().flat_map(extract_string_candidates).collect(),
        _ => Vec::new(),
    }
}

fn is_key_field_name(field: &str) -> bool {
    let f = field.to_lowercase();
    [
        "license",
        "productkey",
        "product_key",
        "cdkey",
        "serial",
        "office",
        "windows",
        "activation",
        "key",
    ]
    .iter()
    .any(|needle| f.contains(needle))
}

fn is_license_key_like(value: &str) -> bool {
    let v = value.trim();
    if v.len() < 10 || v.len() > 80 {
        return false;
    }
    if !v.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == ' ') {
        return false;
    }
    let alnum = v.chars().filter(|c| c.is_ascii_alphanumeric()).count();
    let hyphen = v.contains('-');
    alnum >= 10 && hyphen
}

fn classify_category(product: &str) -> LicenseCategory {
    let p = product.to_lowercase();
    if p.contains("windows") {
        LicenseCategory::Windows
    } else if p.contains("office") || p.contains("microsoft 365") {
        LicenseCategory::Office
    } else {
        LicenseCategory::ThirdParty
    }
}

fn field_to_product(field: &str) -> String {
    let f = field.to_lowercase();
    if f.contains("office") {
        "Microsoft Office".to_string()
    } else if f.contains("windows") {
        "Microsoft Windows".to_string()
    } else {
        format!("Field: {field}")
    }
}
