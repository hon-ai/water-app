// M2 editor bake-off — Lexical harness.
//
// Dev-only harness page reached via `?bakeoff=lexical`. Exercises the six
// criteria the spec lists in §5.1:
//   1. Block-ID maintenance ergonomics
//   2. Decoration API (pill-style highlights + outer glow)
//   3. Selection/mark stability under autosave write-backs
//   4. Bundle-size impact (measured externally on `pnpm build`)
//   5. 50k-word typing latency
//   6. Long-undo behavior (200 step stress)
//
// Not wired to the production EditorCanvas. Isolated module.

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { LexicalComposer } from "@lexical/react/LexicalComposer";
import { RichTextPlugin } from "@lexical/react/LexicalRichTextPlugin";
import { ContentEditable } from "@lexical/react/LexicalContentEditable";
import { HistoryPlugin } from "@lexical/react/LexicalHistoryPlugin";
import { ListPlugin } from "@lexical/react/LexicalListPlugin";
import { LexicalErrorBoundary } from "@lexical/react/LexicalErrorBoundary";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import {
  $createTextNode,
  $getRoot,
  $getSelection,
  $isRangeSelection,
  $isTextNode,
  $isElementNode,
  REDO_COMMAND,
  UNDO_COMMAND,
  type LexicalEditor,
} from "lexical";
import { $createListNode, ListNode } from "@lexical/list";
import {
  $createBkHeadingNode,
  $createBkListItemNode,
  $createBkParagraphNode,
  $createDialogueNode,
  $createSceneBreakNode,
  $isBlockNode,
  bakeoffNodes,
  type BlockNode,
} from "./nodes";
import {
  BlockIdPlugin,
  createIdRegistry,
  $reconcileAllIds,
  type IdRegistry,
} from "./BlockIdPlugin";
import { LatencyPlugin, type LatencyController, emptyStats, type LatencyStats } from "./LatencyPlugin";
import "./harness.css";

// --------------------------------------------------------------------------
// Constants & helpers
// --------------------------------------------------------------------------

const LOREM_WORDS = (
  "lorem ipsum dolor sit amet consectetur adipiscing elit sed do eiusmod tempor " +
  "incididunt ut labore et dolore magna aliqua ut enim ad minim veniam quis " +
  "nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat " +
  "duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore " +
  "eu fugiat nulla pariatur excepteur sint occaecat cupidatat non proident sunt " +
  "in culpa qui officia deserunt mollit anim id est laborum"
).split(/\s+/);

function loremWords(n: number): string[] {
  const out: string[] = [];
  for (let i = 0; i < n; i++) {
    out.push(LOREM_WORDS[i % LOREM_WORDS.length] ?? "lorem");
  }
  return out;
}

// --------------------------------------------------------------------------
// Toolbar — wired inside the composer so it can use useLexicalComposerContext
// --------------------------------------------------------------------------

type LogEntry = { ts: number; level: "ok" | "err"; text: string };

