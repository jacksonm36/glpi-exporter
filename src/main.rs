// GLPI Software Inventory Explorer
// Copyright (C) 2025 jacksonm36
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod aggregator;
mod app;
mod config;
mod date_util;
mod export;
mod glpi_client;
mod history_query;
mod history_store;
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
