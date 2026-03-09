import type { SaveDocumentInput, StoredDocument } from "../types";

export function toSaveDocumentInput(document: StoredDocument): SaveDocumentInput {
  return {
    id: document.id,
    title: document.title,
    content: document.content,
    html: document.html,
    plain_text: document.plain_text,
  };
}

export function isSameSaveDocumentInput(left: SaveDocumentInput, right: SaveDocumentInput) {
  return (
    left.id === right.id &&
    left.title === right.title &&
    left.html === right.html &&
    left.plain_text === right.plain_text &&
    JSON.stringify(left.content) === JSON.stringify(right.content)
  );
}

export function hasDocumentChanges(
  document: StoredDocument,
  baseline: SaveDocumentInput | undefined,
) {
  if (!baseline) {
    return true;
  }

  return !isSameSaveDocumentInput(toSaveDocumentInput(document), baseline);
}

export function mergeSavedDocument(
  current: StoredDocument,
  saved: StoredDocument,
  sentPayload: SaveDocumentInput,
): StoredDocument {
  const stillMatchesSent = isSameSaveDocumentInput(toSaveDocumentInput(current), sentPayload);

  return {
    ...saved,
    title: stillMatchesSent ? saved.title : current.title,
    content: stillMatchesSent ? saved.content : current.content,
    html: stillMatchesSent ? saved.html : current.html,
    plain_text: stillMatchesSent ? saved.plain_text : current.plain_text,
    preview: stillMatchesSent ? saved.preview : current.preview,
    thread_id: current.thread_id,
    messages: current.messages,
  };
}

export function mergeAssistantMetadata(
  current: StoredDocument,
  assistantDocument: StoredDocument,
): StoredDocument {
  return {
    ...current,
    updated_at: assistantDocument.updated_at,
    content_revision: assistantDocument.content_revision,
    thread_id: assistantDocument.thread_id,
    messages: assistantDocument.messages,
    snapshot_count: assistantDocument.snapshot_count,
  };
}
