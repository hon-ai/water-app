import { describe, expect, it, beforeEach } from "vitest";
import { render, screen, fireEvent, act } from "@testing-library/react";
import { EditorState, TextSelection } from "prosemirror-state";
import { EditorView } from "prosemirror-view";
import { schema } from "./schema";
import { SelectionToolbar } from "./SelectionToolbar";

function mountEditorWithSelection(
  initialText: string,
  selStart: number,
  selEnd: number,
): { view: EditorView; host: HTMLElement } {
  const host = document.createElement("div");
  document.body.appendChild(host);
  const state = EditorState.create({
    doc: schema.node("doc", null, [
      schema.node("paragraph", { blockId: "^bk-0001" }, [schema.text(initialText)]),
    ]),
    schema,
  });
  const view = new EditorView(host, { state });
  const tr = view.state.tr.setSelection(
    TextSelection.create(view.state.doc, 1 + selStart, 1 + selEnd),
  );
  view.dispatch(tr);
  return { view, host };
}

beforeEach(() => {
  document.body.innerHTML = "";
});

describe("SelectionToolbar", () => {
  it("does not render when selection is collapsed", () => {
    const { view } = mountEditorWithSelection("hello world", 5, 5);
    render(<SelectionToolbar editorView={view} />);
    expect(screen.queryByLabelText("Bold")).toBeNull();
    expect(screen.queryByLabelText("Italic")).toBeNull();
    expect(screen.queryByLabelText("Link")).toBeNull();
  });

  it("renders three icon buttons when selection is non-empty", () => {
    const { view } = mountEditorWithSelection("hello world", 0, 5);
    render(<SelectionToolbar editorView={view} />);
    expect(screen.getByLabelText("Bold")).toBeInTheDocument();
    expect(screen.getByLabelText("Italic")).toBeInTheDocument();
    expect(screen.getByLabelText("Link")).toBeInTheDocument();
  });

  it("shows active state when the selection is fully bold", () => {
    const { view } = mountEditorWithSelection("hello world", 0, 5);
    const tr = view.state.tr.addMark(
      view.state.selection.from,
      view.state.selection.to,
      schema.marks.strong!.create(),
    );
    view.dispatch(tr);
    render(<SelectionToolbar editorView={view} />);
    const boldButton = screen.getByLabelText("Bold");
    expect(boldButton.getAttribute("data-active")).toBe("true");
  });

  it("hides toolbar on Escape keydown", () => {
    const { view } = mountEditorWithSelection("hello world", 0, 5);
    render(<SelectionToolbar editorView={view} />);
    expect(screen.queryByLabelText("Bold")).toBeInTheDocument();
    act(() => {
      fireEvent.keyDown(window, { key: "Escape" });
    });
    expect(screen.queryByLabelText("Bold")).toBeNull();
  });
});
