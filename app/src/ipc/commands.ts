import { invoke } from "@tauri-apps/api/core";

/**
 * Phase 5 — a persisted editor pill (diagnostic finding). Mirrors
 * `EditorPillRow` in `crates/water-core/src/editor/store.rs`.
 */
export interface EditorPillRow {
  id: string;
  scene_id: string;
  rule: string;
  severity: "observation" | "suggestion" | "warning";
  message: string;
  suggestion: string | null;
  anchor_block_id: string;
  anchor_start: number;
  anchor_end: number;
  text_snippet: string;
  content_hash: string;
  dismissed: boolean;
  created_at: string;
  updated_at: string;
}

export interface OpenProjectInfo {
  root: string;
  name: string;
  project_id: string;
  default_manuscript_id: string;
}

export interface SceneInfo {
  id: string;
  name: string;
  ordering: number;
  word_count: number;
}

/**
 * Per-scene location pill payload. Mirrors
 * `commands::scene::SceneLocationPayload` on the Rust side. Returned as
 * part of `SceneMetadata.location` so the SceneMetadataSheet can render
 * the location pill (name + segment slug for hue selection) without a
 * second IPC round-trip to `world_entry_read`. `segment_slug` is the
 * parent `world_segment.slug` (e.g. `"locations"`).
 */
export interface SceneLocationPayload {
  id: string;
  name: string;
  segment_slug: string;
}

/**
 * Per-scene character + location metadata. Mirrors
 * `commands::scene::SceneMetadata` on the Rust side. Used by the
 * SceneMetadataSheet (M3 T21, M4 T11) to populate its checkbox + POV
 * select + location pill on open. `pov_character_id` is `null` when no
 * POV is set; `location` is `null` when `scene.location_id IS NULL`.
 */
export interface SceneMetadata {
  characters_present: string[];
  pov_character_id: string | null;
  location: SceneLocationPayload | null;
  /** Brief summary of what happens in the scene. */
  summary: string | null;
}

export interface SidecarInfo {
  base_url: string;
  status: "loading" | "ready" | "error";
  last_status_detail: string | null;
}

export interface ProviderHealth {
  id: string;
  ok: boolean;
  error: string | null;
}

export interface DiagnosticsStatus {
  has_open_project: boolean;
  project_root: string | null;
  router_primary_id: string | null;
  sidecar: SidecarInfo | null;
  provider_health: ProviderHealth[];
}

/**
 * One row in the character index panel. Mirrors
 * `commands::character::CharacterIndexEntry` on the Rust side. `completion`
 * is the 0..=100 percent of LSM v2.1 required fields filled (rounded
 * down).
 */
export interface CharacterIndexEntry {
  id: string;
  full_name: string;
  role: string | null;
  hue_token: string;
  completion: number;
}

/**
 * On-disk projection of a character TOML. Mirrors `water_core::CharacterFile`.
 * `data` is the raw LSM v2.1 sheet (with sections `main`, `bonus_traits`,
 * `arc`, `perspectives` at the top level).
 */
export interface CharacterFile {
  id: string;
  name: string;
  schema_version: string;
  // `data` is flattened on the Rust side via `#[serde(flatten)]`, so the
  // section keys appear at the top level alongside `id`/`name`/etc.
  // We type the catch-all here as `unknown` per-key so call sites have to
  // narrow before reading.
  [key: string]: unknown;
}

/**
 * The variant shape of `IntakeField.kind`. Mirrors
 * `water_core::character::intake::IntakeFieldKind`, which derives
 * `#[serde(rename_all = "snake_case", tag = "type", content = "options")]`.
 * `choice` carries an `options` array; the other variants do not.
 */
export type IntakeFieldKind =
  | { type: "short_text" }
  | { type: "long_text" }
  | { type: "string_list" }
  | { type: "choice"; options: string[] };

/**
 * One question in an intake schema. Mirrors
 * `water_core::character::intake::IntakeField` — note that the Rust side
 * uses `&'static str` everywhere and serde emits plain JSON strings, so
 * every field below is a `string`/`string[]`/`null`. `optional_skip`
 * controls whether the renderer shows a "Skip" affordance.
 */
export interface IntakeField {
  id: string;
  section: string;
  label: string;
  prompt_question: string;
  helper: string | null;
  examples: string[];
  kind: IntakeFieldKind;
  optional_skip: boolean;
}

