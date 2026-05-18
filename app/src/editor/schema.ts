// ProseMirror schema for the Water block editor.
//
// All block-level nodes carry a `blockId` attribute (`^bk-XXXX`). The
// block-id plugin (./blockIdPlugin.ts) assigns and de-duplicates those ids
// on every transaction, so it's safe for callers to construct blocks with
// `blockId: ""` and let the plugin fill them in.
//
// Block kinds supported: paragraph, heading (h2/h3), scene_break, dialogue,
// ordered_list, bullet_list (via prosemirror-schema-list).

import {
  Schema,
  type DOMOutputSpec,
  type Mark,
  type NodeSpec,
} from "prosemirror-model";
import { schema as basicSchema } from "prosemirror-schema-basic";
import { addListNodes } from "prosemirror-schema-list";

const blockAttrs = { blockId: { default: "" } };

const basicParagraph = basicSchema.spec.nodes.get("paragraph");
const basicHeading = basicSchema.spec.nodes.get("heading");
if (!basicParagraph || !basicHeading) {
  throw new Error("prosemirror-schema-basic must define paragraph + heading");
}

const paragraphSpec: NodeSpec = {
  ...basicParagraph,
  attrs: blockAttrs,
  parseDOM: [
    {
      tag: "p",
      getAttrs: (dom) => ({
        blockId: (dom as HTMLElement).getAttribute("data-bid") ?? "",
      }),
    },
  ],
  toDOM: (node): DOMOutputSpec => ["p", { "data-bid": node.attrs.blockId }, 0],
};

const headingSpec: NodeSpec = {
  ...basicHeading,
  attrs: { ...blockAttrs, level: { default: 2 } },
  parseDOM: [
    {
      tag: "h2",
      getAttrs: (d) => ({
        level: 2,
        blockId: (d as HTMLElement).getAttribute("data-bid") ?? "",
      }),
    },
    {
      tag: "h3",
      getAttrs: (d) => ({
        level: 3,
        blockId: (d as HTMLElement).getAttribute("data-bid") ?? "",
      }),
    },
  ],
  toDOM: (node): DOMOutputSpec => [
    `h${node.attrs.level}`,
    { "data-bid": node.attrs.blockId },
    0,
  ],
};

const sceneBreakSpec: NodeSpec = {
  group: "block",
  atom: true,
  attrs: blockAttrs,
  parseDOM: [{ tag: "hr.scene-break" }],
  toDOM: (node): DOMOutputSpec => [
    "hr",
    { class: "scene-break", "data-bid": node.attrs.blockId },
  ],
};

const dialogueSpec: NodeSpec = {
  group: "block",
  content: "inline*",
  attrs: blockAttrs,
  parseDOM: [
    {
      tag: "p.dialogue",
      getAttrs: (d) => ({
        blockId: (d as HTMLElement).getAttribute("data-bid") ?? "",
      }),
    },
  ],
  toDOM: (node): DOMOutputSpec => [
    "p",
    { class: "dialogue", "data-bid": node.attrs.blockId },
    0,
  ],
};

const blockNodes = basicSchema.spec.nodes
  .update("paragraph", paragraphSpec)
  .update("heading", headingSpec)
  .addToEnd("scene_break", sceneBreakSpec)
  .addToEnd("dialogue", dialogueSpec);

const withListsBase = addListNodes(blockNodes, "paragraph block*", "block");

// Extend `list_item` to carry a blockId attr. M2 spec § 5.2 requires list
// items in both `list_ordered` and `list_unordered` to each carry their
// own block-id (per-bullet block addressing). The block-id plugin recurses
// into the list containers to find these.
const baseListItem = withListsBase.get("list_item");
if (!baseListItem) {
  throw new Error("prosemirror-schema-list must define list_item");
}

const listItemSpec: NodeSpec = {
  ...baseListItem,
  attrs: blockAttrs,
  parseDOM: [
    {
      tag: "li",
      getAttrs: (d) => ({
        blockId: (d as HTMLElement).getAttribute("data-bid") ?? "",
      }),
    },
  ],
  toDOM: (node): DOMOutputSpec => [
    "li",
    { "data-bid": node.attrs.blockId },
    0,
  ],
};

const withLists = withListsBase.update("list_item", listItemSpec);

// Override the link mark's toDOM so rendered `<a>` elements expose a
// `title` hint telling the writer how to open the link (M2.5 T7).
// Mod-click is wired up in Editor.tsx's `handleClickOn` and dispatches
// the URL to the Tauri shell plugin. The serializer + parser only care
// about `href`, so adding `title` here is invisible to T1 mark tests.
const baseMarks = basicSchema.spec.marks;
const linkSpec = baseMarks.get("link");
const marksWithLinkTitle = linkSpec
  ? baseMarks.update("link", {
      ...linkSpec,
      toDOM: (mark: Mark): DOMOutputSpec => [
        "a",
        {
          href: mark.attrs["href"],
          title:
            typeof navigator !== "undefined" &&
            /Mac|iPod|iPhone|iPad/.test(navigator.platform)
              ? "Cmd-click to open"
              : "Ctrl-click to open",
        },
        0,
      ],
    })
  : baseMarks;

export const schema = new Schema({
  nodes: withLists,
  marks: marksWithLinkTitle,
});
