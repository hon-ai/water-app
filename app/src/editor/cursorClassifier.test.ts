import { describe, expect, it } from "vitest";
import { classifyCursor } from "./cursorClassifier";

describe("classifyCursor", () => {
  it("EOL is at_sentence_end", () => {
    expect(classifyCursor("hello world", 11)).toBe("at_sentence_end");
  });
  it("terminal period mid-line is at_sentence_end", () => {
    expect(classifyCursor("hello. more text", 6)).toBe("at_sentence_end");
  });
  it("comma mid-line is mid_sentence", () => {
    expect(classifyCursor("hello, more text", 6)).toBe("mid_sentence");
  });
  it("dialogue closing quote-period at EOL is at_sentence_end", () => {
    expect(classifyCursor("\"I love you,\" she said.", 23)).toBe("at_sentence_end");
  });
  it("dialogue comma-quote mid-line is mid_sentence", () => {
    expect(classifyCursor("\"I love you,\" she said,", 13)).toBe("mid_sentence");
  });
  it("question mark closing quote at EOL is at_sentence_end", () => {
    expect(classifyCursor("\"Why?\"", 6)).toBe("at_sentence_end");
  });
  it("list item with no period at EOL is at_sentence_end", () => {
    expect(classifyCursor("Buy milk", 8)).toBe("at_sentence_end");
  });
  it("paragraph-end detection (\\n\\n following)", () => {
    expect(classifyCursor("paragraph.\n\nnext", 10)).toBe("at_paragraph_end");
  });
});
