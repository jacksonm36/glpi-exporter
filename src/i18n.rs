use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Lang {
    En,
    Hu,
}

impl Default for Lang {
    fn default() -> Self {
        Lang::En
    }
}

impl Lang {
    pub fn label(self) -> &'static str {
        match self {
            Lang::En => "EN",
            Lang::Hu => "HU",
        }
    }

    pub fn toggle(self) -> Self {
        match self {
            Lang::En => Lang::Hu,
            Lang::Hu => Lang::En,
        }
    }
}

pub struct T {
    // Window / heading
    pub app_title: &'static str,

    // Connection panel
    pub glpi_url: &'static str,
    pub glpi_url_hint: &'static str,
    pub user_token: &'static str,
    pub user_token_hint: &'static str,
    pub app_token: &'static str,
    pub app_token_hint: &'static str,
    pub accept_invalid_tls: &'static str,
    pub accept_invalid_tls_tip: &'static str,
    pub tokens_warning: &'static str,
    pub connect_fetch: &'static str,
    pub status_prefix: &'static str,

    // Fetch status
    pub status_idle: &'static str,
    pub status_connecting: &'static str,
    pub status_fetching_software: &'static str,
    pub status_fetching_versions: &'static str,
    pub status_fetching_installations: &'static str,
    pub status_fetching_computers: &'static str,
    pub status_aggregating: &'static str,
    pub status_loaded: &'static str, // "{count} software across {hosts} hosts"
    pub status_error: &'static str,

    // Filter panel
    pub software_name: &'static str,
    pub search_hint: &'static str,
    pub publisher: &'static str,
    pub min_hosts: &'static str,
    pub updated_in_last: &'static str,
    pub days: &'static str,
    pub top_n: &'static str,
    pub all_hint: &'static str,
    pub hide_os_defaults: &'static str,
    pub hide_os_defaults_tip: &'static str,
    pub clear_filters: &'static str,
    pub show_selected_only: &'static str,
    pub show_selected_only_tip: &'static str, // "Only show the {n} software..."
    pub n_selected: &'static str, // "{n} selected"
    pub select_all_visible: &'static str,
    pub select_all_visible_tip: &'static str,
    pub deselect_all: &'static str,
    pub show_pcs: &'static str,
    pub hide_pcs: &'static str,
    pub show_pcs_tip: &'static str,

    // Export panel
    pub export: &'static str,
    pub save_csv: &'static str,
    pub csv_files: &'static str,
    pub csv_saved: &'static str, // "CSV saved to {path}"
    pub csv_error: &'static str,
    pub save_excel: &'static str,
    pub excel_files: &'static str,
    pub excel_saved: &'static str,
    pub excel_error: &'static str,
    pub save_json: &'static str,
    pub json_files: &'static str,
    pub json_saved: &'static str,
    pub json_error: &'static str,

    // Status bar
    pub showing_of: &'static str, // "Showing {n} of {total} software"
    pub total_installations: &'static str,
    pub no_data_loaded: &'static str,

    // Software table
    pub no_data_msg: &'static str,
    pub col_rank: &'static str,
    pub col_software_name: &'static str,
    pub col_publisher: &'static str,
    pub col_hosts: &'static str,
    pub col_latest_version: &'static str,
    pub col_last_updated: &'static str,
    pub col_recent: &'static str,
    pub yes: &'static str,
    pub no: &'static str,
    pub no_date: &'static str,
    pub versions_tab: &'static str, // "Versions ({n})"
    pub pcs_tab: &'static str,     // "PCs ({n})"
    pub col_version: &'static str,
    pub col_last_install: &'static str,
    pub no_version_data: &'static str,
    pub pcs_with_software: &'static str, // "{n} PCs with this software:"
    pub col_pc_name: &'static str,
    pub col_user_contact: &'static str,
    pub no_install_data: &'static str,
    pub unknown: &'static str,

    // PC panel
    pub pcs_with_selected: &'static str,
    pub no_install_data_selected: &'static str,
    pub pcs_found_across: &'static str, // "{n} PCs found across {m} selected software"
    pub user_prefix: &'static str,      // "User: "
}

