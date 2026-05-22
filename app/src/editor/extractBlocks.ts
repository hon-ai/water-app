/**
 * Walk the current editor's DOM and return `(blockId, text)` pairs
 * for every top-level block that carries a `data-bid` attribute.
 *
 * The diagnostic engine needs per-block input — it runs each rule
 * against a single block of prose. The PM schema's `toDOM` writes
 * the `blockId` attr as `data-bid`, so reading from the DOM is the
 * cheapest way to get the up-to-date set without serializing the
 * markdown or walking the PM doc directly. Shares the same pattern
 * as `PillLayer.snapshotEditorBlocks`.
 */
export function extractEditorBlocks(): Array<{
  blockId: string;
  text: string;
}> {
  const out: Array<{ blockId: string; text: string }> = [];
  document.querySelectorAll<HTMLElement>(".water-editor [data-bid]").forEach((el) => {
    const id = el.getAttribute("data-bid") ?? "";
    if (!id) return;
    const text = el.textContent ?? "";
    out.push({ blockId: id, text });
  });
  return out;
}
