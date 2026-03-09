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
import {
  activateBrowserPreviewModel,
  bootstrapBrowserPreview,
  createBrowserPreviewDocument,
  exportBrowserPreviewDocument,
  openBrowserPreviewDocument,
  prepareBrowserPreviewLocalAi,
  refreshBrowserPreviewRuntimeStatus,
  repairBrowserPreviewLocalAi,
  saveBrowserPreviewDocument,
  saveBrowserPreviewSettings,
  sendBrowserPreviewAssistantTurn,
} from "./browserPreview";

type TauriInternalsWindow = typeof globalThis & {
  __TAURI_INTERNALS__?: {
    invoke?: unknown;
  };
};

function hasTauriRuntime() {
  const maybeWindow = globalThis as TauriInternalsWindow;
  return typeof maybeWindow.__TAURI_INTERNALS__?.invoke === "function";
}

export function bootstrapApp() {
  if (!hasTauriRuntime()) {
    return Promise.resolve(bootstrapBrowserPreview());
  }

  return invoke<AppBootstrap>("bootstrap");
}

export function createDocument(title?: string) {
  if (!hasTauriRuntime()) {
    return Promise.resolve(createBrowserPreviewDocument(title));
  }

  return invoke<WorkspaceSnapshot>("create_document", { title });
}

export function openDocument(documentId: string) {
  if (!hasTauriRuntime()) {
    return Promise.resolve(openBrowserPreviewDocument(documentId));
  }

  return invoke<WorkspaceSnapshot>("open_document", { documentId });
}

export function saveDocument(document: SaveDocumentInput) {
  if (!hasTauriRuntime()) {
    return Promise.resolve(saveBrowserPreviewDocument(document));
  }

  return invoke<StoredDocument>("save_document", { request: document });
}

export function sendAssistantTurn(request: AssistantTurnInput) {
  if (!hasTauriRuntime()) {
    return Promise.resolve(sendBrowserPreviewAssistantTurn(request));
  }

  return invoke<AssistantTurnResult>("send_assistant_turn", { request });
}

export function refreshRuntimeStatus() {
  if (!hasTauriRuntime()) {
    return Promise.resolve(refreshBrowserPreviewRuntimeStatus());
  }

  return invoke<RuntimeStatus>("refresh_runtime_status");
}

export function prepareLocalAi() {
  if (!hasTauriRuntime()) {
    return Promise.resolve(prepareBrowserPreviewLocalAi());
  }

  return invoke<RuntimeActionResult>("prepare_local_ai");
}

export function repairLocalAi() {
  if (!hasTauriRuntime()) {
    return Promise.resolve(repairBrowserPreviewLocalAi());
  }

  return invoke<RuntimeActionResult>("repair_local_ai");
}

export function activateModel(modelId: string) {
  if (!hasTauriRuntime()) {
    return Promise.resolve(activateBrowserPreviewModel(modelId));
  }

  return invoke<RuntimeActionResult>("activate_model", { modelId });
}

export function exportDocument(documentId: string, format: OutputFormat) {
  if (!hasTauriRuntime()) {
    return Promise.resolve(exportBrowserPreviewDocument(documentId, format));
  }

  return invoke<ExportResult>("export_document", { documentId, format });
}

export function saveSettings(settings: AppSettings) {
  if (!hasTauriRuntime()) {
    return Promise.resolve(saveBrowserPreviewSettings(settings));
  }

  return invoke<AppSettings>("save_settings", { settings });
}
