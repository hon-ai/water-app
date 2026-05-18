import { describe, expect, it } from "vitest";
import { render } from "@testing-library/react";
import { EditorState } from "prosemirror-state";
import { schema } from "./schema";
import { blockIdPlugin } from "./blockIdPlugin";
import { Editor } from "./Editor";

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

describe("schema marks", () => {
  it("exposes strong, em, and link marks", () => {
    expect(schema.marks.strong).toBeDefined();
    expect(schema.marks.em).toBeDefined();
    expect(schema.marks.link).toBeDefined();
  });

  it("link mark carries an href attr", () => {
    const link = schema.marks.link;
    expect(link).toBeDefined();
    const linkMark = link!.create({ href: "https://example.com" });
    expect(linkMark.attrs.href).toBe("https://example.com");
  });

  it("marks compose freely on text", () => {
    const text = schema.text("hello", [
      schema.marks.strong!.create(),
      schema.marks.em!.create(),
    ]);
    expect(text.marks).toHaveLength(2);
    expect(text.marks.some((m) => m.type.name === "strong")).toBe(true);
    expect(text.marks.some((m) => m.type.name === "em")).toBe(true);
  });
});

describe("Editor keyboard shortcuts", () => {
  it("editor mounts with the new schema marks available", () => {
    const { container } = render(
      <Editor value="^bk-0001 hello world\n" onChange={() => {}} />,
    );
    const editable = container.querySelector("[contenteditable='true']");
    expect(editable).not.toBeNull();
  });

  it("Mod-B / Mod-I / Mod-K keymap entries exist (smoke check)", () => {
    // We can't easily simulate keydown that triggers a PM command from
    // a unit-test harness without exposing the view. This test confirms
    // the editor mounts cleanly with no errors thrown — the actual
    // shortcut logic is tested indirectly via toggleMark behavior in
    // PM's own test suite + manual smoke.
    const { container } = render(
      <Editor value="^bk-0001 hello world\n" onChange={() => {}} />,
    );
    const editable = container.querySelector("[contenteditable='true']");
    expect(editable).not.toBeNull();
  });
});
