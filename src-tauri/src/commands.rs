use anyhow::Context;
use tauri::State;

use crate::{
    models::{
        default_workflows, AppBootstrap, AppSettings, AssistantTurnRequest, AssistantTurnResult,
        ExportResult, OutputFormat, RuntimeActionResult, RuntimeStatus, SaveDocumentRequest,
        WorkspaceSnapshot,
    },
    AppState,
};

const DOCUMENT_CONFLICT_PREFIX: &str = "document_conflict:";

fn stringify_error(error: anyhow::Error) -> String {
    error.to_string()
}

fn document_conflict(message: &str) -> String {
    format!("{DOCUMENT_CONFLICT_PREFIX} {message}")
}

#[tauri::command]
pub async fn bootstrap(state: State<'_, AppState>) -> Result<AppBootstrap, String> {
    let bootstrap = state.storage.bootstrap().map_err(stringify_error)?;
    let runtime_status = state.agent.runtime_status(&bootstrap.settings).await;

    Ok(AppBootstrap {
        documents: bootstrap.workspace.documents,
        active_document: bootstrap.workspace.active_document,
        runtime_status,
        settings: bootstrap.settings,
        workflow_modules: default_workflows(),
        recovery_notices: bootstrap.workspace.recovery_notices,
    })
}

#[tauri::command]
pub async fn create_document(
    title: Option<String>,
    state: State<'_, AppState>,
) -> Result<WorkspaceSnapshot, String> {
    state
        .storage
        .create_document(title)
        .map_err(stringify_error)
}

#[tauri::command]
pub async fn open_document(
    document_id: String,
    state: State<'_, AppState>,
) -> Result<WorkspaceSnapshot, String> {
    state
        .storage
        .open_document(&document_id)
        .map_err(stringify_error)
}

#[tauri::command]
pub async fn save_document(
    request: SaveDocumentRequest,
    state: State<'_, AppState>,
) -> Result<crate::models::StoredDocument, String> {
    state
        .storage
        .save_document(request)
        .map_err(stringify_error)
}

#[tauri::command]
pub async fn refresh_runtime_status(state: State<'_, AppState>) -> Result<RuntimeStatus, String> {
    let settings = state.storage.settings().map_err(stringify_error)?;
    Ok(state.agent.runtime_status(&settings).await)
}

#[tauri::command]
pub async fn send_assistant_turn(
    request: AssistantTurnRequest,
    state: State<'_, AppState>,
) -> Result<AssistantTurnResult, String> {
    let _turn_guard = state.turn_coordinator.lock(&request.document_id).await;
    let settings = state.storage.settings().map_err(stringify_error)?;
    let document = state
        .storage
        .document(&request.document_id)
        .map_err(stringify_error)?;

    if document.content_revision != request.document_revision {
        return Err(document_conflict(
            "Dokumentet ble endret før AI-forespørselen kunne starte. Oppdater dokumentet og prøv igjen.",
        ));
    }

    let (assistant_reply, editor_action, thread_id) = state
        .agent
        .run_assistant_turn(&state.storage, &settings, &request, &document)
        .await
        .map_err(stringify_error)?;

    let user_prompt = request
        .quick_action
        .clone()
        .map(|action| format!("{action}: {}", request.prompt))
        .unwrap_or_else(|| request.prompt.clone());

    let current_document = state
        .storage
        .document(&request.document_id)
        .map_err(stringify_error)?;
    if current_document.content_revision != request.document_revision {
        return Err(document_conflict(
            "Dokumentet ble endret mens AI jobbet. Dokumentet er oppdatert, og du kan sende forespørselen på nytt.",
        ));
    }

    let updated_document = state
        .storage
        .append_turn_result(
            &request.document_id,
            &user_prompt,
            &assistant_reply,
            Some(thread_id),
        )
        .map_err(stringify_error)?;

    let runtime_status = state.agent.runtime_status(&settings).await;

    Ok(AssistantTurnResult {
        document: updated_document,
        runtime_status,
        assistant_reply,
        editor_action,
    })
}

async fn runtime_action_payload(
    state: State<'_, AppState>,
    message: String,
    settings: AppSettings,
) -> Result<RuntimeActionResult, String> {
    let runtime_status = state.agent.runtime_status(&settings).await;
    Ok(RuntimeActionResult {
        message,
        runtime_status,
        settings,
    })
}

#[tauri::command]
pub async fn prepare_local_ai(state: State<'_, AppState>) -> Result<RuntimeActionResult, String> {
    let settings = state.storage.settings().map_err(stringify_error)?;
    let message = state
        .agent
        .prepare_local_ai(&settings)
        .await
        .map_err(stringify_error)?;
    runtime_action_payload(state, message, settings).await
}

#[tauri::command]
pub async fn repair_local_ai(state: State<'_, AppState>) -> Result<RuntimeActionResult, String> {
    let settings = state.storage.settings().map_err(stringify_error)?;
    let message = state
        .agent
        .repair_local_ai(&settings)
        .await
        .map_err(stringify_error)?;
    runtime_action_payload(state, message, settings).await
}

#[tauri::command]
pub async fn activate_model(
    model_id: String,
    state: State<'_, AppState>,
) -> Result<RuntimeActionResult, String> {
    state
        .agent
        .activate_model(&model_id)
        .await
        .map_err(stringify_error)?;

    let settings = state
        .storage
        .save_settings(AppSettings {
            selected_model: model_id,
            preferred_tone: state
                .storage
                .settings()
                .map_err(stringify_error)?
                .preferred_tone,
        })
        .map_err(stringify_error)?;

    runtime_action_payload(
        state,
        format!("Byttet til {}", settings.selected_model),
        settings,
    )
    .await
}

#[tauri::command]
pub async fn export_document(
    document_id: String,
    format: OutputFormat,
    state: State<'_, AppState>,
) -> Result<ExportResult, String> {
    state
        .storage
        .export_document(&document_id, format)
        .map_err(stringify_error)
}

#[tauri::command]
pub async fn save_settings(
    settings: AppSettings,
    state: State<'_, AppState>,
) -> Result<AppSettings, String> {
    state
        .storage
        .save_settings(settings)
        .with_context(|| "Kunne ikke lagre appinnstillinger")
        .map_err(stringify_error)
}
