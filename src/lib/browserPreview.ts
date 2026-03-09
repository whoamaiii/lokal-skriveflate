import type {
  AppBootstrap,
  AppSettings,
  AssistantTurnInput,
  AssistantTurnResult,
  ChatMessage,
  CommandError,
  ExportResult,
  LocalModelOption,
  OutputFormat,
  RuntimeActionResult,
  RuntimeStatus,
  SaveDocumentInput,
  StoredDocument,
  WorkspaceSnapshot,
} from "../types";

type BrowserPreviewState = {
  documents: StoredDocument[];
  activeDocumentId: string;
  settings: AppSettings;
  runtimeStatus: RuntimeStatus;
  recoveryNotices: string[];
};

const STORAGE_KEY = "lokal-skriveflate.browser-preview.v1";
const BROWSER_THREAD_ID = "browser-preview-thread";
const DEFAULT_MODEL_ID = "neurologg-q4_k_m";

const AVAILABLE_MODELS: LocalModelOption[] = [
  {
    id: DEFAULT_MODEL_ID,
    label: "Neurologg Q4_K_M",
    tier: "Standard",
    bundled: true,
    installed: true,
    active: true,
    details: "Simulert standardmodell for nettleserforhåndsvisning.",
  },
];

let cachedState: BrowserPreviewState | null = null;

function cloneValue<T>(value: T): T {
  return JSON.parse(JSON.stringify(value)) as T;
}

function nowIso() {
  return new Date().toISOString();
}

function generateId(prefix: string) {
  if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
    return `${prefix}-${crypto.randomUUID()}`;
  }

  return `${prefix}-${Math.random().toString(16).slice(2)}-${Date.now()}`;
}

function summarizeText(input: string) {
  return input
    .split(/\s+/)
    .filter(Boolean)
    .slice(0, 20)
    .join(" ");
}

function escapeHtml(value: string) {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll("\"", "&quot;")
    .replaceAll("'", "&#039;");
}

function textToHtml(text: string) {
  return text
    .split(/\n{2,}/)
    .map((paragraph) => paragraph.trim())
    .filter(Boolean)
    .map((paragraph) => `<p>${escapeHtml(paragraph).replaceAll("\n", "<br />")}</p>`)
    .join("");
}

function textToContent(text: string) {
  return {
    type: "doc",
    content: text
      .split(/\n{2,}/)
      .map((paragraph) => paragraph.trim())
      .filter(Boolean)
      .map((paragraph) => ({
        type: "paragraph",
        content: [{ type: "text", text: paragraph }],
      })),
  };
}

function createMessage(role: ChatMessage["role"], text: string): ChatMessage {
  return {
    id: generateId(`message-${role}`),
    role,
    text,
    created_at: nowIso(),
  };
}

function syncModels(selectedModel: string) {
  return AVAILABLE_MODELS.map((model) => ({
    ...model,
    active: model.id === selectedModel,
  }));
}

function makeCommandError(error: CommandError): CommandError {
  return error;
}

function createRuntimeStatus(selectedModel: string): RuntimeStatus {
  return {
    offline_mode: true,
    local_only: true,
    selected_model: selectedModel,
    runtime_state: "ready",
    can_send: true,
    will_start_on_demand: false,
    blocking_reason: null,
    codex: {
      available: true,
      running: true,
      details: "Browserforhåndsvisning bruker en simulert Tauri-bro for UI-testing.",
    },
    local_ai: {
      available: true,
      running: true,
      details: "Lokal AI er simulert i nettleseren. Desktopappen bruker den ekte runtime-en.",
    },
    available_models: syncModels(selectedModel),
    runtime_home: "browser-preview",
    listen_address: null,
  };
}

function createDocument(options: {
  id?: string;
  title: string;
  plainText: string;
  workflowHints?: string[];
  messages?: ChatMessage[];
  createdAt?: string;
  updatedAt?: string;
  contentRevision?: number;
  snapshotCount?: number;
  threadId?: string | null;
}): StoredDocument {
  const createdAt = options.createdAt ?? nowIso();
  const updatedAt = options.updatedAt ?? createdAt;

  return {
    id: options.id ?? generateId("document"),
    title: options.title,
    preview: summarizeText(options.plainText),
    updated_at: updatedAt,
    workflow_hints: options.workflowHints ?? [],
    created_at: createdAt,
    plain_text: options.plainText,
    html: textToHtml(options.plainText),
    content: textToContent(options.plainText),
    content_revision: options.contentRevision ?? 0,
    thread_id: options.threadId ?? null,
    messages: options.messages ?? [],
    snapshot_count: options.snapshotCount ?? 0,
  };
}

