import { FilePlus2, Sparkles, Workflow } from "lucide-react";
import type { DocumentSummary, WorkflowModule } from "../types";

interface SidebarProps {
  documents: DocumentSummary[];
  activeDocumentId: string;
  workflows: WorkflowModule[];
  onCreateDocument: () => void;
  onSelectDocument: (documentId: string) => void;
}

export function Sidebar({
  documents,
  activeDocumentId,
  workflows,
  onCreateDocument,
  onSelectDocument,
}: SidebarProps) {
  return (
    <aside className="sidebar-panel">
      <div className="sidebar-header">
        <div>
          <p className="eyebrow">Offline skriveflate</p>
          <h1>Lokal Skriveflate</h1>
        </div>
        <button className="primary-button" onClick={onCreateDocument} type="button">
          <FilePlus2 size={16} />
          Nytt dokument
        </button>
      </div>

      <section className="sidebar-section">
        <div className="section-title">
          <Sparkles size={15} />
          Dokumenter
        </div>
        <div className="document-list">
          {documents.map((document) => (
            <button
              key={document.id}
              className={`document-card ${document.id === activeDocumentId ? "active" : ""}`}
              onClick={() => onSelectDocument(document.id)}
              type="button"
            >
              <div className="document-card-top">
                <strong>{document.title}</strong>
                <span>{new Date(document.updated_at).toLocaleTimeString("nb-NO", { hour: "2-digit", minute: "2-digit" })}</span>
              </div>
              <p>{document.preview || "Tomt dokument"}</p>
              <div className="document-tags">
                {document.workflow_hints.map((hint) => (
                  <span key={hint}>{hint}</span>
                ))}
              </div>
            </button>
          ))}
        </div>
      </section>

      <section className="sidebar-section">
        <div className="section-title">
          <Workflow size={15} />
          Planlagte moduler
        </div>
        <div className="workflow-list">
          {workflows.map((workflow) => (
            <div className="workflow-card" key={workflow.id}>
              <div>
                <strong>{workflow.name}</strong>
                <span>{workflow.status === "planned" ? "Planlagt" : "Aktiv"}</span>
              </div>
              <p>{workflow.description}</p>
            </div>
          ))}
        </div>
      </section>
    </aside>
  );
}
