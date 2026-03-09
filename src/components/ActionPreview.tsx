import { Check, Sparkles, X } from "lucide-react";
import type { EditorAction } from "../types";

interface ActionPreviewProps {
  action: EditorAction;
  isStale?: boolean;
  onApply: () => void;
  onDismiss: () => void;
}

export function ActionPreview({ action, isStale = false, onApply, onDismiss }: ActionPreviewProps) {
  return (
    <section className="action-preview">
      <div className="action-preview-header">
        <div>
          <p className="eyebrow">Forslag til dokumentendring</p>
          <h3>{action.title}</h3>
        </div>
        <Sparkles size={18} />
      </div>
      <p className="action-preview-rationale">{action.rationale}</p>
      {isStale ? (
        <p className="setup-note error">
          Dette forslaget er utdatert fordi dokumentet ble endret etter at det ble laget. Be om et nytt forslag før du setter inn tekst.
        </p>
      ) : null}
      <pre>{action.content}</pre>
      <div className="action-preview-actions">
        <button className="primary-button" disabled={isStale} onClick={onApply} type="button">
          <Check size={15} />
          Sett inn i dokumentet
        </button>
        <button className="ghost-button" onClick={onDismiss} type="button">
          <X size={15} />
          Avbryt
        </button>
      </div>
    </section>
  );
}
