// Custom node subclasses that carry a stable `^bk-XXXX` block ID and a
// `highlight` flag (used by the decoration exercise). Each block kind gets
// its own subclass so the mutation listener / node transform can target a
// single Klass and the DOM gets the right CSS classes for the demo.
//
// Block-ID strategy:
//   - The ID lives on the node instance (`__blockId`).
//   - The `static clone()` method is the ONLY safe place to carry the field
//     across update cycles — Lexical clones a node every time it's marked
//     dirty, so missing this is the canonical "IDs renumber under you" bug.
//   - `static importJSON` / `exportJSON` round-trip the field so undo/redo
//     and any future serialization preserves it.
//   - Splits: the original block stays (its clone keeps the ID); the *new*
//     block returned by `insertNewAfter` is constructed without an ID so
//     the BlockIdPlugin transform assigns a fresh one.
//   - Merges: Lexical's default `mergeWithSibling` keeps the receiving node
//     and removes the absorbed sibling, so the receiver's ID survives. We
//     do nothing special.

import {
  $applyNodeReplacement,
  ElementNode,
  ParagraphNode,
  type EditorConfig,
  type LexicalNode,
  type LexicalUpdateJSON,
  type NodeKey,
  type RangeSelection,
  type SerializedElementNode,
  type Spread,
} from "lexical";
import {
  HeadingNode,
  type SerializedHeadingNode,
} from "@lexical/rich-text";
import {
  ListItemNode,
  type SerializedListItemNode,
} from "@lexical/list";

// ---------------------------------------------------------------------------
// Block-ID generation
// ---------------------------------------------------------------------------

/** RFC-flavored short ID used in the harness. Real product uses ULIDs/etc. */
export function generateBlockId(): string {
  // 4 hex chars (~65k space) is plenty for the harness's worst-case 50k blocks.
  const n = Math.floor(Math.random() * 0xffff);
  return `^bk-${n.toString(16).padStart(4, "0").toUpperCase()}`;
}

// ---------------------------------------------------------------------------
// Serialized shapes — each spreads the parent's serialized type so we don't
// accidentally drop fields like `textFormat`.
// ---------------------------------------------------------------------------

type WithBlockId = { blockId: string | null; highlight: boolean };
type SerializedBkParagraph = Spread<
  WithBlockId,
  // Parent ParagraphNode declares its own SerializedParagraphNode but it isn't
  // exported in a way TS sees here uniformly across versions; use ReturnType.
  ReturnType<ParagraphNode["exportJSON"]>
>;
type SerializedBkHeading = Spread<WithBlockId, SerializedHeadingNode>;
type SerializedBkListItem = Spread<WithBlockId, SerializedListItemNode>;
type SerializedDialogue = Spread<WithBlockId, SerializedElementNode>;
type SerializedSceneBreak = Spread<WithBlockId, SerializedElementNode>;

// ---------------------------------------------------------------------------
// BkParagraphNode — paragraph with block-id + highlight slot
// ---------------------------------------------------------------------------

export class BkParagraphNode extends ParagraphNode {
  __blockId: string | null;
  __highlight: boolean;

  constructor(blockId: string | null = null, highlight = false, key?: NodeKey) {
    super(key);
    this.__blockId = blockId;
    this.__highlight = highlight;
  }

  static override getType(): string {
    return "bk-paragraph";
  }

  static override clone(node: BkParagraphNode): BkParagraphNode {
    return new BkParagraphNode(node.__blockId, node.__highlight, node.__key);
  }

  override createDOM(config: EditorConfig): HTMLElement {
    const dom = super.createDOM(config);
    applyBlockChrome(dom, this.__blockId, this.__highlight, "paragraph");
    return dom;
  }

  override updateDOM(prev: ParagraphNode, dom: HTMLElement, config: EditorConfig): boolean {
    const reconciled = super.updateDOM(prev, dom, config);
    const prevBk = prev as BkParagraphNode;
    if (
      prevBk.__blockId !== this.__blockId ||
      prevBk.__highlight !== this.__highlight
    ) {
      applyBlockChrome(dom, this.__blockId, this.__highlight, "paragraph");
    }
    return reconciled;
  }

  static override importJSON(serialized: SerializedBkParagraph): BkParagraphNode {
    const node = $createBkParagraphNode(serialized.blockId, serialized.highlight);
    return node.updateFromJSON(serialized);
  }

  override exportJSON(): SerializedBkParagraph {
    return {
      ...super.exportJSON(),
      blockId: this.__blockId,
      highlight: this.__highlight,
    };
  }

