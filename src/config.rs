use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;

fn exe_dir() -> PathBuf {
    let exe = std::env::current_exe().unwrap_or_default();
    exe.parent()
        .unwrap_or(std::path::Path::new("."))
        .to_path_buf()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    #[serde(default)]
    pub glpi_url: String,
    #[serde(default)]
    pub user_token: String,
    #[serde(default)]
    pub app_token: String,
    #[serde(default)]
    pub accept_invalid_certs: bool,
    #[serde(default)]
    pub language: crate::i18n::Lang,
}

impl AppConfig {
    pub fn load() -> Self {
        let path = exe_dir().join("config.json");
        if path.exists() {
            if let Ok(data) = std::fs::read_to_string(&path) {
                if let Ok(cfg) = serde_json::from_str(&data) {
                    return cfg;
                }
            }
        }
        Self::default()
    }

    pub fn save(&self) {
        let path = exe_dir().join("config.json");
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, json);
        }
    }
}

pub fn load_selections() -> HashSet<u64> {
    let path = exe_dir().join("selections.json");
    if path.exists() {
        if let Ok(data) = std::fs::read_to_string(&path) {
            if let Ok(ids) = serde_json::from_str::<Vec<u64>>(&data) {
                return ids.into_iter().collect();
            }
        }
    }
    HashSet::new()
}

pub fn save_selections(selected: &HashSet<u64>) {
    let path = exe_dir().join("selections.json");
    let ids: Vec<u64> = selected.iter().copied().collect();
    if let Ok(json) = serde_json::to_string_pretty(&ids) {
        let _ = std::fs::write(path, json);
    }
}
