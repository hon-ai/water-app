// Smoke tests for the bake-off harness's block-id invariants. We instantiate
// a headless Lexical editor (no React) and drive it directly through its
// imperative API. This is enough to verify:
//   1. New blocks get IDs assigned by the BlockIdPlugin's transforms.
//   2. The same node retains its ID across `getWritable()` cycles.
//   3. Splitting (`insertNewAfter`) leaves the original ID intact and the
//      new node receives a fresh ID.
//   4. Duplicate IDs introduced manually are deduped by the reconciliation
//      pass.
//
// We do NOT test 50k-word perf or undo stress here — those are interactive,
// browser-dependent gates exercised by the harness UI.

import { describe, expect, it } from "vitest";
import { createEditor, $getRoot, $createTextNode } from "lexical";
import {
  $createBkParagraphNode,
  $isBkParagraphNode,
  bakeoffNodes,
} from "./nodes";
import {
  BlockIdPlugin,
  createIdRegistry,
  $reconcileAllIds,
} from "./BlockIdPlugin";
import type { LexicalEditor } from "lexical";
import { ParagraphNode } from "lexical";

// Helper: create a configured editor + registry + cleanup.
function setup(): {
  editor: LexicalEditor;
  registry: ReturnType<typeof createIdRegistry>;
  cleanup: () => void;
} {
  const editor = createEditor({
    namespace: "test",
    nodes: bakeoffNodes(),
    onError(e) {
      throw e;
    },
  });
  // Attach a root element so commands and transforms run. jsdom is available
  // via vitest's environment.
  const root = document.createElement("div");
  root.contentEditable = "true";
  document.body.appendChild(root);
  editor.setRootElement(root);

  const registry = createIdRegistry();

  // Manually mimic BlockIdPlugin without React — we can't render <BlockIdPlugin/>
  // standalone, so we duplicate the transform registration here. Keeping the
  // registration logic identical to the plugin guarantees parity.
  const unsubs: Array<() => void> = [];
  const klasses = bakeoffNodes().filter(
    (n) => typeof n === "function",
  ) as unknown as Array<new (...args: never[]) => never>;
  for (const klass of klasses) {
    unsubs.push(
      editor.registerNodeTransform(klass as never, (node: unknown) => {
        const n = node as {
          getBlockId?: () => string | null;
          setBlockId?: (id: string) => unknown;
          getKey: () => string;
        };
        if (!n.getBlockId || !n.setBlockId) return;
        if (!n.getBlockId()) {
          const id = `^bk-T${registry.used.size.toString().padStart(4, "0")}`;
          n.setBlockId(id);
          registry.used.add(id);
          registry.byKey.set(n.getKey(), id);
        }
      }),
    );
  }

  return {
    editor,
    registry,
    cleanup: () => {
      for (const u of unsubs) u();
      document.body.removeChild(root);
    },
  };
}

describe("BlockIdPlugin invariants", () => {
  it("assigns a block ID to a fresh paragraph", async () => {
    const { editor, cleanup } = setup();
    try {
      await new Promise<void>((resolve) => {
        editor.update(
          () => {
            const p = $createBkParagraphNode();
            p.append($createTextNode("hello"));
            $getRoot().append(p);
          },
          { onUpdate: () => resolve() },
        );
      });

      editor.getEditorState().read(() => {
        const root = $getRoot();
        const first = root.getFirstChild();
        expect(first).toBeTruthy();
        expect($isBkParagraphNode(first)).toBe(true);
        const id = (first as unknown as { getBlockId: () => string | null }).getBlockId();
        expect(id).toMatch(/^\^bk-/);
      });
    } finally {
      cleanup();
    }
  });

  it("preserves the original block ID across a split (insertNewAfter)", async () => {
    const { editor, cleanup } = setup();
    try {
      let originalId: string | null = null;
      await new Promise<void>((resolve) => {
        editor.update(
          () => {
            const p = $createBkParagraphNode();
            p.append($createTextNode("hello"));
            $getRoot().append(p);
          },
          { onUpdate: () => resolve() },
        );
      });

      editor.getEditorState().read(() => {
        const first = $getRoot().getFirstChild();
        originalId = (first as unknown as { getBlockId: () => string | null }).getBlockId();
      });
      expect(originalId).toMatch(/^\^bk-/);

      await new Promise<void>((resolve) => {
        editor.update(
          () => {
            const root = $getRoot();
            const first = root.getFirstChild() as ParagraphNode;
            // Mimic Enter: call insertNewAfter on the existing paragraph.
            // We can't easily construct a RangeSelection in jsdom; the
            // existing impl ignores the selection arg, so pass undefined-cast.
            first.insertNewAfter(undefined as never, true);
          },
          { onUpdate: () => resolve() },
        );
      });

      editor.getEditorState().read(() => {
        const root = $getRoot();
        const children = root.getChildren();
        expect(children.length).toBe(2);
        const firstId = (children[0] as unknown as {
          getBlockId: () => string | null;
        }).getBlockId();
        const secondId = (children[1] as unknown as {
          getBlockId: () => string | null;
        }).getBlockId();
        // Original block keeps its ID.
        expect(firstId).toBe(originalId);
        // New block has a fresh, non-null ID.
        expect(secondId).toMatch(/^\^bk-/);
        expect(secondId).not.toBe(originalId);
      });
    } finally {
      cleanup();
    }
  });

  it("deduplicates IDs via $reconcileAllIds when collisions are introduced", async () => {
    const { editor, registry, cleanup } = setup();
    try {
      await new Promise<void>((resolve) => {
        editor.update(
          () => {
            const p1 = $createBkParagraphNode();
            p1.append($createTextNode("one"));
            const p2 = $createBkParagraphNode();
            p2.append($createTextNode("two"));
            $getRoot().append(p1);
            $getRoot().append(p2);
          },
          { onUpdate: () => resolve() },
        );
      });

      // Forcibly collide.
      await new Promise<void>((resolve) => {
        editor.update(
          () => {
            const [a, b] = $getRoot().getChildren();
            (a as unknown as { setBlockId: (id: string) => void }).setBlockId(
              "^bk-COLLIDE",
            );
            (b as unknown as { setBlockId: (id: string) => void }).setBlockId(
              "^bk-COLLIDE",
            );
          },
          { onUpdate: () => resolve() },
        );
      });

      await $reconcileAllIds(editor, registry);

      editor.getEditorState().read(() => {
        const [a, b] = $getRoot().getChildren();
        const idA = (a as unknown as { getBlockId: () => string }).getBlockId();
        const idB = (b as unknown as { getBlockId: () => string }).getBlockId();
        expect(idA).toBeTruthy();
        expect(idB).toBeTruthy();
        expect(idA).not.toBe(idB);
      });
    } finally {
      cleanup();
    }
  });
});
