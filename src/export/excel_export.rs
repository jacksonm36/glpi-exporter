use crate::date_util;
use crate::models::AggregatedSoftware;
use rust_xlsxwriter::*;
use std::path::Path;

pub fn export_excel(data: &[AggregatedSoftware], path: &Path, recent_days: i64) -> Result<(), String> {
    let mut workbook = Workbook::new();

    write_summary_sheet(&mut workbook, data, recent_days)?;
    write_detail_sheet(&mut workbook, data)?;

    workbook
        .save(path)
        .map_err(|e| format!("Failed to save Excel file: {e}"))?;

    Ok(())
}

fn write_summary_sheet(
    workbook: &mut Workbook,
    data: &[AggregatedSoftware],
    recent_days: i64,
) -> Result<(), String> {
    let sheet = workbook.add_worksheet();
    sheet
        .set_name("Summary")
        .map_err(|e| format!("Sheet name error: {e}"))?;

    let header_fmt = Format::new()
        .set_bold()
        .set_background_color(Color::RGB(0x4472C4))
        .set_font_color(Color::White);

    let green_fmt = Format::new()
        .set_background_color(Color::RGB(0xC6EFCE))
        .set_font_color(Color::RGB(0x006100));

    let headers = [
        "Rank",
        "Software Name",
        "Publisher",
        "Host Count",
        "Latest Version",
        "Last Updated",
        "Recently Updated",
    ];

    for (col, h) in headers.iter().enumerate() {
        sheet
            .write_string_with_format(0, col as u16, *h, &header_fmt)
            .map_err(|e| format!("Excel write error: {e}"))?;
    }

    let widths = [8.0, 40.0, 25.0, 12.0, 20.0, 20.0, 16.0];
    for (col, w) in widths.iter().enumerate() {
        let _ = sheet.set_column_width(col as u16, *w);
    }

    let now = chrono::Local::now().naive_local().date();

    for (i, sw) in data.iter().enumerate() {
        let row = (i + 1) as u32;
        let recent = date_util::is_recent(&sw.last_updated, now, recent_days);
        let last_updated = sw.last_updated.as_deref().unwrap_or("");

        let _ = sheet.write_number_with_format(row, 0, (i + 1) as f64, &green_fmt);
        let _ = sheet.write_string_with_format(row, 1, &sw.name, &green_fmt);
        let _ = sheet.write_string_with_format(row, 2, &sw.publisher, &green_fmt);
        let _ = sheet.write_number_with_format(row, 3, sw.total_host_count as f64, &green_fmt);
        let _ = sheet.write_string_with_format(row, 4, &sw.latest_version, &green_fmt);
        let _ = sheet.write_string_with_format(row, 5, last_updated, &green_fmt);
        if recent {
            let _ = sheet.write_string_with_format(row, 6, last_updated, &green_fmt);
        } else {
            let _ = sheet.write_string_with_format(row, 6, "No", &green_fmt);
        }
    }

    Ok(())
}

fn write_detail_sheet(
    workbook: &mut Workbook,
    data: &[AggregatedSoftware],
) -> Result<(), String> {
    let sheet = workbook.add_worksheet();
    sheet
        .set_name("Detailed Versions")
        .map_err(|e| format!("Sheet name error: {e}"))?;

    let header_fmt = Format::new()
        .set_bold()
        .set_background_color(Color::RGB(0x4472C4))
        .set_font_color(Color::White);

    let headers = [
        "Software Name",
        "Publisher",
        "Total Hosts",
        "Version",
        "Version Hosts",
        "Last Install Date",
    ];

    for (col, h) in headers.iter().enumerate() {
        sheet
            .write_string_with_format(0, col as u16, *h, &header_fmt)
            .map_err(|e| format!("Excel write error: {e}"))?;
    }

    let widths = [40.0, 25.0, 12.0, 25.0, 14.0, 20.0];
    for (col, w) in widths.iter().enumerate() {
        let _ = sheet.set_column_width(col as u16, *w);
    }

    let mut row: u32 = 1;
    for sw in data {
        for ver in &sw.versions {
            let _ = sheet.write_string(row, 0, &sw.name);
            let _ = sheet.write_string(row, 1, &sw.publisher);
            let _ = sheet.write_number(row, 2, sw.total_host_count as f64);
            let _ = sheet.write_string(row, 3, &ver.version_name);
            let _ = sheet.write_number(row, 4, ver.host_count as f64);
            let _ = sheet.write_string(
                row,
                5,
                ver.last_install_date.as_deref().unwrap_or(""),
            );
            row += 1;
        }
    }

    Ok(())
}
