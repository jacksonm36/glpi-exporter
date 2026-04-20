use crate::models::*;
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue};
use serde_json::Value;
use std::sync::mpsc::Sender;

const DEFAULT_PAGE_SIZE: usize = 500;

pub struct GlpiClient {
    base_url: String,
    client: Client,
    session_token: Option<String>,
    app_token: Option<String>,
}

impl GlpiClient {
    pub fn new(
        base_url: &str,
        app_token: Option<&str>,
        accept_invalid_certs: bool,
    ) -> Result<Self, String> {
        let client = Client::builder()
            .danger_accept_invalid_certs(accept_invalid_certs)
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .map_err(|e| format!("Failed to create HTTP client: {e}"))?;

        let mut url = base_url.trim_end_matches('/').to_string();
        if !url.contains("/apirest.php") {
            url.push_str("/apirest.php");
        }

        Ok(Self {
            base_url: url,
            client,
            session_token: None,
            app_token: app_token.map(|s| s.to_string()),
        })
    }

    fn build_headers(&self) -> Result<HeaderMap, String> {
        let mut headers = HeaderMap::new();
        if let Some(ref token) = self.session_token {
            headers.insert(
                "Session-Token",
                HeaderValue::from_str(token)
                    .map_err(|e| format!("Invalid session token characters: {e}"))?,
            );
        }
        if let Some(ref app) = self.app_token {
            if !app.is_empty() {
                headers.insert(
                    "App-Token",
                    HeaderValue::from_str(app)
                        .map_err(|e| format!("Invalid app token characters: {e}"))?,
                );
            }
        }
        headers.insert("Content-Type", HeaderValue::from_static("application/json"));
        Ok(headers)
    }

    pub fn init_session(&mut self, user_token: &str) -> Result<(), String> {
        let mut headers = self.build_headers()?;
        headers.insert(
            "Authorization",
            HeaderValue::from_str(&format!("user_token {user_token}"))
                .map_err(|e| format!("Invalid user token characters: {e}"))?,
        );

        let url = format!("{}/initSession", self.base_url);
        let resp = self
            .client
            .get(&url)
            .headers(headers)
            .send()
            .map_err(|e| format!("Connection failed: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().unwrap_or_default();
            return Err(format!("Auth failed ({status}): {body}"));
        }

        let json: serde_json::Value = resp.json().map_err(|e| format!("Bad response: {e}"))?;
        let token = json["session_token"]
            .as_str()
            .ok_or("No session_token in response")?
            .to_string();

        self.session_token = Some(token);
        Ok(())
    }

    pub fn kill_session(&self) {
        let url = format!("{}/killSession", self.base_url);
        if let Ok(headers) = self.build_headers() {
            let _ = self.client.get(&url).headers(headers).send();
        }
    }

    /// Parse the `Accept-Range` header to discover the server's max page size.
    /// Format: `itemtype max` e.g. `Software 990`
    fn parse_accept_range(resp: &reqwest::blocking::Response) -> Option<usize> {
        resp.headers()
            .get("Accept-Range")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| {
                let parts: Vec<&str> = s.split_whitespace().collect();
                if parts.len() >= 2 {
                    parts[1].parse::<usize>().ok()
                } else {
                    None
                }
            })
    }