function Toolbar({
  registry,
  latencyRef,
  log,
}: {
  registry: IdRegistry;
  latencyRef: React.MutableRefObject<LatencyController | null>;
  log: (entry: Omit<LogEntry, "ts">) => void;
}) {
  const [editor] = useLexicalComposerContext();
  const [latencyRunning, setLatencyRunning] = useState(false);
  const [lastStats, setLastStats] = useState<LatencyStats>(emptyStats());
  const [idCount, setIdCount] = useState(0);

  useEffect(() => {
    const u = editor.registerUpdateListener(() => {
      setIdCount(registry.used.size);
    });
    return u;
  }, [editor, registry]);

  // --- Seed content -------------------------------------------------------
  const handleSeed = useCallback(() => {
    editor.update(() => {
      const root = $getRoot();
      root.clear();

      const h2 = $createBkHeadingNode("h2");
      h2.append($createTextNode("Chapter Six"));
      root.append(h2);

      const h3 = $createBkHeadingNode("h3");
      h3.append($createTextNode("The Crossing"));
      root.append(h3);

      const p1 = $createBkParagraphNode();
      p1.append($createTextNode("The river was wider than they remembered."));
      root.append(p1);

      const d = $createDialogueNode();
      d.append($createTextNode('"We can\'t cross here," she said.'));
      root.append(d);

      const p2 = $createBkParagraphNode();
      p2.append(
        $createTextNode(
          "He nodded but did not answer. The horses smelled the water and slowed of their own accord.",
        ),
      );
      root.append(p2);

      const sb = $createSceneBreakNode();
      root.append(sb);

      const ol = $createListNode("number");
      for (const t of ["Rope", "Two horses", "Three lanterns"]) {
        const li = $createBkListItemNode();
        li.append($createTextNode(t));
        ol.append(li);
      }
      root.append(ol);

      const ul = $createListNode("bullet");
      for (const t of ["No wagon", "No bridge", "No moon yet"]) {
        const li = $createBkListItemNode();
        li.append($createTextNode(t));
        ul.append(li);
      }
      root.append(ul);
    });
    log({ level: "ok", text: "Seeded sample content" });
  }, [editor, log]);

  // --- Highlight random block --------------------------------------------
  const handleHighlight = useCallback(() => {
    editor.update(() => {
      const blocks = listBlocks(editor);
      if (blocks.length === 0) {
        log({ level: "err", text: "No blocks to highlight" });
        return;
      }
      const pick = blocks[Math.floor(Math.random() * blocks.length)];
      if (!pick) return;
      pick.setHighlight(!pick.__highlight);
      log({
        level: "ok",
        text: `Toggled highlight on ${pick.getBlockId() ?? "(no id)"}`,
      });
    });
  }, [editor, log]);

  // --- Paste 50k words ----------------------------------------------------
  const handlePaste50k = useCallback(() => {
    const words = loremWords(50_000);
    // Build 250 paragraphs of 200 words each.
    editor.update(() => {
      const root = $getRoot();
      let i = 0;
      while (i < words.length) {
        const p = $createBkParagraphNode();
        p.append($createTextNode(words.slice(i, i + 200).join(" ")));
        root.append(p);
        i += 200;
      }
    });
    log({ level: "ok", text: "Pasted ~50,000 words (250 paragraphs of 200)" });
  }, [editor, log]);

  // --- Latency window ----------------------------------------------------
  const handleStartLatency = useCallback(() => {
    if (!latencyRef.current) {
      log({ level: "err", text: "Latency controller not ready" });
      return;
    }
    latencyRef.current.start();
    setLatencyRunning(true);
    log({
      level: "ok",
      text: "Latency window started. Type into the editor for 60s. Click Stop to summarize.",
    });
    // Auto-stop after 60s.
    setTimeout(() => {
      if (latencyRef.current?.isRunning()) {
        const stats = latencyRef.current.stop();
        setLastStats(stats);
        setLatencyRunning(false);
        log({
          level: "ok",
          text: `Latency window auto-stopped after 60s. n=${stats.count} median=${stats.medianMs.toFixed(1)}ms p95=${stats.p95Ms.toFixed(1)}ms`,
        });
      }
    }, 60_000);
  }, [latencyRef, log]);

  const handleStopLatency = useCallback(() => {
    if (!latencyRef.current?.isRunning()) {
      log({ level: "err", text: "No latency window running" });
      return;
    }
    const stats = latencyRef.current.stop();
    setLastStats(stats);
    setLatencyRunning(false);
    log({
      level: "ok",
      text: `Latency window stopped. n=${stats.count} median=${stats.medianMs.toFixed(1)}ms p95=${stats.p95Ms.toFixed(1)}ms`,
    });
  }, [latencyRef, log]);

  // --- Fuzz: 50 random splits / merges / deletes -------------------------
  const handleFuzz = useCallback(async () => {
    let collisions = 0;
    let ops = 0;
    for (let step = 0; step < 50; step++) {
      await runFuzzStep(editor);
      ops++;
      // After each step, verify uniqueness by reading the live state.
      await new Promise<void>((resolve) =>
        editor.getEditorState().read(() => {
          const ids = new Set<string>();
          const dup: string[] = [];
          const blocks = listBlocks(editor);
          for (const b of blocks) {
            const id = b.getBlockId();
            if (!id) continue;
            if (ids.has(id)) dup.push(id);
            ids.add(id);
          }
          if (dup.length > 0) {
            collisions += dup.length;
          }
          resolve();
        }),
      );
    }
    // Final reconciliation pass.
    await $reconcileAllIds(editor, registry);
    log({
      level: collisions === 0 ? "ok" : "err",
      text: `Fuzz: ${ops} steps, ${collisions} ID collisions detected. Final unique IDs: ${registry.used.size}`,
    });
  }, [editor, registry, log]);

  // --- Long undo stress --------------------------------------------------
  const handleLongUndo = useCallback(async () => {
    // 1. Capture snapshot of IDs and highlights BEFORE the typing.
    const before = snapshotIds(editor);

    // 2. Type 200 characters into the last paragraph (one update each so each
    //    is a discrete history entry).
    for (let i = 0; i < 200; i++) {
      await new Promise<void>((resolve) =>
        editor.update(
          () => {
            const root = $getRoot();
            const last = root.getLastChild();
            if (last && $isElementNode(last)) {
              const tn = last.getLastDescendant();
              if (tn && $isTextNode(tn)) {
                tn.setTextContent(tn.getTextContent() + "x");
              } else {
                last.append($createTextNode("x"));
              }
            }
          },
          { onUpdate: () => resolve() },
        ),
      );
    }

    // 3. Undo 200 times.
    for (let i = 0; i < 200; i++) {
      editor.dispatchCommand(UNDO_COMMAND, undefined);
    }
    // Wait for history machinery to settle.
    await flush(editor);

    const afterUndo = snapshotIds(editor);

    // 4. Redo 200 times.
    for (let i = 0; i < 200; i++) {
      editor.dispatchCommand(REDO_COMMAND, undefined);
    }
    await flush(editor);

    const afterRedo = snapshotIds(editor);

    const idsBefore = [...before.keys()].sort().join(",");
    const idsAfterUndo = [...afterUndo.keys()].sort().join(",");
    const idsAfterRedo = [...afterRedo.keys()].sort().join(",");

    const driftOnUndo = idsBefore !== idsAfterUndo;
    const driftOnRedo = idsBefore !== idsAfterRedo;

    log({
      level: driftOnUndo || driftOnRedo ? "err" : "ok",
      text: `Long-undo: ${before.size} → ${afterUndo.size} → ${afterRedo.size} IDs. drift_on_undo=${driftOnUndo} drift_on_redo=${driftOnRedo}`,
    });
  }, [editor, log]);

  return (
    <>
      <div className="bk-toolbar">
        <button onClick={handleSeed}>Seed sample</button>
        <button onClick={handleHighlight}>Highlight random paragraph</button>
        <button onClick={handlePaste50k}>Paste 50k words</button>
        {!latencyRunning ? (
          <button onClick={handleStartLatency}>Start 60s latency window</button>
        ) : (
          <button onClick={handleStopLatency}>Stop latency window</button>
        )}
        <button onClick={handleFuzz}>Random edit fuzz (50 steps)</button>
        <button onClick={handleLongUndo}>Long-undo stress (200 steps)</button>
        <div className="bk-stats">
          <span>ids: {idCount}</span>
          <span>
            latency: median {lastStats.medianMs.toFixed(1)}ms · p95{" "}
            {lastStats.p95Ms.toFixed(1)}ms · n={lastStats.count}
          </span>
        </div>
        <a className="bk-back" href="?">
          ← back to app
        </a>
      </div>
    </>
  );
}

