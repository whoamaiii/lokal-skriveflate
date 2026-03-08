use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};

use anyhow::{Context, Result};
use chrono::Utc;
use serde_json::json;
use uuid::Uuid;

use crate::{
    export::render_text_pdf,
    models::{
        AppSettings, AppStateFile, DocumentSnapshot, DocumentSummary, ExportResult, OutputFormat,
        PersistedDocument, SaveDocumentRequest, StoredDocument, WorkspaceSnapshot,
    },
};

pub struct StorageBootstrap {
    pub workspace: WorkspaceSnapshot,
    pub settings: AppSettings,
}

pub struct StorageService {
    root: PathBuf,
    gate: Mutex<()>,
}

impl StorageService {
    pub fn new(root: PathBuf) -> Result<Self> {
        let service = Self {
            root,
            gate: Mutex::new(()),
        };
        service.ensure_layout()?;
        Ok(service)
    }

    pub fn codex_home_dir(&self) -> PathBuf {
        self.root.join("codex-home")
    }

    pub fn bootstrap(&self) -> Result<StorageBootstrap> {
        let _guard = self.gate.lock().unwrap();
        self.ensure_seed_data_locked()?;

        let mut state = self.read_state_locked()?;
        let mut documents = self.read_documents_locked()?;
        documents.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

        let active_id = state
            .active_document_id
            .clone()
            .filter(|id| documents.iter().any(|document| &document.id == id))
            .or_else(|| documents.first().map(|document| document.id.clone()))
            .context("Fant ikke et aktivt dokument")?;

        state.active_document_id = Some(active_id.clone());
        self.write_state_locked(&state)?;

        let active_document = documents
            .iter()
            .find(|document| document.id == active_id)
            .context("Aktivt dokument mangler")?;

        Ok(StorageBootstrap {
            workspace: WorkspaceSnapshot {
                documents: documents.iter().map(DocumentSummary::from).collect(),
                active_document: StoredDocument::from(active_document),
            },
            settings: state.settings,
        })
    }

    pub fn create_document(&self, title: Option<String>) -> Result<WorkspaceSnapshot> {
        let _guard = self.gate.lock().unwrap();
        self.ensure_seed_data_locked()?;

        let timestamp = now_iso();
        let document = PersistedDocument {
            id: Uuid::new_v4().to_string(),
            title: title.unwrap_or_else(|| "Nytt lokalt dokument".to_string()),
            content: json!({
                "type": "doc",
                "content": [
                    {
                        "type": "paragraph",
                        "content": [
                            {
                                "type": "text",
                                "text": "Start her. Bruk chatpanelet for å få forslag som preview før de settes inn."
                            }
                        ]
                    }
                ]
            }),
            html: "<p>Start her. Bruk chatpanelet for å få forslag som preview før de settes inn.</p>"
                .to_string(),
            plain_text: "Start her. Bruk chatpanelet for å få forslag som preview før de settes inn."
                .to_string(),
            created_at: timestamp.clone(),
            updated_at: timestamp,
            thread_id: None,
            messages: vec![system_message(
                "Lokalt dokument opprettet. AI-forslag går alltid via preview.",
            )],
            snapshots: Vec::new(),
            workflow_hints: vec![
                "skriveassistent".to_string(),
                "rapport".to_string(),
                "prosjekt".to_string(),
            ],
        };

        self.write_document_locked(&document)?;

        let mut state = self.read_state_locked()?;
        state.active_document_id = Some(document.id.clone());
        self.write_state_locked(&state)?;

        self.workspace_for_document_locked(&document.id)
    }

    pub fn open_document(&self, document_id: &str) -> Result<WorkspaceSnapshot> {
        let _guard = self.gate.lock().unwrap();
        self.ensure_seed_data_locked()?;

        let mut state = self.read_state_locked()?;
        state.active_document_id = Some(document_id.to_string());
        self.write_state_locked(&state)?;

        self.workspace_for_document_locked(document_id)
    }

    pub fn save_document(&self, request: SaveDocumentRequest) -> Result<StoredDocument> {
        let _guard = self.gate.lock().unwrap();
        let mut document = self.read_document_locked(&request.id)?;

        if document.plain_text != request.plain_text {
            document.snapshots.push(DocumentSnapshot {
                created_at: now_iso(),
                plain_text: document.plain_text.clone(),
                html: document.html.clone(),
                content: document.content.clone(),
            });
            if document.snapshots.len() > 12 {
                let extra = document.snapshots.len() - 12;
                document.snapshots.drain(0..extra);
            }
        }

        document.title = request.title;
        document.content = request.content;
        document.html = request.html;
        document.plain_text = request.plain_text;
        document.updated_at = now_iso();

        self.write_document_locked(&document)?;
        Ok(StoredDocument::from(&document))
    }

