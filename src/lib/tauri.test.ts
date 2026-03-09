import { beforeEach, describe, expect, it, vi } from "vitest";
import type { AssistantTurnInput } from "../types";

const invokeMock = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
}));

import {
  bootstrapApp,
  createDocument,
  sendAssistantTurn,
} from "./tauri";

describe("tauri bridge", () => {
  beforeEach(() => {
    localStorage.clear();
    invokeMock.mockReset();
    delete (globalThis as typeof globalThis & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__;
  });

  it("boots with browser preview data when Tauri is unavailable", async () => {
    const bootstrap = await bootstrapApp();

    expect(invokeMock).not.toHaveBeenCalled();
    expect(bootstrap.active_document.title).toBeTruthy();
    expect(bootstrap.documents.length).toBeGreaterThan(0);
    expect(bootstrap.runtime_status.local_ai.running).toBe(true);
  });

  it("keeps browser preview flows usable without Tauri", async () => {
    const created = await createDocument("Testdokument");
    const request: AssistantTurnInput = {
      document_id: created.active_document.id,
      document_revision: created.active_document.content_revision,
      prompt: "Skriv et kort neste steg",
      quick_action: "continue_writing",
      tone: "Klar og profesjonell",
      selection_text: null,
    };

    const result = await sendAssistantTurn(request);
    const bootstrap = await bootstrapApp();

    expect(result.editor_action?.content).toContain("Neste forslag");
    expect(
      result.document.messages[result.document.messages.length - 1]?.role,
    ).toBe("assistant");
    expect(
      bootstrap.documents.some((document) => document.id === created.active_document.id),
    ).toBe(true);
  });

  it("still uses Tauri invoke when the desktop bridge is present", async () => {
    (globalThis as typeof globalThis & {
      __TAURI_INTERNALS__?: { invoke: typeof invokeMock };
    }).__TAURI_INTERNALS__ = {
      invoke: invokeMock,
    };
    invokeMock.mockResolvedValue({
      ok: true,
    });

    await bootstrapApp();

    expect(invokeMock).toHaveBeenCalledWith("bootstrap");
  });
});