function seedState(): BrowserPreviewState {
  const now = Date.now();
  const activeDocument = createDocument({
    id: "browser-doc-welcome",
    title: "Velkommen til Lokal Skriveflate",
    plainText:
      "Dette er nettleserforhåndsvisningen av skriveflaten. Du kan redigere dokumentet, teste snarveier og se AI-preview uten at desktopbroen er lastet.\n\nNår du åpner Tauri-appen brukes den ekte lokale lagringen og runtime-en igjen.",
    workflowHints: ["skriveassistent"],
    createdAt: new Date(now - 1000 * 60 * 60).toISOString(),
    updatedAt: new Date(now - 1000 * 60 * 8).toISOString(),
    messages: [
      createMessage(
        "system",
        "Browserforhåndsvisning er aktiv. Dokument- og AI-data lagres lokalt i nettleseren for denne demoen.",
      ),
      createMessage(
        "assistant",
        "Jeg kan gi simulerte forslag her, mens desktopappen bruker den ekte lokale motoren.",
      ),
    ],
    snapshotCount: 2,
    threadId: BROWSER_THREAD_ID,
  });

  const reportDocument = createDocument({
    id: "browser-doc-report",
    title: "Rapportutkast for uke 10",
    plainText:
      "Måloppnåelsen er stabil denne uken. Teamet har lukket to prioriterte saker og forbereder neste leveranse.\n\nForeslåtte neste steg: ferdigstille sammendrag, bekrefte risikoer og sende rapporten til gjennomlesing.",
    workflowHints: ["skriveassistent"],
    createdAt: new Date(now - 1000 * 60 * 60 * 3).toISOString(),
    updatedAt: new Date(now - 1000 * 60 * 35).toISOString(),
  });

  const meetingDocument = createDocument({
    id: "browser-doc-meeting",
    title: "Motenotat",
    plainText:
      "Status fra siste mote: beslutning om leveransevindu, ansvar for oppfolging og behov for kort sammendrag til ledelsen.",
    workflowHints: ["skriveassistent"],
    createdAt: new Date(now - 1000 * 60 * 60 * 8).toISOString(),
    updatedAt: new Date(now - 1000 * 60 * 90).toISOString(),
  });

  const settings: AppSettings = {
    selected_model: DEFAULT_MODEL_ID,
    preferred_tone: "Klar og profesjonell",
  };

  return {
    documents: [activeDocument, reportDocument, meetingDocument],
    activeDocumentId: activeDocument.id,
    settings,
    runtimeStatus: createRuntimeStatus(settings.selected_model),
    recoveryNotices: [],
  };
}

function hasBrowserStorage() {
  return typeof window !== "undefined" && typeof window.localStorage !== "undefined";
}

function persistState(state: BrowserPreviewState) {
  cachedState = cloneValue(state);

  if (!hasBrowserStorage()) {
    return;
  }

  window.localStorage.setItem(STORAGE_KEY, JSON.stringify(cachedState));
}

function isStoredDocument(value: unknown): value is StoredDocument {
  return Boolean(
    value &&
      typeof value === "object" &&
      "id" in value &&
      "title" in value &&
      "plain_text" in value &&
      "content" in value,
  );
}

function sanitizeState(state: BrowserPreviewState) {
  const documents = state.documents.filter(isStoredDocument);
  const activeDocumentId =
    documents.find((document) => document.id === state.activeDocumentId)?.id ??
    documents[0]?.id ??
    createDocument({
      title: "Nytt lokalt dokument",
      plainText:
        "Start her. Denne nettleserforhåndsvisningen bruker lokale demo-data i stedet for Tauri-broen.",
      workflowHints: ["skriveassistent"],
    }).id;

  const nextDocuments =
    documents.length > 0
      ? documents
      : [
          createDocument({
            id: activeDocumentId,
            title: "Nytt lokalt dokument",
            plainText:
              "Start her. Denne nettleserforhåndsvisningen bruker lokale demo-data i stedet for Tauri-broen.",
            workflowHints: ["skriveassistent"],
          }),
        ];

  const selectedModel =
    AVAILABLE_MODELS.find((model) => model.id === state.settings.selected_model)?.id ??
    DEFAULT_MODEL_ID;

  return {
    ...state,
    documents: nextDocuments
      .map((document) => ({
        ...document,
        preview: summarizeText(document.plain_text),
      }))
      .sort((left, right) => right.updated_at.localeCompare(left.updated_at)),
    activeDocumentId:
      nextDocuments.find((document) => document.id === activeDocumentId)?.id ??
      nextDocuments[0].id,
    settings: {
      selected_model: selectedModel,
      preferred_tone: state.settings.preferred_tone || "Klar og profesjonell",
    },
    runtimeStatus: {
      ...state.runtimeStatus,
      ...createRuntimeStatus(selectedModel),
      selected_model: selectedModel,
      available_models: syncModels(selectedModel),
    },
  };
}