  /**
   * When the user hits Enter, Lexical splits the current block by calling
   * `insertNewAfter`. We return a fresh BkParagraphNode with a NULL blockId —
   * the BlockIdPlugin's node transform will pick it up on the next pass and
   * assign a new one. The original block keeps its ID via `clone`.
   */
  override insertNewAfter(_selection: RangeSelection, restoreSelection?: boolean): ParagraphNode {
    const next = $createBkParagraphNode();
    const direction = this.getDirection();
    next.setDirection(direction);
    this.insertAfter(next, restoreSelection ?? true);
    return next;
  }

  // Setters used by the BlockIdPlugin transform.
  setBlockId(id: string | null): this {
    const w = this.getWritable();
    w.__blockId = id;
    return w;
  }
  getBlockId(): string | null {
    return this.getLatest().__blockId;
  }
  setHighlight(v: boolean): this {
    const w = this.getWritable();
    w.__highlight = v;
    return w;
  }
}

export function $createBkParagraphNode(
  blockId: string | null = null,
  highlight = false,
): BkParagraphNode {
  return $applyNodeReplacement(new BkParagraphNode(blockId, highlight));
}
export function $isBkParagraphNode(n: LexicalNode | null | undefined): n is BkParagraphNode {
  return n instanceof BkParagraphNode;
}

// ---------------------------------------------------------------------------
// BkHeadingNode — heading_2 and heading_3
// ---------------------------------------------------------------------------

export class BkHeadingNode extends HeadingNode {
  __blockId: string | null;
  __highlight: boolean;

  constructor(tag: "h2" | "h3", blockId: string | null = null, highlight = false, key?: NodeKey) {
    super(tag);
    if (key !== undefined) this.__key = key;
    this.__blockId = blockId;
    this.__highlight = highlight;
  }

  static override getType(): string {
    return "bk-heading";
  }

  static override clone(node: BkHeadingNode): BkHeadingNode {
    return new BkHeadingNode(
      node.getTag() as "h2" | "h3",
      node.__blockId,
      node.__highlight,
      node.__key,
    );
  }

  override createDOM(config: EditorConfig): HTMLElement {
    const dom = super.createDOM(config);
    applyBlockChrome(dom, this.__blockId, this.__highlight, this.getTag());
    return dom;
  }

  static override importJSON(serialized: SerializedBkHeading): BkHeadingNode {
    const node = $createBkHeadingNode(
      serialized.tag as "h2" | "h3",
      serialized.blockId,
      serialized.highlight,
    );
    return node.updateFromJSON(serialized as LexicalUpdateJSON<SerializedHeadingNode>);
  }

  override exportJSON(): SerializedBkHeading {
    return {
      ...super.exportJSON(),
      blockId: this.__blockId,
      highlight: this.__highlight,
    };
  }

  setBlockId(id: string | null): this {
    const w = this.getWritable();
    w.__blockId = id;
    return w;
  }
  getBlockId(): string | null {
    return this.getLatest().__blockId;
  }
  setHighlight(v: boolean): this {
    const w = this.getWritable();
    w.__highlight = v;
    return w;
  }
}

export function $createBkHeadingNode(
  tag: "h2" | "h3",
  blockId: string | null = null,
  highlight = false,
): BkHeadingNode {
  return $applyNodeReplacement(new BkHeadingNode(tag, blockId, highlight));
}
export function $isBkHeadingNode(n: LexicalNode | null | undefined): n is BkHeadingNode {
  return n instanceof BkHeadingNode;
}

// ---------------------------------------------------------------------------
// BkListItemNode — list items each carry their own block-id
// ---------------------------------------------------------------------------

export class BkListItemNode extends ListItemNode {
  __blockId: string | null;
  __highlight: boolean;

  constructor(blockId: string | null = null, highlight = false, key?: NodeKey) {
    super();
    if (key !== undefined) this.__key = key;
    this.__blockId = blockId;
    this.__highlight = highlight;
  }

  static override getType(): string {
    return "bk-listitem";
  }

  static override clone(node: BkListItemNode): BkListItemNode {
    return new BkListItemNode(node.__blockId, node.__highlight, node.__key);
  }

  override createDOM(config: EditorConfig): HTMLElement {
    const dom = super.createDOM(config);
    applyBlockChrome(dom, this.__blockId, this.__highlight, "listitem");
    return dom;
  }

  override updateDOM(prev: ListItemNode, dom: HTMLElement, config: EditorConfig): boolean {
    const r = super.updateDOM(prev, dom, config);
    applyBlockChrome(dom, this.__blockId, this.__highlight, "listitem");
    return r;
  }

