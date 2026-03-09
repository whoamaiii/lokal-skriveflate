import {
  startTransition,
  useEffect,
  useEffectEvent,
  useMemo,
  useRef,
  useState,
} from "react";
import { AlertTriangle, RefreshCcw } from "lucide-react";
import type {
  AppBootstrap,
  AssistantTurnInput,
  CommandError,
  EditorSelection,
  PendingActionPreview,
  StoredDocument,
  WorkspaceSnapshot,
} from "./types";
import {
  bootstrapApp,
  createDocument,
  exportDocument,
  openDocument,
  prepareLocalAi,
  refreshRuntimeStatus,
  repairLocalAi,
  saveDocument,
  saveSettings,
  sendAssistantTurn,
} from "./lib/tauri";
import {
  hasDocumentChanges,
  mergeAssistantMetadata,
  mergeSavedDocument,
  toSaveDocumentInput,
} from "./lib/documentState";
import { Sidebar, type LeftDrawerTab } from "./components/Sidebar";
import { EditorPane, type EditorPaneHandle } from "./components/EditorPane";
import { ChatPane } from "./components/ChatPane";
import { ActionPreview } from "./components/ActionPreview";
import { FloatingNav, type NavSection } from "./components/FloatingNav";
import { DrawerToggle } from "./components/DrawerToggle";
import {
  THEME_STORAGE_KEY,
  applyTheme,
  getInitialTheme,
  type ThemeMode,
} from "./lib/theme";
import type { ToolbarActionId } from "./components/editorToolbar";

const MOBILE_NAV_BREAKPOINT = "(max-width: 1180px)";
const AUTOSAVE_DELAY_MS = 700;

type RetryAction = (() => Promise<unknown> | unknown) | null;
type DrawerSide = "left" | "right";
type PendingActionMap = Record<string, PendingActionPreview>;

function updateStoredDocument(
  document: StoredDocument,
  changes: Partial<StoredDocument>,
): StoredDocument {
  return {
    ...document,
    ...changes,
  };
}

function summaryFromDocument(document: StoredDocument) {
  return {
    id: document.id,
    title: document.title,
    preview: document.preview,
    updated_at: document.updated_at,
    workflow_hints: document.workflow_hints,
  };
}

function applyWorkspaceSnapshot(
  current: AppBootstrap | null,
  snapshot: WorkspaceSnapshot,
) {
  if (!current) {
    return current;
  }

  return {
    ...current,
    documents: snapshot.documents,
    active_document: snapshot.active_document,
  };
}

function extractErrorMessage(error: unknown, fallback: string) {
  if (
    error &&
    typeof error === "object" &&
    "message" in error &&
    typeof (error as { message: unknown }).message === "string" &&
    (error as { message: string }).message.trim()
  ) {
    return (error as { message: string }).message;
  }

  if (error instanceof Error && error.message.trim()) {
    return error.message;
  }

  if (typeof error === "string" && error.trim()) {
    return error;
  }

  return fallback;
}

function isCommandError(error: unknown): error is CommandError {
  return Boolean(
    error &&
      typeof error === "object" &&
      "code" in error &&
      "message" in error &&
      "retryable" in error,
  );
}

function isDocumentConflictError(error: unknown) {
  return isCommandError(error) && error.code === "document_conflict";
}

function matchCompactLayout() {
  return typeof window !== "undefined"
    ? window.matchMedia(MOBILE_NAV_BREAKPOINT).matches
    : false;
}

