use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    sync::Mutex,
};

use anyhow::{Context, Result};
use chrono::Utc;
use serde::Serialize;
use serde_json::json;
use uuid::Uuid;

use crate::{
    export::render_text_pdf,
    models::{
        AppSettings, AppStateFile, DocumentSnapshot, DocumentSummary, ExportResult,
        OutputFormat, PersistedDocument, SaveDocumentRequest, StoredDocument, WorkspaceSnapshot,
        APP_STATE_SCHEMA_VERSION, DEFAULT_MODEL, DOCUMENT_SCHEMA_VERSION,
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

struct DocumentsReadResult {
    documents: Vec<PersistedDocument>,
    recovery_notices: Vec<String>,
}

struct StateReadResult {
    state: AppStateFile,
    recovery_notices: Vec<String>,
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
        let mut recovery_notices = self.ensure_seed_data_locked()?;

        let StateReadResult {
            mut state,
            recovery_notices: state_notices,
        } = self.read_state_locked()?;
        extend_notices(&mut recovery_notices, state_notices);

        let DocumentsReadResult {
            mut documents,
            recovery_notices: document_notices,
        } = self.read_documents_locked()?;
        extend_notices(&mut recovery_notices, document_notices);
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
                recovery_notices,
            },
            settings: state.settings,
        })
    }

    pub fn create_document(&self, title: Option<String>) -> Result<WorkspaceSnapshot> {
        let _guard = self.gate.lock().unwrap();
        let mut recovery_notices = self.ensure_seed_data_locked()?;

        let timestamp = now_iso();
        let document = PersistedDocument {
            schema_version: DOCUMENT_SCHEMA_VERSION,
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
            html:
                "<p>Start her. Bruk chatpanelet for å få forslag som preview før de settes inn.</p>"
                    .to_string(),
            plain_text:
                "Start her. Bruk chatpanelet for å få forslag som preview før de settes inn."
                    .to_string(),
            created_at: timestamp.clone(),
            updated_at: timestamp,
            thread_id: None,
            messages: vec![system_message(
                "Lokalt dokument opprettet. AI-forslag går alltid via preview.",
            )],
            snapshots: Vec::new(),
            workflow_hints: vec!["skriveassistent".to_string()],
            content_revision: 0,
        };

        self.write_document_locked(&document)?;

        let StateReadResult {
            mut state,
            recovery_notices: state_notices,
        } = self.read_state_locked()?;
        extend_notices(&mut recovery_notices, state_notices);
        state.active_document_id = Some(document.id.clone());
        self.write_state_locked(&state)?;

        let mut workspace = self.workspace_for_document_locked(&document.id)?;
        extend_notices(&mut workspace.recovery_notices, recovery_notices);
        Ok(workspace)
    }

    pub fn open_document(&self, document_id: &str) -> Result<WorkspaceSnapshot> {
        let _guard = self.gate.lock().unwrap();
        let mut recovery_notices = self.ensure_seed_data_locked()?;
        let mut workspace = self.workspace_for_document_locked(document_id)?;

        let StateReadResult {
            mut state,
            recovery_notices: state_notices,
        } = self.read_state_locked()?;
        extend_notices(&mut recovery_notices, state_notices);
        state.active_document_id = Some(document_id.to_string());
        self.write_state_locked(&state)?;

        extend_notices(&mut workspace.recovery_notices, recovery_notices);
        Ok(workspace)
    }

    pub fn save_document(&self, request: SaveDocumentRequest) -> Result<StoredDocument> {
        let _guard = self.gate.lock().unwrap();
        let mut document = self.read_document_locked(&request.id)?;

        let content_changed = document.title != request.title
            || document.content != request.content
            || document.html != request.html
            || document.plain_text != request.plain_text;

        if !content_changed {
            return Ok(StoredDocument::from(&document));
        }

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
        document.content_revision = document.content_revision.saturating_add(1);

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
        let escaped_title = escape_html(&document.title);
        let sanitized_body = ammonia::clean(&document.html);
        let timestamp = Utc::now().format("%Y%m%d-%H%M%S");

        let (target_path, bytes) = match format {
            OutputFormat::Html => {
                let html = format!(
                    "<!doctype html><html lang=\"no\"><head><meta charset=\"utf-8\" /><title>{}</title><style>body{{font-family:Georgia,serif;max-width:780px;margin:48px auto;padding:0 24px;line-height:1.7;color:#1f1c18}}h1,h2,h3{{line-height:1.2}}table{{border-collapse:collapse;width:100%}}td,th{{border:1px solid #c7c0b5;padding:8px}}</style></head><body><h1>{}</h1>{}</body></html>",
                    escaped_title,
                    escaped_title,
                    sanitized_body
                );
                (
                    self.exports_dir()
                        .join(format!("{safe_title}-{timestamp}.html")),
                    html.into_bytes(),
                )
            }
            OutputFormat::Txt => (
                self.exports_dir()
                    .join(format!("{safe_title}-{timestamp}.txt")),
                document.plain_text.into_bytes(),
            ),
            OutputFormat::Pdf => (
                self.exports_dir()
                    .join(format!("{safe_title}-{timestamp}.pdf")),
                render_text_pdf(&document.plain_text)?,
            ),
        };

        self.write_bytes_atomic(&target_path, &bytes)
            .with_context(|| format!("Kunne ikke skrive eksport til {}", target_path.display()))?;

        Ok(ExportResult {
            path: target_path.display().to_string(),
            format,
        })
    }

    pub fn save_settings(&self, settings: AppSettings) -> Result<AppSettings> {
        let _guard = self.gate.lock().unwrap();
        let StateReadResult { mut state, .. } = self.read_state_locked()?;
        let settings = normalize_settings(settings);
        state.settings = settings.clone();
        self.write_state_locked(&state)?;
        Ok(settings)
    }

    pub fn settings(&self) -> Result<AppSettings> {
        let _guard = self.gate.lock().unwrap();
        Ok(self.read_state_locked()?.state.settings)
    }

    pub fn document(&self, document_id: &str) -> Result<StoredDocument> {
        let _guard = self.gate.lock().unwrap();
        let document = self.read_document_locked(document_id)?;
        Ok(StoredDocument::from(&document))
    }

    fn ensure_layout(&self) -> Result<()> {
        fs::create_dir_all(self.documents_dir())?;
        fs::create_dir_all(self.exports_dir())?;
        fs::create_dir_all(self.corrupt_dir())?;
        fs::create_dir_all(self.codex_home_dir())?;
        Ok(())
    }

    fn ensure_seed_data_locked(&self) -> Result<Vec<String>> {
        let DocumentsReadResult {
            documents,
            recovery_notices,
        } = self.read_documents_locked()?;

        if documents.is_empty() {
            let timestamp = now_iso();
            let document = PersistedDocument {
                schema_version: DOCUMENT_SCHEMA_VERSION,
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
                workflow_hints: vec!["skriveassistent".to_string()],
                content_revision: 0,
            };

            self.write_document_locked(&document)?;
        }

        if !self.state_path().exists() {
            self.write_state_locked(&AppStateFile::default())?;
        }

        Ok(recovery_notices)
    }

    fn workspace_for_document_locked(&self, document_id: &str) -> Result<WorkspaceSnapshot> {
        let DocumentsReadResult {
            mut documents,
            recovery_notices,
        } = self.read_documents_locked()?;
        documents.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

        let active_document = documents
            .iter()
            .find(|document| document.id == document_id)
            .context("Dokumentet finnes ikke")?;

        Ok(WorkspaceSnapshot {
            documents: documents.iter().map(DocumentSummary::from).collect(),
            active_document: StoredDocument::from(active_document),
            recovery_notices,
        })
    }

    fn read_documents_locked(&self) -> Result<DocumentsReadResult> {
        let mut documents = Vec::new();
        let mut recovery_notices = Vec::new();

        for entry in fs::read_dir(self.documents_dir())? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }

            let raw = match fs::read_to_string(&path) {
                Ok(raw) => raw,
                Err(error) => {
                    recovery_notices.push(self.quarantine_corrupt_file(
                        &path,
                        &format!("Kunne ikke lese dokumentfila: {error}"),
                    )?);
                    continue;
                }
            };

            match serde_json::from_str::<PersistedDocument>(&raw) {
                Ok(mut document) => {
                    let changed = normalize_document(&mut document);
                    if changed {
                        self.write_document_locked(&document)?;
                    }
                    documents.push(document);
                }
                Err(error) => recovery_notices.push(self.quarantine_corrupt_file(
                    &path,
                    &format!("Kunne ikke lese dokumentet som gyldig JSON: {error}"),
                )?),
            }
        }

        Ok(DocumentsReadResult {
            documents,
            recovery_notices,
        })
    }

    fn read_document_locked(&self, document_id: &str) -> Result<PersistedDocument> {
        let path = self.document_path(document_id);
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("Kunne ikke lese dokumentet {}", path.display()))?;
        let mut document = serde_json::from_str::<PersistedDocument>(&raw)?;
        if normalize_document(&mut document) {
            self.write_document_locked(&document)?;
        }
        Ok(document)
    }

    fn write_document_locked(&self, document: &PersistedDocument) -> Result<()> {
        self.write_json_atomic(self.document_path(&document.id), document)
    }

    fn read_state_locked(&self) -> Result<StateReadResult> {
        if !self.state_path().exists() {
            return Ok(StateReadResult {
                state: AppStateFile::default(),
                recovery_notices: Vec::new(),
            });
        }

        let state_path = self.state_path();
        let raw = match fs::read_to_string(&state_path) {
            Ok(raw) => raw,
            Err(error) => {
                return Ok(StateReadResult {
                    state: AppStateFile::default(),
                    recovery_notices: vec![self.quarantine_corrupt_file(
                        &state_path,
                        &format!("Kunne ikke lese tilstandsfilen: {error}"),
                    )?],
                });
            }
        };

        match serde_json::from_str::<AppStateFile>(&raw) {
            Ok(mut state) => {
                let (changed, recovery_notices) = normalize_state(&mut state);
                if changed {
                    self.write_state_locked(&state)?;
                }
                Ok(StateReadResult {
                    state,
                    recovery_notices,
                })
            }
            Err(error) => Ok(StateReadResult {
                state: AppStateFile::default(),
                recovery_notices: vec![self.quarantine_corrupt_file(
                    &state_path,
                    &format!("Kunne ikke lese tilstandsfilen som gyldig JSON: {error}"),
                )?],
            }),
        }
    }

    fn write_state_locked(&self, state: &AppStateFile) -> Result<()> {
        self.write_json_atomic(self.state_path(), state)
    }

    fn write_json_atomic<T: Serialize>(&self, path: PathBuf, value: &T) -> Result<()> {
        let payload = serde_json::to_vec_pretty(value)?;
        self.write_bytes_atomic(&path, &payload)
    }

    fn write_bytes_atomic(&self, path: &Path, bytes: &[u8]) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let temp_path = path.with_file_name(format!(
            "{}.{}.tmp",
            path.file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("lokal-skriveflate"),
            Uuid::new_v4()
        ));

        let mut file = File::options()
            .create_new(true)
            .write(true)
            .open(&temp_path)
            .with_context(|| {
                format!(
                    "Kunne ikke opprette midlertidig fil {}",
                    temp_path.display()
                )
            })?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);

        fs::rename(&temp_path, path).with_context(|| {
            format!(
                "Kunne ikke flytte {} til {}",
                temp_path.display(),
                path.display()
            )
        })?;

        if let Some(parent) = path.parent() {
            if let Ok(directory) = File::open(parent) {
                let _ = directory.sync_all();
            }
        }

        Ok(())
    }

    fn quarantine_corrupt_file(&self, path: &Path, reason: &str) -> Result<String> {
        fs::create_dir_all(self.corrupt_dir())?;

        let filename = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("ukjent-fil");
        let target = self.corrupt_dir().join(format!(
            "{}-{}-{}",
            Utc::now().format("%Y%m%d-%H%M%S"),
            Uuid::new_v4(),
            filename
        ));

        if path.exists() {
            fs::rename(path, &target)
                .or_else(|_| {
                    fs::copy(path, &target)?;
                    fs::remove_file(path)
                })
                .with_context(|| {
                    format!(
                        "Kunne ikke flytte korrupt fil {} til {}",
                        path.display(),
                        target.display()
                    )
                })?;
        }

        Ok(format!(
            "Flyttet en korrupt fil ({filename}) til {}. {reason}",
            target.display()
        ))
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

    fn corrupt_dir(&self) -> PathBuf {
        self.root.join("corrupt")
    }

    fn document_path(&self, document_id: &str) -> PathBuf {
        self.documents_dir().join(format!("{document_id}.json"))
    }
}

