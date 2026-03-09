import { describe, expect, it } from "vitest";
import type { StoredDocument } from "../types";
import {
  hasDocumentChanges,
  isSameSaveDocumentInput,
  mergeAssistantMetadata,
  mergeSavedDocument,
  toSaveDocumentInput,
} from "./documentState";

function makeDocument(overrides: Partial<StoredDocument> = {}): StoredDocument {
  return {
    id: "doc-1",
    title: "Dokument",
    preview: "Kort tekst",
    updated_at: "2026-03-08T10:00:00.000Z",
    workflow_hints: ["rapport"],
    created_at: "2026-03-08T09:00:00.000Z",
    plain_text: "Kort tekst",
    html: "<p>Kort tekst</p>",
    content: {
      type: "doc",
      content: [
        {
          type: "paragraph",
          content: [{ type: "text", text: "Kort tekst" }],
        },
      ],
    },
    content_revision: 2,
    thread_id: "thread-1",
    messages: [],
    snapshot_count: 1,
    ...overrides,
  };
}

describe("documentState", () => {
  it("detects when a document has local changes", () => {
    const document = makeDocument();
    const baseline = toSaveDocumentInput(document);

    expect(hasDocumentChanges(document, baseline)).toBe(false);

    const edited = makeDocument({
      title: "Dokument oppdatert",
    });
    expect(hasDocumentChanges(edited, baseline)).toBe(true);
  });

  it("keeps newer local content when an older save response arrives", () => {
    const sent = makeDocument({
      plain_text: "Første lagrede tekst",
      html: "<p>Første lagrede tekst</p>",
      preview: "Første lagrede tekst",
      content: {
        type: "doc",
        content: [
          {
            type: "paragraph",
            content: [{ type: "text", text: "Første lagrede tekst" }],
          },
        ],
      },
    });
    const current = makeDocument({
      plain_text: "Nyere lokal tekst",
      html: "<p>Nyere lokal tekst</p>",
      preview: "Nyere lokal tekst",
      content: {
        type: "doc",
        content: [
          {
            type: "paragraph",
            content: [{ type: "text", text: "Nyere lokal tekst" }],
          },
        ],
      },
      messages: [{ id: "msg-1", role: "assistant", text: "Hei", created_at: "now" }],
      snapshot_count: 5,
    });
    const saved = makeDocument({
      plain_text: sent.plain_text,
      html: sent.html,
      preview: sent.preview,
      content: sent.content,
      updated_at: "2026-03-08T10:30:00.000Z",
      content_revision: 3,
      messages: [],
      snapshot_count: 2,
    });

    const merged = mergeSavedDocument(current, saved, toSaveDocumentInput(sent));

    expect(merged.plain_text).toBe("Nyere lokal tekst");
    expect(merged.content_revision).toBe(3);
    expect(merged.messages).toEqual(current.messages);
  });

  it("merges assistant metadata without overwriting local draft fields", () => {
    const current = makeDocument({
      plain_text: "Lokal tekst",
      preview: "Lokal tekst",
    });
    const assistantDocument = makeDocument({
      plain_text: "Servertekst",
      preview: "Servertekst",
      updated_at: "2026-03-08T11:00:00.000Z",
      content_revision: 4,
      thread_id: "thread-2",
      messages: [{ id: "msg-2", role: "assistant", text: "Svar", created_at: "now" }],
      snapshot_count: 7,
    });

    const merged = mergeAssistantMetadata(current, assistantDocument);

    expect(merged.plain_text).toBe("Lokal tekst");
    expect(merged.updated_at).toBe("2026-03-08T11:00:00.000Z");
    expect(merged.thread_id).toBe("thread-2");
    expect(merged.messages).toEqual(assistantDocument.messages);
  });

  it("compares save payloads by content and metadata", () => {
    const left = toSaveDocumentInput(makeDocument());
    const right = toSaveDocumentInput(makeDocument());
    const changed = toSaveDocumentInput(
      makeDocument({
        plain_text: "Annen tekst",
      }),
    );

    expect(isSameSaveDocumentInput(left, right)).toBe(true);
    expect(isSameSaveDocumentInput(left, changed)).toBe(false);
  });
});
