import { describe, expect, it } from "vitest";
import { EditorState } from "prosemirror-state";
import { schema } from "./schema";
import { blockIdPlugin } from "./blockIdPlugin";

function emptyDoc() {
  return schema.node("doc", null, [schema.node("paragraph", { blockId: "" })]);
}

describe("blockIdPlugin", () => {
  it("assigns ^bk-XXXX to every block on init", () => {
    const state = EditorState.create({
      doc: emptyDoc(),
      schema,
      plugins: [blockIdPlugin()],
    });
    const stateAfter = state.apply(state.tr); // trigger filter
    let bid: string | null = null;
    stateAfter.doc.forEach((node) => {
      bid = node.attrs.blockId;
    });
    expect(bid).toMatch(/^\^bk-[A-Za-z0-9]{4}$/);
  });

  it("preserves block-ids across split", () => {
    const state = EditorState.create({
      doc: schema.node("doc", null, [
        schema.node("paragraph", { blockId: "^bk-0001" }, [schema.text("hello world")]),
      ]),
      schema,
      plugins: [blockIdPlugin()],
    });
    const tr = state.tr.split(7);
    const next = state.apply(tr);
    const ids: string[] = [];
    next.doc.forEach((n) => ids.push(n.attrs.blockId));
    expect(ids.length).toBe(2);
    expect(ids).toContain("^bk-0001");
    expect(new Set(ids).size).toBe(2);
  });

  it("preserves block-ids on merge (delete-backspace at start)", () => {
    const state = EditorState.create({
      doc: schema.node("doc", null, [
        schema.node("paragraph", { blockId: "^bk-0001" }, [schema.text("a")]),
        schema.node("paragraph", { blockId: "^bk-0002" }, [schema.text("b")]),
      ]),
      schema,
      plugins: [blockIdPlugin()],
    });
    const tr = state.tr.join(3); // merge boundary
    const next = state.apply(tr);
    const ids: string[] = [];
    next.doc.forEach((n) => ids.push(n.attrs.blockId));
    expect(ids).toEqual(["^bk-0001"]);
  });

  it("assigns block-ids to ordered list items", () => {
    const state = EditorState.create({
      doc: schema.node("doc", null, [
        schema.node("ordered_list", null, [
          schema.node("list_item", { blockId: "" }, [
            schema.node("paragraph", { blockId: "" }, [schema.text("first")]),
          ]),
          schema.node("list_item", { blockId: "" }, [
            schema.node("paragraph", { blockId: "" }, [schema.text("second")]),
          ]),
        ]),
      ]),
      schema,
      plugins: [blockIdPlugin()],
    });
    const stateAfter = state.apply(state.tr);
    const listItemIds: string[] = [];
    stateAfter.doc.descendants((node) => {
      if (node.type.name === "list_item") listItemIds.push(node.attrs.blockId);
    });
    expect(listItemIds.length).toBe(2);
    for (const id of listItemIds) expect(id).toMatch(/^\^bk-[A-Za-z0-9]{4}$/);
    expect(new Set(listItemIds).size).toBe(2);
  });
});
