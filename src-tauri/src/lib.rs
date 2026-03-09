mod agent;
mod commands;
mod export;
mod models;
mod storage;

use anyhow::{Context, Result};
use tauri::Manager;
use tokio::sync::{Mutex as AsyncMutex, OwnedMutexGuard};

use std::{collections::HashMap, sync::Arc};

use crate::{agent::AgentService, storage::StorageService};

pub struct AppState {
    pub storage: StorageService,
    pub agent: AgentService,
    pub turn_coordinator: TurnCoordinator,
}

pub struct TurnCoordinator {
    gates: AsyncMutex<HashMap<String, Arc<AsyncMutex<()>>>>,
}

impl TurnCoordinator {
    fn new() -> Self {
        Self {
            gates: AsyncMutex::new(HashMap::new()),
        }
    }

    pub async fn lock(&self, document_id: &str) -> OwnedMutexGuard<()> {
        let gate = {
            let mut gates = self.gates.lock().await;
            gates
                .entry(document_id.to_string())
                .or_insert_with(|| Arc::new(AsyncMutex::new(())))
                .clone()
        };

        gate.lock_owned().await
    }
}

fn build_state(app: &tauri::AppHandle) -> Result<AppState> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    let resource_root = resolve_resource_root(app)?;
    let app_version = app.package_info().version.to_string();

    std::fs::create_dir_all(&app_data_dir)?;

    Ok(AppState {
        storage: StorageService::new(app_data_dir.clone())?,
        agent: AgentService::new(app_data_dir, resource_root, app_version),
        turn_coordinator: TurnCoordinator::new(),
    })
}

fn resolve_resource_root(app: &tauri::AppHandle) -> Result<std::path::PathBuf> {
    let mut candidates = Vec::new();

    if let Ok(resource_dir) = app.path().resource_dir() {
        candidates.push(resource_dir);
    }

    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("src-tauri").join("resources"));
    }

    candidates
        .into_iter()
        .find_map(|candidate| {
            if candidate.join("embedded-runtime/manifest.json").exists() {
                Some(candidate)
            } else if candidate
                .join("resources")
                .join("embedded-runtime/manifest.json")
                .exists()
            {
                Some(candidate.join("resources"))
            } else {
                None
            }
        })
        .context("Fant ikke bundlet runtime-ressursmappe")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let state = build_state(&app.handle())?;
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::bootstrap,
            commands::create_document,
            commands::open_document,
            commands::save_document,
            commands::refresh_runtime_status,
            commands::send_assistant_turn,
            commands::prepare_local_ai,
            commands::repair_local_ai,
            commands::activate_model,
            commands::export_document,
            commands::save_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running lokal skriveflate");
}
