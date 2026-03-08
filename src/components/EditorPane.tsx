import { forwardRef, useEffect, useImperativeHandle } from "react";
import { EditorContent, useEditor } from "@tiptap/react";
import StarterKit from "@tiptap/starter-kit";
import Underline from "@tiptap/extension-underline";
import Placeholder from "@tiptap/extension-placeholder";
import { Table } from "@tiptap/extension-table";
import TableRow from "@tiptap/extension-table-row";
import TableCell from "@tiptap/extension-table-cell";
import TableHeader from "@tiptap/extension-table-header";
import {
  Bold,
  Heading1,
  Heading2,
  Italic,
  List,
  Pilcrow,
  Quote,
  Table2,
  Underline as UnderlineIcon,
  type LucideIcon,
} from "lucide-react";
import type { Editor, JSONContent } from "@tiptap/core";
import type { EditorAction, StoredDocument } from "../types";

export interface EditorPaneHandle {
  applyAction: (action: EditorAction) => void;
}

interface EditorPaneProps {
  document: StoredDocument;
  onContentChange: (content: JSONContent, html: string, plainText: string) => void;
  onSelectionChange: (selectionText: string) => void;
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

interface ToolbarButton {
  icon: LucideIcon;
  label: string;
  command: (editor: Editor) => void;
  isActive: (editor: Editor) => boolean;
}

const toolbarButtons: ToolbarButton[] = [
  {
    icon: Heading1,
    label: "H1",
    command: (editor) => {
      editor.chain().focus().toggleHeading({ level: 1 }).run();
    },
    isActive: (editor) => editor.isActive("heading", { level: 1 }),
  },
  {
    icon: Heading2,
    label: "H2",
    command: (editor) => {
      editor.chain().focus().toggleHeading({ level: 2 }).run();
    },
    isActive: (editor) => editor.isActive("heading", { level: 2 }),
  },
  {
    icon: Bold,
    label: "Fet",
    command: (editor) => {
      editor.chain().focus().toggleBold().run();
    },
    isActive: (editor) => editor.isActive("bold"),
  },
  {
    icon: Italic,
    label: "Kursiv",
    command: (editor) => {
      editor.chain().focus().toggleItalic().run();
    },
    isActive: (editor) => editor.isActive("italic"),
  },
  {
    icon: UnderlineIcon,
    label: "Understreket",
    command: (editor) => {
      editor.chain().focus().toggleUnderline().run();
    },
    isActive: (editor) => editor.isActive("underline"),
  },
  {
    icon: List,
    label: "Liste",
    command: (editor) => {
      editor.chain().focus().toggleBulletList().run();
    },
    isActive: (editor) => editor.isActive("bulletList"),
  },
  {
    icon: Quote,
    label: "Sitat",
    command: (editor) => {
      editor.chain().focus().toggleBlockquote().run();
    },
    isActive: (editor) => editor.isActive("blockquote"),
  },
  {
    icon: Table2,
    label: "Tabell",
    command: (editor) => {
      editor.chain().focus().insertTable({ rows: 3, cols: 3, withHeaderRow: true }).run();
    },
    isActive: () => false,
  },
  {
    icon: Pilcrow,
    label: "Avsnitt",
    command: (editor) => {
      editor.chain().focus().setParagraph().run();
    },
    isActive: (editor) => editor.isActive("paragraph"),
  },
];

export const EditorPane = forwardRef<EditorPaneHandle, EditorPaneProps>(function EditorPane(
  { document, onContentChange, onSelectionChange },
  ref,
) {
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
    },
    onSelectionUpdate({ editor: currentEditor }) {
      const { from, to } = currentEditor.state.selection;
      onSelectionChange(currentEditor.state.doc.textBetween(from, to, "\n"));
    },
  });

  useEffect(() => {
    if (!editor) {
      return;
    }

    editor.commands.setContent(document.content, { emitUpdate: false });
    onSelectionChange("");
  }, [document.id, editor, onSelectionChange]);

  useImperativeHandle(ref, () => ({
    applyAction(action) {
      if (!editor) {
        return;
      }

      const html = textToHtml(action.content);

      if (action.action_type === "replace_selection") {
        editor.chain().focus().insertContent(html).run();
        return;
      }

      if (action.action_type === "insert_after_cursor") {
        editor.chain().focus().insertContent(html).run();
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
  }), [editor]);

  return (
    <section className="editor-shell">
      <div className="editor-toolbar">
        {toolbarButtons.map(({ icon: Icon, label, command, isActive }) => (
          <button
            key={label}
            className={`toolbar-button ${editor && isActive(editor) ? "active" : ""}`}
            onClick={() => editor && command(editor)}
            type="button"
          >
            <Icon size={16} />
            {label}
          </button>
        ))}
      </div>
      <div className="editor-surface">
        <EditorContent editor={editor} />
      </div>
    </section>
  );
});
