import { describe, expect, it } from "vitest";
import { schema } from "./schema";
import { docFromMarkdown, markdownFromDoc } from "./serialize";

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

describe("docFromMarkdown inline marks", () => {
  it("parses **bold** into a strong-marked text node", () => {
    const doc = docFromMarkdown(schema, "^bk-0001 hello **world**\n");
    const para = doc.firstChild!;
    expect(para.type.name).toBe("paragraph");
    const runs: Array<{ text: string; marks: string[] }> = [];
    para.forEach((c) => {
      if (c.isText) runs.push({ text: c.text ?? "", marks: c.marks.map((m) => m.type.name) });
    });
    expect(runs).toEqual([
      { text: "hello ", marks: [] },
      { text: "world", marks: ["strong"] },
    ]);
  });

  it("parses *italic* into an em-marked text node", () => {
    const doc = docFromMarkdown(schema, "^bk-0001 she said *softly*\n");
    const para = doc.firstChild!;
    const runs: Array<{ text: string; marks: string[] }> = [];
    para.forEach((c) => {
      if (c.isText) runs.push({ text: c.text ?? "", marks: c.marks.map((m) => m.type.name) });
    });
    expect(runs).toEqual([
      { text: "she said ", marks: [] },
      { text: "softly", marks: ["em"] },
    ]);
  });

  it("parses [text](url) into a link-marked text node", () => {
    const doc = docFromMarkdown(
      schema,
      "^bk-0001 see [the docs](https://example.com)\n",
    );
    const para = doc.firstChild!;
    const runs: Array<{ text: string; marks: string[]; href?: string }> = [];
    para.forEach((c) => {
      if (c.isText) {
        const link = c.marks.find((m) => m.type.name === "link");
        runs.push({
          text: c.text ?? "",
          marks: c.marks.map((m) => m.type.name),
          ...(link ? { href: link.attrs.href as string } : {}),
        });
      }
    });
    expect(runs).toEqual([
      { text: "see ", marks: [] },
      { text: "the docs", marks: ["link"], href: "https://example.com" },
    ]);
  });

  it("unescapes \\* in source text", () => {
    const doc = docFromMarkdown(schema, "^bk-0001 a\\*b\\*c\n");
    const para = doc.firstChild!;
    const text = para.textContent;
    expect(text).toBe("a*b*c");
  });
});

describe("markdownFromDoc <-> docFromMarkdown round-trip", () => {
  function roundTrip(md: string): string {
    return markdownFromDoc(docFromMarkdown(schema, md));
  }

  it("round-trips a bold paragraph", () => {
    const md = "^bk-0001 hello **world**\n";
    expect(roundTrip(md)).toBe(md);
  });

  it("round-trips an italic paragraph", () => {
    const md = "^bk-0001 she said *softly*\n";
    expect(roundTrip(md)).toBe(md);
  });

  it("round-trips a linked paragraph", () => {
    const md = "^bk-0001 see [the docs](https://example.com)\n";
    expect(roundTrip(md)).toBe(md);
  });

  it("round-trips composed marks", () => {
    const md = "^bk-0001 **bold *italic***\n";
    expect(roundTrip(md)).toBe(md);
  });

  it("round-trips escaped literals", () => {
    const md = "^bk-0001 a\\*b\\*c\n";
    expect(roundTrip(md)).toBe(md);
  });
});

describe("markdownFromDoc round-trip property test", () => {
  function randomRun(seed: number): { text: string; marks: string[]; href?: string } {
    const wordPool = ["the", "rain", "falls", "softly", "quiet", "stone", "bell", "across", "square"];
    const wordCount = 1 + (seed % 4);
    const text = Array.from({ length: wordCount }, (_, i) =>
      wordPool[(seed + i * 7) % wordPool.length]!,
    ).join(" ");
    const marks: string[] = [];
    if (seed % 5 === 0) marks.push("strong");
    if (seed % 3 === 0) marks.push("em");
    let href: string | undefined;
    if (seed % 11 === 0) {
      marks.push("link");
      href = `https://e${seed % 100}.test`;
    }
    return { text, marks, ...(href ? { href } : {}) };
  }

  it("round-trips 50 random docs identically (serialize twice -> same output)", () => {
    const SEED_BASE = 1;
    for (let i = 0; i < 50; i++) {
      const paragraphCount = 1 + (i % 3);
      const paragraphs = [] as Array<ReturnType<typeof schema.node>>;
      for (let p = 0; p < paragraphCount; p++) {
        const runCount = 1 + ((i + p) % 4);
        const runs = Array.from({ length: runCount }, (_, r) => {
          const rr = randomRun(SEED_BASE + i * 31 + p * 7 + r);
          const marks = rr.marks.map((name) => {
            if (name === "link") return schema.marks.link!.create({ href: rr.href });
            if (name === "strong") return schema.marks.strong!.create();
            return schema.marks.em!.create();
          });
          return schema.text(rr.text, marks);
        });
        // Interleave with plain spaces between runs to avoid adjacent
        // marked spans that collapse on serialization.
        const interleaved = runs.flatMap((run, idx) =>
          idx === 0 ? [run] : [schema.text(" "), run],
        );
        paragraphs.push(
          schema.node("paragraph", { blockId: `^bk-${(i * 100 + p).toString().padStart(4, "0")}` }, interleaved),
        );
      }
      const doc = schema.node("doc", null, paragraphs);
      const firstSerialize = markdownFromDoc(doc);
      const secondSerialize = markdownFromDoc(docFromMarkdown(schema, firstSerialize));
      expect(secondSerialize).toBe(firstSerialize);
    }
  });
});
