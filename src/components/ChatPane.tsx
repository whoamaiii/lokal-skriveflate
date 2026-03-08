import { AlertTriangle, LoaderCircle, Play, RefreshCcw, Rocket, WandSparkles } from "lucide-react";
import type { ChatMessage, RuntimeStatus } from "../types";

const quickActions = [
  { id: "continue_writing", label: "Skriv videre" },
  { id: "improve_selection", label: "Forbedre markert tekst" },
  { id: "make_formal", label: "Gjør mer formelt" },
  { id: "summarize", label: "Lag oppsummering" },
  { id: "draft_report", label: "Lag rapportutkast" },
  { id: "meeting_notes", label: "Lag møtereferat" },
];

interface ChatPaneProps {
  messages: ChatMessage[];
  prompt: string;
  tone: string;
  runtimeStatus: RuntimeStatus;
  isSending: boolean;
  isRuntimeActionPending: boolean;
  runtimeNotice: string | null;
  runtimeError: string | null;
  onPromptChange: (value: string) => void;
  onToneChange: (value: string) => void;
  onSend: () => void;
  onQuickAction: (actionId: string) => void;
  onPrepareLocalAi: () => void;
  onRepairLocalAi: () => void;
  onActivateModel: (modelId: string) => void;
}

export function ChatPane({
  messages,
  prompt,
  tone,
  runtimeStatus,
  isSending,
  isRuntimeActionPending,
  runtimeNotice,
  runtimeError,
  onPromptChange,
  onToneChange,
  onSend,
  onQuickAction,
  onPrepareLocalAi,
  onRepairLocalAi,
  onActivateModel,
}: ChatPaneProps) {
  const modelReady =
    runtimeStatus.codex.available &&
    runtimeStatus.local_ai.running &&
    runtimeStatus.runtime_state === "ready";

  const setupText = (() => {
    switch (runtimeStatus.runtime_state) {
      case "ready":
        return "Lokal AI kjører via bundlet runtime. Dokumentforslag blir laget uten nett.";
      case "degraded":
        return "Lokal AI er klargjort, men ikke i gang akkurat nå. Den startes ved første forespørsel eller etter reparasjon.";
      case "repair_required":
        return "Runtime eller modell trenger reparasjon. Appen har oppdaget avvik i de lokale filene.";
      case "extracting":
        return "Appen klargjør lokale runtime-filer i bakgrunnen.";
      case "not_prepared":
      default:
        return "Klargjør bundlet runtime og standardmodellen før skriveassistenten brukes på denne maskinen.";
    }
  })();

  return (
    <aside className="chat-panel">
      <div className="chat-header">
        <div>
          <p className="eyebrow">Lokal skriveassistent</p>
          <h2>Chat og kommandoer</h2>
        </div>
        <span className={`status-dot ${modelReady ? "ready" : "offline"}`}>
          {modelReady ? "Klar" : "Setup"}
        </span>
      </div>

      <div className="setup-card">
        <div>
          <strong>Lokalt modelloppsett</strong>
          <p>{setupText}</p>
          {runtimeStatus.local_ai.details ? (
            <p className="setup-detail">{runtimeStatus.local_ai.details}</p>
          ) : null}
        </div>
        <div className="setup-actions">
          <button
            className="ghost-button"
            disabled={isRuntimeActionPending}
            onClick={onPrepareLocalAi}
            type="button"
          >
            {isRuntimeActionPending ? <LoaderCircle className="spinning" size={14} /> : null}
            Klargjør lokal AI
          </button>
          <button
            className="ghost-button"
            disabled={isRuntimeActionPending}
            onClick={onRepairLocalAi}
            type="button"
          >
            <RefreshCcw size={14} />
            Reparer
          </button>
        </div>
        <label className="tone-control">
          Modellpakke
          <select
            disabled={isRuntimeActionPending}
            onChange={(event) => onActivateModel(event.target.value)}
            value={runtimeStatus.selected_model}
          >
            {runtimeStatus.available_models.map((model) => (
              <option disabled={!model.installed && !model.active} key={model.id} value={model.id}>
                {model.tier}: {model.label}
                {model.installed ? "" : " (ikke klar)"}
              </option>
            ))}
          </select>
        </label>
        {runtimeNotice ? <p className="setup-note ok">{runtimeNotice}</p> : null}
        {runtimeError ? (
          <p className="setup-note error">
            <AlertTriangle size={14} />
            {runtimeError}
          </p>
        ) : null}
      </div>

      <div className="quick-action-grid">
        {quickActions.map((action) => (
          <button
            className="quick-action-button"
            disabled={!modelReady || isSending}
            key={action.id}
            onClick={() => onQuickAction(action.id)}
            type="button"
          >
            <WandSparkles size={14} />
            {action.label}
          </button>
        ))}
      </div>

      <div className="message-list">
        {messages.map((message) => (
          <article className={`message-bubble ${message.role}`} key={message.id}>
            <span>{message.role === "assistant" ? "AI" : message.role === "system" ? "System" : "Du"}</span>
            <p>{message.text}</p>
          </article>
        ))}
      </div>

      <div className="chat-form">
        <label className="tone-control">
          Tone
          <select value={tone} onChange={(event) => onToneChange(event.target.value)}>
            <option value="Klar og profesjonell">Klar og profesjonell</option>
            <option value="Varm og støttende">Varm og støttende</option>
            <option value="Kort og saklig">Kort og saklig</option>
            <option value="Strukturert og formell">Strukturert og formell</option>
          </select>
        </label>
        <textarea
          onChange={(event) => onPromptChange(event.target.value)}
          placeholder="Be AI om å skrive videre, omskrive markert tekst eller lage et rapportutkast."
          rows={6}
          value={prompt}
        />
        <button
          className="primary-button send-button"
          disabled={!modelReady || isSending || !prompt.trim()}
          onClick={onSend}
          type="button"
        >
          {isSending ? <LoaderCircle className="spinning" size={16} /> : <Play size={16} />}
          Send til lokal motor
        </button>
        <p className="chat-footnote">
          <Rocket size={13} />
          Alle dokumentendringer kommer tilbake som preview før de settes inn.
        </p>
      </div>
    </aside>
  );
}