pub fn t(lang: Lang) -> &'static T {
    match lang {
        Lang::En => &EN,
        Lang::Hu => &HU,
    }
}

static EN: T = T {
    app_title: "GLPI Software Inventory Explorer",

    glpi_url: "GLPI URL:",
    glpi_url_hint: "https://glpi.example.com",
    user_token: "User Token:",
    user_token_hint: "Your GLPI API user token",
    app_token: "App Token:",
    app_token_hint: "Optional",
    accept_invalid_tls: "Accept invalid TLS certificates",
    accept_invalid_tls_tip: "Only enable this for self-signed certificates. Disables TLS verification.",
    tokens_warning: "Tokens are saved in plaintext next to the .exe",
    connect_fetch: "Connect & Fetch",
    status_prefix: "Status",

    status_idle: "Not connected",
    status_connecting: "Connecting...",
    status_fetching_software: "Fetching software",
    status_fetching_versions: "Fetching versions",
    status_fetching_installations: "Fetching installations",
    status_fetching_computers: "Fetching computers",
    status_aggregating: "Aggregating data...",
    status_loaded: "Loaded",
    status_error: "Error",

    software_name: "Software Name:",
    search_hint: "Search...",
    publisher: "Publisher:",
    min_hosts: "Min Hosts:",
    updated_in_last: "Updated in last",
    days: "days",
    top_n: "Top N:",
    all_hint: "All",
    hide_os_defaults: "Hide OS defaults",
    hide_os_defaults_tip: "Hide all default Windows / Microsoft OS components\nincluding built-in apps (EN + HU), UWP packages,\nupdates, runtimes, redistributables, and GUID entries",
    clear_filters: "Clear Filters",
    show_selected_only: "Show selected only",
    show_selected_only_tip: "Only show the checked software",
    n_selected: "selected",
    select_all_visible: "Select all visible",
    select_all_visible_tip: "Check all items currently shown in the table",
    deselect_all: "Deselect all",
    show_pcs: "Show PCs",
    hide_pcs: "Hide PCs",
    show_pcs_tip: "Show which computers have the selected software installed",

    export: "Export:",
    save_csv: "Save CSV",
    csv_files: "CSV Files",
    csv_saved: "CSV saved to",
    csv_error: "CSV error",
    save_excel: "Save Excel",
    excel_files: "Excel Files",
    excel_saved: "Excel saved to",
    excel_error: "Excel error",
    save_json: "Save JSON",
    json_files: "JSON Files",
    json_saved: "JSON saved to",
    json_error: "JSON error",

    showing_of: "Showing",
    total_installations: "Total installations",
    no_data_loaded: "No data loaded",

    no_data_msg: "No data loaded. Connect to GLPI and fetch data to begin.",
    col_rank: "#",
    col_software_name: "Software Name",
    col_publisher: "Publisher",
    col_hosts: "Hosts",
    col_latest_version: "Latest Version",
    col_last_updated: "Last Updated",
    col_recent: "Recent",
    yes: "Yes",
    no: "No",
    no_date: "No date",
    versions_tab: "Versions",
    pcs_tab: "PCs",
    col_version: "Version",
    col_last_install: "Last Install",
    no_version_data: "No version data available.",
    pcs_with_software: "PCs with this software",
    col_pc_name: "PC Name",
    col_user_contact: "User / Contact",
    no_install_data: "No installation data.",
    unknown: "Unknown",

    pcs_with_selected: "PCs with selected software",
    no_install_data_selected: "No installation data for the selected software.",
    pcs_found_across: "PCs found across",
    user_prefix: "User",
};

