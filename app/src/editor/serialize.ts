// Markdown <-> ProseMirror doc serializer.
//
// The on-disk format is plain markdown with optional `^bk-XXXX` anchors
// preceding the block text. This lets us round-trip block ids without
// requiring a custom binary format. Headings, dialogue (`>`), scene breaks
// (`---`), and lists are recognized; everything else falls back to
// paragraph. Trailing blank lines collapse to a single `\n` to keep diffs
// stable.

import type { Mark as PMMark, Node as PMNode, Schema } from "prosemirror-model";

const BID_RE = /^(\^bk-[A-Za-z0-9]{4})\s+/;
const BID_ONLY = /^\^bk-[A-Za-z0-9]{4}$/;

const MARK_OPEN: Record<string, (mark: PMMark) => string> = {
  strong: () => "**",
  em: () => "*",
  link: () => `[`, // open of [text](url); close is special-cased below
};

const MARK_CLOSE: Record<string, (mark: PMMark) => string> = {
  strong: () => "**",
  em: () => "*",
  link: (mark) => `](${mark.attrs.href as string})`,
};

// Marks emit in this nesting order (outermost → innermost).
// Link wraps because [foo](url) is atomic in CommonMark; nesting inside
// causes parsing pain. Strong wraps em because **bold *italic*** is the
// canonical nested-emphasis form.
const MARK_PRIORITY = ["link", "strong", "em"] as const;

function escapeLiterals(text: string): string {
  // Escape CommonMark inline metacharacters so literal `*`, `_`, `[`, `]`,
  // `(`, `)`, `\` in the source text round-trip safely. We're conservative:
  // every potential delimiter is escaped, even when context would parse it
  // literally; the cost is a slightly noisier on-disk format, the win is
  // bulletproof round-trip.
  return text.replace(/([\\*_[\]()])/g, "\\$1");
}

export function inlineToMarkdown(blockNode: PMNode): string {
  let out = "";
  const openStack: Array<{ name: string; mark: PMMark }> = [];

  function marksInPriorityOrder(marks: readonly PMMark[]) {
    const byName = new Map<string, PMMark>();
    for (const m of marks) byName.set(m.type.name, m);
    const ordered: Array<{ name: string; mark: PMMark }> = [];
    for (const name of MARK_PRIORITY) {
      const m = byName.get(name);
      if (m) ordered.push({ name, mark: m });
    }
    return ordered;
  }

  function closeUntil(targetSize: number) {
    while (openStack.length > targetSize) {
      const top = openStack.pop()!;
      out += MARK_CLOSE[top.name]!(top.mark);
    }
  }

  blockNode.forEach((child) => {
    if (!child.isText) return;
    const wanted = marksInPriorityOrder(child.marks);
    // Find longest common prefix of openStack and wanted.
    let common = 0;
    while (
      common < openStack.length &&
      common < wanted.length &&
      openStack[common]!.name === wanted[common]!.name &&
      // For link, also require same href; bold/em don't have attrs.
      (openStack[common]!.name !== "link" ||
        openStack[common]!.mark.attrs.href === wanted[common]!.mark.attrs.href)
    ) {
      common += 1;
    }
    closeUntil(common);
    for (let i = common; i < wanted.length; i++) {
      const entry = wanted[i]!;
      out += MARK_OPEN[entry.name]!(entry.mark);
      openStack.push(entry);
    }
    out += escapeLiterals(child.text ?? "");
  });
  closeUntil(0);
  return out;
}

export function markdownFromDoc(doc: PMNode): string {
  const lines: string[] = [];
  doc.forEach((node) => {
    const bid: string = node.attrs?.blockId ? `${node.attrs.blockId} ` : "";
    switch (node.type.name) {
      case "paragraph":
        lines.push(`${bid}${inlineToMarkdown(node)}`);
        lines.push("");
        break;
      case "heading":
        lines.push(`${"#".repeat(node.attrs.level)} ${bid}${inlineToMarkdown(node)}`);
        lines.push("");
        break;
      case "scene_break":
        lines.push(`${bid}---`);
        lines.push("");
        break;
      case "dialogue":
        lines.push(`${bid}> ${inlineToMarkdown(node)}`);
        lines.push("");
        break;
      case "ordered_list":
        node.forEach((item, _o, i) => {
          const itemBid: string = item.attrs?.blockId ?? "";
          lines.push(`${i + 1}. ${itemBid ? itemBid + " " : ""}${inlineToMarkdown(item)}`);
        });
        lines.push("");
        break;
      case "bullet_list":
        node.forEach((item) => {
          const itemBid: string = item.attrs?.blockId ?? "";
          lines.push(`- ${itemBid ? itemBid + " " : ""}${inlineToMarkdown(item)}`);
        });
        lines.push("");
        break;
      default:
        lines.push(inlineToMarkdown(node));
        lines.push("");
    }
  });
  return lines.join("\n").replace(/\n+$/, "\n");
}

