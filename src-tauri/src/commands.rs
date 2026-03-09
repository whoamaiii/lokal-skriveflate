use anyhow::Context;
use tauri::State;
use tracing::{info, warn};
use uuid::Uuid;

use crate::{
    agent::codex_bridge::CodexBridgeError,
    models::{
        AppBootstrap, AppSettings, AssistantTurnRequest, AssistantTurnResult, CommandError,
        CommandErrorCode, ExportResult, OutputFormat, RuntimeActionResult, RuntimeStatus,
        SaveDocumentRequest, WorkspaceSnapshot,
    },
    AppState,
};

type CommandResult<T> = Result<T, CommandError>;

fn command_error(
    code: CommandErrorCode,
    message: impl Into<String>,
    retryable: bool,
    action: Option<&str>,
) -> CommandError {
    CommandError {
        code,
        message: message.into(),
        retryable,
        action: action.map(|value| value.to_string()),
    }
}

fn storage_error(error: anyhow::Error) -> CommandError {
    command_error(
        CommandErrorCode::StorageError,
        error.to_string(),
        true,
        Some("retry"),
    )
}

fn export_error(error: anyhow::Error) -> CommandError {
    command_error(
        CommandErrorCode::ExportFailed,
        error.to_string(),
        true,
        Some("retry"),
    )
}

fn runtime_error(error: anyhow::Error) -> CommandError {
    let message = error.to_string();

    if message.contains("sjekksummen")
        || message.contains("reparasjon")
        || message.contains("reparert")
    {
        return command_error(
            CommandErrorCode::RuntimeRepairRequired,
            message,
            true,
            Some("repair_local_ai"),
        );
    }

    if message.contains("ikke klargjort")
        || message.contains("ikke installert")
        || message.contains("ikke klar")
    {
        return command_error(
            CommandErrorCode::RuntimeNotReady,
            message,
            true,
            Some("prepare_local_ai"),
        );
    }

    command_error(
        CommandErrorCode::RuntimeStartFailed,
        message,
        true,
        Some("repair_local_ai"),
    )
}

fn assistant_error(error: anyhow::Error) -> CommandError {
    if let Some(bridge_error) = error.downcast_ref::<CodexBridgeError>() {
        return match bridge_error {
            CodexBridgeError::Timeout(_) => command_error(
                CommandErrorCode::AssistantTimeout,
                bridge_error.to_string(),
                true,
                Some("retry"),
            ),
            _ => command_error(
                CommandErrorCode::AssistantBridgeError,
                bridge_error.to_string(),
                true,
                Some("retry"),
            ),
        };
    }

    let message = error.to_string();

    if message.contains("Timeout") {
        return command_error(
            CommandErrorCode::AssistantTimeout,
            message,
            true,
            Some("retry"),
        );
    }

    if message.contains("Codex")
        || message.contains("codex")
        || message.contains("thread/")
        || message.contains("turn/")
    {
        return command_error(
            CommandErrorCode::AssistantBridgeError,
            message,
            true,
            Some("retry"),
        );
    }

    if message.contains("Lokal runtime") || message.contains("modell") {
        return runtime_error(anyhow::anyhow!(message));
    }

    command_error(CommandErrorCode::Unknown, message, false, None)
}

fn document_conflict(message: &str) -> CommandError {
    command_error(
        CommandErrorCode::DocumentConflict,
        message,
        true,
        Some("reload_document"),
    )
}

#[tauri::command]
pub async fn bootstrap(state: State<'_, AppState>) -> CommandResult<AppBootstrap> {
    let bootstrap = state.storage.bootstrap().map_err(storage_error)?;
    let runtime_status = state.agent.runtime_status(&bootstrap.settings).await;

    Ok(AppBootstrap {
        documents: bootstrap.workspace.documents,
        active_document: bootstrap.workspace.active_document,
        runtime_status,
        settings: bootstrap.settings,
        recovery_notices: bootstrap.workspace.recovery_notices,
    })
}

#[tauri::command]
pub async fn create_document(
    title: Option<String>,
    state: State<'_, AppState>,
) -> CommandResult<WorkspaceSnapshot> {
    state
        .storage
        .create_document(title)
        .map_err(storage_error)
}

#[tauri::command]
pub async fn open_document(
    document_id: String,
    state: State<'_, AppState>,
) -> CommandResult<WorkspaceSnapshot> {
    state
        .storage
        .open_document(&document_id)
        .map_err(storage_error)
}