// --------------------------------------------------------------------------
// Helpers that run inside editor.read or editor.update
// --------------------------------------------------------------------------

function listBlocks(editor: LexicalEditor): BlockNode[] {
  const out: BlockNode[] = [];
  const state = editor.getEditorState();
  state.read(() => {
    for (const [, n] of state._nodeMap) {
      if ($isBlockNode(n)) out.push(n);
    }
  });
  return out;
}

function snapshotIds(editor: LexicalEditor): Map<string, { highlight: boolean }> {
  const map = new Map<string, { highlight: boolean }>();
  editor.getEditorState().read(() => {
    for (const b of listBlocks(editor)) {
      const id = b.getBlockId();
      if (id) map.set(id, { highlight: !!b.__highlight });
    }
  });
  return map;
}

function flush(editor: LexicalEditor): Promise<void> {
  return new Promise((resolve) => {
    // A no-op update flushes any pending updates / micro-tasks.
    editor.update(() => {}, { onUpdate: () => resolve() });
  });
}

async function runFuzzStep(editor: LexicalEditor): Promise<void> {
  const op = Math.floor(Math.random() * 3); // 0 = split, 1 = merge, 2 = delete
  return new Promise((resolve) =>
    editor.update(
      () => {
        const blocks = collectEditableBlocks();
        if (blocks.length < 2) {
          // Add a paragraph to keep the doc alive.
          const p = $createBkParagraphNode();
          p.append($createTextNode("seed"));
          $getRoot().append(p);
          return;
        }
        if (op === 0) {
          // Split: append a fresh paragraph after a random block.
          const target = blocks[Math.floor(Math.random() * blocks.length)];
          if (!target) return;
          const fresh = $createBkParagraphNode();
          fresh.append($createTextNode("split"));
          target.insertAfter(fresh);
        } else if (op === 1) {
          // "Merge": remove the LAST block and append its text into the
          // previous block (simulates Backspace-at-start collapse).
          if (blocks.length < 2) return;
          const last = blocks[blocks.length - 1];
          const prev = blocks[blocks.length - 2];
          if (!last || !prev) return;
          if (!$isElementNode(prev)) {
            last.remove();
            return;
          }
          const text = last.getTextContent();
          if (text) prev.append($createTextNode(text));
          last.remove();
        } else {
          // Delete: remove a random block (but not the only one).
          if (blocks.length <= 1) return;
          const idx = Math.floor(Math.random() * blocks.length);
          const victim = blocks[idx];
          if (victim) victim.remove();
        }
      },
      { onUpdate: () => resolve() },
    ),
  );

  // Local helper invoked inside the update; gathers top-level blocks.
  function collectEditableBlocks(): BlockNode[] {
    const list: BlockNode[] = [];
    const root = $getRoot();
    for (const child of root.getChildren()) {
      if ($isBlockNode(child)) list.push(child);
      // Lists hold ListItemNodes as their children; expose those too.
      if (child instanceof ListNode) {
        for (const li of child.getChildren()) {
          if ($isBlockNode(li)) list.push(li);
        }
      }
    }
    return list;
  }
}