export function docFromMarkdown(schema: Schema, md: string): PMNode {
  const lines = md.split(/\r?\n/);
  const nodes: PMNode[] = [];

  for (const raw of lines) {
    const trimmed = raw.trim();
    if (trimmed === "") continue;

    // Scene break: `---` or `^bk-XXXX ---`
    if (trimmed === "---") {
      nodes.push(schema.node("scene_break", { blockId: "" }));
      continue;
    }
    const bidDashMatch = trimmed.match(/^(\^bk-[A-Za-z0-9]{4})\s+---$/);
    if (bidDashMatch) {
      nodes.push(schema.node("scene_break", { blockId: bidDashMatch[1] ?? "" }));
      continue;
    }

    // Heading 2: `## [^bk-XXXX] text`
    const h2 = raw.match(/^##\s+(?:(\^bk-[A-Za-z0-9]{4})\s+)?(.*)$/);
    if (h2) {
      const text = h2[2] ?? "";
      nodes.push(
        schema.node(
          "heading",
          { level: 2, blockId: h2[1] ?? "" },
          markdownToInlineNodes(schema, text),
        ),
      );
      continue;
    }

    // Heading 3
    const h3 = raw.match(/^###\s+(?:(\^bk-[A-Za-z0-9]{4})\s+)?(.*)$/);
    if (h3) {
      const text = h3[2] ?? "";
      nodes.push(
        schema.node(
          "heading",
          { level: 3, blockId: h3[1] ?? "" },
          markdownToInlineNodes(schema, text),
        ),
      );
      continue;
    }

    // Dialogue: `> [^bk-XXXX] text`
    const dlg = raw.match(/^>\s+(?:(\^bk-[A-Za-z0-9]{4})\s+)?(.*)$/);
    if (dlg) {
      const text = dlg[2] ?? "";
      nodes.push(
        schema.node(
          "dialogue",
          { blockId: dlg[1] ?? "" },
          markdownToInlineNodes(schema, text),
        ),
      );
      continue;
    }

    // Paragraph: optional leading `^bk-XXXX ` then text. A line that is
    // only the bid (no body) collapses to an empty paragraph anchor.
    if (BID_ONLY.test(trimmed)) {
      nodes.push(schema.node("paragraph", { blockId: trimmed }));
      continue;
    }
    const m = raw.match(BID_RE);
    const text = m ? raw.slice(m[0].length) : raw;
    nodes.push(
      schema.node(
        "paragraph",
        { blockId: m?.[1] ?? "" },
        markdownToInlineNodes(schema, text),
      ),
    );
  }

  return schema.node(
    "doc",
    null,
    nodes.length === 0 ? [schema.node("paragraph", { blockId: "" })] : nodes,
  );
}

type InlineMark = { name: "strong" | "em" | "link"; href?: string };

type InlineRun = { text: string; marks: InlineMark[] };

function isFlankCharOutside(c: string | undefined): boolean {
  // Outside = before an opener or after a closer. Per CommonMark, this is
  // whitespace OR ASCII/Unicode punctuation. We include the emphasis
  // delimiters (`*`, `_`) and backslash so adjacent runs like `***text***`
  // (em opener immediately following strong opener) and `[**x**](u)` (em
  // opener immediately after a `[`) are recognized correctly.
  if (c === undefined) return true; // BOL/EOL
  return /[\s.,;:!?'"()\[\]{}*_~`\\—–-]/.test(c);
}

function isFlankCharInside(c: string | undefined): boolean {
  // Inside = just after an opener or just before a closer. Must NOT be whitespace.
  if (c === undefined) return false;
  return /\S/.test(c);
}

function tokenizeInline(line: string, seedMarks: InlineMark[]): InlineRun[] {
  // Walk left-to-right tracking active marks.
  // Recognized syntax (pragmatic CommonMark subset):
  //   **...**       open/close strong
  //   *...*         open/close em (with left/right-flanking rule)
  //   [text](url)   link span (text is recursively tokenized so marks
  //                 nested inside a link round-trip cleanly)
  //   \X            literal X for X ∈ { *, _, [, ], (, ), \ }
  // Anything else is literal text.
  if (line.length === 0) return [];

  const runs: InlineRun[] = [];
  const active: InlineMark[] = [...seedMarks];
  let buf = "";

  function flush() {
    if (buf.length === 0) return;
    runs.push({ text: buf, marks: [...active] });
    buf = "";
  }

  let i = 0;
  while (i < line.length) {
    const ch = line[i]!;
    const next = line[i + 1];
    const prev = i === 0 ? undefined : line[i - 1];

    // Escape sequence \X → literal X
    if (ch === "\\" && next !== undefined && /[\\*_[\]()]/.test(next)) {
      buf += next;
      i += 2;
      continue;
    }

    // Bold: **
    if (ch === "*" && next === "*") {
      const strongOpen = active.some((m) => m.name === "strong");
      const peekInside2 = line[i + 2];
      // Try to open strong
      if (!strongOpen && isFlankCharOutside(prev) && isFlankCharInside(peekInside2)) {
        flush();
        active.push({ name: "strong" });
        i += 2;
        continue;
      }
      // Try to close strong: prefer closing innermost mark.
      // If the innermost mark is em, prefer closing em with single * first
      // (handles **bold *italic*** where `***` = em-close + strong-close).
      if (strongOpen) {
        const innermost = active[active.length - 1];
        if (innermost && innermost.name === "em") {
          // Close em with single `*`. peek inside for that single-* close
          // is `next` (which is `*`, non-whitespace, so it qualifies).
          if (isFlankCharInside(prev) && isFlankCharInside(next)) {
            flush();
            active.pop();
            i += 1;
            continue;
          }
        }
        if (isFlankCharInside(prev) && isFlankCharOutside(peekInside2)) {
          flush();
          const idx = active.findIndex((m) => m.name === "strong");
          if (idx >= 0) active.splice(idx, 1);
          i += 2;
          continue;
        }
      }
      // Fall through to literal
    }

    // Italic: *
    if (ch === "*" && next !== "*") {
      const emOpen = active.some((m) => m.name === "em");
      const peekInside = next;
      if (!emOpen && isFlankCharOutside(prev) && isFlankCharInside(peekInside)) {
        flush();
        active.push({ name: "em" });
        i += 1;
        continue;
      }
      if (emOpen && isFlankCharInside(prev) && isFlankCharOutside(peekInside)) {
        flush();
        const idx = active.findIndex((m) => m.name === "em");
        if (idx >= 0) active.splice(idx, 1);
        i += 1;
        continue;
      }
      // Fall through to literal
    }

    // Link: [text](url) — text is recursively tokenized so inline marks
    // inside a link survive round-trip. Links themselves do not nest.
    if (ch === "[" && !active.some((m) => m.name === "link")) {
      const closeBracket = line.indexOf("]", i + 1);
      const openParen = closeBracket >= 0 ? line[closeBracket + 1] : undefined;
      if (closeBracket > i + 1 && openParen === "(") {
        const closeParen = line.indexOf(")", closeBracket + 2);
        if (closeParen > closeBracket + 1) {
          const linkText = line.slice(i + 1, closeBracket);
          const href = line.slice(closeBracket + 2, closeParen);
          if (linkText.length > 0 && href.length > 0) {
            flush();
            const inner = tokenizeInline(linkText, [
              ...active,
              { name: "link", href },
            ]);
            for (const r of inner) runs.push(r);
            i = closeParen + 1;
            continue;
          }
        }
      }
      // Fall through to literal
    }

    buf += ch;
    i += 1;
  }
  flush();

  return runs;
}

export function markdownToInlineNodes(
  schema: import("prosemirror-model").Schema,
  line: string,
): import("prosemirror-model").Node[] {
  if (line.length === 0) return [];
  const runs = tokenizeInline(line, []);

  return runs
    .filter((r) => r.text.length > 0)
    .map((r) => {
      const marks = r.marks.map((m) => {
        if (m.name === "link") return schema.marks.link!.create({ href: m.href ?? "" });
        if (m.name === "strong") return schema.marks.strong!.create();
        return schema.marks.em!.create();
      });
      return schema.text(r.text, marks);
    });
}
