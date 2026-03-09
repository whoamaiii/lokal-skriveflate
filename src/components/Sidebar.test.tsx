import { fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { Sidebar } from "./Sidebar";
import type { DocumentSummary, StoredDocument, WorkflowModule } from "../types";
import type { ToolbarActionId } from "./editorToolbar";

const activeDocument: StoredDocument = {
  id: "doc-1",
  title: "Velkommen til Lokal Skriveflate",
  preview: "Dette er aktivt dokument.",
  updated_at: "2026-03-08T20:19:00Z",
  workflow_hints: ["skriveassistent", "rapport", "logg"],
  created_at: "2026-03-08T19:00:00Z",
  plain_text: "Dette er aktivt dokument.",
  html: "<p>Dette er aktivt dokument.</p>",
  content: {
    type: "doc",
    content: [
      {
        type: "paragraph",
        content: [{ type: "text", text: "Dette er aktivt dokument." }],
      },
    ],
  },
  content_revision: 2,
  thread_id: "thread-1",
  messages: [],
  snapshot_count: 2,
};

const documents: DocumentSummary[] = [
  {
    id: "doc-1",
    title: "Velkommen til Lokal Skriveflate",
    preview: "Dette er aktivt dokument.",
    updated_at: "2026-03-08T20:19:00Z",
    workflow_hints: ["skriveassistent", "rapport", "logg"],
  },
  {
    id: "doc-2",
    title: "Motenotat",
    preview: "Kort oppsummering av siste mote.",
    updated_at: "2026-03-08T18:10:00Z",
    workflow_hints: ["mote"],
  },
  {
    id: "doc-3",
    title: "Statusrapport",
    preview: "Utkast til ukentlig statusrapport.",
    updated_at: "2026-03-08T16:45:00Z",
    workflow_hints: ["rapport"],
  },
];

const workflows: WorkflowModule[] = [
  {
    id: "wf-1",
    name: "Rapportflyt",
    description: "Planlagt modul for rapportmaler og kvalitetssikret struktur.",
    status: "planned",
  },
  {
    id: "wf-2",
    name: "Prosjektflyt",
    description: "Aktiv modul for prosjektplaner og oppfolging.",
    status: "active",
  },
];

function makeSidebarProps() {
  return {
    documents,
    activeDocument,
    activeTab: "documents" as const,
    workflows,
    recoveryNotices: [],
    wordCount: 12,
    selection: {
      text: "",
      from: 1,
      to: 1,
    },
    hasUnsavedChanges: false,
    lastExportPath: null,
    activeToolbarActions: ["paragraph"] as ToolbarActionId[],
    onTabChange: vi.fn(),
    onCreateDocument: vi.fn(),
    onDismissRecoveryNotice: vi.fn(),
    onExport: vi.fn(),
    onSelectDocument: vi.fn(),
    onToolbarAction: vi.fn(),
  };
}

describe("Sidebar", () => {
  it("renders the active document separately from recent documents", () => {
    render(<Sidebar {...makeSidebarProps()} />);

    const activeSection = screen.getByText("Aktivt dokument").closest("section");
    const recentSection = screen.getByText("Nylige dokumenter").closest("section");

    expect(activeSection).not.toBeNull();
    expect(recentSection).not.toBeNull();

    expect(
      within(activeSection as HTMLElement).getByText("Velkommen til Lokal Skriveflate"),
    ).toBeTruthy();
    expect(
      within(recentSection as HTMLElement).queryByText("Velkommen til Lokal Skriveflate"),
    ).toBeNull();
    expect(within(recentSection as HTMLElement).getByText("Motenotat")).toBeTruthy();
    expect(within(recentSection as HTMLElement).getByText("Statusrapport")).toBeTruthy();
  });

  it("calls create and recent document selection handlers", () => {
    const props = makeSidebarProps();

    render(<Sidebar {...props} />);

    fireEvent.click(screen.getByRole("button", { name: /Nytt dokument/i }));
    fireEvent.click(screen.getByRole("button", { name: /Motenotat/i }));

    expect(props.onCreateDocument).toHaveBeenCalledTimes(1);
    expect(props.onSelectDocument).toHaveBeenCalledWith("doc-2");
  });

  it("renders tools content and dispatches toolbar and export actions", () => {
    const props = makeSidebarProps();

    render(
      <Sidebar
        {...props}
        activeTab="tools"
        lastExportPath="/tmp/eksportert.txt"
      />,
    );

    expect(screen.getByText("Formatering")).toBeTruthy();
    expect(screen.getByRole("button", { name: "H1" })).toBeTruthy();
    expect(screen.getByText("Sist eksportert til /tmp/eksportert.txt")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Fet" }));
    fireEvent.click(screen.getByRole("button", { name: "TXT" }));

    expect(props.onToolbarAction).toHaveBeenCalledWith("bold");
    expect(props.onExport).toHaveBeenCalledWith("txt");
  });

  it("renders workflow rows with the correct status labels in the modules tab", () => {
    render(<Sidebar {...makeSidebarProps()} activeTab="modules" />);

    expect(screen.getByText("Rapportflyt")).toBeTruthy();
    expect(screen.getByText("Prosjektflyt")).toBeTruthy();
    expect(screen.getByText("Planlagt")).toBeTruthy();
    expect(screen.getByText("Aktiv")).toBeTruthy();
  });
});
