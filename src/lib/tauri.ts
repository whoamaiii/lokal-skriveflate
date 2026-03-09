import { invoke } from "@tauri-apps/api/core";
import type {
  AppBootstrap,
  AppSettings,
  AssistantTurnInput,
  AssistantTurnResult,
  CommandError,
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

function isCommandError(value: unknown): value is CommandError {
  return Boolean(
    value &&
      typeof value === "object" &&
      "code" in value &&
      "message" in value &&
      "retryable" in value,
  );
}

function normalizeCommandError(error: unknown): CommandError {
  if (isCommandError(error)) {
    return error;
  }

  if (error instanceof Error && error.message.trim()) {
    return {
      code: "unknown",
      message: error.message,
      retryable: false,
      action: null,
    };
  }

  if (typeof error === "string" && error.trim()) {
    return {
      code: "unknown",
      message: error,
      retryable: false,
      action: null,
    };
  }

  return {
    code: "unknown",
    message: "Ukjent feil fra lokal kommando.",
    retryable: false,
    action: null,
  };
}

async function invokeCommand<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  try {
    if (typeof args === "undefined") {
      return await invoke<T>(command);
    }

    return await invoke<T>(command, args);
  } catch (error) {
    throw normalizeCommandError(error);
  }
}

export function bootstrapApp() {
  if (!hasTauriRuntime()) {
    return Promise.resolve(bootstrapBrowserPreview());
  }

  return invokeCommand<AppBootstrap>("bootstrap");
}

export function createDocument(title?: string) {
  if (!hasTauriRuntime()) {
    return Promise.resolve(createBrowserPreviewDocument(title));
  }

  return invokeCommand<WorkspaceSnapshot>("create_document", { title });
}

export function openDocument(documentId: string) {
  if (!hasTauriRuntime()) {
    return Promise.resolve(openBrowserPreviewDocument(documentId));
  }

  return invokeCommand<WorkspaceSnapshot>("open_document", { documentId });
}

export function saveDocument(document: SaveDocumentInput) {
  if (!hasTauriRuntime()) {
    return Promise.resolve(saveBrowserPreviewDocument(document));
  }

  return invokeCommand<StoredDocument>("save_document", { request: document });
}

export function sendAssistantTurn(request: AssistantTurnInput) {
  if (!hasTauriRuntime()) {
    return Promise.resolve(sendBrowserPreviewAssistantTurn(request));
  }

  return invokeCommand<AssistantTurnResult>("send_assistant_turn", { request });
}

export function refreshRuntimeStatus() {
  if (!hasTauriRuntime()) {
    return Promise.resolve(refreshBrowserPreviewRuntimeStatus());
  }

  return invokeCommand<RuntimeStatus>("refresh_runtime_status");
}

export function prepareLocalAi() {
  if (!hasTauriRuntime()) {
    return Promise.resolve(prepareBrowserPreviewLocalAi());
  }

  return invokeCommand<RuntimeActionResult>("prepare_local_ai");
}

export function repairLocalAi() {
  if (!hasTauriRuntime()) {
    return Promise.resolve(repairBrowserPreviewLocalAi());
  }

  return invokeCommand<RuntimeActionResult>("repair_local_ai");
}

export function activateModel(modelId: string) {
  if (!hasTauriRuntime()) {
    return Promise.resolve(activateBrowserPreviewModel(modelId));
  }

  return invokeCommand<RuntimeActionResult>("activate_model", { modelId });
}

export function exportDocument(documentId: string, format: OutputFormat) {
  if (!hasTauriRuntime()) {
    return Promise.resolve(exportBrowserPreviewDocument(documentId, format));
  }

  return invokeCommand<ExportResult>("export_document", { documentId, format });
}

export function saveSettings(settings: AppSettings) {
  if (!hasTauriRuntime()) {
    return Promise.resolve(saveBrowserPreviewSettings(settings));
  }

  return invokeCommand<AppSettings>("save_settings", { settings });
}