/** One section of an intake schema. */
export interface IntakeSchemaSection {
  section: string;
  fields: IntakeField[];
}

/**
 * One hit from the scene-character autosuggest scanner. Snake_case to
 * match the rest of the character command surface (`hue_token`, etc.).
 */
export interface AutosuggestResult {
  character_id: string;
  full_name: string;
  mention_count: number;
}

/**
 * One row in the World Bible segment index. Mirrors
 * `commands::world::WorldSegmentPayload` on the Rust side. `is_collection`
 * distinguishes the single-doc segments (e.g. Concept, Culture) from the
 * collection segment (Locations) which carries multiple entries.
 *
 * `has_template_override` is `true` iff this segment has a user-edited
 * `template_json` row — used by the UI to badge segments whose intake
 * schema diverges from the built-in default.
 */
export interface WorldSegment {
  id: string;
  slug: string;
  name: string;
  ordering: number;
  is_collection: boolean;
  hue_token: string;
  hidden: boolean;
  has_template_override: boolean;
}

/**
 * Variant shape of `WorldTemplateField.kind`. Mirrors
 * `water_core::world::templates::WorldTemplateFieldKind`, which derives
 * `#[serde(tag = "type", rename_all = "snake_case")]`. The `choice`
 * variant carries an `options` array; the others do not.
 *
 * Note: M4 templates and M3 character intake fields share this exact
 * discriminator shape (`short_text` / `long_text` / `string_list` /
 * `choice`), so the renderer's field-input components can be reused. The
 * containing field types differ — see `WorldTemplateField` below vs.
 * `IntakeField`.
 */
export type WorldTemplateFieldKind =
  | { type: "short_text" }
  | { type: "long_text" }
  | { type: "string_list" }
  | { type: "choice"; options: string[] };

/**
 * One question in a world segment template. Mirrors
 * `water_core::world::templates::WorldTemplateField`. M4 templates are
 * runtime-loaded from `world_segment.template_json` (or a built-in
 * default), so every field below is owned/String on the Rust side.
 *
 * Field-id convention: `main.<key>` for scalars/long-text, `lists.<key>`
 * for `string_list` kinds — matches the on-disk `[main]` / `[lists]` TOML
 * sections.
 */
export interface WorldTemplateField {
  id: string;
  label: string;
  prompt_question: string;
  kind: WorldTemplateFieldKind;
  optional_skip: boolean;
}

/**
 * One world segment template (a full intake schema). Mirrors
 * `water_core::world::templates::WorldTemplateSchema`. Note this differs
 * from M3's `IntakeSchemaSection` shape (M3 emits `{section, fields}`,
 * M4 emits `{id, label, fields}`) — see the module docs in
 * `crates/water-core/src/world/templates.rs` for the rationale.
 */
export interface WorldTemplateSchema {
  id: string;
  label: string;
  fields: WorldTemplateField[];
}

/**
 * One world doc — either a single-doc segment or one collection entry.
 * Mirrors `commands::world::WorldEntryFilePayload`. Section keys
 * (`"main"`, `"lists"`, …) land at top level via `#[serde(flatten)]` on
 * the Rust side, so callers should read them as `file["main"]` etc.
 *
 * For single-doc segments `aliases` is always `[]`; for collection
 * entries it carries the per-entry alias list (used by world
 * autosuggest's case-insensitive name+alias index).
 */
export type WorldEntryFile = {
  id: string;
  segment_id: string;
  schema_version: string;
  name: string;
  aliases: string[];
  // Section keys (e.g. "main", "lists") land at top level via
  // #[serde(flatten)] on the Rust side.
  [key: string]: unknown;
};

/**
 * One row in a collection-segment index, or one hit from the world
 * autosuggest scanner. Mirrors `commands::world::WorldEntryIndexPayload`.
 * `preview` is server-computed (first non-empty `[main]` field truncated
 * to 80 chars) for index rows and empty (`""`) for autosuggest hits.
 */
export type WorldEntryIndexEntry = {
  id: string;
  segment_id: string;
  name: string;
  preview: string;
};

/**
 * M5: one row of a per-(scene, paragraph_ix, metric) heat cache.
 * Mirrors `water_core::heat::HeatRow`. Returned by `heatRead`; the
 * renderer (HeatmapStrip) plots `value` along the strip with
 * paragraph_ix mapping to x-position.
 */