function readState() {
  if (cachedState) {
    return cloneValue(cachedState);
  }

  if (!hasBrowserStorage()) {
    const seeded = seedState();
    persistState(seeded);
    return cloneValue(seeded);
  }

  const rawValue = window.localStorage.getItem(STORAGE_KEY);
  if (!rawValue) {
    const seeded = seedState();
    persistState(seeded);
    return cloneValue(seeded);
  }

  try {
    const parsed = JSON.parse(rawValue) as BrowserPreviewState;
    const sanitized = sanitizeState(parsed);
    persistState(sanitized);
    return cloneValue(sanitized);
  } catch {
    const seeded = seedState();
    persistState(seeded);
    return cloneValue(seeded);
  }
}

function getState() {
  return readState();
}

function saveState(state: BrowserPreviewState) {
  persistState(
    sanitizeState({
      ...state,
      runtimeStatus: {
        ...state.runtimeStatus,
        selected_model: state.settings.selected_model,
        available_models: syncModels(state.settings.selected_model),
      },
    }),
  );
}

function activeDocumentFromState(state: BrowserPreviewState) {
  return (
    state.documents.find((document) => document.id === state.activeDocumentId) ??
    state.documents[0]
  );
}

function toWorkspaceSnapshot(state: BrowserPreviewState): WorkspaceSnapshot {
  const activeDocument = activeDocumentFromState(state);

  return {
    documents: cloneValue(
      state.documents.map((document) => ({
        id: document.id,
        title: document.title,
        preview: document.preview,
        updated_at: document.updated_at,
        workflow_hints: document.workflow_hints,
      })),
    ),
    active_document: cloneValue(activeDocument),
    recovery_notices: [...state.recoveryNotices],
  };
}

function updateDocumentInState(
  state: BrowserPreviewState,
  documentId: string,
  updater: (document: StoredDocument) => StoredDocument,
) {
  const document = state.documents.find((entry) => entry.id === documentId);

  if (!document) {
    throw new Error("Fant ikke dokumentet i nettleserforhåndsvisningen.");
  }

  const updatedDocument = updater(document);
  state.documents = state.documents
    .map((entry) => (entry.id === documentId ? updatedDocument : entry))
    .sort((left, right) => right.updated_at.localeCompare(left.updated_at));
  return updatedDocument;
}

function buildAssistantText(request: AssistantTurnInput, document: StoredDocument) {
  const quickActionLabel = (() => {
    switch (request.quick_action) {
      case "continue_writing":
        return "Skriv videre";
      case "improve_selection":
        return "Forbedre markert tekst";
      case "make_formal":
        return "Gjør mer formelt";
      case "summarize":
        return "Lag oppsummering";
      case "draft_report":
        return "Lag rapportutkast";
      case "meeting_notes":
        return "Lag motereferat";
      default:
        return "Svar på foresporsel";
    }
  })();

  const sourceText = request.selection_text?.trim() || document.plain_text.trim();
  const excerpt = sourceText
    .split(/\s+/)
    .filter(Boolean)
    .slice(0, 24)
    .join(" ");

  return `${quickActionLabel} i ${request.tone.toLowerCase()} tone.

Arbeidsgrunnlag: ${excerpt || "Dokumentet er klart for nytt innhold."}

Neste forslag:
- Tydeliggor hovedpoenget i forste linje.
- Hold spraket konkret og lokalt forankret.
- Avslutt med et kort neste steg eller anbefaling.`;
}

function buildAssistantReply(request: AssistantTurnInput) {
  if (request.quick_action) {
    return `Simulert AI-preview klar for ${request.quick_action}. I desktopappen blir dette erstattet av et ekte lokalt svar.`;
  }

  return "Simulert AI-preview klar. Desktopappen bruker den ekte lokale motoren, men nettleserforhåndsvisningen lar deg teste hele flyten.";
}

