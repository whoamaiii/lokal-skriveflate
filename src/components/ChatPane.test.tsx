import type { ComponentProps } from "react";
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ChatPane } from "./ChatPane";
import type { RuntimeStatus } from "../types";

function makeRuntimeStatus(overrides: Partial<RuntimeStatus> = {}): RuntimeStatus {
  const base: RuntimeStatus = {
    offline_mode: true,
    local_only: true,
    selected_model: "lokal-4b",
    runtime_state: "ready",
    can_send: true,
    will_start_on_demand: false,
    blocking_reason: null,
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
    runtime_home: "/tmp/local-ai",
    listen_address: "127.0.0.1:8000",
  };

  return {
    ...base,
    ...overrides,
    codex: {
      ...base.codex,
      ...overrides.codex,
    },
    local_ai: {
      ...base.local_ai,
      ...overrides.local_ai,
    },
    available_models: overrides.available_models ?? base.available_models,
  };
}

function makeChatPaneProps(
  overrides: Partial<ComponentProps<typeof ChatPane>> = {},
): ComponentProps<typeof ChatPane> {
  return {
    messages: [
      {
        id: "msg-1",
        role: "assistant",
        text: "Hei fra lokal AI",
        created_at: "2026-03-08T10:00:00Z",
      },
    ],
    prompt: "",
    tone: "Klar og profesjonell",
    runtimeStatus: makeRuntimeStatus(),
    isSending: false,
    isRuntimeActionPending: false,
    runtimeNotice: null,
    runtimeError: null,
    runtimeRetryLabel: null,
    onPromptChange: vi.fn(),
    onToneChange: vi.fn(),
    onSend: vi.fn(),
    onQuickAction: vi.fn(),
    onPrepareLocalAi: vi.fn(),
    onRepairLocalAi: vi.fn(),
    onRefreshRuntime: vi.fn(),
    onRetryRuntimeAction: undefined,
    onActivate: undefined,
    panelRef: undefined,
    ...overrides,
  };
}

describe("ChatPane", () => {
  it("defaults to the chat tab when the runtime is ready", () => {
    render(<ChatPane {...makeChatPaneProps()} />);

    const chatTab = screen.getByRole("tab", { name: "Chat" });
    const systemTab = screen.getByRole("tab", { name: "System" });

    expect(chatTab.getAttribute("aria-selected")).toBe("true");
    expect(chatTab.getAttribute("tabindex")).toBe("0");
    expect(systemTab.getAttribute("tabindex")).toBe("-1");
  });

  it("defaults to the system tab when the runtime is not ready", () => {
    render(
      <ChatPane
        {...makeChatPaneProps({
          runtimeStatus: makeRuntimeStatus({
            runtime_state: "not_prepared",
            can_send: false,
            will_start_on_demand: false,
            blocking_reason: "runtime_not_prepared",
            local_ai: {
              available: true,
              running: false,
              details: null,
            },
          }),
        })}
      />,
    );

    const systemTab = screen.getByRole("tab", { name: "System" });

    expect(systemTab.getAttribute("aria-selected")).toBe("true");
    expect(screen.getByText("Lokalt modelloppsett")).toBeTruthy();
  });

  it("supports keyboard navigation across assistant tabs", () => {
    render(<ChatPane {...makeChatPaneProps()} />);

    const chatTab = screen.getByRole("tab", { name: "Chat" });
    const actionsTab = screen.getByRole("tab", { name: "Snarveier" });
    const systemTab = screen.getByRole("tab", { name: "System" });

    (chatTab as HTMLButtonElement).focus();
    fireEvent.keyDown(chatTab, { key: "ArrowRight" });

    expect(actionsTab.getAttribute("aria-selected")).toBe("true");
    expect(document.activeElement).toBe(actionsTab);

    fireEvent.keyDown(actionsTab, { key: "End" });

    expect(systemTab.getAttribute("aria-selected")).toBe("true");
    expect(document.activeElement).toBe(systemTab);

    fireEvent.keyDown(systemTab, { key: "Home" });

    expect(chatTab.getAttribute("aria-selected")).toBe("true");
    expect(document.activeElement).toBe(chatTab);

    fireEvent.keyDown(chatTab, { key: "ArrowLeft" });

    expect(systemTab.getAttribute("aria-selected")).toBe("true");
    expect(document.activeElement).toBe(systemTab);
  });

  it("shows a setup notice in chat if the runtime becomes unavailable and opens the system tab from the CTA", () => {
    const props = makeChatPaneProps();
    const { rerender } = render(<ChatPane {...props} />);

    rerender(
      <ChatPane
        {...props}
        runtimeStatus={makeRuntimeStatus({
          runtime_state: "repair_required",
          can_send: false,
          will_start_on_demand: false,
          blocking_reason: "runtime_repair_required",
          local_ai: {
            available: true,
            running: false,
            details: "Runtime trenger reparasjon.",
          },
        })}
      />,
    );

    expect(screen.getByText("Lokal AI trenger oppsett")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Åpne systemfanen" }));

    expect(
      screen.getByRole("tab", { name: "System" }).getAttribute("aria-selected"),
    ).toBe("true");
    expect(screen.getByText("Lokalt modelloppsett")).toBeTruthy();
  });

  it("keeps retry and setup controls reachable in the system tab", () => {
    const onRetryRuntimeAction = vi.fn();

    render(
      <ChatPane
        {...makeChatPaneProps({
          runtimeStatus: makeRuntimeStatus({
            runtime_state: "repair_required",
            can_send: false,
            will_start_on_demand: false,
            blocking_reason: "runtime_repair_required",
            local_ai: {
              available: true,
              running: false,
              details: "Reparasjon kreves.",
            },
          }),
          runtimeError: "Kunne ikke starte lokal AI.",
          runtimeRetryLabel: "Prøv igjen",
          onRetryRuntimeAction,
        })}
      />,
    );

    expect(screen.getByRole("button", { name: "Klargjør lokal AI" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Reparer" })).toBeTruthy();
    expect(screen.getByText("Standardmodell: Lokal 4B")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Prøv igjen" }));

    expect(onRetryRuntimeAction).toHaveBeenCalledTimes(1);
  });

  it("allows sends while the runtime will self-start on demand", () => {
    const onSend = vi.fn();

    render(
      <ChatPane
        {...makeChatPaneProps({
          prompt: "Skriv videre",
          onSend,
          runtimeStatus: makeRuntimeStatus({
            runtime_state: "degraded",
            can_send: true,
            will_start_on_demand: true,
            blocking_reason: null,
            local_ai: {
              available: true,
              running: false,
              details: "Starter ved første forespørsel.",
            },
          }),
        })}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Send til lokal motor" }));

    expect(onSend).toHaveBeenCalledTimes(1);
    expect(
      screen.getByText("Lokal AI starter ved første forespørsel"),
    ).toBeTruthy();
  });
});