fn extend_notices(target: &mut Vec<String>, notices: Vec<String>) {
    for notice in notices {
        if !target.contains(&notice) {
            target.push(notice);
        }
    }
}

fn normalize_document(document: &mut PersistedDocument) -> bool {
    let mut changed = false;

    if document.schema_version != DOCUMENT_SCHEMA_VERSION {
        document.schema_version = DOCUMENT_SCHEMA_VERSION;
        changed = true;
    }

    changed
}

fn normalize_settings(mut settings: AppSettings) -> AppSettings {
    if settings.selected_model != DEFAULT_MODEL {
        settings.selected_model = DEFAULT_MODEL.to_string();
    }

    settings
}

fn normalize_state(state: &mut AppStateFile) -> (bool, Vec<String>) {
    let mut recovery_notices = Vec::new();
    let mut changed = false;

    if state.schema_version != APP_STATE_SCHEMA_VERSION {
        state.schema_version = APP_STATE_SCHEMA_VERSION;
        changed = true;
    }

    let normalized_settings = normalize_settings(state.settings.clone());
    if normalized_settings.selected_model != state.settings.selected_model {
        recovery_notices.push(
            "Oppdaterte lokale innstillinger til v1-standardmodellen for denne utgaven."
                .to_string(),
        );
        changed = true;
    }
    state.settings = normalized_settings;

    (changed, recovery_notices)
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

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#039;")
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::OutputFormat;

    fn temp_root() -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("lokal-skriveflate-storage-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn bootstrap_quarantines_corrupt_documents_and_returns_notices() {
        let root = temp_root();
        let service = StorageService::new(root.clone()).unwrap();
        let document_path = root.join("documents").join("broken.json");
        fs::write(&document_path, "{not-json").unwrap();

        let bootstrap = service.bootstrap().unwrap();

        assert!(!bootstrap.workspace.recovery_notices.is_empty());
        assert!(!document_path.exists());
        assert!(fs::read_dir(root.join("corrupt")).unwrap().next().is_some());
    }

    #[test]
    fn save_document_increments_content_revision() {
        let root = temp_root();
        let service = StorageService::new(root).unwrap();
        let bootstrap = service.bootstrap().unwrap();
        let document = bootstrap.workspace.active_document;

        let saved = service
            .save_document(SaveDocumentRequest {
                id: document.id.clone(),
                title: format!("{} oppdatert", document.title),
                content: document.content.clone(),
                html: document.html.clone(),
                plain_text: document.plain_text.clone(),
            })
            .unwrap();

        assert_eq!(saved.content_revision, document.content_revision + 1);
    }

    #[test]
    fn export_document_sanitizes_html_title() {
        let root = temp_root();
        let service = StorageService::new(root.clone()).unwrap();
        let bootstrap = service.bootstrap().unwrap();
        let document = bootstrap.workspace.active_document;

        service
            .save_document(SaveDocumentRequest {
                id: document.id.clone(),
                title: "</title><script>alert(1)</script>".to_string(),
                content: document.content,
                html: "<p>Hei</p><script>alert(1)</script>".to_string(),
                plain_text: "Hei".to_string(),
            })
            .unwrap();

        let export = service
            .export_document(&document.id, OutputFormat::Html)
            .unwrap();
        let html = fs::read_to_string(export.path).unwrap();

        assert!(html.contains("&lt;/title&gt;&lt;script&gt;alert(1)&lt;/script&gt;"));
        assert!(!html.contains("<script>alert(1)</script>"));
    }
}
