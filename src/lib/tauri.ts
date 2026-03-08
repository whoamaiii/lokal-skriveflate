import { invoke } from "@tauri-apps/api/core";
import type {
  AppBootstrap,
  AppSettings,
  AssistantTurnInput,
  AssistantTurnResult,
  ExportResult,
  OutputFormat,
  RuntimeActionResult,
  SaveDocumentInput,
  RuntimeStatus,
  StoredDocument,
  WorkspaceSnapshot,
} from "../types";

export function bootstrapApp() {
  return invoke<AppBootstrap>("bootstrap");
}

export function createDocument(title?: string) {
  return invoke<WorkspaceSnapshot>("create_document", { title });
}

export function openDocument(documentId: string) {
  return invoke<WorkspaceSnapshot>("open_document", { documentId });
}

export function saveDocument(document: SaveDocumentInput) {
  return invoke<StoredDocument>("save_document", { request: document });
}

export function sendAssistantTurn(request: AssistantTurnInput) {
  return invoke<AssistantTurnResult>("send_assistant_turn", { request });
}

export function refreshRuntimeStatus() {
  return invoke<RuntimeStatus>("refresh_runtime_status");
}

export function prepareLocalAi() {
  return invoke<RuntimeActionResult>("prepare_local_ai");
}

export function repairLocalAi() {
  return invoke<RuntimeActionResult>("repair_local_ai");
}

export function activateModel(modelId: string) {
  return invoke<RuntimeActionResult>("activate_model", { modelId });
}

export function exportDocument(documentId: string, format: OutputFormat) {
  return invoke<ExportResult>("export_document", { documentId, format });
}

export function saveSettings(settings: AppSettings) {
  return invoke<AppSettings>("save_settings", { settings });
}
