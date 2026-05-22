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
  strike: () => "~~",
  link: () => `[`, // open of [text](url); close is special-cased below
  // Wikilink: open as `[[`; if `target` differs from the text content
  // the close-side appends `|text]]` instead of the bare `]]` — that
  // logic lives in the per-child serializer in inlineToMarkdown
  // because it needs the text we're about to emit, not just the mark.
  wikilink: () => `[[`,
};

const MARK_CLOSE: Record<string, (mark: PMMark) => string> = {
  strong: () => "**",
  em: () => "*",
  strike: () => "~~",
  link: (mark) => `](${mark.attrs.href as string})`,
  // Closing for wikilink is handled inline in inlineToMarkdown
  // (it depends on whether target equals the visible text). This
  // entry is a safe fallback for the close-stack drain path.
  wikilink: () => `]]`,
};

// Marks emit in this nesting order (outermost → innermost).
// Link wraps because [foo](url) is atomic in CommonMark; nesting inside
// causes parsing pain. Strike wraps strong wraps em so the canonical
// rendering is `~~**bold *italic***~~` when all three combine.
// Wikilink behaves like link — atomic, no nesting of marks inside it
// for round-trip safety.
const MARK_PRIORITY = ["wikilink", "link", "strike", "strong", "em"] as const;

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
    // Wikilink fast path: atomic emit, bypassing the generic mark
    // stack. A wikilink span is one text run with the mark applied;
    // we emit `[[target]]` when target == text, otherwise
    // `[[target|alias]]`. Other marks combine fine on top in CSS
    // but for round-trip we deliberately don't nest formatting
    // marks inside wikilinks (Obsidian's parser doesn't support
    // that and the resulting source would be brittle).
    const wikilinkMark = child.marks.find((m) => m.type.name === "wikilink");
    if (wikilinkMark) {
      closeUntil(0);
      const target = (wikilinkMark.attrs["target"] as string) ?? "";
      const text = child.text ?? "";
      if (target === text || target === "") {
        out += `[[${text}]]`;
      } else {
        out += `[[${target}|${text}]]`;
      }
      return;
    }
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

type InlineMark = {
  name: "strong" | "em" | "strike" | "link" | "wikilink";
  href?: string;
  /** Wikilink target — set when `name === "wikilink"`. */
  target?: string;
};

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

    // Strikethrough: ~~
    if (ch === "~" && next === "~") {
      const strikeOpen = active.some((m) => m.name === "strike");
      const peekInside2 = line[i + 2];
      if (!strikeOpen && isFlankCharOutside(prev) && isFlankCharInside(peekInside2)) {
        flush();
        active.push({ name: "strike" });
        i += 2;
        continue;
      }
      if (strikeOpen && isFlankCharInside(prev) && isFlankCharOutside(peekInside2)) {
        flush();
        const idx = active.findIndex((m) => m.name === "strike");
        if (idx >= 0) active.splice(idx, 1);
        i += 2;
        continue;
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

    // Wikilink: [[target]] or [[target|alias]]. Recognized before
    // the regular link parser because `[[` would otherwise be eaten
    // by `[`. Wikilinks are atomic and never contain other marks,
    // so the inner text is emitted as a single run.
    if (
      ch === "[" &&
      next === "[" &&
      !active.some((m) => m.name === "wikilink" || m.name === "link")
    ) {
      const closeIdx = line.indexOf("]]", i + 2);
      if (closeIdx > i + 2) {
        const inside = line.slice(i + 2, closeIdx);
        // Reject if the inside contains another `[[` — that would
        // be malformed; treat the outer `[` as literal so we don't
        // swallow real content.
        if (!inside.includes("[[")) {
          const pipe = inside.indexOf("|");
          const target = pipe >= 0 ? inside.slice(0, pipe) : inside;
          const alias = pipe >= 0 ? inside.slice(pipe + 1) : inside;
          flush();
          runs.push({
            text: alias,
            marks: [...active, { name: "wikilink", target }],
          });
          i = closeIdx + 2;
          continue;
        }
      }
      // Fall through to literal — single `[` consumed below.
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
        if (m.name === "wikilink")
          return schema.marks.wikilink!.create({ target: m.target ?? "" });
        if (m.name === "strong") return schema.marks.strong!.create();
        if (m.name === "strike") return schema.marks.strike!.create();
        return schema.marks.em!.create();
      });
      return schema.text(r.text, marks);
    });
}
