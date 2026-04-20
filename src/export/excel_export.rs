use crate::models::{AggregatedSoftware, ComputerInfo};
use rust_xlsxwriter::*;
use std::collections::HashMap;
use std::path::Path;

pub fn export_software_inventory_excel(data: &[AggregatedSoftware], path: &Path) -> Result<(), String> {
    let mut workbook = Workbook::new();
    let sheet = workbook.add_worksheet();
    sheet
        .set_name("Software")
        .map_err(|e| format!("Sheet name error: {e}"))?;

    let header_fmt = Format::new()
        .set_bold()
        .set_background_color(Color::RGB(0x4472C4))
        .set_font_color(Color::White);

    let headers = [
        "Rank",
        "Software ID",
        "Software Name",
        "Publisher",
        "Host Count",
        "Latest Version",
        "Last install (any host)",
        "All-hosts install floor",
        "Last agent pull",
        "Last host inventory",
    ];

    for (col, h) in headers.iter().enumerate() {
        sheet
            .write_string_with_format(0, col as u16, *h, &header_fmt)
            .map_err(|e| format!("Excel write error: {e}"))?;
    }

    let widths = [6.0, 10.0, 36.0, 22.0, 10.0, 18.0, 16.0, 22.0, 16.0, 18.0];
    for (col, w) in widths.iter().enumerate() {
        let _ = sheet.set_column_width(col as u16, *w);
    }

    for (i, sw) in data.iter().enumerate() {
        let row = (i + 1) as u32;
        let _ = sheet.write_number(row, 0, (i + 1) as f64);
        let _ = sheet.write_string(row, 1, &sw.software_id.to_string());
        let _ = sheet.write_string(row, 2, &sw.name);
        let _ = sheet.write_string(row, 3, &sw.publisher);
        let _ = sheet.write_number(row, 4, sw.total_host_count as f64);
        let _ = sheet.write_string(row, 5, &sw.latest_version);
        let _ = sheet.write_string(row, 6, sw.last_install_date.as_deref().unwrap_or(""));
        let _ = sheet.write_string(row, 7, sw.all_hosts_install_floor.as_deref().unwrap_or(""));
        let _ = sheet.write_string(row, 8, sw.last_agent_pull.as_deref().unwrap_or(""));
        let _ = sheet.write_string(row, 9, sw.last_host_inventory.as_deref().unwrap_or(""));
    }

    workbook
        .save(path)
        .map_err(|e| format!("Failed to save Excel file: {e}"))?;

    Ok(())
}

pub fn export_excel(computers: &HashMap<u64, ComputerInfo>, path: &Path) -> Result<(), String> {
    let mut workbook = Workbook::new();
    write_inventory_sheet(&mut workbook, computers)?;

    workbook
        .save(path)
        .map_err(|e| format!("Failed to save Excel file: {e}"))?;

    Ok(())
}

fn write_inventory_sheet(
    workbook: &mut Workbook,
    computers: &HashMap<u64, ComputerInfo>,
) -> Result<(), String> {
    let sheet = workbook.add_worksheet();
    sheet
        .set_name("Computer Inventory")
        .map_err(|e| format!("Sheet name error: {e}"))?;

    let header_fmt = Format::new()
        .set_bold()
        .set_background_color(Color::RGB(0x4472C4))
        .set_font_color(Color::White);

    let headers = ["Hostname", "Serial Number", "Model"];

    for (col, h) in headers.iter().enumerate() {
        sheet
            .write_string_with_format(0, col as u16, *h, &header_fmt)
            .map_err(|e| format!("Excel write error: {e}"))?;
    }

    let widths = [35.0, 28.0, 28.0];
    for (col, w) in widths.iter().enumerate() {
        let _ = sheet.set_column_width(col as u16, *w);
    }

    let mut rows: Vec<&ComputerInfo> = computers.values().collect();
    rows.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    for (i, info) in rows.iter().enumerate() {
        let row = (i + 1) as u32;
        let _ = sheet.write_string(row, 0, &info.name);
        let _ = sheet.write_string(row, 1, &info.serial_number);
        let _ = sheet.write_string(row, 2, &info.model);
    }

    Ok(())
}