  static override importJSON(serialized: SerializedBkListItem): BkListItemNode {
    const node = $createBkListItemNode(serialized.blockId, serialized.highlight);
    return node.updateFromJSON(serialized as LexicalUpdateJSON<SerializedListItemNode>);
  }

  override exportJSON(): SerializedBkListItem {
    return {
      ...super.exportJSON(),
      blockId: this.__blockId,
      highlight: this.__highlight,
    };
  }

  setBlockId(id: string | null): this {
    const w = this.getWritable();
    w.__blockId = id;
    return w;
  }
  getBlockId(): string | null {
    return this.getLatest().__blockId;
  }
  setHighlight(v: boolean): this {
    const w = this.getWritable();
    w.__highlight = v;
    return w;
  }
}

export function $createBkListItemNode(
  blockId: string | null = null,
  highlight = false,
): BkListItemNode {
  return $applyNodeReplacement(new BkListItemNode(blockId, highlight));
}
export function $isBkListItemNode(n: LexicalNode | null | undefined): n is BkListItemNode {
  return n instanceof BkListItemNode;
}

// ---------------------------------------------------------------------------
// DialogueNode — paragraph variant rendered with distinct styling.
// ---------------------------------------------------------------------------

export class DialogueNode extends ElementNode {
  __blockId: string | null;
  __highlight: boolean;

  constructor(blockId: string | null = null, highlight = false, key?: NodeKey) {
    super(key);
    this.__blockId = blockId;
    this.__highlight = highlight;
  }

  static override getType(): string {
    return "bk-dialogue";
  }
  static override clone(n: DialogueNode): DialogueNode {
    return new DialogueNode(n.__blockId, n.__highlight, n.__key);
  }

  override createDOM(_config: EditorConfig): HTMLElement {
    const dom = document.createElement("p");
    dom.classList.add("bk-dialogue");
    applyBlockChrome(dom, this.__blockId, this.__highlight, "dialogue");
    return dom;
  }
  override updateDOM(prev: ElementNode, dom: HTMLElement): boolean {
    const prevBk = prev as DialogueNode;
    if (prevBk.__blockId !== this.__blockId || prevBk.__highlight !== this.__highlight) {
      applyBlockChrome(dom, this.__blockId, this.__highlight, "dialogue");
    }
    return false;
  }
  override insertNewAfter(_sel: RangeSelection, restoreSelection?: boolean): ParagraphNode {
    // Pressing enter inside dialogue returns to a regular paragraph.
    const next = $createBkParagraphNode();
    this.insertAfter(next, restoreSelection ?? true);
    return next;
  }
  override collapseAtStart(): boolean {
    const p = $createBkParagraphNode(this.__blockId, this.__highlight);
    const children = this.getChildren();
    for (const c of children) p.append(c);
    this.replace(p);
    return true;
  }

  static override importJSON(serialized: SerializedDialogue): DialogueNode {
    const node = $createDialogueNode(serialized.blockId, serialized.highlight);
    return node.updateFromJSON(serialized as LexicalUpdateJSON<SerializedElementNode>);
  }

  override exportJSON(): SerializedDialogue {
    return {
      ...super.exportJSON(),
      blockId: this.__blockId,
      highlight: this.__highlight,
      type: "bk-dialogue",
      version: 1,
    };
  }

  setBlockId(id: string | null): this {
    const w = this.getWritable();
    w.__blockId = id;
    return w;
  }
  getBlockId(): string | null {
    return this.getLatest().__blockId;
  }
  setHighlight(v: boolean): this {
    const w = this.getWritable();
    w.__highlight = v;
    return w;
  }
}

export function $createDialogueNode(blockId: string | null = null, highlight = false): DialogueNode {
  return $applyNodeReplacement(new DialogueNode(blockId, highlight));
}
export function $isDialogueNode(n: LexicalNode | null | undefined): n is DialogueNode {
  return n instanceof DialogueNode;
}

// ---------------------------------------------------------------------------
// SceneBreakNode — non-editable horizontal rule with a stable id.
// Uses an ElementNode rather than DecoratorNode so it slots into the document
// as a block. We mark it as not directly editable.
// ---------------------------------------------------------------------------

export class SceneBreakNode extends ElementNode {
  __blockId: string | null;
  __highlight: boolean;

  constructor(blockId: string | null = null, highlight = false, key?: NodeKey) {
    super(key);
    this.__blockId = blockId;
    this.__highlight = highlight;
  }

