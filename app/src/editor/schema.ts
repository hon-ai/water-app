// ProseMirror schema for the Water block editor.
//
// All block-level nodes carry a `blockId` attribute (`^bk-XXXX`). The
// block-id plugin (./blockIdPlugin.ts) assigns and de-duplicates those ids
// on every transaction, so it's safe for callers to construct blocks with
// `blockId: ""` and let the plugin fill them in.
//
// Block kinds supported: paragraph, heading (h2/h3), scene_break, dialogue,
// ordered_list, bullet_list (via prosemirror-schema-list).

import { Schema, type DOMOutputSpec, type NodeSpec } from "prosemirror-model";
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

const withLists = addListNodes(blockNodes, "paragraph block*", "block");

export const schema = new Schema({
  nodes: withLists,
  marks: basicSchema.spec.marks,
});
