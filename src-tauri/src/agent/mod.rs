pub mod codex_bridge;
pub mod local_model;
pub mod responses_proxy;

use anyhow::{Context, Result};
use serde_json::{json, Value};

use crate::{
    models::{
        AppSettings, AssistantTurnRequest, EditorAction, RuntimeStatus, StoredDocument,
        StructuredAssistantResponse,
    },
    storage::StorageService,
};

use self::{
    codex_bridge::{CodexBridge, CodexTurnOutput},
    local_model::{EmbeddedLlamaCppProvider, LocalModelProvider},
};

pub struct AgentService {
    provider: EmbeddedLlamaCppProvider,
}

impl AgentService {
    pub fn new(
        app_data_root: std::path::PathBuf,
        resource_root: std::path::PathBuf,
        app_version: String,
    ) -> Self {
        Self {
            provider: EmbeddedLlamaCppProvider::new(app_data_root, resource_root, app_version),
        }
    }

    pub async fn runtime_status(&self, settings: &AppSettings) -> RuntimeStatus {
        self.provider.runtime_status(settings).await
    }

    pub async fn prepare_local_ai(&self, settings: &AppSettings) -> Result<String> {
        self.provider.prepare_runtime(settings).await
    }

    pub async fn repair_local_ai(&self, settings: &AppSettings) -> Result<String> {
        self.provider.repair_runtime(settings).await
    }

    pub async fn activate_model(&self, model: &str) -> Result<String> {
        self.provider.activate_model(model).await
    }

    pub async fn shutdown(&self) -> Result<()> {
        self.provider.shutdown().await
    }

    pub async fn run_assistant_turn(
        &self,
        storage: &StorageService,
        settings: &AppSettings,
        request: &AssistantTurnRequest,
        document: &StoredDocument,
    ) -> Result<(String, Option<EditorAction>, String)> {
        let runtime_endpoint = self.provider.runtime_endpoint(settings).await?;
        let prompt = build_prompt(request, document);
        let output_schema = response_schema();

        let CodexTurnOutput {
            thread_id,
            raw_response,
        } = CodexBridge::run_turn(
            &storage.codex_home_dir(),
            &settings.selected_model,
            &runtime_endpoint,
            &prompt,
            output_schema,
            document.thread_id.as_deref(),
        )
        .await?;

        let parsed = parse_structured_response(&raw_response).with_context(|| {
            format!("Kunne ikke tolke Codex-svaret som strukturert JSON: {raw_response}")
        })?;

        Ok((parsed.assistant_reply, parsed.editor_action, thread_id))
    }
}

fn parse_structured_response(input: &str) -> Result<StructuredAssistantResponse> {
    if let Ok(response) = serde_json::from_str::<StructuredAssistantResponse>(input) {
        return Ok(response);
    }

    let trimmed = input.trim();
    if trimmed.starts_with("```") {
        let without_fence = trimmed
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();
        if let Ok(response) = serde_json::from_str::<StructuredAssistantResponse>(without_fence) {
            return Ok(response);
        }
    }

    Ok(StructuredAssistantResponse {
        assistant_reply: trimmed.to_string(),
        editor_action: None,
    })
}

fn response_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["assistant_reply", "editor_action"],
        "properties": {
            "assistant_reply": {
                "type": "string",
                "description": "Short explanation to the user in Norwegian about what was produced."
            },
            "editor_action": {
                "anyOf": [
                    { "type": "null" },
                    {
                        "type": "object",
                        "additionalProperties": false,
                        "required": ["action_type", "title", "rationale", "content"],
                        "properties": {
                            "action_type": {
                                "type": "string",
                                "enum": [
                                    "replace_selection",
                                    "insert_after_cursor",
                                    "append_to_document",
                                    "create_new_section"
                                ]
                            },
                            "title": { "type": "string" },
                            "rationale": { "type": "string" },
                            "content": {
                                "type": "string",
                                "description": "Plain text only, no markdown fences."
                            }
                        }
                    }
                ]
            }
        }
    })
}

fn build_prompt(request: &AssistantTurnRequest, document: &StoredDocument) -> String {
    let quick_action_instruction = match request.quick_action.as_deref() {
        Some("continue_writing") => {
            "User clicked 'Skriv videre'. Continue the document from the current cursor area or end of text. Prefer insert_after_cursor or append_to_document."
        }
        Some("improve_selection") => {
            "User clicked 'Forbedre markert tekst'. If selection exists, improve it and use replace_selection."
        }
        Some("make_formal") => {
            "User clicked 'Gjør mer formelt'. Rewrite in a more formal, professional tone. If selection exists, use replace_selection."
        }
        Some("summarize") => {
            "User clicked 'Lag oppsummering'. Summarize the document and prefer create_new_section or append_to_document."
        }
        Some("draft_report") => {
            "User clicked 'Lag rapportutkast'. Produce a structured report draft with a clear heading and sections."
        }
        Some("meeting_notes") => {
            "User clicked 'Lag møtereferat'. Produce concise meeting notes with decisions and next steps."
        }
        _ => "Use the user prompt as the main instruction.",
    };

    let selection = request
        .selection_text
        .clone()
        .filter(|text| !text.trim().is_empty())
        .unwrap_or_else(|| "<ingen markert tekst>".to_string());

    let excerpt = excerpt(&document.plain_text, 5_500);

    format!(
        r#"You are the AI engine inside a strictly local macOS writing app.

App contract:
- Never apply edits invisibly.
- Output must match the JSON schema exactly.
- assistant_reply should be in Norwegian Bokmal.
- If you propose text for the document, put it in editor_action.content as plain text.
- Do not use markdown code fences.
- Be practical, concise, and useful.

Quick action guidance:
{quick_action_instruction}

Document title:
{title}

Preferred tone:
{tone}

Selected text:
{selection}

Document excerpt:
{excerpt}

User request:
{user_prompt}
"#,
        quick_action_instruction = quick_action_instruction,
        title = document.title,
        tone = request.tone,
        selection = selection,
        excerpt = excerpt,
        user_prompt = request.prompt
    )
}

fn excerpt(input: &str, limit: usize) -> String {
    if input.chars().count() <= limit {
        return input.to_string();
    }

    let prefix = input.chars().take(limit / 2).collect::<String>();
    let suffix = input
        .chars()
        .rev()
        .take(limit / 3)
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>();

    format!("{prefix}\n\n[...forkortet for lokal kontekst...]\n\n{suffix}")
}