export default function App() {
  const [workspace, setWorkspace] = useState<AppBootstrap | null>(null);
  const [prompt, setPrompt] = useState("");
  const [selection, setSelection] = useState<EditorSelection>({
    text: "",
    from: 1,
    to: 1,
  });
  const [pendingActionsByDocument, setPendingActionsByDocument] =
    useState<PendingActionMap>({});
  const [isSending, setIsSending] = useState(false);
  const [isLoading, setIsLoading] = useState(true);
  const [bootstrapError, setBootstrapError] = useState<string | null>(null);
  const [workspaceError, setWorkspaceError] = useState<string | null>(null);
  const [workspaceRetryLabel, setWorkspaceRetryLabel] = useState<string | null>(null);
  const [recoveryNotices, setRecoveryNotices] = useState<string[]>([]);
  const [isRuntimeActionPending, setIsRuntimeActionPending] = useState(false);
  const [lastExportPath, setLastExportPath] = useState<string | null>(null);
  const [runtimeNotice, setRuntimeNotice] = useState<string | null>(null);
  const [runtimeError, setRuntimeError] = useState<string | null>(null);
  const [runtimeRetryLabel, setRuntimeRetryLabel] = useState<string | null>(null);
  const [activeNav, setActiveNav] = useState<NavSection>("search");
  const [themeMode, setThemeMode] = useState<ThemeMode>(getInitialTheme);
  const [isThemeAnimating, setIsThemeAnimating] = useState(false);
  const [isCompactLayout, setIsCompactLayout] = useState(matchCompactLayout);
  const [isLeftDrawerOpen, setIsLeftDrawerOpen] = useState(false);
  const [isRightDrawerOpen, setIsRightDrawerOpen] = useState(false);
  const [activeLeftDrawerTab, setActiveLeftDrawerTab] =
    useState<LeftDrawerTab>("documents");
  const [activeToolbarActions, setActiveToolbarActions] = useState<ToolbarActionId[]>(
    [],
  );
  const editorRef = useRef<EditorPaneHandle | null>(null);
  const leftDrawerRef = useRef<HTMLElement | null>(null);
  const workspacePanelRef = useRef<HTMLElement | null>(null);
  const rightDrawerRef = useRef<HTMLElement | null>(null);
  const titleInputRef = useRef<HTMLInputElement | null>(null);
  const themeAnimationTimeoutRef = useRef<number | null>(null);
  const lastActiveDrawerRef = useRef<DrawerSide | null>(null);
  const workspaceRef = useRef<AppBootstrap | null>(null);
  const workspaceRetryRef = useRef<RetryAction>(null);
  const runtimeRetryRef = useRef<RetryAction>(null);
  const persistedDocumentsRef = useRef(
    new Map<string, ReturnType<typeof toSaveDocumentInput>>(),
  );
  const queuedSavesRef = useRef(
    new Map<string, ReturnType<typeof toSaveDocumentInput>>(),
  );
  const saveLoopsRef = useRef(new Map<string, Promise<StoredDocument | null>>());

  const activeDocument = workspace?.active_document ?? null;
  workspaceRef.current = workspace;
  const activePendingAction = activeDocument
    ? pendingActionsByDocument[activeDocument.id] ?? null
    : null;

  const setFocusedZone = useEffectEvent((section: NavSection) => {
    setActiveNav(section);

    if (section === "home") {
      lastActiveDrawerRef.current = "left";
    } else if (section === "user") {
      lastActiveDrawerRef.current = "right";
    }
  });

  const focusWorkspace = useEffectEvent((shouldFocusEditor = false) => {
    setFocusedZone("search");

    if (isCompactLayout) {
      setIsLeftDrawerOpen(false);
      setIsRightDrawerOpen(false);
    }

    if (!shouldFocusEditor) {
      return;
    }

    window.requestAnimationFrame(() => {
      if (editorRef.current) {
        editorRef.current.focusEditor();
        return;
      }

      if (titleInputRef.current) {
        titleInputRef.current.focus();
        return;
      }

      workspacePanelRef.current?.focus();
    });
  });

  const closeLeftDrawer = useEffectEvent(() => {
    setIsLeftDrawerOpen(false);

    if (isRightDrawerOpen) {
      setFocusedZone("user");
      return;
    }

    setFocusedZone("search");
  });

  const closeRightDrawer = useEffectEvent(() => {
    setIsRightDrawerOpen(false);

    if (isLeftDrawerOpen) {
      setFocusedZone("home");
      return;
    }

    setFocusedZone("search");
  });

  const openLeftDrawer = useEffectEvent(
    (tab: LeftDrawerTab = activeLeftDrawerTab, shouldFocus = false) => {
      setActiveLeftDrawerTab(tab);
      setIsLeftDrawerOpen(true);
      if (isCompactLayout) {
        setIsRightDrawerOpen(false);
      }
      setFocusedZone("home");

      if (shouldFocus) {
        window.requestAnimationFrame(() => {
          leftDrawerRef.current?.focus();
        });
      }
    },
  );

  const openRightDrawer = useEffectEvent((shouldFocus = false) => {
    setIsRightDrawerOpen(true);
    if (isCompactLayout) {
      setIsLeftDrawerOpen(false);
    }
    setFocusedZone("user");

    if (shouldFocus) {
      window.requestAnimationFrame(() => {
        rightDrawerRef.current?.focus();
      });
    }
  });

  const toggleLeftDrawer = useEffectEvent(() => {
    if (isLeftDrawerOpen) {
      closeLeftDrawer();
      return;
    }

    openLeftDrawer(activeLeftDrawerTab, true);
  });

  const toggleRightDrawer = useEffectEvent(() => {
    if (isRightDrawerOpen) {
      closeRightDrawer();
      return;
    }

    openRightDrawer(true);
  });

  function rememberPersistedDocument(document: StoredDocument) {
    persistedDocumentsRef.current.set(document.id, toSaveDocumentInput(document));
  }

  function hasUnsavedChanges(document: StoredDocument | null) {
    if (!document) {
      return false;
    }

    return hasDocumentChanges(
      document,
      persistedDocumentsRef.current.get(document.id),
    );
  }

  function clearNoticeState() {
    setWorkspaceError(null);
    setWorkspaceRetryLabel(null);
    workspaceRetryRef.current = null;
    setRuntimeError(null);
    setRuntimeRetryLabel(null);
    runtimeRetryRef.current = null;
  }

  function clearWorkspaceAlert() {
    setWorkspaceError(null);
    setWorkspaceRetryLabel(null);
    workspaceRetryRef.current = null;
  }

  function showWorkspaceAlert(
    message: string,
    retryLabel?: string,
    retryAction?: RetryAction,
  ) {
    setWorkspaceError(message);
    setWorkspaceRetryLabel(retryLabel ?? null);
    workspaceRetryRef.current = retryAction ?? null;
  }

  function clearRuntimeAlert() {
    setRuntimeError(null);
    setRuntimeRetryLabel(null);
    runtimeRetryRef.current = null;
  }

  function clearPendingAction(documentId: string) {
    setPendingActionsByDocument((current) => {
      if (!current[documentId]) {
        return current;
      }

      const next = { ...current };
      delete next[documentId];
      return next;
    });
  }

  function showRuntimeAlert(
    message: string,
    retryLabel?: string,
    retryAction?: RetryAction,
  ) {
    setRuntimeError(message);
    setRuntimeRetryLabel(retryLabel ?? null);
    runtimeRetryRef.current = retryAction ?? null;
  }

  const bootstrap = useEffectEvent(async () => {
    setIsLoading(true);
    setBootstrapError(null);

    try {
      const payload = await bootstrapApp();
      rememberPersistedDocument(payload.active_document);
      startTransition(() => {
        setWorkspace(payload);
        setRecoveryNotices(payload.recovery_notices);
        clearNoticeState();
        setIsLoading(false);
      });
    } catch (error) {
      setBootstrapError(
        extractErrorMessage(error, "Kunne ikke starte arbeidsflaten."),
      );
      setIsLoading(false);
    }
  });

  useEffect(() => {
    void bootstrap();
  }, []);

  useEffect(() => {
    applyTheme(themeMode);
    window.localStorage.setItem(THEME_STORAGE_KEY, themeMode);
  }, [themeMode]);

  useEffect(() => {
    const mediaQuery = window.matchMedia(MOBILE_NAV_BREAKPOINT);
    const handleChange = (event: MediaQueryListEvent) => {
      setIsCompactLayout(event.matches);
    };

    setIsCompactLayout(mediaQuery.matches);

    if (typeof mediaQuery.addEventListener === "function") {
      mediaQuery.addEventListener("change", handleChange);
      return () => mediaQuery.removeEventListener("change", handleChange);
    }

    mediaQuery.addListener(handleChange);
    return () => mediaQuery.removeListener(handleChange);
  }, []);

  useEffect(() => {
    if (!isCompactLayout || !isLeftDrawerOpen || !isRightDrawerOpen) {
      return;
    }

    if (lastActiveDrawerRef.current === "right") {
      setIsLeftDrawerOpen(false);
      return;
    }

    setIsRightDrawerOpen(false);
  }, [isCompactLayout, isLeftDrawerOpen, isRightDrawerOpen]);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape") {
        return;
      }

      if (isCompactLayout) {
        if (isRightDrawerOpen) {
          event.preventDefault();
          closeRightDrawer();
          return;
        }

        if (isLeftDrawerOpen) {
          event.preventDefault();
          closeLeftDrawer();
        }

        return;
      }

      if (lastActiveDrawerRef.current === "right" && isRightDrawerOpen) {
        event.preventDefault();
        closeRightDrawer();
        return;
      }

      if (lastActiveDrawerRef.current === "left" && isLeftDrawerOpen) {
        event.preventDefault();
        closeLeftDrawer();
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [
    closeLeftDrawer,
    closeRightDrawer,
    isCompactLayout,
    isLeftDrawerOpen,
    isRightDrawerOpen,
  ]);

  useEffect(() => {
    return () => {
      if (themeAnimationTimeoutRef.current) {
        window.clearTimeout(themeAnimationTimeoutRef.current);
      }
    };
  }, []);

  const applySavedDocumentToWorkspace = useEffectEvent(
    (saved: StoredDocument, sentPayload: ReturnType<typeof toSaveDocumentInput>) => {
      rememberPersistedDocument(saved);
      startTransition(() => {
        setWorkspace((current) => {
          if (!current) {
            return current;
          }

          const nextActiveDocument =
            current.active_document.id === saved.id
              ? mergeSavedDocument(current.active_document, saved, sentPayload)
              : current.active_document;

          return {
            ...current,
            active_document: nextActiveDocument,
            documents: current.documents.map((document) =>
              document.id === saved.id
                ? summaryFromDocument(
                    nextActiveDocument.id === saved.id ? nextActiveDocument : saved,
                  )
                : document,
            ),
          };
        });
      });
    },
  );

  const runSaveLoop = useEffectEvent(async (documentId: string) => {
    let latestSaved: StoredDocument | null = null;

    while (true) {
      const nextSave = queuedSavesRef.current.get(documentId);
      if (!nextSave) {
        return latestSaved;
      }

      queuedSavesRef.current.delete(documentId);

      try {
        const saved = await saveDocument(nextSave);
        latestSaved = saved;
        applySavedDocumentToWorkspace(saved, nextSave);
        clearWorkspaceAlert();
      } catch (error) {
        if (!queuedSavesRef.current.has(documentId)) {
          queuedSavesRef.current.set(documentId, nextSave);
        }

        throw error;
      }
    }
  });

  const ensureSaveLoop = useEffectEvent((documentId: string) => {
    const existingPromise = saveLoopsRef.current.get(documentId);
    if (existingPromise) {
      return existingPromise;
    }

    const loopPromise = runSaveLoop(documentId).finally(() => {
      saveLoopsRef.current.delete(documentId);
    });
    saveLoopsRef.current.set(documentId, loopPromise);
    return loopPromise;
  });

  const flushActiveDocument = useEffectEvent(async () => {
    const currentDocument = workspaceRef.current?.active_document ?? null;
    if (!currentDocument || !hasUnsavedChanges(currentDocument)) {
      return null;
    }

    queuedSavesRef.current.set(currentDocument.id, toSaveDocumentInput(currentDocument));

    try {
      const saved = await ensureSaveLoop(currentDocument.id);
      clearWorkspaceAlert();
      return saved;
    } catch (error) {
      showWorkspaceAlert(
        extractErrorMessage(
          error,
          "Kunne ikke lagre dokumentet før handlingen fortsatte.",
        ),
        "Prøv lagring igjen",
        () => flushActiveDocument(),
      );
      throw error;
    }
  });

  useEffect(() => {
    if (!activeDocument || !hasUnsavedChanges(activeDocument)) {
      return;
    }

    const timer = window.setTimeout(() => {
      void flushActiveDocument();
    }, AUTOSAVE_DELAY_MS);

    return () => window.clearTimeout(timer);
  }, [
    activeDocument?.content_revision,
    activeDocument?.html,
    activeDocument?.id,
    activeDocument?.plain_text,
    activeDocument?.title,
    flushActiveDocument,
  ]);

  const wordCount = activeDocument?.plain_text.trim()
    ? activeDocument.plain_text.trim().split(/\s+/).length
    : 0;

  const pendingActionIsStale = Boolean(
    activePendingAction &&
      (!activeDocument ||
        activePendingAction.document_id !== activeDocument.id ||
        activePendingAction.document_revision !== activeDocument.content_revision ||
        hasUnsavedChanges(activeDocument)),
  );
  const previewStateByDocumentId = Object.fromEntries(
    Object.entries(pendingActionsByDocument).map(([documentId, preview]) => [
      documentId,
      documentId === activeDocument?.id &&
      (preview.document_revision !== activeDocument.content_revision ||
        hasUnsavedChanges(activeDocument))
        ? "stale"
        : "ready",
    ]),
  ) as Record<string, "ready" | "stale">;

  const workspaceRetryAction = useMemo(
    () =>
      workspaceRetryLabel && workspaceRetryRef.current
        ? () => workspaceRetryRef.current?.()
        : undefined,
    [workspaceRetryLabel],
  );

  const runtimeRetryAction = useMemo(
    () =>
      runtimeRetryLabel && runtimeRetryRef.current
        ? () => runtimeRetryRef.current?.()
        : undefined,
    [runtimeRetryLabel],
  );

  function handleDocumentContent(
    content: StoredDocument["content"],
    html: string,
    plainText: string,
  ) {
    setWorkspace((current) => {
      if (!current || !current.active_document) {
        return current;
      }

      const updatedDocument = updateStoredDocument(current.active_document, {
        content,
        html,
        plain_text: plainText,
        preview: plainText.slice(0, 120),
      });

      return {
        ...current,
        active_document: updatedDocument,
        documents: current.documents.map((document) =>
          document.id === updatedDocument.id
            ? summaryFromDocument(updatedDocument)
            : document,
        ),
      };
    });
  }

  function handleTitleChange(title: string) {
    setWorkspace((current) => {
      if (!current || !current.active_document) {
        return current;
      }

      const updatedDocument = updateStoredDocument(current.active_document, {
        title,
      });

      return {
        ...current,
        active_document: updatedDocument,
        documents: current.documents.map((document) =>
          document.id === updatedDocument.id
            ? summaryFromDocument(updatedDocument)
            : document,
        ),
      };
    });
  }

  async function handleCreateDocument() {
    try {
      await flushActiveDocument();
      const snapshot = await createDocument();
      rememberPersistedDocument(snapshot.active_document);
      clearNoticeState();
      startTransition(() => {
        setWorkspace((current) => applyWorkspaceSnapshot(current, snapshot));
        setRecoveryNotices(snapshot.recovery_notices);
        setPrompt("");
      });
      focusWorkspace(true);
    } catch (error) {
      showWorkspaceAlert(
        extractErrorMessage(error, "Kunne ikke opprette et nytt dokument."),
        "Prøv igjen",
        () => handleCreateDocument(),
      );
    }
  }

  async function handleOpenDocument(documentId: string) {
    try {
      await flushActiveDocument();
      const snapshot = await openDocument(documentId);
      rememberPersistedDocument(snapshot.active_document);
      clearNoticeState();
      startTransition(() => {
        setWorkspace((current) => applyWorkspaceSnapshot(current, snapshot));
        setRecoveryNotices(snapshot.recovery_notices);
        setPrompt("");
      });
      focusWorkspace(true);
    } catch (error) {
      showWorkspaceAlert(
        extractErrorMessage(error, "Kunne ikke åpne dokumentet."),
        "Prøv igjen",
        () => handleOpenDocument(documentId),
      );
    }
  }

  async function reloadDocument(documentId: string) {
    try {
      await flushActiveDocument();
      const snapshot = await openDocument(documentId);
      rememberPersistedDocument(snapshot.active_document);
      startTransition(() => {
        setWorkspace((current) => applyWorkspaceSnapshot(current, snapshot));
        setRecoveryNotices(snapshot.recovery_notices);
      });
      focusWorkspace();
    } catch (error) {
      showWorkspaceAlert(
        extractErrorMessage(error, "Kunne ikke oppdatere dokumentet."),
        "Prøv igjen",
        () => reloadDocument(documentId),
      );
    }
  }

  async function handleSend(quickAction: string | null = null) {
    const trimmedPrompt = prompt.trim();
    if (!trimmedPrompt && !quickAction) {
      return;
    }

    setIsSending(true);
    clearRuntimeAlert();
    clearWorkspaceAlert();

    const selectionSnapshot = selection;

    try {
      const flushedDocument = await flushActiveDocument();
      const currentWorkspace = workspaceRef.current;
      const currentDocument = currentWorkspace?.active_document ?? null;

      if (!currentWorkspace || !currentDocument) {
        return;
      }

      const request: AssistantTurnInput = {
        document_id: currentDocument.id,
        document_revision:
          flushedDocument?.id === currentDocument.id
            ? flushedDocument.content_revision
            : currentDocument.content_revision,
        prompt:
          trimmedPrompt || "Bruk den valgte hurtigkommandoen på dokumentet.",
        quick_action: quickAction,
        tone: currentWorkspace.settings.preferred_tone,
        selection_text: selectionSnapshot.text || null,
      };

      const result = await sendAssistantTurn(request);
      rememberPersistedDocument(result.document);
      startTransition(() => {
        setWorkspace((current) => {
          if (!current) {
            return current;
          }

          const nextActiveDocument =
            current.active_document.id === result.document.id
              ? mergeAssistantMetadata(current.active_document, result.document)
              : current.active_document;

          return {
            ...current,
            active_document: nextActiveDocument,
            documents: current.documents.map((document) =>
              document.id === result.document.id
                ? summaryFromDocument(
                    nextActiveDocument.id === result.document.id
                      ? nextActiveDocument
                      : result.document,
                  )
                : document,
            ),
            runtime_status: result.runtime_status,
          };
        });

        setPendingActionsByDocument((current) => {
          const next = { ...current };

          if (result.editor_action) {
            next[result.document.id] = {
              action: result.editor_action,
              document_id: result.document.id,
              document_revision: result.document.content_revision,
              selection_from: selectionSnapshot.from,
              selection_to: selectionSnapshot.to,
              created_at: new Date().toISOString(),
            };
          } else {
            delete next[result.document.id];
          }

          return next;
        });
        setPrompt("");
      });
      focusWorkspace();
    } catch (error) {
      const nextMessage = extractErrorMessage(
        error,
        "Lokal AI kunne ikke svare akkurat nå.",
      );
      showRuntimeAlert(nextMessage);

      if (isDocumentConflictError(error)) {
        const currentDocumentId = workspaceRef.current?.active_document.id;
        if (currentDocumentId) {
          await reloadDocument(currentDocumentId);
        }
      } else {
        try {
          await handleRefreshRuntime();
        } catch {
          // handleRefreshRuntime already updated the workspace error state.
        }
      }
    } finally {
      setIsSending(false);
    }
  }

  function handleApplyAction() {
    if (!activePendingAction || !activeDocument) {
      return;
    }

    if (pendingActionIsStale) {
      showWorkspaceAlert(
        "Forslaget er utdatert fordi dokumentet ble endret. Be om et nytt forslag før du setter inn tekst.",
      );
      clearPendingAction(activeDocument.id);
      return;
    }

    editorRef.current?.applyAction(activePendingAction);
    clearPendingAction(activeDocument.id);
    clearWorkspaceAlert();
    focusWorkspace();
  }

  async function handleRefreshRuntime() {
    try {
      const status = await refreshRuntimeStatus();
      startTransition(() => {
        setWorkspace((current) =>
          current
            ? {
                ...current,
                runtime_status: status,
              }
            : null,
        );
      });
      clearWorkspaceAlert();
      return status;
    } catch (error) {
      showWorkspaceAlert(
        extractErrorMessage(error, "Kunne ikke oppdatere lokal runtime-status."),
        "Prøv igjen",
        () => handleRefreshRuntime(),
      );
      throw error;
    }
  }

  function applyRuntimeAction(
    message: string,
    runtimeStatus: AppBootstrap["runtime_status"],
    settings: AppBootstrap["settings"],
  ) {
    startTransition(() => {
      setWorkspace((current) =>
        current
          ? {
              ...current,
              runtime_status: runtimeStatus,
              settings,
            }
          : current,
      );
      setRuntimeNotice(message);
      clearRuntimeAlert();
      clearWorkspaceAlert();
    });
  }

  async function handleExport(format: "html" | "pdf" | "txt") {
    const currentDocument = workspaceRef.current?.active_document ?? null;
    if (!currentDocument) {
      return;
    }

    try {
      await flushActiveDocument();
      const result = await exportDocument(currentDocument.id, format);
      setLastExportPath(result.path);
      clearWorkspaceAlert();
    } catch (error) {
      showWorkspaceAlert(
        extractErrorMessage(error, "Kunne ikke eksportere dokumentet."),
        "Prøv igjen",
        () => handleExport(format),
      );
    }
  }

  async function handleToneChange(value: string) {
    setWorkspace((current) =>
      current
        ? {
            ...current,
            settings: {
              ...current.settings,
              preferred_tone: value,
            },
          }
        : current,
    );

    const currentWorkspace = workspaceRef.current;
    if (!currentWorkspace) {
      return;
    }

    try {
      await saveSettings({
        ...currentWorkspace.settings,
        preferred_tone: value,
      });
      clearWorkspaceAlert();
    } catch (error) {
      showWorkspaceAlert(
        extractErrorMessage(error, "Kunne ikke lagre tonevalget."),
        "Prøv igjen",
        () => handleToneChange(value),
      );
    }
  }

  async function handleSetupAction(action: "prepare" | "repair") {
    setIsRuntimeActionPending(true);
    try {
      const result =
        action === "prepare" ? await prepareLocalAi() : await repairLocalAi();
      applyRuntimeAction(result.message, result.runtime_status, result.settings);
    } catch (error) {
      showRuntimeAlert(
        extractErrorMessage(error, "Kunne ikke oppdatere lokal AI."),
        action === "prepare" ? "Prøv å klargjøre igjen" : "Prøv å reparere igjen",
        () => handleSetupAction(action),
      );
      try {
        await handleRefreshRuntime();
      } catch {
        // handleRefreshRuntime already updated the workspace error state.
      }
    } finally {
      setIsRuntimeActionPending(false);
    }
  }

  function handleThemeToggle() {
    if (themeAnimationTimeoutRef.current) {
      window.clearTimeout(themeAnimationTimeoutRef.current);
    }

    setThemeMode((currentTheme) => (currentTheme === "dark" ? "light" : "dark"));
    setIsThemeAnimating(true);

    themeAnimationTimeoutRef.current = window.setTimeout(() => {
      setIsThemeAnimating(false);
      themeAnimationTimeoutRef.current = null;
    }, 620);
  }

  function dismissRecoveryNotice(index: number) {
    setRecoveryNotices((current) =>
      current.filter((_, currentIndex) => currentIndex !== index),
    );
  }

  if (isLoading) {
    return (
      <main className="loading-screen" data-theme={themeMode}>
        <div className="loading-card">
          <p className="eyebrow">Starter lokal arbeidsflate</p>
          <h1>Setter opp dokumenter, lokal lagring og agentbro</h1>
        </div>
      </main>
    );
  }

  if (bootstrapError || !workspace || !activeDocument) {
    return (
      <main className="loading-screen" data-theme={themeMode}>
        <div className="loading-card loading-card-error">
          <p className="eyebrow">Kunne ikke starte arbeidsflaten</p>
          <h1>Oppstarten stoppet før dokumentene ble klare</h1>
          <p className="loading-card-message">
            {bootstrapError ?? "Ukjent oppstartsfeil."}
          </p>
          <div className="loading-card-actions">
            <button className="primary-button" onClick={() => void bootstrap()} type="button">
              <RefreshCcw size={16} />
              Prøv igjen
            </button>
          </div>
        </div>
      </main>
    );
  }

  const showMobileScrim = isCompactLayout && (isLeftDrawerOpen || isRightDrawerOpen);

  return (
    <>
      <main className="app-shell app-shell-minimal" data-active-nav={activeNav} data-theme={themeMode}>
        {showMobileScrim ? (
          <button
            aria-label="Lukk sidepanel"
            className="drawer-scrim"
            onClick={() => {
              if (isRightDrawerOpen) {
                closeRightDrawer();
              } else {
                closeLeftDrawer();
              }
            }}
            type="button"
          />
        ) : null}

        <DrawerToggle
          controls="documents-drawer"
          isOpen={isLeftDrawerOpen}
          label="Dokumenter"
          onActivate={() => setFocusedZone("home")}
          onToggle={() => void toggleLeftDrawer()}
          side="left"
        />

        <DrawerToggle
          controls="assistant-drawer"
          isOpen={isRightDrawerOpen}
          label="Assistent"
          onActivate={() => setFocusedZone("user")}
          onToggle={() => void toggleRightDrawer()}
          side="right"
        />

        <Sidebar
          activeDocument={activeDocument}
          activeTab={activeLeftDrawerTab}
          activeToolbarActions={activeToolbarActions}
          ariaHidden={!isLeftDrawerOpen}
          className={`drawer-panel drawer-panel-left ${isLeftDrawerOpen ? "is-open" : ""}`}
          documents={workspace.documents}
          hasUnsavedChanges={hasUnsavedChanges(activeDocument)}
          id="documents-drawer"
          lastExportPath={lastExportPath}
          onActivate={() => setFocusedZone("home")}
          onCreateDocument={() => void handleCreateDocument()}
          onDismissRecoveryNotice={dismissRecoveryNotice}
          onExport={(format) => void handleExport(format)}
          onSelectDocument={(documentId) => void handleOpenDocument(documentId)}
          onTabChange={(tab) => {
            setActiveLeftDrawerTab(tab);
            setFocusedZone("home");
          }}
          onToolbarAction={(actionId) => {
            editorRef.current?.runToolbarAction(actionId);
            focusWorkspace();
          }}
          panelRef={(node) => {
            leftDrawerRef.current = node;
          }}
          previewStateByDocumentId={previewStateByDocumentId}
          recoveryNotices={recoveryNotices}
          selection={selection}
          wordCount={wordCount}
        />

        <div className="workspace-shell">
          {workspaceError ? (
            <div className="workspace-toast">
              <div>
                <strong>Arbeidsflaten trenger oppmerksomhet</strong>
                <p>{workspaceError}</p>
              </div>
              <div className="workspace-alert-actions">
                {workspaceRetryAction && workspaceRetryLabel ? (
                  <button
                    className="ghost-button"
                    onClick={() => void workspaceRetryAction()}
                    type="button"
                  >
                    <RefreshCcw size={14} />
                    {workspaceRetryLabel}
                  </button>
                ) : null}
                <AlertTriangle size={18} />
              </div>
            </div>
          ) : null}

          <section
            className="workspace-panel workspace-panel-minimal app-section app-section-search"
            onFocusCapture={() => setFocusedZone("search")}
            onPointerDownCapture={() => setFocusedZone("search")}
            ref={(node) => {
              workspacePanelRef.current = node;
            }}
            tabIndex={-1}
          >
            <header className="workspace-header workspace-header-minimal">
              <div>
                <p className="eyebrow">Aktivt dokument</p>
                <input
                  className="document-title-input"
                  onChange={(event) => handleTitleChange(event.target.value)}
                  ref={titleInputRef}
                  value={activeDocument.title}
                />
              </div>
            </header>

            <EditorPane
              document={activeDocument}
              onContentChange={handleDocumentContent}
              onSelectionChange={setSelection}
              onToolbarStateChange={setActiveToolbarActions}
              ref={editorRef}
            />
          </section>

          {activePendingAction ? (
            <div className="floating-preview-tray">
              <ActionPreview
                action={activePendingAction.action}
                isStale={pendingActionIsStale}
                onApply={handleApplyAction}
                onDismiss={() => clearPendingAction(activeDocument.id)}
              />
            </div>
          ) : null}
        </div>

        <ChatPane
          ariaHidden={!isRightDrawerOpen}
          className={`drawer-panel drawer-panel-right ${isRightDrawerOpen ? "is-open" : ""}`}
          id="assistant-drawer"
          isRuntimeActionPending={isRuntimeActionPending}
          isSending={isSending}
          messages={activeDocument.messages}
          onActivate={() => setFocusedZone("user")}
          onPrepareLocalAi={() => void handleSetupAction("prepare")}
          onPromptChange={setPrompt}
          onQuickAction={(actionId) => void handleSend(actionId)}
          onRefreshRuntime={() => void handleRefreshRuntime()}
          onRepairLocalAi={() => void handleSetupAction("repair")}
          onRetryRuntimeAction={
            runtimeRetryAction ? () => void runtimeRetryAction() : undefined
          }
          onSend={() => void handleSend()}
          onToneChange={(value) => void handleToneChange(value)}
          panelRef={(node) => {
            rightDrawerRef.current = node;
          }}
          prompt={prompt}
          runtimeError={runtimeError}
          runtimeNotice={runtimeNotice}
          runtimeRetryLabel={runtimeRetryLabel}
          runtimeStatus={workspace.runtime_status}
          tone={workspace.settings.preferred_tone}
        />
      </main>

      <FloatingNav
        activeNav={activeNav}
        isThemeAnimating={isThemeAnimating}
        onSelectNav={(section) => {
          if (section === "home") {
            openLeftDrawer(activeLeftDrawerTab, true);
            return;
          }

          if (section === "user") {
            openRightDrawer(true);
            return;
          }

          focusWorkspace(true);
        }}
        onToggleTheme={handleThemeToggle}
        themeMode={themeMode}
      />
    </>
  );
}
