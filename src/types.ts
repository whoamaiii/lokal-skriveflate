import type { JSONContent } from "@tiptap/core";

export type MessageRole = "user" | "assistant" | "system";
export type OutputFormat = "html" | "pdf" | "txt";
export type RuntimeState =
  | "not_prepared"
  | "extracting"
  | "ready"
  | "degraded"
  | "repair_required";
export type RuntimeBlockingReason =
  | "codex_unavailable"
  | "runtime_not_prepared"
  | "runtime_repair_required"
  | "runtime_start_failed"
  | "unknown";
export type EditorActionType =
  | "replace_selection"
  | "insert_after_cursor"
  | "append_to_document"
  | "create_new_section";
export type CommandErrorCode =
  | "document_conflict"
  | "runtime_not_ready"
  | "runtime_start_failed"
  | "runtime_repair_required"
  | "assistant_timeout"
  | "assistant_bridge_error"
  | "export_failed"
  | "storage_error"
  | "unknown";

export interface ChatMessage {
  id: string;
  role: MessageRole;
  text: string;
  created_at: string;
}

export interface EditorAction {
  action_type: EditorActionType;
  title: string;
  rationale: string;
  content: string;
}

export interface EditorSelection {
  text: string;
  from: number;
  to: number;
}

export interface PendingActionPreview {
  action: EditorAction;
  document_id: string;
  document_revision: number;
  selection_from: number;
  selection_to: number;
  created_at: string;
}

export interface DocumentSummary {
  id: string;
  title: string;
  preview: string;
  updated_at: string;
  workflow_hints: string[];
}

export interface StoredDocument extends DocumentSummary {
  created_at: string;
  plain_text: string;
  html: string;
  content: JSONContent;
  content_revision: number;
  thread_id: string | null;
  messages: ChatMessage[];
  snapshot_count: number;
}

export interface RuntimeComponentStatus {
  available: boolean;
  running: boolean;
  details: string | null;
}

export interface LocalModelOption {
  id: string;
  label: string;
  tier: string;
  bundled: boolean;
  installed: boolean;
  active: boolean;
  details: string | null;
}

export interface RuntimeStatus {
  offline_mode: boolean;
  local_only: boolean;
  selected_model: string;
  runtime_state: RuntimeState;
  can_send: boolean;
  will_start_on_demand: boolean;
  blocking_reason: RuntimeBlockingReason | null;
  codex: RuntimeComponentStatus;
  local_ai: RuntimeComponentStatus;
  available_models: LocalModelOption[];
  runtime_home: string | null;
  listen_address: string | null;
}

export interface CommandError {
  code: CommandErrorCode;
  message: string;
  retryable: boolean;
  action?: string | null;
}

export interface AppSettings {
  selected_model: string;
  preferred_tone: string;
}

export interface AppBootstrap {
  documents: DocumentSummary[];
  active_document: StoredDocument;
  runtime_status: RuntimeStatus;
  settings: AppSettings;
  recovery_notices: string[];
}

export interface WorkspaceSnapshot {
  documents: DocumentSummary[];
  active_document: StoredDocument;
  recovery_notices: string[];
}

export interface SaveDocumentInput {
  id: string;
  title: string;
  content: JSONContent;
  html: string;
  plain_text: string;
}

export interface AssistantTurnInput {
  document_id: string;
  document_revision: number;
  prompt: string;
  quick_action: string | null;
  tone: string;
  selection_text: string | null;
}

export interface AssistantTurnResult {
  document: StoredDocument;
  runtime_status: RuntimeStatus;
  assistant_reply: string;
  editor_action: EditorAction | null;
}

export interface RuntimeActionResult {
  message: string;
  runtime_status: RuntimeStatus;
  settings: AppSettings;
}

export interface ExportResult {
  path: string;
  format: OutputFormat;
}
