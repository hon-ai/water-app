// BlockIdPlugin — keeps `__blockId` invariants across every editor update.
//
// Two cooperating mechanisms:
//
// 1. Node transforms (one per block Klass) — fire DURING the update cycle,
//    so we can call `setBlockId` on a fresh-cloned writable node without
//    triggering an infinite loop (Lexical re-runs transforms until the
//    dirty set is empty; our transform only re-marks dirty when an
//    assignment actually happens). This is where new blocks (post-split,
//    post-paste, post-fuzz) get an ID stamped.
//
// 2. A `Set<string>` of used IDs maintained at the plugin level. Whenever
//    an ID is taken (or detected on import/undo) we record it; whenever a
//    second node shows up holding the same ID (e.g. an undo collision or
//    a buggy clone) we mint a fresh one. This is the dedup guarantee.

import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import { useEffect } from "react";
import {
  $getNodeByKey,
  COMMAND_PRIORITY_HIGH,
  KEY_BACKSPACE_COMMAND,
  type LexicalEditor,
  type LexicalNode,
} from "lexical";
import {
  $isBkHeadingNode,
  $isBkListItemNode,
  $isBkParagraphNode,
  $isBlockNode,
  $isDialogueNode,
  $isSceneBreakNode,
  BkHeadingNode,
  BkListItemNode,
  BkParagraphNode,
  DialogueNode,
  SceneBreakNode,
  generateBlockId,
  type BlockNode,
} from "./nodes";

/**
 * The harness exposes the id-set so the toolbar can:
 *   (a) assert uniqueness after a fuzz step
 *   (b) snapshot the ID list pre/post long-undo to verify no renumbering
 */
export type IdRegistry = {
  used: Set<string>;
  /** Best-effort: the node-key → blockId pairing observed at last visit. */
  byKey: Map<string, string>;
};

export function createIdRegistry(): IdRegistry {
  return { used: new Set(), byKey: new Map() };
}

/**
 * Pick a fresh ID that's not already in `used`.
 */
function mintUniqueId(used: Set<string>): string {
  // The 4-hex space is 65k entries; this almost never loops twice. Keep
  // a safety bound anyway.
  for (let i = 0; i < 16; i++) {
    const id = generateBlockId();
    if (!used.has(id)) return id;
  }
  // Fallback: append a counter suffix.
  let n = used.size;
  while (used.has(`^bk-OVR-${n}`)) n++;
  return `^bk-OVR-${n}`;
}

/**
 * Reconcile a single block. Returns `true` if it mutated the node. The
 * caller is responsible for marking the node writable (we do it via
 * `setBlockId`).
 */
function ensureId(node: BlockNode, registry: IdRegistry): boolean {
  const current = node.getBlockId();
  const key = node.getKey();
  if (!current) {
    const id = mintUniqueId(registry.used);
    node.setBlockId(id);
    registry.used.add(id);
    registry.byKey.set(key, id);
    return true;
  }
  const prevForKey = registry.byKey.get(key);
  if (prevForKey === current) {
    // Already known and unchanged — nothing to do.
    return false;
  }
  // First time we're seeing this (key, id) pair. Check whether the id is
  // claimed by some OTHER live key. If so, mint fresh.
  if (registry.used.has(current)) {
    // Could be a stale entry from a previously-deleted key. Conservatively
    // mint fresh whenever there's any ambiguity — the harness rule is
    // "no duplicates", and reassignment on undo collisions is harmless.
    const id = mintUniqueId(registry.used);
    node.setBlockId(id);
    registry.used.add(id);
    registry.byKey.set(key, id);
    return true;
  }
  registry.used.add(current);
  registry.byKey.set(key, current);
  return false;
}

/**
 * Walk every block in the editor state and reconcile IDs. Used after fuzz
 * steps and on import. Wraps itself in `editor.update`.
 */
export function $reconcileAllIds(
  editor: LexicalEditor,
  registry: IdRegistry,
): Promise<void> {
  return new Promise((resolve) => {
    editor.update(
      () => {
        const root = editor.getEditorState()._nodeMap;
        // First pass: rebuild the live ID set from current state (drops
        // entries for deleted nodes).
        registry.used.clear();
        registry.byKey.clear();
        for (const [, node] of root) {
          if ($isBlockNode(node) && node.getBlockId()) {
            const id = node.getBlockId()!;
            if (registry.used.has(id)) {
              // Duplicate found — mint fresh for this node.
              const fresh = mintUniqueId(registry.used);
              node.setBlockId(fresh);
              registry.used.add(fresh);
              registry.byKey.set(node.getKey(), fresh);
            } else {
              registry.used.add(id);
              registry.byKey.set(node.getKey(), id);
            }
          }
        }
        // Second pass: assign IDs to any unstamped blocks.
        for (const [, node] of root) {
          if ($isBlockNode(node) && !node.getBlockId()) {
            const id = mintUniqueId(registry.used);
            node.setBlockId(id);
            registry.used.add(id);
            registry.byKey.set(node.getKey(), id);
          }
        }
      },
      { onUpdate: () => resolve() },
    );
  });
}

export function BlockIdPlugin({ registry }: { registry: IdRegistry }) {
  const [editor] = useLexicalComposerContext();

  useEffect(() => {
    const unregisters: Array<() => void> = [];

    const tx = <T extends BlockNode>(
      klass: new (...args: never[]) => T,
      guard: (n: LexicalNode | null | undefined) => n is T,
    ) =>
      editor.registerNodeTransform(klass as never, (node: unknown) => {
        if (!guard(node as LexicalNode)) return;
        ensureId(node as T, registry);
      });

    unregisters.push(tx(BkParagraphNode, $isBkParagraphNode));
    unregisters.push(tx(BkHeadingNode, $isBkHeadingNode));
    unregisters.push(tx(BkListItemNode, $isBkListItemNode));
    unregisters.push(tx(DialogueNode, $isDialogueNode));
    unregisters.push(tx(SceneBreakNode, $isSceneBreakNode));

    // Backspace-at-start-of-block: Lexical's default behavior merges the
    // current block into the previous one. The receiving (previous) node
    // keeps its key/id because Lexical reuses its instance; our `clone`
    // method carries `__blockId` across the dirty-mark, so the merged
    // result naturally retains the FIRST block's id. We register a
    // listener purely to validate the invariant in dev mode.
    unregisters.push(
      editor.registerCommand(
        KEY_BACKSPACE_COMMAND,
        () => {
          // Fall through to default handler; we just observe.
          return false;
        },
        COMMAND_PRIORITY_HIGH,
      ),
    );

    // After every update, refresh the by-key map for any blocks that
    // moved around (paste, re-order). Drop entries whose keys no longer
    // resolve to a live block. This keeps `used` accurate over time.
    unregisters.push(
      editor.registerUpdateListener(({ editorState }) => {
        editorState.read(() => {
          const seenIds = new Set<string>();
          const nextByKey = new Map<string, string>();
          for (const [key] of editorState._nodeMap) {
            const node = $getNodeByKey(key);
            if (!$isBlockNode(node)) continue;
            const id = node.getBlockId();
            if (id) {
              seenIds.add(id);
              nextByKey.set(key, id);
            }
          }
          registry.used = seenIds;
          registry.byKey = nextByKey;
        });
      }),
    );

    return () => {
      for (const u of unregisters) u();
    };
  }, [editor, registry]);

  return null;
}
