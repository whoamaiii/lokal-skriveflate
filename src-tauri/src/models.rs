use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const DEFAULT_MODEL: &str = "qwen3-4b-instruct-q4_k_m";
pub const QUALITY_MODEL: &str = "qwen3-8b-instruct-q4_k_m";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: String,
    pub role: String,
    pub text: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentSnapshot {
    pub created_at: String,
    pub plain_text: String,
    pub html: String,
    pub content: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedDocument {
    pub id: String,
    pub title: String,
    pub content: Value,
    pub html: String,
    pub plain_text: String,
    pub created_at: String,
    pub updated_at: String,
    pub thread_id: Option<String>,
    pub messages: Vec<ChatMessage>,
    pub snapshots: Vec<DocumentSnapshot>,
    pub workflow_hints: Vec<String>,
    #[serde(default)]
    pub content_revision: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredDocument {
    pub id: String,
    pub title: String,
    pub preview: String,
    pub updated_at: String,
    pub workflow_hints: Vec<String>,
    pub created_at: String,
    pub plain_text: String,
    pub html: String,
    pub content: Value,
    pub thread_id: Option<String>,
    pub messages: Vec<ChatMessage>,
    pub snapshot_count: usize,
    pub content_revision: u64,
}

impl From<&PersistedDocument> for StoredDocument {
    fn from(value: &PersistedDocument) -> Self {
        Self {
            id: value.id.clone(),
            title: value.title.clone(),
            preview: summarize_text(&value.plain_text),
            updated_at: value.updated_at.clone(),
            workflow_hints: value.workflow_hints.clone(),
            created_at: value.created_at.clone(),
            plain_text: value.plain_text.clone(),
            html: value.html.clone(),
            content: value.content.clone(),
            thread_id: value.thread_id.clone(),
            messages: value.messages.clone(),
            snapshot_count: value.snapshots.len(),
            content_revision: value.content_revision,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentSummary {
    pub id: String,
    pub title: String,
    pub preview: String,
    pub updated_at: String,
    pub workflow_hints: Vec<String>,
}

impl From<&PersistedDocument> for DocumentSummary {
    fn from(value: &PersistedDocument) -> Self {
        Self {
            id: value.id.clone(),
            title: value.title.clone(),
            preview: summarize_text(&value.plain_text),
            updated_at: value.updated_at.clone(),
            workflow_hints: value.workflow_hints.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeComponentStatus {
    pub available: bool,
    pub running: bool,
    pub details: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RuntimePhase {
    NotPrepared,
    Extracting,
    Ready,
    Degraded,
    RepairRequired,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalModelOption {
    pub id: String,
    pub label: String,
    pub tier: String,
    pub bundled: bool,
    pub installed: bool,
    pub active: bool,
    pub details: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeStatus {
    pub offline_mode: bool,
    pub local_only: bool,
    pub selected_model: String,
    pub runtime_state: RuntimePhase,
    pub codex: RuntimeComponentStatus,
    pub local_ai: RuntimeComponentStatus,
    pub available_models: Vec<LocalModelOption>,
    pub runtime_home: Option<String>,
    pub listen_address: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub selected_model: String,
    pub preferred_tone: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            selected_model: DEFAULT_MODEL.to_string(),
            preferred_tone: "Klar og profesjonell".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppStateFile {
    pub active_document_id: Option<String>,
    pub settings: AppSettings,
}

impl Default for AppStateFile {
    fn default() -> Self {
        Self {
            active_document_id: None,
            settings: AppSettings::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowModule {
    pub id: String,
    pub name: String,
    pub description: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceSnapshot {
    pub documents: Vec<DocumentSummary>,
    pub active_document: StoredDocument,
    #[serde(default)]
    pub recovery_notices: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppBootstrap {
    pub documents: Vec<DocumentSummary>,
    pub active_document: StoredDocument,
    pub runtime_status: RuntimeStatus,
    pub settings: AppSettings,
    pub workflow_modules: Vec<WorkflowModule>,
    #[serde(default)]
    pub recovery_notices: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveDocumentRequest {
    pub id: String,
    pub title: String,
    pub content: Value,
    pub html: String,
    pub plain_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantTurnRequest {
    pub document_id: String,
    pub document_revision: u64,
    pub prompt: String,
    pub quick_action: Option<String>,
    pub tone: String,
    pub selection_text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorAction {
    pub action_type: String,
    pub title: String,
    pub rationale: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantTurnResult {
    pub document: StoredDocument,
    pub runtime_status: RuntimeStatus,
    pub assistant_reply: String,
    pub editor_action: Option<EditorAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeActionResult {
    pub message: String,
    pub runtime_status: RuntimeStatus,
    pub settings: AppSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportResult {
    pub path: String,
    pub format: OutputFormat,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    Html,
    Pdf,
    Txt,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuredAssistantResponse {
    pub assistant_reply: String,
    pub editor_action: Option<EditorAction>,
}

pub fn default_workflows() -> Vec<WorkflowModule> {
    vec![
        WorkflowModule {
            id: "report_workflow".to_string(),
            name: "Rapportflyt".to_string(),
            description: "Planlagt modul for rapportmaler og kvalitetssikret struktur.".to_string(),
            status: "planned".to_string(),
        },
        WorkflowModule {
            id: "log_workflow".to_string(),
            name: "Loggflyt".to_string(),
            description: "Planlagt modul for løpende loggføring, dagnotater og dokumentasjon."
                .to_string(),
            status: "planned".to_string(),
        },
        WorkflowModule {
            id: "project_workflow".to_string(),
            name: "Prosjektflyt".to_string(),
            description: "Planlagt modul for prosjektplaner, møtenotater og oppfølging."
                .to_string(),
            status: "planned".to_string(),
        },
    ]
}

pub fn summarize_text(input: &str) -> String {
    input
        .split_whitespace()
        .take(20)
        .collect::<Vec<_>>()
        .join(" ")
}