    pub fn append_turn_result(
        &self,
        document_id: &str,
        user_prompt: &str,
        assistant_reply: &str,
        thread_id: Option<String>,
    ) -> Result<StoredDocument> {
        let _guard = self.gate.lock().unwrap();
        let mut document = self.read_document_locked(document_id)?;

        document.messages.push(user_message(user_prompt));
        document.messages.push(assistant_message(assistant_reply));
        if document.messages.len() > 40 {
            let extra = document.messages.len() - 40;
            document.messages.drain(0..extra);
        }

        document.thread_id = thread_id;
        document.updated_at = now_iso();
        self.write_document_locked(&document)?;

        Ok(StoredDocument::from(&document))
    }

    pub fn export_document(&self, document_id: &str, format: OutputFormat) -> Result<ExportResult> {
        let _guard = self.gate.lock().unwrap();
        let document = self.read_document_locked(document_id)?;
        let safe_title = slugify(&document.title);
        let timestamp = Utc::now().format("%Y%m%d-%H%M%S");

        let (target_path, bytes) = match format {
            OutputFormat::Html => {
                let html = format!(
                    "<!doctype html><html lang=\"no\"><head><meta charset=\"utf-8\" /><title>{}</title><style>body{{font-family:Georgia,serif;max-width:780px;margin:48px auto;padding:0 24px;line-height:1.7;color:#1f1c18}}h1,h2,h3{{line-height:1.2}}table{{border-collapse:collapse;width:100%}}td,th{{border:1px solid #c7c0b5;padding:8px}}</style></head><body><h1>{}</h1>{}</body></html>",
                    document.title,
                    document.title,
                    document.html
                );
                (
                    self.exports_dir().join(format!("{safe_title}-{timestamp}.html")),
                    html.into_bytes(),
                )
            }
            OutputFormat::Txt => (
                self.exports_dir().join(format!("{safe_title}-{timestamp}.txt")),
                document.plain_text.into_bytes(),
            ),
            OutputFormat::Pdf => (
                self.exports_dir().join(format!("{safe_title}-{timestamp}.pdf")),
                render_text_pdf(&document.plain_text)?,
            ),
        };

        fs::write(&target_path, bytes)
            .with_context(|| format!("Kunne ikke skrive eksport til {}", target_path.display()))?;

        Ok(ExportResult {
            path: target_path.display().to_string(),
            format,
        })
    }

    pub fn save_settings(&self, settings: AppSettings) -> Result<AppSettings> {
        let _guard = self.gate.lock().unwrap();
        let mut state = self.read_state_locked()?;
        state.settings = settings.clone();
        self.write_state_locked(&state)?;
        Ok(settings)
    }

    pub fn settings(&self) -> Result<AppSettings> {
        let _guard = self.gate.lock().unwrap();
        Ok(self.read_state_locked()?.settings)
    }

    pub fn document(&self, document_id: &str) -> Result<StoredDocument> {
        let _guard = self.gate.lock().unwrap();
        let document = self.read_document_locked(document_id)?;
        Ok(StoredDocument::from(&document))
    }

    fn ensure_layout(&self) -> Result<()> {
        fs::create_dir_all(self.documents_dir())?;
        fs::create_dir_all(self.exports_dir())?;
        fs::create_dir_all(self.codex_home_dir())?;
        Ok(())
    }

