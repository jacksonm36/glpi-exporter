use crate::date_util;
use crate::i18n::T;
use crate::models::{AggregatedSoftware, FilterState};
use eframe::egui;
use std::collections::HashSet;

pub fn show(
    ui: &mut egui::Ui,
    filters: &mut FilterState,
    visible_data: &[AggregatedSoftware],
    selected: &mut HashSet<u64>,
    show_pc_panel: &mut bool,
    t: &T,
) -> bool {
    let mut changed = false;

    ui.horizontal(|ui| {
        ui.label(t.software_name);
        let r = ui.add(
            egui::TextEdit::singleline(&mut filters.software_name)
                .desired_width(200.0)
                .hint_text(t.search_hint),
        );
        if r.changed() {
            changed = true;
        }

        ui.add_space(10.0);
        ui.label(t.publisher);
        let r = ui.add(
            egui::TextEdit::singleline(&mut filters.publisher)
                .desired_width(200.0)
                .hint_text(t.search_hint),
        );
        if r.changed() {
            changed = true;
        }
    });

    ui.horizontal(|ui| {
        ui.label(t.min_hosts);
        let r = ui.add(
            egui::TextEdit::singleline(&mut filters.min_hosts)
                .desired_width(60.0)
                .hint_text("0"),
        );
        if r.changed() {
            filters.min_hosts.retain(|c| c.is_ascii_digit());
            changed = true;
        }

        ui.add_space(10.0);
        if ui
            .checkbox(&mut filters.recently_updated, t.updated_in_last)
            .changed()
        {
            changed = true;
        }
        let r = ui.add(
            egui::TextEdit::singleline(&mut filters.days)
                .desired_width(40.0)
                .hint_text("30"),
        );
        if r.changed() {
            filters.days.retain(|c| c.is_ascii_digit());
            changed = true;
        }
        ui.label(t.days);

        ui.add_space(10.0);
        ui.label(t.top_n);
        let r = ui.add(
            egui::TextEdit::singleline(&mut filters.top_n)
                .desired_width(60.0)
                .hint_text(t.all_hint),
        );
        if r.changed() {
            filters.top_n.retain(|c| c.is_ascii_digit());
            changed = true;
        }

        ui.add_space(10.0);
        if ui
            .checkbox(&mut filters.hide_os_defaults, t.hide_os_defaults)
            .on_hover_text(t.hide_os_defaults_tip)
            .changed()
        {
            changed = true;
        }

        ui.add_space(10.0);
        if ui.button(t.clear_filters).clicked() {
            *filters = FilterState::default();
            changed = true;
        }
    });

    ui.horizontal(|ui| {
        let sel_count = selected.len();
        if ui
            .checkbox(&mut filters.show_selected_only, t.show_selected_only)
            .on_hover_text(t.show_selected_only_tip)
            .changed()
        {
            changed = true;
        }

        ui.add_space(10.0);
        ui.label(format!("{} {}", sel_count, t.n_selected));

        ui.add_space(10.0);
        if ui
            .button(t.select_all_visible)
            .on_hover_text(t.select_all_visible_tip)
            .clicked()
        {
            for sw in visible_data {
                selected.insert(sw.software_id);
            }
        }

        if ui.button(t.deselect_all).clicked() {
            selected.clear();
            if filters.show_selected_only {
                filters.show_selected_only = false;
                changed = true;
            }
        }

        ui.add_space(10.0);
        ui.separator();
        ui.add_space(10.0);
        let pc_label = if *show_pc_panel { t.hide_pcs } else { t.show_pcs };
        if ui
            .add_enabled(!selected.is_empty(), egui::Button::new(pc_label))
            .on_hover_text(t.show_pcs_tip)
            .clicked()
        {
            *show_pc_panel = !*show_pc_panel;
        }
    });

    changed
}

// ── OS default detection ──────────────────────────────────────────────────

fn is_os_default(name: &str, publisher: &str) -> bool {
    let lower = name.to_lowercase();
    let pub_lower = publisher.to_lowercase();

    // 0) Office allowlist -- never hide these
    if is_office_product(&lower) {
        return false;
    }

    // 1) GUID-style names (e.g. "1527c705-839a-4832-9118-54d4Bd6a0c89")
    if looks_like_guid(&lower) {
        return true;
    }

    // 2) KBxxxxxxx Windows updates
    if lower.starts_with("kb") && lower.len() > 2 && lower.as_bytes()[2].is_ascii_digit() {
        return true;
    }

    // 3) UWP package-style names: "Something.Something" from Microsoft
    if is_uwp_package_name(&lower) {
        return true;
    }

    // 4) Prefix matches
    for prefix in OS_DEFAULT_PREFIXES {
        if lower.starts_with(prefix) {
            return true;
        }
    }

    // 5) Substring matches
    for pattern in OS_DEFAULT_CONTAINS {
        if lower.contains(pattern) {
            return true;
        }
    }

    // 6) Exact matches
    for exact in OS_DEFAULT_EXACT {
        if lower == *exact {
            return true;
        }
    }

    // 7) Microsoft Corporation publisher + Hungarian/system component fragments
    if pub_lower == "microsoft corporation" || pub_lower == "microsoft corp." {
        for frag in MS_CORP_FRAGMENTS {
            if lower.contains(frag) {
                return true;
            }
        }
    }

    false
}

