/**
 * Phase-3 content-signal taxonomy. Drives the pill's left-rail color
 * (see UX_SPEC.md §C.1) — the *what kind of remark this is*, decoupled
 * from the *who is saying it* (which lives on the speaker chip).
 *
 * - observation: default; ambient noticing. No rail.
 * - suggestion:  craft nudge in the speaker's voice. Sea-300 rail.
 * - warning:     mechanical / continuity issue. Sea-600 rail.
 * - praise:      writer landed something. Sea-glow rail.
 *
 * Optional on the payload — the backend hasn't started emitting this
 * yet (Phase 6 prompt overhaul plumbs it). When absent the renderer
 * treats the pill as `observation` and shows no rail.
 */
export type PillContentSignal =
  | "observation"
  | "suggestion"
  | "warning"
  | "praise";

export interface Pill {
  pill_id: string;
  speaker_id: string;
  hue_token: string;
  text: string;
  block_target_id: string | null;
  trigger_id: string;
  content_signal?: PillContentSignal;
}
