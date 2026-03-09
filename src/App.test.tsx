import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import type {
  AppBootstrap,
  AssistantTurnInput,
  AssistantTurnResult,
  ExportResult,
  RuntimeStatus,
  StoredDocument,
  WorkspaceSnapshot,
} from "./types";
import { vi, beforeEach, describe, expect, it } from "vitest";
import App from "./App";

const tauriMock = vi.hoisted(() => ({
  activateModel: vi.fn(),
  bootstrapApp: vi.fn(),
  createDocument: vi.fn(),
  exportDocument: vi.fn(),
  openDocument: vi.fn(),
  prepareLocalAi: vi.fn(),
  refreshRuntimeStatus: vi.fn(),
  repairLocalAi: vi.fn(),
  saveDocument: vi.fn(),
  saveSettings: vi.fn(),
  sendAssistantTurn: vi.fn(),
}));

const editorMock = vi.hoisted(() => ({
  applyAction: vi.fn(),
  focusEditor: vi.fn(),
  runToolbarAction: vi.fn(),
}));

vi.mock("./lib/tauri", () => tauriMock);

vi.mock("./components/FloatingNav", () => ({
  FloatingNav({
    activeNav,
    onSelectNav,
    onToggleTheme,
  }: {
    activeNav: "home" | "search" | "user";
    onSelectNav: (section: "home" | "search" | "user") => void;
    onToggleTheme: () => void;
  }) {
    return (
      <nav>
        <button
          aria-pressed={activeNav === "home"}
          onClick={() => onSelectNav("home")}
          type="button"
        >
          Hjem
        </button>
        <button
          aria-pressed={activeNav === "search"}
          onClick={() => onSelectNav("search")}
          type="button"
        >
          Dokument
        </button>
        <button
          aria-pressed={activeNav === "user"}
          onClick={() => onSelectNav("user")}
          type="button"
        >
          Assistent
        </button>
        <button aria-label="Bytt tema" onClick={onToggleTheme} type="button">
          Tema
        </button>
      </nav>
    );
  },
}));

vi.mock("./components/EditorPane", async () => {
  const React = await import("react");

  const EditorPane = React.forwardRef(function EditorPaneMock(
    {
      document,
      onContentChange,
      onSelectionChange,
      onToolbarStateChange,
    }: {
      document: StoredDocument;
      onContentChange: (
        content: StoredDocument["content"],
        html: string,
        plainText: string,
      ) => void;
      onSelectionChange: (selection: {
        text: string;
        from: number;
        to: number;
      }) => void;
      onToolbarStateChange?: (activeActions: string[]) => void;
    },
    ref: React.ForwardedRef<{
      applyAction: () => void;
      focusEditor: () => void;
      runToolbarAction: () => void;
    }>,
  ) {
    const editCountRef = React.useRef(0);

    React.useEffect(() => {
      onToolbarStateChange?.([]);
    }, [document.id, onToolbarStateChange]);

    React.useImperativeHandle(
      ref,
      () => ({
        applyAction: editorMock.applyAction,
        focusEditor: editorMock.focusEditor,
        runToolbarAction: editorMock.runToolbarAction,
      }),
      [],
    );

    return (
      <div>
        <div data-testid="editor-document-id">{document.id}</div>
        <button
          onClick={() => {
            editCountRef.current += 1;
            const text = `Endret ${document.id} ${editCountRef.current}`;
            onContentChange(
              {
                type: "doc",
                content: [
                  {
                    type: "paragraph",
                    content: [{ type: "text", text }],
                  },
                ],
              },
              `<p>${text}</p>`,
              text,
            );
          }}
          type="button"
        >
          Rediger dokument
        </button>
        <button
          onClick={() =>
            onSelectionChange({
              text: "markert tekst",
              from: 3,
              to: 11,
            })
          }
          type="button"
        >
          Velg tekst
        </button>
      </div>
    );
  });

  return {
    EditorPane,
  };
});

function setViewportCompact(matches: boolean) {
  window.matchMedia = vi.fn().mockImplementation((query: string) => ({
    matches: query === "(max-width: 1180px)" ? matches : false,
    media: query,
    onchange: null,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    addListener: vi.fn(),
    removeListener: vi.fn(),
    dispatchEvent: vi.fn(),
  }));
}

function makeRuntimeStatus(overrides: Partial<RuntimeStatus> = {}): RuntimeStatus {
  return {
    offline_mode: true,
    local_only: true,
    selected_model: "lokal-4b",
    runtime_state: "ready",
    codex: {
      available: true,
      running: true,
      details: null,
    },
    local_ai: {
      available: true,
      running: true,
      details: null,
    },
    available_models: [
      {
        id: "lokal-4b",
        label: "Lokal 4B",
        tier: "Standard",
        bundled: true,
        installed: true,
        active: true,
        details: null,
      },
    ],
    runtime_home: null,
    listen_address: null,
    ...overrides,
  };
}