// --------------------------------------------------------------------------
// The harness page
// --------------------------------------------------------------------------

export function LexicalBakeoffHarness() {
  const [log, setLog] = useState<LogEntry[]>([]);
  const appendLog = useCallback((entry: Omit<LogEntry, "ts">) => {
    setLog((prev) => [...prev.slice(-50), { ...entry, ts: Date.now() }]);
  }, []);

  const registry = useMemo<IdRegistry>(() => createIdRegistry(), []);
  const latencyRef = useRef<LatencyController | null>(null);

  const initialConfig = useMemo(
    () => ({
      namespace: "water-bakeoff-lexical",
      theme: {},
      nodes: bakeoffNodes(),
      onError(err: Error) {
        // eslint-disable-next-line no-console
        console.error("[bakeoff-lexical]", err);
      },
    }),
    [],
  );

  return (
    <div className="bk-root">
      <LexicalComposer initialConfig={initialConfig}>
        <Toolbar registry={registry} latencyRef={latencyRef} log={appendLog} />
        <div className="bk-editor-wrap">
          <RichTextPlugin
            contentEditable={
              <ContentEditable className="bk-editor" aria-label="Bake-off Lexical editor" />
            }
            placeholder={
              <div style={{ position: "absolute", color: "#aaa", pointerEvents: "none" }}>
                Click here and start typing, or "Seed sample" above.
              </div>
            }
            ErrorBoundary={LexicalErrorBoundary}
          />
          <HistoryPlugin />
          <ListPlugin />
          <BlockIdPlugin registry={registry} />
          <LatencyPlugin controllerRef={latencyRef} />
        </div>
      </LexicalComposer>
      <div className="bk-log">
        {log.map((e, i) => (
          <div key={i} className={e.level}>
            [{new Date(e.ts).toLocaleTimeString()}] {e.text}
          </div>
        ))}
        {log.length === 0 && (
          <div>
            Bake-off harness ready. Click "Seed sample" then exercise the criteria above.
          </div>
        )}
      </div>
    </div>
  );
}

export default LexicalBakeoffHarness;