export interface HeatRow {
  paragraph_ix: number;
  value: number;
  text_hash: string;
  updated_at: string;
}

/**
 * M5: the five heat metric kinds. Mirrors `HeatMetricKind` on the
 * Rust side; values are the snake-case strings stored in
 * `heat_metric.metric`.
 */
export type HeatMetricKind =
  | "pacing"
  | "valence"
  | "coherence"
  | "presence"
  | "world_refs";

/**
 * M5: response shape from `heat_read`. Map keyed by metric kind;
 * value is the (possibly empty) per-paragraph row list. Empty vec
 * means "no data yet for this metric" — render an empty track.
 */
export interface HeatReadResponse {
  metrics: Record<HeatMetricKind, HeatRow[]>;
}

/**
 * M5: writer's persisted metric-picker state. Read on mount,
 * written when the writer toggles a row in the metric picker.
 */
export interface HeatMetricEnabledMap {
  enabled: Partial<Record<HeatMetricKind, boolean>>;
}

/**
 * M6: one scene's row on the macro spatial canvas. Mirrors
 * `commands::canvas::SceneCanvasRow`. `canvas_x` / `canvas_y` are
 * `null` for unplaced scenes — the renderer's auto-flow layout
 * (see `crates/water-core/src/canvas/layout.rs::auto_flow`) is
 * applied client-side to fill in those slots.
 */
export interface Presence {
  id: string;
  name: string;
}

export interface SceneCanvasRow {
  id: string;
  name: string;
  manuscript_ordering: number;
  canvas_x: number | null;
  canvas_y: number | null;
  canvas_group: string | null;
  word_count: number;
  /** M6 lanes: POV character (LEFT JOIN'd at the IPC boundary). */
  pov_character_id: string | null;
  pov_character_name: string | null;
  /** M6 lanes: primary location entry. */
  location_id: string | null;
  location_name: string | null;
  /**
   * Writer-supplied brief summary (from scene.scene_goal) — shows on
   * the SceneCard so event order is legible at a glance when
   * rearranging scenes.
   */
  summary: string | null;
  /**
   * All characters present in the scene (POV + any extras from
   * `scene_character_presence`). The first entry is the primary
   * (POV) when set; the renderer uses it as the primary lane.
   */
  character_presences: Presence[];
  /**
   * All locations the scene touches (primary + extras from
   * `scene_location_presence`). First entry is the primary.
   */
  location_presences: Presence[];
}

/**
 * Response from `pill_pin`. Mirrors `commands::pill::PinPillResponse` on
 * the Rust side. `stub_entry_id` is non-null only for the Chorus +
 * `no_universe_yet` path (M4 T29), which seeds a new `locations`
 * `world_entry` with the pill's snippet so the writer can elaborate
 * without leaving the surface.
 */
export interface PinPillResponse {
  pin_id: string;
  stub_entry_id: string | null;
  /**
   * Set whenever `stub_entry_id` is set — the segment id of the newly-
   * created `locations` stub. The renderer routes to the new entry
   * sheet (addressed by (segmentId, entryId)) using this pair.
   */
  stub_segment_id: string | null;
}

