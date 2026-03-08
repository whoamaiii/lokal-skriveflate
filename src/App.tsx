import {
  startTransition,
  useEffect,
  useEffectEvent,
  useRef,
  useState,
} from "react";
import { Download, RefreshCcw, Save } from "lucide-react";
import type {
  AppBootstrap,
  AssistantTurnInput,
  EditorAction,
  OutputFormat,
  SaveDocumentInput,
  StoredDocument,
} from "./types";
import {
  activateModel,
  bootstrapApp,
  createDocument,
  exportDocument,
  openDocument,
  prepareLocalAi,
  repairLocalAi,
  refreshRuntimeStatus,
  saveDocument,
  saveSettings,
  sendAssistantTurn,
} from "./lib/tauri";
import { Sidebar } from "./components/Sidebar";
import { EditorPane, type EditorPaneHandle } from "./components/EditorPane";
import { ChatPane } from "./components/ChatPane";
import { ActionPreview } from "./components/ActionPreview";
import {
  FloatingNav,
  type NavSection,
} from "./components/FloatingNav";
import {
  THEME_STORAGE_KEY,
  applyTheme,
  getInitialTheme,
  type ThemeMode,
} from "./lib/theme";
import { StatusStrip } from "./components/StatusStrip";

const MOBILE_NAV_BREAKPOINT = "(max-width: 1180px)";

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