function buildEditorAction(
  request: AssistantTurnInput,
  document: StoredDocument,
): AssistantTurnResult["editor_action"] {
  const content = buildAssistantText(request, document);

  return {
    action_type: request.selection_text?.trim() ? "replace_selection" : "append_to_document",
    title:
      request.quick_action === "summarize"
        ? "Kort oppsummering"
        : request.quick_action === "meeting_notes"
          ? "Motenotat"
          : request.quick_action === "draft_report"
            ? "Rapportutkast"
            : "Forslag fra lokal preview",
    rationale:
      "Dette er et simulert forslag for nettleserforhåndsvisningen, slik at du kan vurdere flyt og UI uten Tauri-broen.",
    content,
  };
}

function slugify(value: string) {
  return value
    .toLowerCase()
    .normalize("NFKD")
    .replace(/[^\w\s-]/g, "")
    .trim()
    .replace(/\s+/g, "-")
    .replace(/-+/g, "-");
}

function triggerBrowserDownload(filename: string, content: string, mimeType: string) {
  if (typeof document === "undefined" || typeof URL === "undefined") {
    return;
  }

  const blob = new Blob([content], { type: mimeType });
  const link = document.createElement("a");
  const objectUrl = URL.createObjectURL(blob);

  link.href = objectUrl;
  link.download = filename;
  link.rel = "noopener";
  link.click();

  window.setTimeout(() => {
    URL.revokeObjectURL(objectUrl);
  }, 0);
}

export function bootstrapBrowserPreview(): AppBootstrap {
  const state = getState();
  const snapshot = toWorkspaceSnapshot(state);

  return {
    ...snapshot,
    runtime_status: cloneValue(state.runtimeStatus),
    settings: cloneValue(state.settings),
  };
}

export function createBrowserPreviewDocument(title?: string): WorkspaceSnapshot {
  const state = getState();
  const timestamp = nowIso();
  const document = createDocument({
    title: title?.trim() || "Nytt lokalt dokument",
    plainText:
      "Start her. Browserforhåndsvisningen lagrer dette lokalt i nettleseren, mens desktopappen bruker den ekte Tauri-lagringen.",
      workflowHints: ["skriveassistent"],
    createdAt: timestamp,
    updatedAt: timestamp,
    messages: [
      createMessage(
        "system",
        "Nytt dokument opprettet i nettleserforhåndsvisningen. Endringer lagres lokalt i denne nettleseren.",
      ),
    ],
  });

  state.documents = [document, ...state.documents];
  state.activeDocumentId = document.id;
  saveState(state);
  return toWorkspaceSnapshot(state);
}

export function openBrowserPreviewDocument(documentId: string): WorkspaceSnapshot {
  const state = getState();

  if (!state.documents.some((document) => document.id === documentId)) {
    throw new Error("Fant ikke dokumentet du prøvde å åpne.");
  }

  state.activeDocumentId = documentId;
  saveState(state);
  return toWorkspaceSnapshot(state);
}

export function saveBrowserPreviewDocument(document: SaveDocumentInput): StoredDocument {
  const state = getState();
  const savedDocument = updateDocumentInState(state, document.id, (currentDocument) => {
    const contentChanged =
      currentDocument.title !== document.title ||
      JSON.stringify(currentDocument.content) !== JSON.stringify(document.content) ||
      currentDocument.html !== document.html ||
      currentDocument.plain_text !== document.plain_text;

    if (!contentChanged) {
      return currentDocument;
    }

    return {
      ...currentDocument,
      title: document.title,
      content: cloneValue(document.content),
      html: document.html,
      plain_text: document.plain_text,
      preview: summarizeText(document.plain_text),
      updated_at: nowIso(),
      snapshot_count:
        currentDocument.plain_text !== document.plain_text
          ? Math.min(currentDocument.snapshot_count + 1, 12)
          : currentDocument.snapshot_count,
      content_revision: currentDocument.content_revision + 1,
    };
  });

  saveState(state);
  return cloneValue(savedDocument);
}