    /// Parse `Content-Range` header to extract total count.
    /// Format: `offset-limit/count` e.g. `0-49/200`
    fn parse_content_range(resp: &reqwest::blocking::Response) -> Option<usize> {
        resp.headers()
            .get("Content-Range")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| {
                s.find('/').and_then(|slash| s[slash + 1..].parse::<usize>().ok())
            })
    }

    fn fetch_paginated<T: serde::de::DeserializeOwned>(
        &self,
        endpoint: &str,
        status_tx: &Sender<FetchStatus>,
        make_status: fn(usize, Option<usize>) -> FetchStatus,
        extra_params: &[(&str, &str)],
    ) -> Result<Vec<T>, String> {
        let mut page_size: usize = DEFAULT_PAGE_SIZE;
        let mut all_items: Vec<T> = Vec::new();
        let mut offset: usize = 0;
        let mut total: Option<usize> = None;

        loop {
            let range_val = format!("{}-{}", offset, offset + page_size - 1);
            let url = format!("{}/{endpoint}", self.base_url);

            let headers = self.build_headers()?;
            let mut req = self
                .client
                .get(&url)
                .headers(headers)
                .query(&[("range", &range_val)]);

            for (k, v) in extra_params {
                req = req.query(&[(*k, *v)]);
            }

            let resp = req.send().map_err(|e| format!("Fetch {endpoint} failed: {e}"))?;

            if let Some(server_max) = Self::parse_accept_range(&resp) {
                if server_max > 0 && server_max < page_size {
                    page_size = server_max;
                }
            }

            if let Some(t) = Self::parse_content_range(&resp) {
                total = Some(t);
            }

            let status_code = resp.status();
            if status_code.as_u16() == 400 {
                let body = resp.text().unwrap_or_default();
                if body.contains("ERROR_RANGE_EXCEED_TOTAL") {
                    break;
                }
                return Err(format!("API error for {endpoint}: {body}"));
            }

            if !status_code.is_success() && status_code.as_u16() != 206 {
                let body = resp.text().unwrap_or_default();
                return Err(format!("{endpoint} returned {status_code}: {body}"));
            }

            let items: Vec<T> = resp
                .json()
                .map_err(|e| format!("Parse {endpoint} failed: {e}"))?;

            let count = items.len();
            all_items.extend(items);

            let _ = status_tx.send(make_status(all_items.len(), total));

            if count < page_size {
                break;
            }
            offset += page_size;
        }

        Ok(all_items)
    }

    pub fn fetch_software(
        &self,
        tx: &Sender<FetchStatus>,
    ) -> Result<Vec<GlpiSoftware>, String> {
        self.fetch_paginated(
            "Software",
            tx,
            |done, total| FetchStatus::FetchingSoftware { done, total },
            &[("expand_dropdowns", "true")],
        )
    }

    pub fn fetch_software_versions(
        &self,
        tx: &Sender<FetchStatus>,
    ) -> Result<Vec<GlpiSoftwareVersion>, String> {
        self.fetch_paginated(
            "SoftwareVersion",
            tx,
            |done, total| FetchStatus::FetchingVersions { done, total },
            &[],
        )
    }

    pub fn fetch_item_software_versions(
        &self,
        tx: &Sender<FetchStatus>,
    ) -> Result<Vec<GlpiItemSoftwareVersion>, String> {
        self.fetch_paginated(
            "Item_SoftwareVersion",
            tx,
            |done, total| FetchStatus::FetchingInstallations { done, total },
            &[],
        )
    }

    pub fn fetch_software_licenses(
        &self,
        tx: &Sender<FetchStatus>,
    ) -> Result<Vec<GlpiSoftwareLicense>, String> {
        self.fetch_paginated(
            "SoftwareLicense",
            tx,
            |done, total| FetchStatus::FetchingLicenses { done, total },
            &[("expand_dropdowns", "true")],
        )
    }

    pub fn fetch_computers(
        &self,
        tx: &Sender<FetchStatus>,
    ) -> Result<Vec<GlpiComputer>, String> {
        self.fetch_paginated(
            "Computer",
            tx,
            |done, total| FetchStatus::FetchingComputers { done, total },
            &[("expand_dropdowns", "true")],
        )
    }

    pub fn fetch_agents(
        &self,
        tx: &Sender<FetchStatus>,
    ) -> Result<Vec<GlpiAgent>, String> {
        self.fetch_paginated(
            "Agent",
            tx,
            |done, total| FetchStatus::FetchingAgents { done, total },
            &[],
        )
    }

    pub fn fetch_computer_logs(&self, computer_id: u64) -> Result<Vec<GlpiLogEntry>, String> {
        let url = format!("{}/Computer/{computer_id}", self.base_url);
        let headers = self.build_headers()?;
        let resp = self
            .client
            .get(&url)
            .headers(headers)
            .query(&[("with_logs", "1")])
            .send()
            .map_err(|e| format!("Fetch Computer/{computer_id} logs failed: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().unwrap_or_default();
            return Err(format!("Computer/{computer_id} logs returned {status}: {body}"));
        }

        let json: serde_json::Value = resp
            .json()
            .map_err(|e| format!("Parse Computer/{computer_id} logs failed: {e}"))?;

        let logs_value = json.get("_logs").cloned().unwrap_or(serde_json::Value::Array(Vec::new()));

        let entries: Vec<GlpiLogEntry> = match logs_value {
            serde_json::Value::Array(arr) => {
                arr.into_iter()
                    .filter_map(|v| serde_json::from_value(v).ok())
                    .collect()
            }
            serde_json::Value::Object(map) => {
                map.into_values()
                    .filter_map(|v| serde_json::from_value(v).ok())
                    .collect()
            }
            _ => Vec::new(),
        };

        Ok(entries)
    }

    pub fn fetch_computer_by_id(&self, machine_id: u64) -> Result<GlpiComputer, String> {
        let url = format!("{}/Computer/{machine_id}", self.base_url);
        let headers = self.build_headers()?;
        let resp = self
            .client
            .get(&url)
            .headers(headers)
            .query(&[("expand_dropdowns", "true")])
            .send()
            .map_err(|e| format!("Fetch Computer/{machine_id} failed: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().unwrap_or_default();
            return Err(format!("Computer/{machine_id} returned {status}: {body}"));
        }

        resp.json()
            .map_err(|e| format!("Parse Computer/{machine_id} failed: {e}"))
    }

    pub fn fetch_windows_product_key_by_machine(&self, machine_id: u64) -> Result<Option<String>, String> {
        // Try multiple GLPI API shapes because this data moved across versions/APIs.
        let endpoints = [
            format!("Assets/Computer/{machine_id}/OSInstallation"), // HLAPI v2.2+
            format!("Computer/{machine_id}/Item_OperatingSystem"),  // legacy sub-item route
            format!("Computer/{machine_id}/Item_OperatingSystem/"), // some instances need trailing slash
            format!("Computer/{machine_id}"), // some instances expose OS fields on detail payload
        ];
        let mut last_error: Option<String> = None;

        for endpoint in &endpoints {
            match self.fetch_json_optional(
                endpoint,
                &[
                    ("range", "0-200"),
                    ("expand_dropdowns", "true"),
                    ("with_operatingsystems", "true"),
                ],
            ) {
                Ok(Some(payload)) => {
                    if let Some(found) = find_windows_key_in_value(&payload) {
                        return Ok(Some(found));
                    }
                }
                Ok(None) => {}
                Err(e) => last_error = Some(e),
            }
        }

        // Last fallback: some instances expose all Item_OperatingSystem rows.
        match self.fetch_json_optional(
            "Item_OperatingSystem",
            &[("range", "0-1000"), ("expand_dropdowns", "true")],
        ) {
            Ok(Some(payload)) => {
                if let Some(found) = find_windows_key_for_machine(&payload, machine_id) {
                    return Ok(Some(found));
                }
            }
            Ok(None) => {}
            Err(e) => last_error = Some(e),
        }

        if let Some(err) = last_error {
            return Err(err);
        }
        Ok(None)
    }
}

impl Drop for GlpiClient {
    fn drop(&mut self) {
        if self.session_token.is_some() {
            self.kill_session();
        }
    }
}

fn find_windows_key_in_value(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => {
            let trimmed = s.trim();
            if looks_like_windows_key(trimmed) {
                Some(trimmed.to_string())
            } else {
                None
            }
        }
        Value::Array(arr) => {
            for v in arr {
                if let Some(found) = find_windows_key_in_value(v) {
                    return Some(found);
                }
            }
            None
        }
        Value::Object(map) => {
            // Only search fields whose names are likely to carry OS activation keys.
            // Scanning all values unconditionally risks returning serial numbers, UUIDs,
            // asset tags, or other alphanumeric strings that happen to pass the key-format
            // check as a false Windows product key.
            let preferred_fields = [
                "serial",
                "serialnumber",
                "productid",
                "product_id",
                "product key",
                "license",
            ];
            for (k, v) in map {
                let key_lc = k.to_lowercase().replace('_', "").replace(' ', "");
                if preferred_fields.iter().any(|f| key_lc.contains(&f.replace(' ', ""))) {
                    if let Some(found) = find_windows_key_in_value(v) {
                        return Some(found);
                    }
                }
            }
            None
        }
        _ => None,
    }
}