  static override getType(): string {
    return "bk-scene-break";
  }
  static override clone(n: SceneBreakNode): SceneBreakNode {
    return new SceneBreakNode(n.__blockId, n.__highlight, n.__key);
  }

  override createDOM(_config: EditorConfig): HTMLElement {
    const dom = document.createElement("div");
    dom.classList.add("bk-scene-break");
    dom.setAttribute("contenteditable", "false");
    dom.innerHTML = '<hr aria-hidden="true" />';
    applyBlockChrome(dom, this.__blockId, this.__highlight, "scene-break");
    return dom;
  }
  override updateDOM(prev: ElementNode, dom: HTMLElement): boolean {
    const prevBk = prev as SceneBreakNode;
    if (prevBk.__blockId !== this.__blockId || prevBk.__highlight !== this.__highlight) {
      applyBlockChrome(dom, this.__blockId, this.__highlight, "scene-break");
    }
    return false;
  }
  override isInline(): false {
    return false;
  }
  override canBeEmpty(): false {
    return false;
  }
  override isShadowRoot(): false {
    return false;
  }

  static override importJSON(serialized: SerializedSceneBreak): SceneBreakNode {
    const node = $createSceneBreakNode(serialized.blockId, serialized.highlight);
    return node.updateFromJSON(serialized as LexicalUpdateJSON<SerializedElementNode>);
  }
  override exportJSON(): SerializedSceneBreak {
    return {
      ...super.exportJSON(),
      blockId: this.__blockId,
      highlight: this.__highlight,
      type: "bk-scene-break",
      version: 1,
    };
  }
  setBlockId(id: string | null): this {
    const w = this.getWritable();
    w.__blockId = id;
    return w;
  }
  getBlockId(): string | null {
    return this.getLatest().__blockId;
  }
  setHighlight(v: boolean): this {
    const w = this.getWritable();
    w.__highlight = v;
    return w;
  }
}

export function $createSceneBreakNode(
  blockId: string | null = null,
  highlight = false,
): SceneBreakNode {
  return $applyNodeReplacement(new SceneBreakNode(blockId, highlight));
}
export function $isSceneBreakNode(n: LexicalNode | null | undefined): n is SceneBreakNode {
  return n instanceof SceneBreakNode;
}

// ---------------------------------------------------------------------------
// Common DOM chrome application — the badge that displays the `^bk-XXXX`
// label and the `.bk-highlight` class that triggers the demo decoration.
// ---------------------------------------------------------------------------

function applyBlockChrome(
  dom: HTMLElement,
  blockId: string | null,
  highlight: boolean,
  kind: string,
): void {
  dom.classList.add("bk-block");
  dom.classList.add(`bk-kind-${kind}`);
  if (blockId) {
    dom.setAttribute("data-block-id", blockId);
  } else {
    dom.removeAttribute("data-block-id");
  }
  if (highlight) {
    dom.classList.add("bk-highlight");
  } else {
    dom.classList.remove("bk-highlight");
  }
}

// ---------------------------------------------------------------------------
// Type guards collection used by the BlockIdPlugin and the harness toolbar.
// ---------------------------------------------------------------------------

export type BlockNode =
  | BkParagraphNode
  | BkHeadingNode
  | BkListItemNode
  | DialogueNode
  | SceneBreakNode;

export function $isBlockNode(n: LexicalNode | null | undefined): n is BlockNode {
  return (
    $isBkParagraphNode(n) ||
    $isBkHeadingNode(n) ||
    $isBkListItemNode(n) ||
    $isDialogueNode(n) ||
    $isSceneBreakNode(n)
  );
}

/**
 * The full set of node classes the editor must register. Includes
 * `LexicalNodeReplacement` entries so that built-in `$createParagraphNode`
 * calls (inside selection / list / history machinery) produce our subclass
 * instead of plain ParagraphNode.
 */
export function bakeoffNodes() {
  return [
    BkParagraphNode,
    BkHeadingNode,
    BkListItemNode,
    DialogueNode,
    SceneBreakNode,
    {
      replace: ParagraphNode,
      with: (_n: ParagraphNode) => $createBkParagraphNode(),
      withKlass: BkParagraphNode,
    },
    {
      replace: HeadingNode,
      with: (n: HeadingNode) => $createBkHeadingNode(n.getTag() as "h2" | "h3"),
      withKlass: BkHeadingNode,
    },
    {
      replace: ListItemNode,
      with: (_n: ListItemNode) => $createBkListItemNode(),
      withKlass: BkListItemNode,
    },
  ];
}
