#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod aggregator;
mod app;
mod config;
mod date_util;
mod export;
mod glpi_client;
pub mod i18n;
mod models;
mod ui;
mod worker;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 700.0])
            .with_min_inner_size([800.0, 500.0])
            .with_title("GLPI Software Inventory Explorer"),
        ..Default::default()
    };

    eframe::run_native(
        "GLPI Software Inventory Explorer",
        options,
        Box::new(|cc| Ok(Box::new(app::GlpiApp::new(cc)))),
    )
}
