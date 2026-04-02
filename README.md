# GLPI Software Inventory Explorer

A standalone Windows desktop application that connects to your [GLPI](https://glpi-project.org/) instance via its REST API, fetches the full software inventory, and lets you browse, filter, and export it -- all from a single `.exe` with no installer needed.

Built with Rust and [egui](https://github.com/emilk/egui) for a fast, native GUI.

## Features

- **Live GLPI connection** -- authenticates with your User Token (and optional App Token), fetches software, versions, installations, and computers with paginated requests
- **Sortable & filterable table** -- search by software name or publisher, filter by minimum host count, recently updated, top N, and hide default OS components (English + Hungarian Windows built-ins, UWP packages, GUIDs)
- **Expandable detail view** -- click any software row to reveal a tabbed panel with:
  - **Versions tab** -- every version of that software with host count, last install date, and recency indicator
  - **PCs tab** -- every computer that has the software installed, with the assigned user/contact
- **Selection & PC panel** -- check software with checkboxes, then open the side panel to see all PCs across your selections grouped by computer
- **Multi-format export** -- export the filtered table to **CSV**, **Excel (.xlsx)**, or **JSON** with a single click
- **Excel formatting** -- blue header row, green data rows, "Recently Updated" column shows the actual date for recent items
- **Bilingual UI** -- switch between English and Hungarian with one click (top-right button); choice is saved automatically
- **Persistent settings** -- connection URL, tokens, and language preference are saved in `config.json`; selected software IDs are saved in `selections.json`; both files live next to the `.exe`
- **Self-signed TLS support** -- optional checkbox to accept invalid certificates for internal GLPI servers

## Prerequisites

### To run the pre-built `.exe`

- Windows 10 or later (x64)

### To build from source

- [Rust](https://rustup.rs/) 1.75 or later (2021 edition)
- A C/C++ build toolchain (Visual Studio Build Tools on Windows)

## Building from source

```bash
git clone https://github.com/jacksonm36/glpi-exporter.git
cd glpi-exporter
cargo build --release
```

The compiled binary will be at `target/release/glpi-software-export.exe`.

## GLPI API setup

Before using the application you need to enable the GLPI REST API and generate tokens.

1. **Enable the API** -- in GLPI go to **Setup > General > API**, make sure the REST API is enabled and note your API URL (e.g. `https://glpi.yourcompany.com/apirest.php`)
2. **Generate a User Token** -- go to **Administration > Users**, select your user, open the **Settings** tab (or **Remote access keys**), and generate an **API token** (sometimes called "User token")
3. **Generate an App Token** *(optional but recommended)* -- in **Setup > General > API > API clients**, create or edit a client and copy the **Application Token**

## Tutorial

### 1. Launch the application

Double-click `glpi-software-export.exe`. The window opens with the connection panel at the top and an empty table in the center.

### 2. Connect to GLPI

| Field | What to enter |
|---|---|
| **GLPI URL** | Your GLPI base URL, e.g. `https://glpi.yourcompany.com` (the app appends `/apirest.php` automatically) |
| **User Token** | The API user token from your GLPI user profile |
| **App Token** | The application token from GLPI API settings (leave blank if not required) |

If your GLPI server uses a self-signed certificate, check **Accept invalid TLS certificates**.

Click **Connect & Fetch**. The status line shows progress as the app fetches:
1. Software catalog
2. Software versions
3. Installation records (which PC has which version)
4. Computer list (names and contacts)

Once complete, the table populates with all software sorted by host count (most installed first).

### 3. Filter the data

Use the filter bar below the connection panel:

| Filter | What it does |
|---|---|
| **Software Name** | Free-text search (case-insensitive substring match) |
| **Publisher** | Free-text search on the publisher/manufacturer field |
| **Min Hosts** | Only show software installed on at least N computers |
| **Updated in last __ days** | Only show software with an installation record within the last N days |
| **Top N** | Limit the table to the top N entries |
| **Hide OS defaults** | Remove default Windows components, UWP packages, system updates, runtimes, redistributables, and GUID-named entries (covers both English and Hungarian names) |
| **Clear Filters** | Reset all filters to default |

### 4. Explore software details

Click any software name (the **arrow** icon) to expand it. A panel appears with two tabs:

- **Versions** -- lists every version with its host count, last install date, and whether it's recent
- **PCs** -- lists every computer that has any version of this software, along with the assigned user/contact name

### 5. Select software and view PCs

- Use the **checkboxes** to select individual software entries
- **Select all visible** checks everything currently shown after filtering
- Click **Show PCs** to open a right-side panel listing all computers that have any of the selected software installed, grouped by PC with collapsible details

### 6. Export

Click one of the export buttons in the toolbar:

| Format | Details |
|---|---|
| **CSV** | Flat table with one row per software-version combination. Columns: Rank, Software Name, Publisher, Host Count, Version, Version Host Count, Last Updated, Recently Updated |
| **Excel** | Two sheets: **Summary** (one row per software, blue header, green data rows) and **Detailed Versions** (one row per version). The "Recently Updated" column shows the actual date for recent entries or "No" otherwise |
| **JSON** | Structured report with metadata (`generated_at`, `software_count`) and nested version arrays |

A native "Save As" dialog appears for each export. A green confirmation message shows the saved path.

### 7. Switch language

Click the **HU** / **EN** button in the top-right corner to toggle between Hungarian and English. The entire UI updates instantly and the preference is saved for next launch.

## Project structure

```
src/
  main.rs              -- entry point, window configuration
  app.rs               -- application state, UI layout orchestration
  config.rs            -- config.json and selections.json persistence
  i18n.rs              -- English and Hungarian UI string definitions
  models.rs            -- data structures (GLPI API models, aggregated models, filters, status)
  glpi_client.rs       -- GLPI REST API client with paginated fetching
  worker.rs            -- background thread for non-blocking API calls
  aggregator.rs        -- aggregates raw API data into per-software summaries
  date_util.rs         -- date parsing and recency comparison
  ui/
    mod.rs             -- UI module declarations
    connection_panel.rs -- URL/token inputs and connect button
    filter_panel.rs    -- search, filter controls, and OS-default filtering logic
    export_panel.rs    -- CSV/Excel/JSON export buttons
    software_table.rs  -- main data table with expandable version/PC detail tabs
    pc_panel.rs        -- right-side panel showing PCs for selected software
    status_bar.rs      -- bottom bar with counts
  export/
    mod.rs             -- export module declarations
    csv_export.rs      -- CSV file writer
    excel_export.rs    -- Excel file writer with formatting
    json_export.rs     -- JSON file writer
```

## Configuration files

The application creates two files next to the `.exe` (both are gitignored):

| File | Contents |
|---|---|
| `config.json` | GLPI URL, user token, app token, TLS setting, language preference |
| `selections.json` | Array of software IDs that you have checked |

**Security note:** Tokens are stored in plaintext. Keep the `.exe` directory access-controlled and do not share these files.

## Dependencies

All dependencies use permissive licenses (MIT and/or Apache 2.0) that are compatible with GPL v3.

| Crate | License | Purpose |
|---|---|---|
| `eframe` / `egui_extras` | MIT / Apache 2.0 | Native GUI framework |
| `reqwest` | MIT / Apache 2.0 | HTTP client (blocking, rustls-tls) |
| `serde` / `serde_json` | MIT / Apache 2.0 | JSON serialization |
| `csv` | MIT / Unlicense | CSV export |
| `rust_xlsxwriter` | MIT / Apache 2.0 | Excel export with formatting |
| `chrono` | MIT / Apache 2.0 | Date/time handling |
| `rfd` | MIT | Native file dialogs |
| `image` | MIT / Apache 2.0 | Image support (PNG) |

## License

This project is licensed under the **GNU General Public License v3.0 or later** (GPL-3.0-or-later).

See the [LICENSE](LICENSE) file for the full license text.

This project communicates with [GLPI](https://glpi-project.org/) (licensed under GPL v3) exclusively through its public REST API. No GLPI source code is included in or linked by this project.

### In short

- You are free to use, modify, and distribute this software
- If you distribute modified versions, you must also release the source code under GPL v3
- This software comes with no warranty

For the full terms, see the [GNU GPL v3](https://www.gnu.org/licenses/gpl-3.0.html).

## Acknowledgements

- [GLPI Project](https://glpi-project.org/) -- the open-source IT asset management platform this tool connects to (GPL v3)
- [egui](https://github.com/emilk/egui) -- the immediate-mode GUI library powering the interface (MIT / Apache 2.0)
