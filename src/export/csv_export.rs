use crate::date_util;
use crate::models::AggregatedSoftware;
use std::path::Path;

pub fn export_csv(data: &[AggregatedSoftware], path: &Path, recent_days: i64) -> Result<(), String> {
    let mut wtr =
        csv::Writer::from_path(path).map_err(|e| format!("Cannot create CSV file: {e}"))?;

    wtr.write_record([
        "Rank",
        "Software Name",
        "Publisher",
        "Host Count",
        "Version",
        "Version Host Count",
        "Last Updated",
        "Recently Updated",
    ])
    .map_err(|e| format!("CSV write error: {e}"))?;

    let now = chrono::Local::now().naive_local().date();

    for (i, sw) in data.iter().enumerate() {
        let recently = if date_util::is_recent(&sw.last_updated, now, recent_days) {
            "Yes"
        } else {
            "No"
        };
        let rank = (i + 1).to_string();
        let host_count = sw.total_host_count.to_string();
        let last_updated = sw.last_updated.as_deref().unwrap_or("");

        if sw.versions.is_empty() {
            wtr.write_record(&[
                &rank, &sw.name, &sw.publisher, &host_count, "", "", last_updated, recently,
            ])
            .map_err(|e| format!("CSV write error: {e}"))?;
        } else {
            for ver in &sw.versions {
                let ver_hosts = ver.host_count.to_string();
                let ver_date = ver.last_install_date.as_deref().unwrap_or("");
                wtr.write_record(&[
                    &rank,
                    &sw.name,
                    &sw.publisher,
                    &host_count,
                    &ver.version_name,
                    &ver_hosts,
                    ver_date,
                    recently,
                ])
                .map_err(|e| format!("CSV write error: {e}"))?;
            }
        }
    }

    wtr.flush().map_err(|e| format!("CSV flush error: {e}"))?;
    Ok(())
}