export const ipc = {
  createProject: (parentDir: string, name: string): Promise<OpenProjectInfo> =>
    invoke("create_project", { parentDir, name }),
  openProject: (root: string): Promise<OpenProjectInfo> =>
    invoke("open_project", { root }),
  closeProject: (): Promise<void> => invoke("close_project"),

  sceneCreate: (name: string): Promise<SceneInfo> =>
    invoke("scene_create", { name }),
  sceneRead: (id: string): Promise<string> => invoke("scene_read", { id }),
  sceneWriteBody: (id: string, body: string): Promise<SceneInfo> =>
    invoke("scene_write_body", { id, body }),
  sceneList: (): Promise<SceneInfo[]> => invoke("scene_list"),
  sceneRename: (id: string, name: string): Promise<SceneInfo> =>
    invoke("scene_rename", { id, name }),
  // Per-scene character metadata (M3 T21). Read-only — mutations go
  // through `characterLinkToScene` / `characterUnlinkFromScene` /
  // `characterSetPov` so the per-scene write lock + presence-FK rules
  // (spec § 20) stay in the character commands' code path.
  sceneReadMetadata: (id: string): Promise<SceneMetadata> =>
    invoke("scene_read_metadata", { id }),
  // Attach or clear the scene→location FK (M4 T11). Pass `locationId:
  // null` to clear. The Rust command validates the location id at the
  // boundary (ULID parse) so a malformed string never reaches SQLite.
  sceneSetLocation: (req: {
    sceneId: string;
    locationId: string | null;
  }): Promise<void> =>
    invoke("scene_set_location", {
      sceneId: req.sceneId,
      locationId: req.locationId,
    }),
  /** Persist a scene's brief summary. Pass null to clear. */
  sceneSetSummary: (sceneId: string, summary: string | null): Promise<void> =>
    invoke("scene_set_summary", { sceneId, summary }),

  // Character CRUD (M3 T12). `characterUpdateField` is called once per
  // answer in the Conversational Intake flow; the Rust side serializes
  // concurrent calls for the same id via a per-character write lock so
  // the on-disk `.toml` cannot tear.
  characterCreate: (): Promise<CharacterIndexEntry> =>
    invoke("character_create"),
  characterRead: (id: string): Promise<CharacterFile> =>
    invoke("character_read", { id }),
  characterList: (): Promise<CharacterIndexEntry[]> =>
    invoke("character_list"),
  characterUpdateField: (
    id: string,
    fieldId: string,
    value: unknown,
  ): Promise<CharacterIndexEntry> =>
    invoke("character_update_field", { id, fieldId, value }),
  characterDelete: (id: string): Promise<void> =>
    invoke("character_delete", { id }),

  // Scene linkage (M3 T13). `characterSetPov` accepts `null` to clear the
  // POV; passing a non-null id requires the character to already be linked
  // via `characterLinkToScene` (spec § 20). Unlinking a character who is
  // currently POV transactionally clears POV as part of the same call.
  characterLinkToScene: (sceneId: string, characterId: string): Promise<void> =>
    invoke("character_link_to_scene", { sceneId, characterId }),
  characterUnlinkFromScene: (
    sceneId: string,
    characterId: string,
  ): Promise<void> =>
    invoke("character_unlink_from_scene", { sceneId, characterId }),
  characterSetPov: (
    sceneId: string,
    characterId: string | null,
  ): Promise<void> => invoke("character_set_pov", { sceneId, characterId }),

  // Intake schema + autosuggest (M3 T14). `intakeSchema` is stateless;
  // `characterAutosuggestForScene` validates the scene id at the command
  // boundary but currently autosuggests based on body text alone (the
  // scene id is reserved for future presence-aware filtering — see the
  // command's `_core` helper for rationale).
  intakeSchema: (schemaId: string): Promise<IntakeSchemaSection[]> =>
    invoke("intake_schema", { schemaId }),
  characterAutosuggestForScene: (
    sceneId: string,
    bodyText: string,
  ): Promise<AutosuggestResult[]> =>
    invoke("character_autosuggest_for_scene", { sceneId, bodyText }),

  // World/Setting Bible segment CRUD (M4 T8). All segment-id and
  // project-id values are opaque ULID strings on the wire (the Rust side
  // parses them via `Id::parse`). The renderer treats them as opaque.
  //
  // `worldSegmentCreate` returns the new segment's stringified id.
  // `worldSegmentDelete` refuses to delete the six built-in slugs
  // (`concept`, `locations`, `politics_and_social`, `culture`, `world`,
  // `history`) — use `worldSegmentSetHidden` to hide a built-in instead.
  worldSegmentList: (): Promise<WorldSegment[]> => invoke("world_segment_list"),
  worldSegmentCreate: (req: {
    name: string;
    isCollection: boolean;
    template: WorldTemplateSchema;
  }): Promise<string> =>
    invoke("world_segment_create", {
      req: {
        name: req.name,
        is_collection: req.isCollection,
        template: req.template,
      },
    }),
  worldSegmentUpdateTemplate: (req: {
    segmentId: string;
    template: WorldTemplateSchema;
  }): Promise<void> =>
    invoke("world_segment_update_template", {
      req: { segment_id: req.segmentId, template: req.template },
    }),
  worldSegmentSetHidden: (req: {
    segmentId: string;
    hidden: boolean;
  }): Promise<void> =>
    invoke("world_segment_set_hidden", {
      req: { segment_id: req.segmentId, hidden: req.hidden },
    }),
  worldSegmentDelete: (segmentId: string): Promise<void> =>
    invoke("world_segment_delete", { segmentId }),
  worldIntakeSchema: (segmentId: string): Promise<WorldTemplateSchema> =>
    invoke("world_intake_schema", { segmentId }),

  // World single-doc commands (M4 T9). `worldSingleDocRead` lazily
  // materializes an empty row the first time a segment is opened, so the
  // first read after `seed_builtins` is always non-null.
  worldSingleDocRead: (segmentId: string): Promise<WorldEntryFile> =>
    invoke("world_single_doc_read", { segmentId }),
  worldSingleDocUpdateField: (req: {
    segmentId: string;
    fieldId: string;
    value: unknown;
  }): Promise<void> =>
    invoke("world_single_doc_update_field", {
      req: {
        segment_id: req.segmentId,
        field_id: req.fieldId,
        value: req.value,
      },
    }),

  // World collection-entry CRUD + autosuggest (M4 T10).
  //
  // `worldEntryCreate` accepts an empty `name` — the Chorus stub flow
  // intentionally relies on this to create an empty entry that the
  // orphan reaper (`worldEntryDeleteIfEmpty`) can collect later if the
  // user abandons it.
  //
  // `worldAutosuggest` is scoped to `locations`-slug entries only in
  // M4; other segments are filtered out server-side. `sceneId` is
  // accepted now but currently unused (reserved for presence-aware
  // filtering, matching the character autosuggest surface convention).
  worldEntryList: (segmentId: string): Promise<WorldEntryIndexEntry[]> =>
    invoke("world_entry_list", { segmentId }),
  worldEntryRead: (entryId: string): Promise<WorldEntryFile> =>
    invoke("world_entry_read", { entryId }),
  worldEntryCreate: (req: {
    segmentId: string;
    name: string;
  }): Promise<string> =>
    invoke("world_entry_create", {
      req: { segment_id: req.segmentId, name: req.name },
    }),
  worldEntryUpdateField: (req: {
    entryId: string;
    fieldId: string;
    value: unknown;
  }): Promise<void> =>
    invoke("world_entry_update_field", {
      req: {
        entry_id: req.entryId,
        field_id: req.fieldId,
        value: req.value,
      },
    }),
  worldEntryUpdateAliases: (req: {
    entryId: string;
    aliases: string[];
  }): Promise<void> =>
    invoke("world_entry_update_aliases", {
      req: { entry_id: req.entryId, aliases: req.aliases },
    }),
  worldEntryDeleteIfEmpty: (entryId: string): Promise<boolean> =>
    invoke("world_entry_delete_if_empty", { entryId }),
  worldEntryDelete: (entryId: string): Promise<void> =>
    invoke("world_entry_delete", { entryId }),
  worldAutosuggest: (req: {
    sceneId: string;
    paragraph: string;
  }): Promise<WorldEntryIndexEntry[]> =>
    invoke("world_autosuggest", {
      req: { scene_id: req.sceneId, paragraph: req.paragraph },
    }),

  /**
   * M5 Heatmap. Read the cached per-paragraph metric rows for one
   * scene. Subscribe to the `heat:updated` event to know when a
   * recompute landed; refetch then.
   */
  heatRead: (sceneId: string): Promise<HeatReadResponse> =>
    invoke("heat_read", { sceneId }),
  /**
   * Persist the writer's metric-picker toggle for this project. The
   * orchestrator currently writes all enabled metrics; this is the
   * renderer-side opt-in/out (the metric picker popover).
   */
  heatSetMetricEnabled: (
    kind: HeatMetricKind,
    enabled: boolean,
  ): Promise<void> => invoke("heat_set_metric_enabled", { kind, enabled }),
  /**
   * Read the persisted picker state on mount so toggles survive
   * restarts. Returns an empty map on first launch.
   */
  heatReadSettings: (): Promise<HeatMetricEnabledMap> =>
    invoke("heat_read_settings"),

  /**
   * M6 Canvas. List every scene in the open project with its
   * canvas position + group (NULL = unplaced; the renderer auto-
   * flows those locally).
   */
  sceneCanvasList: (): Promise<SceneCanvasRow[]> =>
    invoke("scene_canvas_list"),
  /**
   * Persist a drag target's new position. Pass `null` for both
   * coords to clear (the scene falls back to auto-flow on next
   * paint).
   */
  sceneCanvasSetPosition: (
    sceneId: string,
    x: number | null,
    y: number | null,
  ): Promise<void> =>
    invoke("scene_canvas_set_position", { sceneId, x, y }),
  sceneCanvasSetGroup: (
    sceneId: string,
    group: string | null,
  ): Promise<void> => invoke("scene_canvas_set_group", { sceneId, group }),
  /**
   * Right-click "reset to manuscript order" action. Clears every
   * scene's canvas_x/canvas_y in the open project; the renderer
   * re-applies auto-flow.
   */
  sceneCanvasResetAll: (): Promise<void> =>
    invoke("scene_canvas_reset_all"),

  /** Test a provider end-to-end. `model` (optional) is the model id the
   *  writer has chosen in the model picker — if omitted, the Rust side
   *  falls back to the adapter's hardcoded default. The chosen model is
   *  also applied as the router's default-model override after a
   *  successful test, so subsequent calls keep using it. */
  providerTest: (providerId: string, model?: string): Promise<string[]> =>
    invoke("provider_test", { providerId, model: model ?? null }),
  providerSetKey: (providerId: string, key: string): Promise<void> =>
    invoke("provider_set_key", { providerId, key }),
  /** Set the active model override for a provider. Empty `model`
   *  clears the override (provider falls back to its default).
   *  Renderer persists locally + re-applies on boot so the choice
   *  survives restarts. */
  providerSetModel: (providerId: string, model: string): Promise<void> =>
    invoke("provider_set_model", { providerId, model }),

  diagnosticsStatus: (): Promise<DiagnosticsStatus> =>
    invoke("diagnostics_status"),

  pillExpand: (parent_pill_id: string): Promise<void> =>
    invoke("pill_expand", { parentPillId: parent_pill_id }),
  pillRegenerate: (parent_pill_id: string): Promise<void> =>
    invoke("pill_regenerate", { parentPillId: parent_pill_id }),

  /**
   * Push the current scene + project snapshot into the orchestrator. The
   * renderer should call this whenever a scene loads and after each
   * successful body save; the orchestrator caches `bodyText` so subsequent
   * `typing:telemetry` ticks can build prompt excerpts without re-reading
   * from disk.
   *
   * `characterCount` and `worldEntryCount` are project-level and stay 0
   * for M2 (the `no_universe_yet` trigger remains the eager-fire path
   * until M3/M4 wire CharacterStore/WorldStore).
   */
  sceneState: (payload: {
    sceneId: string;
    povCharacterId?: string | null;
    locationId?: string | null;
    charactersPresent: string[];
    wordCount: number;
    bodyText: string;
    characterCount: number;
    worldEntryCount: number;
    /** Phase 6 — 0-indexed position of this scene in its manuscript.
     *  Optional; absent → orchestrator omits the arc-position line. */
    sceneOrdering?: number | null;
    /** Phase 6 — total scenes in the manuscript. See `sceneOrdering`. */
    manuscriptSceneCount?: number | null;
  }): Promise<void> =>
    invoke("scene_state", {
      payload: {
        scene_id: payload.sceneId,
        pov_character_id: payload.povCharacterId ?? null,
        location_id: payload.locationId ?? null,
        characters_present: payload.charactersPresent,
        word_count: payload.wordCount,
        body_text: payload.bodyText,
        character_count: payload.characterCount,
        world_entry_count: payload.worldEntryCount,
        scene_ordering: payload.sceneOrdering ?? null,
        manuscript_scene_count: payload.manuscriptSceneCount ?? null,
      },
    }),
  /**
   * Pin a pill. Returns the row id (currently the pill's own id) plus
   * an optional `stub_entry_id` — populated only when the pin path
   * created a new `world_entry` (Chorus + `no_universe_yet`). The
   * renderer (Bouquet) routes to the new entry sheet when
   * `stub_entry_id` is non-null. Mirrors `commands::pill::PinPillResponse`
   * on the Rust side.
   */
  pillPin: (
    pill: import("../pill/types").Pill,
    sceneId: string,
    blockId: string,
    snippet: string,
  ): Promise<PinPillResponse> =>
    invoke("pill_pin", { pill, sceneId, blockId, snippet }),
  pillDismiss: (pill_id: string): Promise<void> =>
    invoke("pill_dismiss", { pillId: pill_id }),
  /** v8: notify the orchestrator that the renderer evicted this
   *  pill via FIFO. Used by the adaptive sensitivity learner —
   *  fire-and-forget; failures are swallowed. */
  pillEvicted: (pill_id: string): Promise<void> =>
    invoke("pill_evicted", { pillId: pill_id }),
  /** v8: Settings "Reset trigger learning" — wipes
   *  `trigger_feedback` and resets the orchestrator's in-memory
   *  tuning. Buried in Settings on purpose; the system is meant
   *  to be invisible. */
  feedbackReset: (): Promise<void> => invoke("feedback_reset"),

  /** Phase 4 — open the rabbit hole on a pill. Persists a root
   *  thought and dispatches the first fan_4 LLM call. The renderer
   *  listens for `deepen:ready` to receive the four children.
   *
   *  Passes the pill's text + speaker + block target through the
   *  IPC because the service-side Pill record never gets its
   *  `text` field written back after the LLM call lands; the
   *  renderer is the source of truth here. */
  pillDeepen: (
    pill_id: string,
    parent_text: string,
    speaker_id: string,
    block_target_id: string | null,
  ): Promise<void> =>
    invoke("pill_deepen", {
      pillId: pill_id,
      parentText: parent_text,
      speakerId: speaker_id,
      blockTargetId: block_target_id,
    }),
  /** Phase 4 — fan four further children from an existing rabbit
   *  thought (writer descended via the deepen panel). */
  rabbitDeepenThought: (thought_id: string): Promise<void> =>
    invoke("rabbit_deepen_thought", { thoughtId: thought_id }),
  /** Phase 4 — toggle the resonance flag on a rabbit thought.
   *  Resonant nodes (and ancestors) are protected from auto-trim;
   *  Phase 6 prompts will read recent resonant picks as a
   *  voice-preference signal. */
  rabbitSetResonance: (thought_id: string, resonant: boolean): Promise<void> =>
    invoke("rabbit_set_resonance", { thoughtId: thought_id, resonant }),

  /** Phase 5 — kick the diagnostic engine across a scene's blocks.
   *  Renderer extracts [(blockId, text), …] from the PM doc and
   *  passes them; the orchestrator persists findings and emits
   *  `editor_pills:updated`. Returns the live (non-dismissed) set
   *  for the caller to render synchronously. */
  editorPillsRun: (
    scene_id: string,
    blocks: Array<{ blockId: string; text: string }>,
  ): Promise<EditorPillRow[]> =>
    invoke("editor_pills_run", {
      payload: {
        scene_id,
        blocks: blocks.map((b) => ({ block_id: b.blockId, text: b.text })),
      },
    }),
  /** Phase 5 — read active (non-dismissed) editor pills for a scene. */
  editorPillsList: (scene_id: string): Promise<EditorPillRow[]> =>
    invoke("editor_pills_list", { sceneId: scene_id }),
  /** Phase 5 — flag an editor pill as dismissed. */
  editorPillDismiss: (id: string): Promise<void> =>
    invoke("editor_pill_dismiss", { id }),
  /** Phase 5.8 — request an LLM polish pass on a paragraph. The
   *  orchestrator throttles per scene (5/session) and per block
   *  (30s cooldown) before spending the call. Fire-and-forget;
   *  success surfaces via `editor_pills:updated`. */
  editorPolishRequest: (
    scene_id: string,
    block_id: string,
    text: string,
  ): Promise<void> =>
    invoke("editor_polish_request", {
      sceneId: scene_id,
      blockId: block_id,
      text,
    }),
  pinnedList: (): Promise<import("../pill/types").Pill[]> =>
    invoke("pinned_list"),

  // Open an external URL via the OS default handler. Backed by the Tauri
  // `shell` plugin, whose capability scope (see
  // app/src-tauri/capabilities/default.json) restricts URLs to
  // https://*, http://*, and mailto:* schemes. Anything outside that
  // scope rejects with a "scope denied" error from the plugin.
  openExternalLink: (url: string): Promise<void> =>
    import("@tauri-apps/plugin-shell").then(({ open }) => open(url)),
};
