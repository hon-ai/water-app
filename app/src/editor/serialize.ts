// Markdown <-> ProseMirror doc serializer.
//
// The on-disk format is plain markdown with optional `^bk-XXXX` anchors
// preceding the block text. This lets us round-trip block ids without
// requiring a custom binary format. Headings, dialogue (`>`), scene breaks
// (`---`), and lists are recognized; everything else falls back to
// paragraph. Trailing blank lines collapse to a single `\n` to keep diffs
// stable.

import type { Node as PMNode, Schema } from "prosemirror-model";

const BID_RE = /^(\^bk-[A-Za-z0-9]{4})\s+/;
const BID_ONLY = /^\^bk-[A-Za-z0-9]{4}$/;

export function markdownFromDoc(doc: PMNode): string {
  const lines: string[] = [];
  doc.forEach((node) => {
    const bid: string = node.attrs?.blockId ? `${node.attrs.blockId} ` : "";
    switch (node.type.name) {
      case "paragraph":
        lines.push(`${bid}${node.textContent}`);
        lines.push("");
        break;
      case "heading":
        lines.push(`${"#".repeat(node.attrs.level)} ${bid}${node.textContent}`);
        lines.push("");
        break;
      case "scene_break":
        lines.push(`${bid}---`);
        lines.push("");
        break;
      case "dialogue":
        lines.push(`${bid}> ${node.textContent}`);
        lines.push("");
        break;
      case "ordered_list":
        node.forEach((item, _o, i) => {
          const itemBid: string = item.attrs?.blockId ?? "";
          lines.push(`${i + 1}. ${itemBid ? itemBid + " " : ""}${item.textContent}`);
        });
        lines.push("");
        break;
      case "bullet_list":
        node.forEach((item) => {
          const itemBid: string = item.attrs?.blockId ?? "";
          lines.push(`- ${itemBid ? itemBid + " " : ""}${item.textContent}`);
        });
        lines.push("");
        break;
      default:
        lines.push(node.textContent);
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
          text ? [schema.text(text)] : [],
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
          text ? [schema.text(text)] : [],
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
          text ? [schema.text(text)] : [],
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
        text ? [schema.text(text)] : [],
      ),
    );
  }

  return schema.node(
    "doc",
    null,
    nodes.length === 0 ? [schema.node("paragraph", { blockId: "" })] : nodes,
  );
}
