import { Activity, Bot, Cpu, ShieldCheck } from "lucide-react";
import type { RuntimeStatus } from "../types";

interface StatusStripProps {
  runtimeStatus: RuntimeStatus;
}

export function StatusStrip({ runtimeStatus }: StatusStripProps) {
  const runtimeLabel =
    runtimeStatus.runtime_state === "ready"
      ? "Klar"
      : runtimeStatus.runtime_state === "degraded"
        ? "Klargjort"
        : runtimeStatus.runtime_state === "repair_required"
          ? "Reparer"
          : "Ikke klar";

  return (
    <footer className="status-strip">
      <div className="status-pill">
        <ShieldCheck size={14} />
        {runtimeStatus.local_only ? "Kun lokal kjøring" : "Blandet modus"}
      </div>
      <div className={`status-pill ${runtimeStatus.codex.available ? "ok" : "warn"}`}>
        <Bot size={14} />
        Codex {runtimeStatus.codex.available ? "funnet" : "mangler"}
      </div>
      <div
        className={`status-pill ${
          runtimeStatus.local_ai.running ? "ok" : runtimeStatus.local_ai.available ? "subtle" : "warn"
        }`}
      >
        <Activity size={14} />
        Lokal AI {runtimeLabel}
      </div>
      <div className={`status-pill ${runtimeStatus.local_ai.available ? "ok" : "warn"}`}>
        <Cpu size={14} />
        {runtimeStatus.local_ai.available ? "Runtime klargjort" : "Runtime mangler"}
      </div>
      <div className="status-pill subtle">
        Modell: {runtimeStatus.selected_model}
      </div>
    </footer>
  );
}