fn find_windows_key_for_machine(value: &Value, machine_id: u64) -> Option<String> {
    match value {
        Value::Array(items) => {
            for item in items {
                if let Some(found) = find_windows_key_for_machine(item, machine_id) {
                    return Some(found);
                }
            }
            None
        }
        Value::Object(map) => {
            let itemtype_matches = map
                .get("itemtype")
                .and_then(|v| v.as_str())
                .map(|s| s.eq_ignore_ascii_case("Computer"))
                .unwrap_or(true);

            let items_id_matches = map
                .get("items_id")
                .and_then(value_as_u64)
                .map(|id| id == machine_id)
                .unwrap_or(false);

            if itemtype_matches && items_id_matches {
                if let Some(found) = find_windows_key_in_value(value) {
                    return Some(found);
                }
            }

            // Continue deep search for wrappers like { data: [...] }.
            for v in map.values() {
                if let Some(found) = find_windows_key_for_machine(v, machine_id) {
                    return Some(found);
                }
            }
            None
        }
        _ => None,
    }
}

fn value_as_u64(value: &Value) -> Option<u64> {
    match value {
        Value::Number(n) => n.as_u64(),
        Value::String(s) => s.trim().parse::<u64>().ok(),
        _ => None,
    }
}

fn looks_like_windows_key(value: &str) -> bool {
    let v = value.trim();
    if v.len() < 15 || v.len() > 80 {
        return false;
    }
    let has_hyphen = v.contains('-');
    let valid_chars = v.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
    let alnum_count = v.chars().filter(|c| c.is_ascii_alphanumeric()).count();
    has_hyphen && valid_chars && alnum_count >= 15
}

impl GlpiClient {
    fn fetch_json_optional(
        &self,
        endpoint: &str,
        params: &[(&str, &str)],
    ) -> Result<Option<Value>, String> {
        let url = format!("{}/{}", self.base_url, endpoint);
        let headers = self.build_headers()?;
        let mut req = self.client.get(&url).headers(headers);
        for (k, v) in params {
            req = req.query(&[(*k, *v)]);
        }
        let resp = req
            .send()
            .map_err(|e| format!("Fetch {endpoint} failed: {e}"))?;
        let status = resp.status();
        if status.as_u16() == 404 {
            return Ok(None);
        }
        if !(status.is_success() || status.as_u16() == 206) {
            let body = resp.text().unwrap_or_default();
            return Err(format!("{endpoint} returned {status}: {body}"));
        }
        let payload = resp
            .json::<Value>()
            .map_err(|e| format!("Parse {endpoint} failed: {e}"))?;
        Ok(Some(payload))
    }
}