export default function App() {
  const [workspace, setWorkspace] = useState<AppBootstrap | null>(null);
  const [prompt, setPrompt] = useState("");
  const [selectionText, setSelectionText] = useState("");
  const [pendingAction, setPendingAction] = useState<EditorAction | null>(null);
  const [isSending, setIsSending] = useState(false);
  const [isLoading, setIsLoading] = useState(true);
  const [isRuntimeActionPending, setIsRuntimeActionPending] = useState(false);
  const [lastExportPath, setLastExportPath] = useState<string | null>(null);
  const [runtimeNotice, setRuntimeNotice] = useState<string | null>(null);
  const [runtimeError, setRuntimeError] = useState<string | null>(null);
  const [activeNav, setActiveNav] = useState<NavSection>("search");
  const [themeMode, setThemeMode] = useState<ThemeMode>(getInitialTheme);
  const [isThemeAnimating, setIsThemeAnimating] = useState(false);
  const editorRef = useRef<EditorPaneHandle | null>(null);
  const sectionRefs = useRef<Record<NavSection, HTMLElement | null>>({
    home: null,
    search: null,
    user: null,
  });
  const themeAnimationTimeoutRef = useRef<number | null>(null);

  const activeDocument = workspace?.active_document ?? null;

  const bootstrap = useEffectEvent(async () => {
    const payload = await bootstrapApp();
    startTransition(() => {
      setWorkspace(payload);
      setIsLoading(false);
    });
  });

  useEffect(() => {
    void bootstrap();
  }, [bootstrap]);

  useEffect(() => {
    applyTheme(themeMode);
    window.localStorage.setItem(THEME_STORAGE_KEY, themeMode);
  }, [themeMode]);

  useEffect(() => {
    return () => {
      if (themeAnimationTimeoutRef.current) {
        window.clearTimeout(themeAnimationTimeoutRef.current);
      }
    };
  }, []);

  const persistDocument = useEffectEvent(async (nextDocument: SaveDocumentInput) => {
    const saved = await saveDocument(nextDocument);
    startTransition(() => {
      setWorkspace((current) => {
        if (!current) {
          return current;
        }

        return {
          ...current,
          active_document: saved,
          documents: current.documents.map((document) =>
            document.id === saved.id ? summaryFromDocument(saved) : document,
          ),
        };
      });
    });
  });

  useEffect(() => {
    if (!activeDocument) {
      return;
    }

    const timer = window.setTimeout(() => {
      void persistDocument({
        id: activeDocument.id,
        title: activeDocument.title,
        content: activeDocument.content,
        html: activeDocument.html,
        plain_text: activeDocument.plain_text,
      });
    }, 700);

    return () => window.clearTimeout(timer);
  }, [
    activeDocument?.html,
    activeDocument?.id,
    activeDocument?.plain_text,
    activeDocument?.title,
    persistDocument,
  ]);

  const wordCount = activeDocument?.plain_text.trim()
    ? activeDocument.plain_text.trim().split(/\s+/).length
    : 0;

  const activateNavSection = useEffectEvent((section: NavSection, shouldScrollIntoView = false) => {
    setActiveNav(section);

    if (!shouldScrollIntoView || !window.matchMedia(MOBILE_NAV_BREAKPOINT).matches) {
      return;
    }

    const target = sectionRefs.current[section];
    if (!target) {
      return;
    }

    const prefersReducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    target.scrollIntoView({
      behavior: prefersReducedMotion ? "auto" : "smooth",
      block: "start",
    });
  });

  async function handleCreateDocument() {
    const snapshot = await createDocument();
    startTransition(() => {
      setWorkspace((current) =>
        current
          ? {
              ...current,
              documents: snapshot.documents,
              active_document: snapshot.active_document,
            }
          : null,
      );
      setPendingAction(null);
      setPrompt("");
    });
  }

  async function handleOpenDocument(documentId: string) {
    const snapshot = await openDocument(documentId);
    startTransition(() => {
      setWorkspace((current) =>
        current
          ? {
              ...current,
              documents: snapshot.documents,
              active_document: snapshot.active_document,
            }
          : null,
      );
      setPendingAction(null);
      setPrompt("");
    });
  }

  function handleDocumentContent(content: SaveDocumentInput["content"], html: string, plainText: string) {
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
          document.id === updatedDocument.id ? summaryFromDocument(updatedDocument) : document,
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
          document.id === updatedDocument.id ? summaryFromDocument(updatedDocument) : document,
        ),
      };
    });
  }

  async function handleSend(quickAction: string | null = null) {
    if (!workspace || !activeDocument) {
      return;
    }

    const trimmedPrompt = prompt.trim();
    if (!trimmedPrompt && !quickAction) {
      return;
    }

    setIsSending(true);

    const request: AssistantTurnInput = {
      document_id: activeDocument.id,
      prompt: trimmedPrompt || "Bruk den valgte hurtigkommandoen på dokumentet.",
      quick_action: quickAction,
      tone: workspace.settings.preferred_tone,
      selection_text: selectionText || null,
    };

    try {
      const result = await sendAssistantTurn(request);
      startTransition(() => {
        setWorkspace((current) =>
          current
            ? {
                ...current,
                active_document: result.document,
                documents: current.documents.map((document) =>
                  document.id === result.document.id
                    ? summaryFromDocument(result.document)
                    : document,
                ),
                runtime_status: result.runtime_status,
              }
            : null,
        );
        setPendingAction(result.editor_action);
        setPrompt("");
        setRuntimeError(null);
      });
    } catch (error) {
      setRuntimeError(
        error instanceof Error ? error.message : "Lokal AI kunne ikke svare akkurat nå.",
      );
      await handleRefreshRuntime();
    } finally {
      setIsSending(false);
    }
  }

  function handleApplyAction() {
    if (!pendingAction) {
      return;
    }

    editorRef.current?.applyAction(pendingAction);
    setPendingAction(null);
  }

  async function handleRefreshRuntime() {
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
  }

  function applyRuntimeAction(message: string, runtimeStatus: AppBootstrap["runtime_status"], settings: AppBootstrap["settings"]) {
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
      setRuntimeError(null);
    });
  }

  async function handleExport(format: OutputFormat) {
    if (!activeDocument) {
      return;
    }

    const result = await exportDocument(activeDocument.id, format);
    setLastExportPath(result.path);
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

    if (!workspace) {
      return;
    }

    await saveSettings({
      ...workspace.settings,
      preferred_tone: value,
    });
  }

  async function handleSetupAction(action: "prepare" | "repair") {
    setIsRuntimeActionPending(true);
    try {
      const result = action === "prepare" ? await prepareLocalAi() : await repairLocalAi();
      applyRuntimeAction(result.message, result.runtime_status, result.settings);
    } catch (error) {
      setRuntimeError(
        error instanceof Error ? error.message : "Kunne ikke oppdatere lokal AI.",
      );
      await handleRefreshRuntime();
    } finally {
      setIsRuntimeActionPending(false);
    }
  }

  async function handleModelChange(modelId: string) {
    setIsRuntimeActionPending(true);
    try {
      const result = await activateModel(modelId);
      applyRuntimeAction(result.message, result.runtime_status, result.settings);
    } catch (error) {
      setRuntimeError(
        error instanceof Error ? error.message : "Kunne ikke bytte modell.",
      );
      await handleRefreshRuntime();
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

  if (isLoading || !workspace || !activeDocument) {
    return (
      <main className="loading-screen" data-theme={themeMode}>
        <div className="loading-card">
          <p className="eyebrow">Starter lokal arbeidsflate</p>
          <h1>Setter opp dokumenter, lokal lagring og agentbro</h1>
        </div>
      </main>
    );
  }

  return (
    <>
      <main className="app-shell" data-active-nav={activeNav} data-theme={themeMode}>
        <Sidebar
          activeDocumentId={activeDocument.id}
          documents={workspace.documents}
          onActivate={() => activateNavSection("home")}
          onCreateDocument={handleCreateDocument}
          onSelectDocument={handleOpenDocument}
          panelRef={(node) => {
            sectionRefs.current.home = node;
          }}
          workflows={workspace.workflow_modules}
        />

        <section
          className="workspace-panel app-section app-section-search"
          onFocusCapture={() => activateNavSection("search")}
          onPointerDownCapture={() => activateNavSection("search")}
          ref={(node) => {
            sectionRefs.current.search = node;
          }}
        >
          <header className="workspace-header">
            <div>
              <p className="eyebrow">Aktivt dokument</p>
              <input
                className="document-title-input"
                onChange={(event) => handleTitleChange(event.target.value)}
                value={activeDocument.title}
              />
            </div>
            <div className="workspace-actions">
              <button className="ghost-button" onClick={handleRefreshRuntime} type="button">
                <RefreshCcw size={15} />
                Oppdater status
              </button>
              <button className="ghost-button" onClick={() => handleExport("html")} type="button">
                <Download size={15} />
                HTML
              </button>
              <button className="ghost-button" onClick={() => handleExport("pdf")} type="button">
                <Download size={15} />
                PDF
              </button>
              <button className="ghost-button" onClick={() => handleExport("txt")} type="button">
                <Save size={15} />
                TXT
              </button>
            </div>
          </header>

          <div className="workspace-meta">
            <span>{wordCount} ord</span>
            <span>{activeDocument.snapshot_count} snapshots</span>
            <span>{selectionText ? `${selectionText.length} tegn markert` : "Ingen tekst markert"}</span>
            {lastExportPath ? <span className="export-path">Sist eksportert til {lastExportPath}</span> : null}
          </div>

          <EditorPane
            document={activeDocument}
            onContentChange={handleDocumentContent}
            onSelectionChange={setSelectionText}
            ref={editorRef}
          />

          {pendingAction ? (
            <ActionPreview
              action={pendingAction}
              onApply={handleApplyAction}
              onDismiss={() => setPendingAction(null)}
            />
          ) : null}

          <StatusStrip runtimeStatus={workspace.runtime_status} />
        </section>

        <ChatPane
          isSending={isSending}
          isRuntimeActionPending={isRuntimeActionPending}
          messages={activeDocument.messages}
          onActivate={() => activateNavSection("user")}
          onActivateModel={(modelId) => void handleModelChange(modelId)}
          onPrepareLocalAi={() => void handleSetupAction("prepare")}
          onPromptChange={setPrompt}
          onQuickAction={(actionId) => void handleSend(actionId)}
          onRepairLocalAi={() => void handleSetupAction("repair")}
          onSend={() => void handleSend()}
          onToneChange={(value) => void handleToneChange(value)}
          panelRef={(node) => {
            sectionRefs.current.user = node;
          }}
          prompt={prompt}
          runtimeError={runtimeError}
          runtimeNotice={runtimeNotice}
          runtimeStatus={workspace.runtime_status}
          tone={workspace.settings.preferred_tone}
        />
      </main>

      <FloatingNav
        activeNav={activeNav}
        isThemeAnimating={isThemeAnimating}
        onSelectNav={(section) => activateNavSection(section, true)}
        onToggleTheme={handleThemeToggle}
        themeMode={themeMode}
      />
    </>
  );
}