#[tauri::command]
pub async fn save_document(
    request: SaveDocumentRequest,
    state: State<'_, AppState>,
) -> CommandResult<crate::models::StoredDocument> {
    state
        .storage
        .save_document(request)
        .map_err(storage_error)
}

#[tauri::command]
pub async fn refresh_runtime_status(state: State<'_, AppState>) -> CommandResult<RuntimeStatus> {
    let settings = state.storage.settings().map_err(storage_error)?;
    Ok(state.agent.runtime_status(&settings).await)
}

#[tauri::command]
pub async fn send_assistant_turn(
    request: AssistantTurnRequest,
    state: State<'_, AppState>,
) -> CommandResult<AssistantTurnResult> {
    let turn_id = Uuid::new_v4().to_string();
    info!(turn_id = %turn_id, document_id = %request.document_id, "starter lokal assistent-turn");
    let _turn_guard = state.turn_coordinator.lock(&request.document_id).await;
    let settings = state.storage.settings().map_err(storage_error)?;
    let document = state
        .storage
        .document(&request.document_id)
        .map_err(storage_error)?;

    if document.content_revision != request.document_revision {
        warn!(turn_id = %turn_id, document_id = %request.document_id, "avbrøt turn på grunn av dokumentkonflikt før start");
        return Err(document_conflict(
            "Dokumentet ble endret før AI-forespørselen kunne starte. Oppdater dokumentet og prøv igjen.",
        ));
    }

    if document.content_revision != request.document_revision {
        return Err(document_conflict(
            "Dokumentet ble endret før AI-forespørselen kunne starte. Oppdater dokumentet og prøv igjen.",
        ));
    }

    let (assistant_reply, editor_action, thread_id) = state
        .agent
        .run_assistant_turn(&state.storage, &settings, &request, &document)
        .await
        .map_err(assistant_error)?;

    let user_prompt = request
        .quick_action
        .clone()
        .map(|action| format!("{action}: {}", request.prompt))
        .unwrap_or_else(|| request.prompt.clone());

    let current_document = state
        .storage
        .document(&request.document_id)
        .map_err(storage_error)?;
    if current_document.content_revision != request.document_revision {
        warn!(turn_id = %turn_id, document_id = %request.document_id, "avbrøt turn på grunn av dokumentkonflikt etter svar");
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
        .map_err(storage_error)?;

    let runtime_status = state.agent.runtime_status(&settings).await;
    info!(turn_id = %turn_id, document_id = %request.document_id, "lokal assistent-turn fullført");

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
) -> CommandResult<RuntimeActionResult> {
    let runtime_status = state.agent.runtime_status(&settings).await;
    Ok(RuntimeActionResult {
        message,
        runtime_status,
        settings,
    })
}

#[tauri::command]
pub async fn prepare_local_ai(state: State<'_, AppState>) -> CommandResult<RuntimeActionResult> {
    let settings = state.storage.settings().map_err(storage_error)?;
    let message = state
        .agent
        .prepare_local_ai(&settings)
        .await
        .map_err(runtime_error)?;
    runtime_action_payload(state, message, settings).await
}

#[tauri::command]
pub async fn repair_local_ai(state: State<'_, AppState>) -> CommandResult<RuntimeActionResult> {
    let settings = state.storage.settings().map_err(storage_error)?;
    let message = state
        .agent
        .repair_local_ai(&settings)
        .await
        .map_err(runtime_error)?;
    runtime_action_payload(state, message, settings).await
}

#[tauri::command]
pub async fn activate_model(
    model_id: String,
    state: State<'_, AppState>,
) -> CommandResult<RuntimeActionResult> {
    state
        .agent
        .activate_model(&model_id)
        .await
        .map_err(runtime_error)?;

    let settings = state
        .storage
        .save_settings(AppSettings {
            selected_model: model_id,
            preferred_tone: state
                .storage
                .settings()
                .map_err(storage_error)?
                .preferred_tone,
        })
        .map_err(storage_error)?;

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
) -> CommandResult<ExportResult> {
    state
        .storage
        .export_document(&document_id, format)
        .map_err(export_error)
}

#[tauri::command]
pub async fn save_settings(
    settings: AppSettings,
    state: State<'_, AppState>,
) -> CommandResult<AppSettings> {
    state
        .storage
        .save_settings(settings)
        .with_context(|| "Kunne ikke lagre appinnstillinger")
        .map_err(storage_error)
}
