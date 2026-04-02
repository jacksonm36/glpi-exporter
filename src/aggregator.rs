use crate::date_util;
use crate::models::*;
use std::collections::{HashMap, HashSet};

pub fn aggregate(
    software_list: &[GlpiSoftware],
    versions: &[GlpiSoftwareVersion],
    installations: &[GlpiItemSoftwareVersion],
) -> Vec<AggregatedSoftware> {
    let sw_map: HashMap<u64, &GlpiSoftware> = software_list
        .iter()
        .map(|s| (s.id, s))
        .collect();

    let version_to_sw: HashMap<u64, u64> = versions
        .iter()
        .map(|v| (v.id, v.softwares_id))
        .collect();

    let version_name_map: HashMap<u64, &str> = versions
        .iter()
        .map(|v| (v.id, v.name.as_str()))
        .collect();

    struct SwAccum {
        host_ids: HashSet<u64>,
        version_hosts: HashMap<u64, HashSet<u64>>,
        version_dates: HashMap<u64, Option<String>>,
        latest_date: Option<String>,
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
            latest_date: None,
        });

        entry.host_ids.insert(inst.items_id);

        entry
            .version_hosts
            .entry(inst.softwareversions_id)
            .or_default()
            .insert(inst.items_id);

        let install_date = inst
            .date_install
            .as_deref()
            .or(inst.date_mod.as_deref())
            .map(|s| s.to_string());

        if let Some(ref d) = install_date {
            let ver_date = entry
                .version_dates
                .entry(inst.softwareversions_id)
                .or_insert(None);
            if ver_date
                .as_ref()
                .map_or(true, |existing| date_util::date_is_newer(d, existing))
            {
                *ver_date = Some(d.clone());
            }
            if entry
                .latest_date
                .as_ref()
                .map_or(true, |existing| date_util::date_is_newer(d, existing))
            {
                entry.latest_date = Some(d.clone());
            }
        }
    }

    let mut result: Vec<AggregatedSoftware> = accum
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

            Some(AggregatedSoftware {
                software_id: sw_id,
                name: sw.name.clone(),
                publisher,
                total_host_count: acc.host_ids.len(),
                latest_version,
                last_updated: acc.latest_date,
                versions: version_details,
                host_ids: acc.host_ids,
            })
        })
        .collect();

    result.sort_by(|a, b| b.total_host_count.cmp(&a.total_host_count));
    result
}
