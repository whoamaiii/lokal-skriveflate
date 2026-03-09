import type { Editor } from "@tiptap/core";
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

export type ToolbarActionId =
  | "heading1"
  | "heading2"
  | "bold"
  | "italic"
  | "underline"
  | "bulletList"
  | "blockquote"
  | "table"
  | "paragraph";

export interface ToolbarAction {
  id: ToolbarActionId;
  icon: LucideIcon;
  label: string;
  command: (editor: Editor) => void;
  isActive: (editor: Editor) => boolean;
}

export const toolbarActions: ToolbarAction[] = [
  {
    id: "heading1",
    icon: Heading1,
    label: "H1",
    command: (editor) => {
      editor.chain().focus().toggleHeading({ level: 1 }).run();
    },
    isActive: (editor) => editor.isActive("heading", { level: 1 }),
  },
  {
    id: "heading2",
    icon: Heading2,
    label: "H2",
    command: (editor) => {
      editor.chain().focus().toggleHeading({ level: 2 }).run();
    },
    isActive: (editor) => editor.isActive("heading", { level: 2 }),
  },
  {
    id: "bold",
    icon: Bold,
    label: "Fet",
    command: (editor) => {
      editor.chain().focus().toggleBold().run();
    },
    isActive: (editor) => editor.isActive("bold"),
  },
  {
    id: "italic",
    icon: Italic,
    label: "Kursiv",
    command: (editor) => {
      editor.chain().focus().toggleItalic().run();
    },
    isActive: (editor) => editor.isActive("italic"),
  },
  {
    id: "underline",
    icon: UnderlineIcon,
    label: "Understreket",
    command: (editor) => {
      editor.chain().focus().toggleUnderline().run();
    },
    isActive: (editor) => editor.isActive("underline"),
  },
  {
    id: "bulletList",
    icon: List,
    label: "Liste",
    command: (editor) => {
      editor.chain().focus().toggleBulletList().run();
    },
    isActive: (editor) => editor.isActive("bulletList"),
  },
  {
    id: "blockquote",
    icon: Quote,
    label: "Sitat",
    command: (editor) => {
      editor.chain().focus().toggleBlockquote().run();
    },
    isActive: (editor) => editor.isActive("blockquote"),
  },
  {
    id: "table",
    icon: Table2,
    label: "Tabell",
    command: (editor) => {
      editor.chain().focus().insertTable({ rows: 3, cols: 3, withHeaderRow: true }).run();
    },
    isActive: () => false,
  },
  {
    id: "paragraph",
    icon: Pilcrow,
    label: "Avsnitt",
    command: (editor) => {
      editor.chain().focus().setParagraph().run();
    },
    isActive: (editor) => editor.isActive("paragraph"),
  },
];
