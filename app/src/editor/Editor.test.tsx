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
});