    fn ensure_seed_data_locked(&self) -> Result<()> {
        if self.read_documents_locked()?.is_empty() {
            let timestamp = now_iso();
            let document = PersistedDocument {
                id: Uuid::new_v4().to_string(),
                title: "Velkommen til Lokal Skriveflate".to_string(),
                content: json!({
                    "type": "doc",
                    "content": [
                        {
                            "type": "heading",
                            "attrs": { "level": 1 },
                            "content": [{ "type": "text", "text": "Velkommen" }]
                        },
                        {
                            "type": "paragraph",
                            "content": [
                                {
                                    "type": "text",
                                    "text": "Dette er en lokal skriveflate der dokumentet og AI-panelet lever side om side."
                                }
                            ]
                        },
                        {
                            "type": "paragraph",
                            "content": [
                                {
                                    "type": "text",
                                    "text": "Når du ber AI om hjelp, kommer forslag alltid tilbake som tekst og preview før noe settes inn."
                                }
                            ]
                        }
                    ]
                }),
                html: "<h1>Velkommen</h1><p>Dette er en lokal skriveflate der dokumentet og AI-panelet lever side om side.</p><p>Når du ber AI om hjelp, kommer forslag alltid tilbake som tekst og preview før noe settes inn.</p>".to_string(),
                plain_text: "Velkommen\n\nDette er en lokal skriveflate der dokumentet og AI-panelet lever side om side.\n\nNår du ber AI om hjelp, kommer forslag alltid tilbake som tekst og preview før noe settes inn.".to_string(),
                created_at: timestamp.clone(),
                updated_at: timestamp,
                thread_id: None,
                messages: vec![
                    system_message("Lokal lagring er aktiv. Ingen dokumentdata forlater maskinen."),
                    assistant_message("Be meg om å skrive videre, forbedre markert tekst eller lage et rapportutkast."),
                ],
                snapshots: Vec::new(),
                workflow_hints: vec![
                    "skriveassistent".to_string(),
                    "rapport".to_string(),
                    "logg".to_string(),
                ],
            };

            self.write_document_locked(&document)?;
        }

        if !self.state_path().exists() {
            self.write_state_locked(&AppStateFile::default())?;
        }

        Ok(())
    }

    fn workspace_for_document_locked(&self, document_id: &str) -> Result<WorkspaceSnapshot> {
        let mut documents = self.read_documents_locked()?;
        documents.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

        let active_document = documents
            .iter()
            .find(|document| document.id == document_id)
            .context("Dokumentet finnes ikke")?;

        Ok(WorkspaceSnapshot {
            documents: documents.iter().map(DocumentSummary::from).collect(),
            active_document: StoredDocument::from(active_document),
        })
    }

    fn read_documents_locked(&self) -> Result<Vec<PersistedDocument>> {
        let mut documents = Vec::new();
        for entry in fs::read_dir(self.documents_dir())? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }

            let raw = fs::read_to_string(&path)?;
            let document = serde_json::from_str::<PersistedDocument>(&raw)
                .with_context(|| format!("Kunne ikke lese dokumentet {}", path.display()))?;
            documents.push(document);
        }

        Ok(documents)
    }

    fn read_document_locked(&self, document_id: &str) -> Result<PersistedDocument> {
        let path = self.document_path(document_id);
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("Kunne ikke lese dokumentet {}", path.display()))?;
        Ok(serde_json::from_str(&raw)?)
    }

    fn write_document_locked(&self, document: &PersistedDocument) -> Result<()> {
        let payload = serde_json::to_string_pretty(document)?;
        fs::write(self.document_path(&document.id), payload)?;
        Ok(())
    }

    fn read_state_locked(&self) -> Result<AppStateFile> {
        if !self.state_path().exists() {
            return Ok(AppStateFile::default());
        }

        let raw = fs::read_to_string(self.state_path())?;
        Ok(serde_json::from_str(&raw)?)
    }

    fn write_state_locked(&self, state: &AppStateFile) -> Result<()> {
        let payload = serde_json::to_string_pretty(state)?;
        fs::write(self.state_path(), payload)?;
        Ok(())
    }

    fn state_path(&self) -> PathBuf {
        self.root.join("app-state.json")
    }

    fn documents_dir(&self) -> PathBuf {
        self.root.join("documents")
    }

    fn exports_dir(&self) -> PathBuf {
        self.root.join("exports")
    }

    fn document_path(&self, document_id: &str) -> PathBuf {
        self.documents_dir().join(format!("{document_id}.json"))
    }
}

fn now_iso() -> String {
    Utc::now().to_rfc3339()
}

fn slugify(value: &str) -> String {
    let cleaned = value
        .chars()
        .map(|character| match character {
            'a'..='z' | 'A'..='Z' | '0'..='9' => character.to_ascii_lowercase(),
            _ => '-',
        })
        .collect::<String>();

    cleaned
        .split('-')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn system_message(text: &str) -> crate::models::ChatMessage {
    crate::models::ChatMessage {
        id: Uuid::new_v4().to_string(),
        role: "system".to_string(),
        text: text.to_string(),
        created_at: now_iso(),
    }
}

fn assistant_message(text: &str) -> crate::models::ChatMessage {
    crate::models::ChatMessage {
        id: Uuid::new_v4().to_string(),
        role: "assistant".to_string(),
        text: text.to_string(),
        created_at: now_iso(),
    }
}

fn user_message(text: &str) -> crate::models::ChatMessage {
    crate::models::ChatMessage {
        id: Uuid::new_v4().to_string(),
        role: "user".to_string(),
        text: text.to_string(),
        created_at: now_iso(),
    }
}

#[allow(dead_code)]
fn _ensure_path(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}
