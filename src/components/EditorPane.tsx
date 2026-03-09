import { forwardRef, useEffect, useEffectEvent, useImperativeHandle } from "react";
import { EditorContent, useEditor } from "@tiptap/react";
import StarterKit from "@tiptap/starter-kit";
import Underline from "@tiptap/extension-underline";
import Placeholder from "@tiptap/extension-placeholder";
import { Table } from "@tiptap/extension-table";
import TableRow from "@tiptap/extension-table-row";
import TableCell from "@tiptap/extension-table-cell";
import TableHeader from "@tiptap/extension-table-header";
import type { Editor, JSONContent } from "@tiptap/core";
import type {
  EditorSelection,
  PendingActionPreview,
  StoredDocument,
} from "../types";
import {
  toolbarActions,
  type ToolbarActionId,
} from "./editorToolbar";

export interface EditorPaneHandle {
  applyAction: (preview: PendingActionPreview) => void;
  focusEditor: () => void;
  runToolbarAction: (actionId: ToolbarActionId) => void;
}

interface EditorPaneProps {
  document: StoredDocument;
  onContentChange: (content: JSONContent, html: string, plainText: string) => void;
  onSelectionChange: (selection: EditorSelection) => void;
  onToolbarStateChange?: (activeActions: ToolbarActionId[]) => void;
}

function escapeHtml(value: string) {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll("\"", "&quot;")
    .replaceAll("'", "&#039;");
}

function textToHtml(text: string) {
  return text
    .split(/\n{2,}/)
    .map((paragraph) => paragraph.trim())
    .filter(Boolean)
    .map((paragraph) => `<p>${escapeHtml(paragraph).replaceAll("\n", "<br />")}</p>`)
    .join("");
}

export const EditorPane = forwardRef<EditorPaneHandle, EditorPaneProps>(function EditorPane(
  { document, onContentChange, onSelectionChange, onToolbarStateChange },
  ref,
) {
  const emitToolbarState = useEffectEvent((currentEditor: Editor | null | undefined) => {
    if (!currentEditor || !onToolbarStateChange) {
      return;
    }

    onToolbarStateChange(
      toolbarActions
        .filter((action) => action.isActive(currentEditor))
        .map((action) => action.id),
    );
  });

  const editor = useEditor({
    extensions: [
      StarterKit.configure({
        heading: { levels: [1, 2, 3] },
      }),
      Underline,
      Placeholder.configure({
        placeholder: "Skriv lokalt. AI-forslag blir alltid vist som preview før de settes inn.",
      }),
      Table.configure({
        resizable: true,
      }),
      TableRow,
      TableHeader,
      TableCell,
    ],
    content: document.content,
    immediatelyRender: false,
    onUpdate({ editor: currentEditor }) {
      onContentChange(
        currentEditor.getJSON(),
        currentEditor.getHTML(),
        currentEditor.getText({ blockSeparator: "\n\n" }),
      );
      emitToolbarState(currentEditor);
    },
    onSelectionUpdate({ editor: currentEditor }) {
      const { from, to } = currentEditor.state.selection;
      onSelectionChange({
        text: currentEditor.state.doc.textBetween(from, to, "\n"),
        from,
        to,
      });
      emitToolbarState(currentEditor);
    },
    onCreate({ editor: currentEditor }) {
      emitToolbarState(currentEditor);
    },
  });

  useEffect(() => {
    if (!editor) {
      return;
    }

    editor.commands.setContent(document.content, { emitUpdate: false });
    onSelectionChange({
      text: "",
      from: 1,
      to: 1,
    });
    emitToolbarState(editor);
  }, [document.id, editor, emitToolbarState, onSelectionChange]);

  useImperativeHandle(ref, () => ({
    applyAction(preview) {
      if (!editor) {
        return;
      }

      const { action } = preview;
      const html = textToHtml(action.content);
      const selectionFrom = Math.max(1, preview.selection_from);
      const selectionTo = Math.max(selectionFrom, preview.selection_to);

      if (action.action_type === "replace_selection") {
        editor
          .chain()
          .focus()
          .setTextSelection({ from: selectionFrom, to: selectionTo })
          .insertContent(html)
          .run();
        return;
      }

      if (action.action_type === "insert_after_cursor") {
        editor
          .chain()
          .focus()
          .setTextSelection(selectionTo)
          .insertContent(html)
          .run();
        return;
      }

      if (action.action_type === "append_to_document") {
        editor.chain().focus("end").insertContent(html).run();
        return;
      }

      editor
        .chain()
        .focus("end")
        .insertContent(`<h2>${escapeHtml(action.title)}</h2>${html}`)
        .run();
    },
    focusEditor() {
      editor?.commands.focus();
    },
    runToolbarAction(actionId) {
      if (!editor) {
        return;
      }

      const action = toolbarActions.find((entry) => entry.id === actionId);
      action?.command(editor);
      emitToolbarState(editor);
    },
  }), [editor, emitToolbarState]);

  return (
    <section className="editor-shell">
      <div className="editor-surface">
        <EditorContent editor={editor} />
      </div>
    </section>
  );
});