function makeDocument(
  overrides: Partial<StoredDocument> = {},
): StoredDocument {
  const plainText = overrides.plain_text ?? "Opprinnelig tekst";
  const title = overrides.title ?? "Dokument en";

  return {
    id: "doc-1",
    title,
    preview: overrides.preview ?? plainText.slice(0, 120),
    updated_at: "2026-03-08T10:00:00.000Z",
    workflow_hints: ["rapport"],
    created_at: "2026-03-08T09:00:00.000Z",
    plain_text: plainText,
    html: overrides.html ?? `<p>${plainText}</p>`,
    content: overrides.content ?? {
      type: "doc",
      content: [
        {
          type: "paragraph",
          content: [{ type: "text", text: plainText }],
        },
      ],
    },
    content_revision: overrides.content_revision ?? 2,
    thread_id: overrides.thread_id ?? "thread-1",
    messages: overrides.messages ?? [],
    snapshot_count: overrides.snapshot_count ?? 1,
    ...overrides,
  };
}

function makeBootstrap(
  activeDocument: StoredDocument,
  options: {
    documents?: AppBootstrap["documents"];
    recoveryNotices?: string[];
  } = {},
): AppBootstrap {
  return {
    documents:
      options.documents ?? [
        {
          id: activeDocument.id,
          title: activeDocument.title,
          preview: activeDocument.preview,
          updated_at: activeDocument.updated_at,
          workflow_hints: activeDocument.workflow_hints,
        },
      ],
    active_document: activeDocument,
    runtime_status: makeRuntimeStatus(),
    settings: {
      selected_model: "lokal-4b",
      preferred_tone: "Klar og profesjonell",
    },
    workflow_modules: [
      {
        id: "wf-1",
        name: "Rapportflyt",
        description: "Planlagt modul for rapportmaler og kvalitetssikret struktur.",
        status: "planned",
      },
    ],
    recovery_notices: options.recoveryNotices ?? [],
  };
}

function makeSnapshot(
  activeDocument: StoredDocument,
  documents?: WorkspaceSnapshot["documents"],
): WorkspaceSnapshot {
  return {
    documents:
      documents ?? [
        {
          id: activeDocument.id,
          title: activeDocument.title,
          preview: activeDocument.preview,
          updated_at: activeDocument.updated_at,
          workflow_hints: activeDocument.workflow_hints,
        },
      ],
    active_document: activeDocument,
    recovery_notices: [],
  };
}

beforeEach(() => {
  setViewportCompact(false);
  vi.clearAllMocks();

  tauriMock.activateModel.mockResolvedValue({
    message: "Modellen ble aktivert.",
    runtime_status: makeRuntimeStatus(),
    settings: {
      selected_model: "lokal-4b",
      preferred_tone: "Klar og profesjonell",
    },
  });
  tauriMock.bootstrapApp.mockResolvedValue(makeBootstrap(makeDocument()));
  tauriMock.createDocument.mockResolvedValue(makeSnapshot(makeDocument({
    id: "doc-2",
    title: "Nytt dokument",
  })));
  tauriMock.exportDocument.mockResolvedValue({
    path: "/tmp/dokument.txt",
    format: "txt",
  } satisfies ExportResult);
  tauriMock.openDocument.mockResolvedValue(makeSnapshot(makeDocument()));
  tauriMock.prepareLocalAi.mockResolvedValue({
    message: "Klar.",
    runtime_status: makeRuntimeStatus(),
    settings: {
      selected_model: "lokal-4b",
      preferred_tone: "Klar og profesjonell",
    },
  });
  tauriMock.refreshRuntimeStatus.mockResolvedValue(makeRuntimeStatus());
  tauriMock.repairLocalAi.mockResolvedValue({
    message: "Reparert.",
    runtime_status: makeRuntimeStatus(),
    settings: {
      selected_model: "lokal-4b",
      preferred_tone: "Klar og profesjonell",
    },
  });
  tauriMock.saveDocument.mockImplementation(async (document) =>
    makeDocument({
      id: document.id,
      title: document.title,
      plain_text: document.plain_text,
      html: document.html,
      content: document.content,
      preview: document.plain_text.slice(0, 120),
      content_revision: 3,
    }),
  );
  tauriMock.saveSettings.mockResolvedValue({
    selected_model: "lokal-4b",
    preferred_tone: "Klar og profesjonell",
  });
  tauriMock.sendAssistantTurn.mockImplementation(
    async (request: AssistantTurnInput): Promise<AssistantTurnResult> => ({
      document: makeDocument({
        id: request.document_id,
        content_revision: request.document_revision,
      }),
      runtime_status: makeRuntimeStatus(),
      assistant_reply: "Svar",
      editor_action: null,
    }),
  );
});