/// Office products that should always be shown.
fn is_office_product(lower: &str) -> bool {
    for prefix in OFFICE_ALLOW_PREFIXES {
        if lower.starts_with(prefix) {
            return true;
        }
    }
    for kw in OFFICE_ALLOW_CONTAINS {
        if lower.contains(kw) {
            return true;
        }
    }
    false
}

const OFFICE_ALLOW_PREFIXES: &[&str] = &[
    "microsoft office",
    "microsoft 365",
    "microsoft word",
    "microsoft excel",
    "microsoft powerpoint",
    "microsoft access",
    "microsoft publisher",
    "microsoft visio",
    "microsoft project",
    "microsoft teams",
    "microsoft outlook",
    "microsoft onenote",
    "microsoft sharepoint",
    "microsoft infopath",
    "microsoft lync",
    "microsoft skype for business",
    "office 16",
    "office 15",
    "office 14",
    "outlook",
    "onenote",
];

const OFFICE_ALLOW_CONTAINS: &[&str] = &[
    "office professional",
    "office standard",
    "office home",
    "office business",
    "office proplus",
    "office 365",
    "office ltsc",
    "office mondo",
    "office plus",
];

/// Detect UWP/AppX package-style names.
/// Pattern: "Word.Word" where both parts start with a letter.
/// Also catches: WindowsAppRuntime.1.7, Microsoft.UI.Xaml 2.8, Clipchamp.Clipchamp
fn is_uwp_package_name(lower: &str) -> bool {
    let base = lower.split_whitespace().next().unwrap_or(lower);

    if let Some(dot_pos) = base.find('.') {
        if dot_pos > 0 && dot_pos < base.len() - 1 {
            let before = &base[..dot_pos];
            let after = &base[dot_pos + 1..];
            let before_is_word = before.chars().next().map_or(false, |c| c.is_alphabetic())
                && before.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_');
            let after_starts_alpha_or_digit =
                after.chars().next().map_or(false, |c| c.is_alphanumeric());

            if before_is_word && after_starts_alpha_or_digit {
                let known_uwp_prefixes = [
                    "microsoft.",
                    "microsoftcorporationii.",
                    "windows",
                    "clipchamp.",
                    "appup.",
                    "realtek.",
                    "nvidia.",
                    "intel.",
                    "dolby.",
                    "disney.",
                    "amazon.",
                    "spotify.",
                    "ad2f1837.", // Windows Store internal
                ];
                for prefix in known_uwp_prefixes {
                    if base.starts_with(prefix) {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// Detect GUID-style names: 8-4-4-4-12 hex pattern.
fn looks_like_guid(s: &str) -> bool {
    let s = s.trim();
    let candidate = match s.get(..36) {
        Some(c) => c,
        None => return false,
    };
    let parts: Vec<&str> = candidate.split('-').collect();
    if parts.len() != 5 {
        return false;
    }
    let expected = [8, 4, 4, 4, 12];
    for (part, &len) in parts.iter().zip(expected.iter()) {
        if part.len() != len || !part.chars().all(|c| c.is_ascii_hexdigit()) {
            return false;
        }
    }
    true
}

/// Names starting with any of these (case-insensitive) are OS defaults.
const OS_DEFAULT_PREFIXES: &[&str] = &[
    // Microsoft / Windows English
    "microsoft visual c++",
    "microsoft .net",
    "microsoft-windows",
    "microsoft windows",
    "microsoft update",
    "microsoft edge",
    "microsoft onedrive",
    "microsoft policy platform",
    "microsoft report viewer",
    "microsoft silverlight",
    "microsoft sql server",
    "microsoft system clr types",
    "microsoft analysis services",
    "microsoft vsix",
    "microsoft solitaire",
    "microsoft tips",
    "microsoft people",
    "microsoft photos",
    "microsoft store",
    "microsoft to do",
    "microsoft 365 (office)",
    "microsoft sticky notes",
    "microsoft pay",
    "microsoft whiteboard",
    "microsoft your phone",
    "microsoft get help",
    "microsoft feedback hub",
    "microsoft bing",
    "microsoft intune",
    "microsoft sec",
    "microsoft-tartalom",
    "microsoft getstarted",
    "msxml",
    "msmq",
    "vs_",
    // Windows prefixes
    "windows sdk",
    "windows software development kit",
    "windows app certification kit",
    "windows shell experience",
    "windows defender",
    "windows security",
    "windows terminal",
    "windows calculator",
    "windows camera",
    "windows clock",
    "windows maps",
    "windows media player",
    "windows notepad",
    "windows voice recorder",
    "windows alarms",
    "windows phone",
    "windows mail",
    "windows feedback",
    "windows store",
    "windows package manager",
    "windows rendszerfel",
    "windows web experience",
    "windows subsystem",
    "windows-szolgáltatás",
    "windowsappruntime",
    // DirectX / runtimes
    "directx",
    "microsoft directx",
    // Internet Explorer
    "internet explorer",
    // Xbox
    "xbox",
    // Cortana
    "cortana",
    // Skype (preinstalled)
    "skype",
    // UDK
    "udk package",
    // Snipping
    "snipping tool",
    // Paint
    "paint",
    // Driver/runtime helpers
    "appup.",
    // Hungarian Windows component prefixes
    "a windows",
    "gépház",
    "fájlkezelő",
    "narrátor",
    "számológép",
    "kamera",
    "képernyővágó",
    "térképek",
    "időjárás",
    "e-mail és fiókok",
    "saját fiók",
    "munkahelyi vagy iskolai fiók",
    "hitelesít",
    "mappajavaslatok",
    "alkalmazásfeloldó",
    "eszköz biztonságos",
    "asztali alkalmazás webes",
    "dedikált hozzáférés",
    "hálózati csatlakozási",
    "vizsga",
    "pinningconfirmationdialog",
    "capturepicker",
    "biorollezés",
    "rendszer-visszaállítás",
    "contactsupport",
    "lockapp",
    "shellexperiencehost",
    "startmenuexperiencehost",
    "searchapp",
    "secureas",
    "win32webviewhost",
    "cbspreview",
    "environmentmanager",
    "assignedaccess",
    "parentalcontrols",
    "fileexplorer",
    "narratorquickstart",
    "oobenetwork",
    "solitaire",
    "adobe refresh manager",
];

/// Names containing any of these substrings are OS defaults.
const OS_DEFAULT_CONTAINS: &[&str] = &[
    "redistributable",
    "security update for",
    "update for microsoft",
    "hotfix for",
    "definition update for",
    "service pack",
    "nyelvi csomag",
    "language pack",
    "input method",
    "beviteli mód",
    "click-to-run",
    "&#38;",
    "élménycsomag",
];

/// Exact name matches (case-insensitive).
const OS_DEFAULT_EXACT: &[&str] = &[
    "skype",
    "paint",
    "notepad",
    "calculator",
    "camera",
    "photos",
    "maps",
    "clock",
    "gépház",
    "fájlkezelő",
    "narrátor",
    "vizsga",
    "számológép",
    "kamera",
    "térképek",
    "időjárás",
    "fényképek",
    "riasztók és óra",
    "hangfelvétel",
    "kapcsolatok",
];

/// When publisher is Microsoft Corporation, these fragments in the name trigger filtering.
const MS_CORP_FRAGMENTS: &[&str] = &[
    "párbeszédpanel",
    "adatfolyam",
    "beállítás",
    "feloldó",
    "zárolás",
    "képernyő",
    "előnézet",
    "rendszerfel",
    "eltávolítás",
    "megjelenítő",
    "csatlakozás",
    "tartalom",
    "smartscreen",
    "experience",
    "webview",
    "confirmation",
    "picker",
    "capture",
    "pinning",
    "resolver",
    "lockscreen",
    "game ui",
    "game bar",
    "game callable",
    "identity provider",
    "speech to text",
    "input app",
    "shellhost",
    "immersive",
    "appinstaller",
    "extensibility component",
    "licensing component",
    "appruntime",
    "sechealth",
    "3dviewer",
    "3d viewer",
    "refresh manager",
];

pub fn apply_filters(
    data: &[AggregatedSoftware],
    filters: &FilterState,
    selected: &HashSet<u64>,
) -> Vec<AggregatedSoftware> {
    let name_lower = filters.software_name.to_lowercase();
    let pub_lower = filters.publisher.to_lowercase();
    let min_hosts: usize = filters.min_hosts.parse().unwrap_or(0);
    let days: i64 = filters.days.parse().unwrap_or(30);
    let top_n: Option<usize> = if filters.top_n.is_empty() {
        None
    } else {
        filters.top_n.parse().ok()
    };

    let now = chrono::Local::now().naive_local().date();

    let mut filtered: Vec<_> = data
        .iter()
        .filter(|sw| {
            if filters.show_selected_only && !selected.contains(&sw.software_id) {
                return false;
            }
            if filters.hide_os_defaults && is_os_default(&sw.name, &sw.publisher) {
                return false;
            }
            if !name_lower.is_empty() && !sw.name.to_lowercase().contains(&name_lower) {
                return false;
            }
            if !pub_lower.is_empty() && !sw.publisher.to_lowercase().contains(&pub_lower) {
                return false;
            }
            if sw.total_host_count < min_hosts {
                return false;
            }
            if filters.recently_updated && !date_util::is_recent(&sw.last_updated, now, days) {
                return false;
            }
            true
        })
        .cloned()
        .collect();

    if let Some(n) = top_n {
        filtered.truncate(n);
    }

    filtered
}
