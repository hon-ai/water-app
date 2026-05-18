import { describe, expect, it } from "vitest";
import { schema } from "./schema";
import { markdownFromDoc } from "./serialize";

describe("markdownFromDoc inline marks", () => {
  it("serializes a bold span as **...**", () => {
    const doc = schema.node("doc", null, [
      schema.node("paragraph", { blockId: "^bk-0001" }, [
        schema.text("hello "),
        schema.text("world", [schema.marks.strong!.create()]),
      ]),
    ]);
    expect(markdownFromDoc(doc)).toBe("^bk-0001 hello **world**\n");
  });

  it("serializes an italic span as *...*", () => {
    const doc = schema.node("doc", null, [
      schema.node("paragraph", { blockId: "^bk-0001" }, [
        schema.text("she said "),
        schema.text("softly", [schema.marks.em!.create()]),
      ]),
    ]);
    expect(markdownFromDoc(doc)).toBe("^bk-0001 she said *softly*\n");
  });

  it("serializes a link as [text](url)", () => {
    const doc = schema.node("doc", null, [
      schema.node("paragraph", { blockId: "^bk-0001" }, [
        schema.text("see "),
        schema.text("the docs", [
          schema.marks.link!.create({ href: "https://example.com" }),
        ]),
      ]),
    ]);
    expect(markdownFromDoc(doc)).toBe(
      "^bk-0001 see [the docs](https://example.com)\n",
    );
  });

  it("composes strong + em as **bold *italic***", () => {
    const doc = schema.node("doc", null, [
      schema.node("paragraph", { blockId: "^bk-0001" }, [
        schema.text("bold ", [schema.marks.strong!.create()]),
        schema.text("italic", [
          schema.marks.strong!.create(),
          schema.marks.em!.create(),
        ]),
      ]),
    ]);
    expect(markdownFromDoc(doc)).toBe("^bk-0001 **bold *italic***\n");
  });

  it("escapes literal asterisks in source text", () => {
    const doc = schema.node("doc", null, [
      schema.node("paragraph", { blockId: "^bk-0001" }, [
        schema.text("a*b*c"),
      ]),
    ]);
    expect(markdownFromDoc(doc)).toBe("^bk-0001 a\\*b\\*c\n");
  });

  it("escapes literal brackets in source text", () => {
    const doc = schema.node("doc", null, [
      schema.node("paragraph", { blockId: "^bk-0001" }, [
        schema.text("see [the docs] please"),
      ]),
    ]);
    expect(markdownFromDoc(doc)).toBe(
      "^bk-0001 see \\[the docs\\] please\n",
    );
  });
});
