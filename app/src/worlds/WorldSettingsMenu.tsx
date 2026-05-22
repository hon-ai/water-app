import { ipc, type WorldSegment } from "../ipc/commands";

/**
 * The six built-in segment slugs that ship with every project. Built-in
 * segments cannot be deleted (use visibility toggle to hide them
 * instead) — the Rust side enforces this at the boundary; this set
 * just lets the UI omit the affordance rather than render-then-fail.
 */
const BUILTIN_SLUGS: ReadonlySet<string> = new Set([
  "concept",
  "locations",
  "politics_and_social",
  "culture",
  "world",
  "history",
]);

interface Props {
  segments: WorldSegment[];
  /** Called after a visibility toggle or delete succeeds so the parent can
   *  refresh the segment list. */
  onChanged: () => void;
  /** Called when the writer clicks the close affordance. */
  onClose: () => void;
}

/**
 * Settings popover for the World Bible index (M4 T32). Lists every
 * segment (including hidden ones, so the writer can restore them) with:
 *
 *  - a visibility checkbox (calls `worldSegmentSetHidden`)
 *  - a delete button (user-added segments only; built-ins are
 *    visibility-only)
 *
 * Delete confirmation uses `window.confirm`. The Rust side enforces the
 * "built-ins cannot be deleted" rule at the boundary; the UI omits the
 * affordance for built-in slugs so the writer is never offered an
 * action that would fail.
 */
export function WorldSettingsMenu({ segments, onChanged, onClose }: Props) {
  return (
    <div className="world-settings-menu" data-testid="world-settings-menu">
      <h3>World segments</h3>
      <button
        type="button"
        onClick={onClose}
        aria-label="Close settings"
        className="close-x"
      >
        ×
      </button>
      <ul>
        {segments.map((s) => {
          const isBuiltin = BUILTIN_SLUGS.has(s.slug);
          return (
            <li key={s.id}>
              <label>
                <input
                  type="checkbox"
                  checked={!s.hidden}
                  onChange={async (e) => {
                    await ipc.worldSegmentSetHidden({
                      segmentId: s.id,
                      hidden: !e.target.checked,
                    });
                    onChanged();
                  }}
                  data-testid={`visibility-${s.slug || s.id}`}
                />
                {s.name}
                {isBuiltin && (
                  <span className="badge-builtin" style={{ marginLeft: 6 }}>
                    built-in
                  </span>
                )}
              </label>
              {!isBuiltin && (
                <button
                  type="button"
                  className="water-button water-button-danger water-button-compact"
                  onClick={async () => {
                    if (
                      window.confirm(
                        `Delete segment "${s.name}"? This cannot be undone.`,
                      )
                    ) {
                      await ipc.worldSegmentDelete(s.id);
                      onChanged();
                    }
                  }}
                  data-testid={`delete-${s.id}`}
                >
                  Delete
                </button>
              )}
            </li>
          );
        })}
      </ul>
    </div>
  );
}