export function sendBrowserPreviewAssistantTurn(
  request: AssistantTurnInput,
): AssistantTurnResult {
  const state = getState();
  const currentDocument = state.documents.find((document) => document.id === request.document_id);

  if (!currentDocument) {
    throw new Error("Fant ikke dokumentet som skulle sendes til lokal preview.");
  }

  if (currentDocument.content_revision !== request.document_revision) {
    throw makeCommandError({
      code: "document_conflict",
      message:
        "Dokumentet ble endret før preview-forespørselen kunne starte. Oppdater dokumentet og prøv igjen.",
      retryable: true,
      action: "reload_document",
    });
  }

  const assistantReply = buildAssistantReply(request);
  const editorAction = buildEditorAction(request, currentDocument);
  const updatedDocument = updateDocumentInState(state, request.document_id, (document) => ({
    ...document,
    updated_at: nowIso(),
    thread_id: BROWSER_THREAD_ID,
    messages: [
      ...document.messages,
      createMessage("user", request.prompt),
      createMessage("assistant", assistantReply),
    ].slice(-40),
  }));

  saveState(state);

  return {
    document: cloneValue(updatedDocument),
    runtime_status: cloneValue(state.runtimeStatus),
    assistant_reply: assistantReply,
    editor_action: editorAction,
  };
}

export function refreshBrowserPreviewRuntimeStatus(): RuntimeStatus {
  const state = getState();
  return cloneValue(state.runtimeStatus);
}

function applyRuntimeMessage(message: string) {
  const state = getState();
  state.runtimeStatus = {
    ...state.runtimeStatus,
    runtime_state: "ready",
    can_send: true,
    will_start_on_demand: false,
    blocking_reason: null,
    codex: {
      ...state.runtimeStatus.codex,
      available: true,
      running: true,
    },
    local_ai: {
      ...state.runtimeStatus.local_ai,
      available: true,
      running: true,
    },
  };
  saveState(state);

  return {
    message,
    runtime_status: cloneValue(state.runtimeStatus),
    settings: cloneValue(state.settings),
  } satisfies RuntimeActionResult;
}

export function prepareBrowserPreviewLocalAi(): RuntimeActionResult {
  return applyRuntimeMessage(
    "Browserforhandsvisningen bruker allerede en simulert lokal AI. Desktopappen bruker ekte oppsett og runtime.",
  );
}

export function repairBrowserPreviewLocalAi(): RuntimeActionResult {
  return applyRuntimeMessage(
    "Simulert runtime er friskmeldt for nettleserforhandsvisningen. Ingen filer trengte reparasjon her.",
  );
}

export function activateBrowserPreviewModel(modelId: string): RuntimeActionResult {
  const state = getState();
  const model = AVAILABLE_MODELS.find((entry) => entry.id === modelId);

  if (!model) {
    throw makeCommandError({
      code: "runtime_not_ready",
      message: "Fant ikke modellen du prøvde å velge.",
      retryable: false,
      action: null,
    });
  }

  state.settings.selected_model = modelId;
  state.runtimeStatus = {
    ...state.runtimeStatus,
    selected_model: modelId,
    available_models: syncModels(modelId),
  };
  saveState(state);

  return {
    message: `Byttet til ${model.label} i nettleserforhandsvisningen.`,
    runtime_status: cloneValue(state.runtimeStatus),
    settings: cloneValue(state.settings),
  };
}

export function exportBrowserPreviewDocument(
  documentId: string,
  format: OutputFormat,
): ExportResult {
  const state = getState();
  const document = state.documents.find((entry) => entry.id === documentId);

  if (!document) {
    throw new Error("Fant ikke dokumentet som skulle eksporteres.");
  }

  const safeTitle = slugify(document.title) || "lokalt-dokument";
  const timestamp = new Date().toISOString().replace(/[:.]/g, "-");

  if (format === "html") {
    const filename = `${safeTitle}-${timestamp}.html`;
    const html = `<!doctype html><html lang="no"><head><meta charset="utf-8" /><title>${escapeHtml(document.title)}</title></head><body>${document.html}</body></html>`;
    triggerBrowserDownload(filename, html, "text/html;charset=utf-8");
    return { path: filename, format };
  }

  if (format === "txt") {
    const filename = `${safeTitle}-${timestamp}.txt`;
    triggerBrowserDownload(filename, document.plain_text, "text/plain;charset=utf-8");
    return { path: filename, format };
  }

  return {
    path: "PDF-eksport krever desktopappen. Nettleserforhandsvisningen holder resten av flyten levende.",
    format,
  };
}

export function saveBrowserPreviewSettings(settings: AppSettings): AppSettings {
  const state = getState();
  state.settings = cloneValue(settings);
  state.runtimeStatus = {
    ...state.runtimeStatus,
    selected_model: settings.selected_model,
    available_models: syncModels(settings.selected_model),
  };
  saveState(state);
  return cloneValue(state.settings);
}
