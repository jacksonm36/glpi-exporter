use crate::aggregator;
use crate::glpi_client::GlpiClient;
use crate::models::*;
use std::collections::HashMap;
use std::sync::mpsc::{Receiver, Sender};

pub enum WorkerRequest {
    FetchAll {
        url: String,
        user_token: String,
        app_token: String,
        accept_invalid_certs: bool,
    },
}

pub enum WorkerResponse {
    Status(FetchStatus),
    Data {
        software: Vec<AggregatedSoftware>,
        computers: HashMap<u64, ComputerInfo>,
    },
    Error(String),
}

pub fn spawn_worker(
    req_rx: Receiver<WorkerRequest>,
    resp_tx: Sender<WorkerResponse>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        while let Ok(request) = req_rx.recv() {
            match request {
                WorkerRequest::FetchAll {
                    url,
                    user_token,
                    app_token,
                    accept_invalid_certs,
                } => {
                    handle_fetch(&url, &user_token, &app_token, accept_invalid_certs, &resp_tx);
                }
            }
        }
    })
}

fn handle_fetch(
    url: &str,
    user_token: &str,
    app_token: &str,
    accept_invalid_certs: bool,
    resp_tx: &Sender<WorkerResponse>,
) {
    let _ = resp_tx.send(WorkerResponse::Status(FetchStatus::Connecting));

    let app_tok = if app_token.is_empty() {
        None
    } else {
        Some(app_token)
    };

    let mut client = match GlpiClient::new(url, app_tok, accept_invalid_certs) {
        Ok(c) => c,
        Err(e) => {
            let _ = resp_tx.send(WorkerResponse::Error(e));
            return;
        }
    };

    if let Err(e) = client.init_session(user_token) {
        let _ = resp_tx.send(WorkerResponse::Error(e));
        return;
    }

    let status_tx = {
        let resp_tx = resp_tx.clone();
        let (tx, rx) = std::sync::mpsc::channel::<FetchStatus>();
        std::thread::spawn(move || {
            while let Ok(status) = rx.recv() {
                let _ = resp_tx.send(WorkerResponse::Status(status));
            }
        });
        tx
    };

    let software = match client.fetch_software(&status_tx) {
        Ok(s) => s,
        Err(e) => {
            let _ = resp_tx.send(WorkerResponse::Error(e));
            return;
        }
    };

    let versions = match client.fetch_software_versions(&status_tx) {
        Ok(v) => v,
        Err(e) => {
            let _ = resp_tx.send(WorkerResponse::Error(e));
            return;
        }
    };

    let installations = match client.fetch_item_software_versions(&status_tx) {
        Ok(i) => i,
        Err(e) => {
            let _ = resp_tx.send(WorkerResponse::Error(e));
            return;
        }
    };

    let computers = match client.fetch_computers(&status_tx) {
        Ok(c) => c,
        Err(e) => {
            let _ = resp_tx.send(WorkerResponse::Error(e));
            return;
        }
    };

    let _ = resp_tx.send(WorkerResponse::Status(FetchStatus::Aggregating));

    let computer_map: HashMap<u64, ComputerInfo> = computers
        .into_iter()
        .map(|c| {
            let info = ComputerInfo {
                name: c.name,
                contact: c.contact.unwrap_or_default(),
            };
            (c.id, info)
        })
        .collect();

    let aggregated = aggregator::aggregate(&software, &versions, &installations);

    let total_hosts: usize = {
        let mut all_hosts = std::collections::HashSet::new();
        for sw in &aggregated {
            for h in &sw.host_ids {
                all_hosts.insert(*h);
            }
        }
        all_hosts.len()
    };

    let _ = resp_tx.send(WorkerResponse::Status(FetchStatus::Done {
        software_count: aggregated.len(),
        total_hosts,
    }));
    let _ = resp_tx.send(WorkerResponse::Data {
        software: aggregated,
        computers: computer_map,
    });
}