describe("App shell", () => {
  it("keeps both drawers closed by default and can open them on desktop", async () => {
    render(<App />);

    await screen.findByDisplayValue("Dokument en");

    const leftTrigger = screen.getByRole("button", { name: "Åpne dokumenter" });
    const rightTrigger = screen.getByRole("button", { name: "Åpne assistent" });

    expect(leftTrigger.getAttribute("aria-expanded")).toBe("false");
    expect(rightTrigger.getAttribute("aria-expanded")).toBe("false");
    expect(screen.getByRole("button", { name: "Dokument" }).getAttribute("aria-pressed")).toBe("true");

    fireEvent.click(leftTrigger);

    expect(leftTrigger.getAttribute("aria-expanded")).toBe("true");
    expect(screen.getByRole("button", { name: "Hjem" }).getAttribute("aria-pressed")).toBe("true");
    expect(screen.getByRole("button", { name: "Nytt dokument" })).toBeTruthy();

    fireEvent.click(rightTrigger);

    expect(rightTrigger.getAttribute("aria-expanded")).toBe("true");
    expect(screen.getByRole("button", { name: "Assistent" }).getAttribute("aria-pressed")).toBe("true");
    expect(screen.getByRole("button", { name: "Send til lokal motor" })).toBeTruthy();
  });

  it("keeps only one drawer open at a time on compact layouts", async () => {
    setViewportCompact(true);

    render(<App />);

    await screen.findByDisplayValue("Dokument en");

    const leftTrigger = screen.getByRole("button", { name: "Åpne dokumenter" });
    const rightTrigger = screen.getByRole("button", { name: "Åpne assistent" });

    fireEvent.click(leftTrigger);
    expect(leftTrigger.getAttribute("aria-expanded")).toBe("true");

    fireEvent.click(rightTrigger);

    expect(leftTrigger.getAttribute("aria-expanded")).toBe("false");
    expect(rightTrigger.getAttribute("aria-expanded")).toBe("true");
  });

  it("flushes pending edits before sending and uses the saved revision", async () => {
    render(<App />);

    await screen.findByDisplayValue("Dokument en");

    fireEvent.click(screen.getByRole("button", { name: "Åpne assistent" }));
    fireEvent.click(screen.getByRole("button", { name: "Rediger dokument" }));
    fireEvent.change(screen.getByPlaceholderText(/Be AI om å skrive videre/i), {
      target: { value: "Skriv videre" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Send til lokal motor" }));

    await waitFor(() => {
      expect(tauriMock.sendAssistantTurn).toHaveBeenCalledTimes(1);
    });

    expect(tauriMock.saveDocument).toHaveBeenCalledWith(
      expect.objectContaining({
        id: "doc-1",
        plain_text: "Endret doc-1 1",
      }),
    );
    expect(tauriMock.sendAssistantTurn).toHaveBeenCalledWith(
      expect.objectContaining({
        document_id: "doc-1",
        document_revision: 3,
        prompt: "Skriv videre",
      }),
    );
    expect(tauriMock.saveDocument.mock.invocationCallOrder[0]).toBeLessThan(
      tauriMock.sendAssistantTurn.mock.invocationCallOrder[0],
    );
  });

  it("flushes pending edits before opening another document from the drawer", async () => {
    const firstDocument = makeDocument({
      id: "doc-1",
      title: "Dokument en",
    });
    const secondDocument = makeDocument({
      id: "doc-2",
      title: "Dokument to",
      thread_id: "thread-2",
    });

    tauriMock.bootstrapApp.mockResolvedValue(
      makeBootstrap(firstDocument, {
        documents: [
          {
            id: firstDocument.id,
            title: firstDocument.title,
            preview: firstDocument.preview,
            updated_at: firstDocument.updated_at,
            workflow_hints: firstDocument.workflow_hints,
          },
          {
            id: secondDocument.id,
            title: secondDocument.title,
            preview: secondDocument.preview,
            updated_at: secondDocument.updated_at,
            workflow_hints: secondDocument.workflow_hints,
          },
        ],
      }),
    );
    tauriMock.openDocument.mockResolvedValue(
      makeSnapshot(secondDocument, [
        {
          id: firstDocument.id,
          title: firstDocument.title,
          preview: firstDocument.preview,
          updated_at: firstDocument.updated_at,
          workflow_hints: firstDocument.workflow_hints,
        },
        {
          id: secondDocument.id,
          title: secondDocument.title,
          preview: secondDocument.preview,
          updated_at: secondDocument.updated_at,
          workflow_hints: secondDocument.workflow_hints,
        },
      ]),
    );

    render(<App />);

    await screen.findByDisplayValue("Dokument en");

    fireEvent.click(screen.getByRole("button", { name: "Åpne dokumenter" }));
    fireEvent.click(screen.getByRole("button", { name: "Rediger dokument" }));
    fireEvent.click(screen.getByRole("button", { name: /Dokument to/i }));

    await waitFor(() => {
      expect(tauriMock.openDocument).toHaveBeenCalledWith("doc-2");
    });

    expect(tauriMock.saveDocument.mock.invocationCallOrder[0]).toBeLessThan(
      tauriMock.openDocument.mock.invocationCallOrder[0],
    );
    await screen.findByDisplayValue("Dokument to");
  });

  it("shows recovery notices in the documents drawer and lets you dismiss them", async () => {
    tauriMock.bootstrapApp.mockResolvedValue(
      makeBootstrap(makeDocument(), {
        recoveryNotices: ["Korrupt tilstandsfil ble flyttet til karantene."],
      }),
    );

    render(<App />);

    await screen.findByDisplayValue("Dokument en");
    fireEvent.click(screen.getByRole("button", { name: "Åpne dokumenter" }));

    await screen.findByText("Korrupt tilstandsfil ble flyttet til karantene.");

    fireEvent.click(screen.getByRole("button", { name: "Skjul melding" }));

    await waitFor(() => {
      expect(
        screen.queryByText("Korrupt tilstandsfil ble flyttet til karantene."),
      ).toBeNull();
    });
  });

  it("renders stale action previews in the floating tray after the document changes again", async () => {
    tauriMock.sendAssistantTurn.mockImplementation(
      async (request: AssistantTurnInput): Promise<AssistantTurnResult> => ({
        document: makeDocument({
          id: request.document_id,
          plain_text: "Endret doc-1 1",
          html: "<p>Endret doc-1 1</p>",
          content: {
            type: "doc",
            content: [
              {
                type: "paragraph",
                content: [{ type: "text", text: "Endret doc-1 1" }],
              },
            ],
          },
          preview: "Endret doc-1 1",
          content_revision: request.document_revision,
        }),
        runtime_status: makeRuntimeStatus(),
        assistant_reply: "Svar",
        editor_action: {
          action_type: "replace_selection",
          title: "Forslag",
          rationale: "Bruk det nye utkastet.",
          content: "Oppdatert innhold",
        },
      }),
    );

    render(<App />);

    await screen.findByDisplayValue("Dokument en");

    fireEvent.click(screen.getByRole("button", { name: "Åpne assistent" }));
    fireEvent.click(screen.getByRole("button", { name: "Velg tekst" }));
    fireEvent.click(screen.getByRole("button", { name: "Rediger dokument" }));
    fireEvent.change(screen.getByPlaceholderText(/Be AI om å skrive videre/i), {
      target: { value: "Gi meg et forslag" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Send til lokal motor" }));

    await screen.findByText("Forslag");

    fireEvent.click(screen.getByRole("button", { name: "Rediger dokument" }));

    await screen.findByText(
      "Dette forslaget er utdatert fordi dokumentet ble endret etter at det ble laget. Be om et nytt forslag før du setter inn tekst.",
    );

    const applyButton = screen.getByRole("button", {
      name: "Sett inn i dokumentet",
    }) as HTMLButtonElement;
    expect(applyButton.disabled).toBe(true);
  });

  it("shows retry UI for export failures from the tools drawer and can retry", async () => {
    tauriMock.exportDocument
      .mockRejectedValueOnce(new Error("Eksport stoppet."))
      .mockResolvedValueOnce({
        path: "/tmp/eksportert.txt",
        format: "txt",
      } satisfies ExportResult);

    render(<App />);

    await screen.findByDisplayValue("Dokument en");

    fireEvent.click(screen.getByRole("button", { name: "Åpne dokumenter" }));
    fireEvent.click(screen.getByRole("tab", { name: "Verktøy" }));
    fireEvent.click(screen.getByRole("button", { name: "TXT" }));

    await screen.findByText("Eksport stoppet.");

    fireEvent.click(screen.getByRole("button", { name: "Prøv igjen" }));

    await waitFor(() => {
      expect(tauriMock.exportDocument).toHaveBeenCalledTimes(2);
    });

    await screen.findByText("Sist eksportert til /tmp/eksportert.txt");
  });
});
