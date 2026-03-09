import type { KeyboardEvent, Ref } from "react";
import { useRef } from "react";
import { Download, FilePlus2, Save, Sparkles, Workflow, X } from "lucide-react";
import type {
  DocumentSummary,
  EditorSelection,
  OutputFormat,
  StoredDocument,
  WorkflowModule,
} from "../types";
import { getNextHorizontalTabId } from "../lib/tabNavigation";
import {
  toolbarActions,
  type ToolbarActionId,
} from "./editorToolbar";

export type LeftDrawerTab = "documents" | "tools" | "modules";

const LEFT_DRAWER_TABS: LeftDrawerTab[] = ["documents", "tools", "modules"];

interface SidebarProps {
  documents: DocumentSummary[];
  activeDocument: StoredDocument;
  activeTab: LeftDrawerTab;
  workflows: WorkflowModule[];
  recoveryNotices: string[];
  wordCount: number;
  selection: EditorSelection;
  hasUnsavedChanges: boolean;
  lastExportPath: string | null;
  activeToolbarActions: ToolbarActionId[];
  onTabChange: (tab: LeftDrawerTab) => void;
  onCreateDocument: () => void;
  onDismissRecoveryNotice: (index: number) => void;
  onExport: (format: OutputFormat) => void;
  onSelectDocument: (documentId: string) => void;
  onToolbarAction: (actionId: ToolbarActionId) => void;
  onActivate?: () => void;
  panelRef?: Ref<HTMLElement>;
  id?: string;
  className?: string;
  ariaHidden?: boolean;
}

function formatUpdatedTime(updatedAt: string) {
  return new Date(updatedAt).toLocaleTimeString("nb-NO", {
    hour: "2-digit",
    minute: "2-digit",
  });
}

