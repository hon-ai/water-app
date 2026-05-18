export type CursorClassification = "at_sentence_end" | "at_paragraph_end" | "mid_sentence";

const SENT_END_RE = /[.!?][")\]]?$/;

/** Pure string classifier. Used by the editor's transaction listener
 *  to emit typing telemetry. Block-kind nuance lives in the caller. */
export function classifyCursor(textBeforeCursor: string, cursorOffset: number): CursorClassification {
  const before = textBeforeCursor.slice(0, cursorOffset);
  const after = textBeforeCursor.slice(cursorOffset);
  if (after.startsWith("\n\n") || after === "\n") return "at_paragraph_end";

  const trimmed = before.replace(/[ \t]+$/, "");
  const atEol = after.length === 0 || after.startsWith("\n");
  if (atEol) return "at_sentence_end";

  if (SENT_END_RE.test(trimmed)) return "at_sentence_end";
  return "mid_sentence";
}