static HU: T = T {
    app_title: "GLPI Szoftver Leltár Kezelő",

    glpi_url: "GLPI URL:",
    glpi_url_hint: "https://glpi.pelda.hu",
    user_token: "Felhasználói token:",
    user_token_hint: "GLPI API felhasználói token",
    app_token: "Alkalmazás token:",
    app_token_hint: "Opcionális",
    accept_invalid_tls: "Érvénytelen TLS tanúsítványok elfogadása",
    accept_invalid_tls_tip: "Csak önaláírt tanúsítványokhoz engedélyezze. Kikapcsolja a TLS ellenőrzést.",
    tokens_warning: "A tokenek egyszerű szövegként kerülnek mentésre az .exe mellé",
    connect_fetch: "Csatlakozás és lekérés",
    status_prefix: "Állapot",

    status_idle: "Nincs csatlakozva",
    status_connecting: "Csatlakozás...",
    status_fetching_software: "Szoftverek lekérése",
    status_fetching_versions: "Verziók lekérése",
    status_fetching_installations: "Telepítések lekérése",
    status_fetching_computers: "Számítógépek lekérése",
    status_aggregating: "Adatok összesítése...",
    status_loaded: "Betöltve",
    status_error: "Hiba",

    software_name: "Szoftver neve:",
    search_hint: "Keresés...",
    publisher: "Kiadó:",
    min_hosts: "Min. gépek:",
    updated_in_last: "Frissítve az elmúlt",
    days: "napban",
    top_n: "Top N:",
    all_hint: "Mind",
    hide_os_defaults: "OS alapértelmezettek elrejtése",
    hide_os_defaults_tip: "Az összes alapértelmezett Windows / Microsoft OS komponens elrejtése\nbeleértve a beépített alkalmazásokat (EN + HU), UWP csomagokat,\nfrissítéseket, futtatókörnyezeteket és GUID bejegyzéseket",
    clear_filters: "Szűrők törlése",
    show_selected_only: "Csak kijelöltek",
    show_selected_only_tip: "Csak a kijelölt szoftverek megjelenítése",
    n_selected: "kijelölve",
    select_all_visible: "Összes látható kijelölése",
    select_all_visible_tip: "A táblázatban látható összes elem kijelölése",
    deselect_all: "Kijelölés törlése",
    show_pcs: "PC-k mutatása",
    hide_pcs: "PC-k elrejtése",
    show_pcs_tip: "A kiválasztott szoftverrel rendelkező számítógépek megjelenítése",

    export: "Exportálás:",
    save_csv: "CSV mentése",
    csv_files: "CSV fájlok",
    csv_saved: "CSV mentve ide",
    csv_error: "CSV hiba",
    save_excel: "Excel mentése",
    excel_files: "Excel fájlok",
    excel_saved: "Excel mentve ide",
    excel_error: "Excel hiba",
    save_json: "JSON mentése",
    json_files: "JSON fájlok",
    json_saved: "JSON mentve ide",
    json_error: "JSON hiba",

    showing_of: "Megjelenítve",
    total_installations: "Összes telepítés",
    no_data_loaded: "Nincs betöltött adat",

    no_data_msg: "Nincs betöltött adat. Csatlakozzon a GLPI-hez az adatok lekéréséhez.",
    col_rank: "#",
    col_software_name: "Szoftver neve",
    col_publisher: "Kiadó",
    col_hosts: "Gépek",
    col_latest_version: "Legújabb verzió",
    col_last_updated: "Utolsó frissítés",
    col_recent: "Friss",
    yes: "Igen",
    no: "Nem",
    no_date: "Nincs dátum",
    versions_tab: "Verziók",
    pcs_tab: "PC-k",
    col_version: "Verzió",
    col_last_install: "Utolsó telepítés",
    no_version_data: "Nincs verzióadat.",
    pcs_with_software: "PC ezzel a szoftverrel",
    col_pc_name: "PC név",
    col_user_contact: "Felhasználó",
    no_install_data: "Nincs telepítési adat.",
    unknown: "Ismeretlen",

    pcs_with_selected: "PC-k a kiválasztott szoftverekkel",
    no_install_data_selected: "Nincs telepítési adat a kiválasztott szoftverekhez.",
    pcs_found_across: "PC található",
    user_prefix: "Felhasználó",
};