export function Sidebar({
  documents,
  activeDocument,
  activeTab,
  workflows,
  recoveryNotices,
  wordCount,
  selection,
  hasUnsavedChanges,
  lastExportPath,
  activeToolbarActions,
  onTabChange,
  onCreateDocument,
  onDismissRecoveryNotice,
  onExport,
  onSelectDocument,
  onToolbarAction,
  onActivate,
  panelRef,
  id,
  className,
  ariaHidden,
}: SidebarProps) {
  const recentDocuments = documents.filter((document) => document.id !== activeDocument.id);
  const tabRefs = useRef<Record<LeftDrawerTab, HTMLButtonElement | null>>({
    documents: null,
    tools: null,
    modules: null,
  });

  function activateTab(tab: LeftDrawerTab, shouldFocus = false) {
    onTabChange(tab);

    if (shouldFocus) {
      tabRefs.current[tab]?.focus();
    }
  }

  function handleTabKeyDown(
    event: KeyboardEvent<HTMLButtonElement>,
    currentTab: LeftDrawerTab,
  ) {
    const nextTab = getNextHorizontalTabId(
      LEFT_DRAWER_TABS,
      currentTab,
      event.key,
    );

    if (!nextTab) {
      return;
    }

    event.preventDefault();
    activateTab(nextTab, true);
  }

  return (
    <aside
      className={`sidebar-panel app-section app-section-home ${className ?? ""}`.trim()}
      id={id}
      aria-hidden={ariaHidden}
      onFocusCapture={onActivate}
      onPointerDownCapture={onActivate}
      ref={panelRef}
      tabIndex={-1}
    >
      <div className="sidebar-header drawer-panel-header">
        <div className="sidebar-header-copy">
          <p className="eyebrow">Offline skriveflate</p>
          <h2 className="sidebar-title">Dokumenter og verktøy</h2>
        </div>
      </div>

      <div
        aria-label="Venstre sidepanel"
        aria-orientation="horizontal"
        className="panel-tabs sidebar-tabs"
        role="tablist"
      >
        <button
          aria-controls="sidebar-documents-panel"
          aria-selected={activeTab === "documents"}
          className="panel-tab-button"
          id="sidebar-documents-tab"
          onClick={() => activateTab("documents")}
          onKeyDown={(event) => handleTabKeyDown(event, "documents")}
          ref={(node) => {
            tabRefs.current.documents = node;
          }}
          role="tab"
          tabIndex={activeTab === "documents" ? 0 : -1}
          type="button"
        >
          Dokumenter
        </button>
        <button
          aria-controls="sidebar-tools-panel"
          aria-selected={activeTab === "tools"}
          className="panel-tab-button"
          id="sidebar-tools-tab"
          onClick={() => activateTab("tools")}
          onKeyDown={(event) => handleTabKeyDown(event, "tools")}
          ref={(node) => {
            tabRefs.current.tools = node;
          }}
          role="tab"
          tabIndex={activeTab === "tools" ? 0 : -1}
          type="button"
        >
          Verktøy
        </button>
        <button
          aria-controls="sidebar-modules-panel"
          aria-selected={activeTab === "modules"}
          className="panel-tab-button"
          id="sidebar-modules-tab"
          onClick={() => activateTab("modules")}
          onKeyDown={(event) => handleTabKeyDown(event, "modules")}
          ref={(node) => {
            tabRefs.current.modules = node;
          }}
          role="tab"
          tabIndex={activeTab === "modules" ? 0 : -1}
          type="button"
        >
          Moduler
        </button>
      </div>

      <div className="sidebar-body sidebar-drawer-body">
        <section
          aria-labelledby="sidebar-documents-tab"
          className="sidebar-tab-panel"
          hidden={activeTab !== "documents"}
          id="sidebar-documents-panel"
          role="tabpanel"
        >
          <button
            className="primary-button sidebar-create-button"
            onClick={onCreateDocument}
            type="button"
          >
            <FilePlus2 size={16} />
            Nytt dokument
          </button>

          {recoveryNotices.length > 0 ? (
            <div className="workspace-alert-stack drawer-alert-stack">
              {recoveryNotices.map((notice, index) => (
                <div className="workspace-alert notice" key={`${notice}-${index}`}>
                  <div>
                    <strong>Gjenoppretting fra lokal lagring</strong>
                    <p>{notice}</p>
                  </div>
                  <button
                    aria-label="Skjul melding"
                    className="ghost-button icon-button"
                    onClick={() => onDismissRecoveryNotice(index)}
                    type="button"
                  >
                    <X size={15} />
                  </button>
                </div>
              ))}
            </div>
          ) : null}

          <section className="sidebar-section">
            <div className="section-title">
              <Sparkles size={15} />
              Aktivt dokument
            </div>
            <button
              className="document-card document-card-active"
              onClick={() => onSelectDocument(activeDocument.id)}
              type="button"
            >
              <div className="document-card-top">
                <strong>{activeDocument.title}</strong>
                <span>{formatUpdatedTime(activeDocument.updated_at)}</span>
              </div>
              <p>{activeDocument.preview || "Tomt dokument"}</p>
              {activeDocument.workflow_hints.length > 0 ? (
                <div className="document-tags">
                  {activeDocument.workflow_hints.slice(0, 2).map((hint) => (
                    <span key={hint}>{hint}</span>
                  ))}
                </div>
              ) : null}
            </button>
          </section>

          {recentDocuments.length > 0 ? (
            <section className="sidebar-section">
              <div className="section-title">
                <Sparkles size={15} />
                Nylige dokumenter
              </div>
              <div className="document-list recent-document-list">
                {recentDocuments.map((document) => (
                  <button
                    className="recent-document-row"
                    key={document.id}
                    onClick={() => onSelectDocument(document.id)}
                    type="button"
                  >
                    <div className="recent-document-row-top">
                      <strong>{document.title}</strong>
                      <span>{formatUpdatedTime(document.updated_at)}</span>
                    </div>
                    <p>{document.preview || "Tomt dokument"}</p>
                  </button>
                ))}
              </div>
            </section>
          ) : null}
        </section>

        <section
          aria-labelledby="sidebar-tools-tab"
          className="sidebar-tab-panel"
          hidden={activeTab !== "tools"}
          id="sidebar-tools-panel"
          role="tabpanel"
        >
          <section className="sidebar-section">
            <div className="section-title">
              <Sparkles size={15} />
              Formatering
            </div>
            <div className="editor-toolbar drawer-toolbar-grid">
              {toolbarActions.map(({ icon: Icon, id: actionId, label }) => (
                <button
                  className={`toolbar-button ${
                    activeToolbarActions.includes(actionId) ? "active" : ""
                  }`}
                  key={actionId}
                  onClick={() => onToolbarAction(actionId)}
                  type="button"
                >
                  <Icon size={16} />
                  {label}
                </button>
              ))}
            </div>
          </section>

          <section className="sidebar-section">
            <div className="section-title">
              <Download size={15} />
              Eksport
            </div>
            <div className="workspace-actions drawer-action-group">
              <button
                className="ghost-button"
                onClick={() => onExport("html")}
                type="button"
              >
                <Download size={15} />
                HTML
              </button>
              <button
                className="ghost-button"
                onClick={() => onExport("pdf")}
                type="button"
              >
                <Download size={15} />
                PDF
              </button>
              <button
                className="ghost-button"
                onClick={() => onExport("txt")}
                type="button"
              >
                <Save size={15} />
                TXT
              </button>
            </div>
          </section>

          <section className="sidebar-section">
            <div className="section-title">
              <Sparkles size={15} />
              Dokumentstatus
            </div>
            <div className="workspace-meta workspace-meta-panel">
              <span>{wordCount} ord</span>
              <span>{activeDocument.snapshot_count} lagringspunkter</span>
              <span>
                {selection.text
                  ? `${selection.text.length} tegn markert`
                  : "Ingen tekst markert"}
              </span>
              <span>
                Revisjon {activeDocument.content_revision}
                {hasUnsavedChanges ? " · ulagret utkast" : ""}
              </span>
              {lastExportPath ? (
                <span className="export-path">Sist eksportert til {lastExportPath}</span>
              ) : null}
            </div>
          </section>
        </section>

        <section
          aria-labelledby="sidebar-modules-tab"
          className="sidebar-tab-panel"
          hidden={activeTab !== "modules"}
          id="sidebar-modules-panel"
          role="tabpanel"
        >
          <section className="sidebar-section">
            <div className="section-title">
              <Workflow size={15} />
              Planlagte moduler
            </div>
            <div className="workflow-list compact-workflow-list">
              {workflows.map((workflow) => (
                <div className="workflow-card workflow-card-compact" key={workflow.id}>
                  <div>
                    <strong>{workflow.name}</strong>
                    <span>{workflow.status === "planned" ? "Planlagt" : "Aktiv"}</span>
                  </div>
                  <p>{workflow.description}</p>
                </div>
              ))}
            </div>
          </section>
        </section>
      </div>
    </aside>
  );
}
