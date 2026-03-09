import type { KeyboardEvent, Ref } from "react";
import { useRef, useState } from "react";
import { AlertTriangle, LoaderCircle, Play, RefreshCcw, Rocket, WandSparkles } from "lucide-react";
import type { ChatMessage, RuntimeStatus } from "../types";
import { getNextHorizontalTabId } from "../lib/tabNavigation";
import { StatusStrip } from "./StatusStrip";

const quickActions = [
  { id: "continue_writing", label: "Skriv videre" },
  { id: "improve_selection", label: "Forbedre markert tekst" },
  { id: "make_formal", label: "Gjør mer formelt" },
  { id: "summarize", label: "Lag oppsummering" },
  { id: "draft_report", label: "Lag rapportutkast" },
  { id: "meeting_notes", label: "Lag møtereferat" },
];

type AssistantPanelTab = "chat" | "actions" | "system";
const ASSISTANT_PANEL_TABS: AssistantPanelTab[] = ["chat", "actions", "system"];

interface ChatPaneProps {
  messages: ChatMessage[];
  prompt: string;
  tone: string;
  runtimeStatus: RuntimeStatus;
  isSending: boolean;
  isRuntimeActionPending: boolean;
  runtimeNotice: string | null;
  runtimeError: string | null;
  runtimeRetryLabel?: string | null;
  onPromptChange: (value: string) => void;
  onToneChange: (value: string) => void;
  onSend: () => void;
  onQuickAction: (actionId: string) => void;
  onPrepareLocalAi: () => void;
  onRepairLocalAi: () => void;
  onActivateModel: (modelId: string) => void;
  onRefreshRuntime: () => void;
  onRetryRuntimeAction?: () => void;
  onActivate?: () => void;
  panelRef?: Ref<HTMLElement>;
  id?: string;
  className?: string;
  ariaHidden?: boolean;
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
  runtimeRetryLabel,
  onPromptChange,
  onToneChange,
  onSend,
  onQuickAction,
  onPrepareLocalAi,
  onRepairLocalAi,
  onActivateModel,
  onRefreshRuntime,
  onRetryRuntimeAction,
  onActivate,
  panelRef,
  id,
  className,
  ariaHidden,
}: ChatPaneProps) {
  const modelReady =
    runtimeStatus.codex.available &&
    runtimeStatus.local_ai.running &&
    runtimeStatus.runtime_state === "ready";
  const [activeTab, setActiveTab] = useState<AssistantPanelTab>(() =>
    modelReady ? "chat" : "system",
  );
  const assistantTabRefs = useRef<Record<AssistantPanelTab, HTMLButtonElement | null>>({
    chat: null,
    actions: null,
    system: null,
  });
  const systemTabHasStatus = Boolean(
    runtimeNotice || runtimeError || isRuntimeActionPending || !modelReady,
  );
  const showChatSetupNotice = !modelReady;

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

  function activateAssistantTab(
    tab: AssistantPanelTab,
    shouldFocus = false,
  ) {
    setActiveTab(tab);

    if (shouldFocus) {
      assistantTabRefs.current[tab]?.focus();
    }
  }

  function handleAssistantTabKeyDown(
    event: KeyboardEvent<HTMLButtonElement>,
    currentTab: AssistantPanelTab,
  ) {
    const nextTab = getNextHorizontalTabId(
      ASSISTANT_PANEL_TABS,
      currentTab,
      event.key,
    );

    if (!nextTab) {
      return;
    }

    event.preventDefault();
    activateAssistantTab(nextTab, true);
  }

  return (
    <aside
      className={`chat-panel app-section app-section-user ${className ?? ""}`.trim()}
      id={id}
      aria-hidden={ariaHidden}
      onFocusCapture={onActivate}
      onPointerDownCapture={onActivate}
      ref={panelRef}
      tabIndex={-1}
    >
      <div className="chat-header">
        <div>
          <p className="eyebrow">Lokal skriveassistent</p>
          <h2>Chat og kommandoer</h2>
        </div>
        <span className={`status-dot ${modelReady ? "ready" : "offline"}`}>
          {modelReady ? "Klar" : "Oppsett"}
        </span>
      </div>

      <div
        aria-label="Assistentpanel"
        aria-orientation="horizontal"
        className="panel-tabs assistant-tabs"
        role="tablist"
      >
        <button
          aria-controls="assistant-chat-panel"
          aria-selected={activeTab === "chat"}
          className="panel-tab-button"
          id="assistant-chat-tab"
          onClick={() => activateAssistantTab("chat")}
          onKeyDown={(event) => handleAssistantTabKeyDown(event, "chat")}
          ref={(node) => {
            assistantTabRefs.current.chat = node;
          }}
          role="tab"
          tabIndex={activeTab === "chat" ? 0 : -1}
          type="button"
        >
          Chat
        </button>
        <button
          aria-controls="assistant-actions-panel"
          aria-selected={activeTab === "actions"}
          className="panel-tab-button"
          id="assistant-actions-tab"
          onClick={() => activateAssistantTab("actions")}
          onKeyDown={(event) => handleAssistantTabKeyDown(event, "actions")}
          ref={(node) => {
            assistantTabRefs.current.actions = node;
          }}
          role="tab"
          tabIndex={activeTab === "actions" ? 0 : -1}
          type="button"
        >
          Snarveier
        </button>
        <button
          aria-controls="assistant-system-panel"
          aria-selected={activeTab === "system"}
          className="panel-tab-button panel-tab-button-system"
          id="assistant-system-tab"
          onClick={() => activateAssistantTab("system")}
          onKeyDown={(event) => handleAssistantTabKeyDown(event, "system")}
          ref={(node) => {
            assistantTabRefs.current.system = node;
          }}
          role="tab"
          tabIndex={activeTab === "system" ? 0 : -1}
          type="button"
        >
          System
          {systemTabHasStatus ? <span className="panel-tab-status-dot" aria-hidden="true" /> : null}
        </button>
      </div>

      <div className="chat-tab-panels">
        <section
          aria-labelledby="assistant-chat-tab"
          className="chat-tab-panel chat-tab-panel-chat"
          hidden={activeTab !== "chat"}
          id="assistant-chat-panel"
          role="tabpanel"
        >
          <div className="message-list">
            {messages.map((message) => (
              <article className={`message-bubble ${message.role}`} key={message.id}>
                <span>{message.role === "assistant" ? "AI" : message.role === "system" ? "System" : "Du"}</span>
                <p>{message.text}</p>
              </article>
            ))}
          </div>

          <div className="chat-form">
            {showChatSetupNotice ? (
              <div className="chat-setup-notice" role="status">
                <strong>Lokal AI trenger oppsett</strong>
                <p>{setupText}</p>
                <button
                  className="ghost-button"
                  onClick={() => activateAssistantTab("system", true)}
                  type="button"
                >
                  Åpne systemfanen
                </button>
              </div>
            ) : null}
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
        </section>

        <section
          aria-labelledby="assistant-actions-tab"
          className="chat-tab-panel chat-tab-panel-actions"
          hidden={activeTab !== "actions"}
          id="assistant-actions-panel"
          role="tabpanel"
        >
          <p className="panel-section-copy">
            Velg en snarvei når du vil sende en rask kommando uten å skrive hele prompten selv.
          </p>
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
        </section>

        <section
          aria-labelledby="assistant-system-tab"
          className="chat-tab-panel chat-tab-panel-system"
          hidden={activeTab !== "system"}
          id="assistant-system-panel"
          role="tabpanel"
        >
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
                onClick={onRefreshRuntime}
                type="button"
              >
                <RefreshCcw size={14} />
                Oppdater status
              </button>
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
            {runtimeError && runtimeRetryLabel && onRetryRuntimeAction ? (
              <button
                className="ghost-button"
                disabled={isRuntimeActionPending}
                onClick={onRetryRuntimeAction}
                type="button"
              >
                <RefreshCcw size={14} />
                {runtimeRetryLabel}
              </button>
            ) : null}
            <StatusStrip compact runtimeStatus={runtimeStatus} />
          </div>
        </section>
      </div>
    </aside>
  );
}
