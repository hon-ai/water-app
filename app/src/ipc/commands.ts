import { invoke } from "@tauri-apps/api/core";

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
 * Per-scene character metadata. Mirrors `commands::scene::SceneMetadata`
 * on the Rust side. Used by the SceneMetadataSheet (M3 T21) to populate
 * its checkbox + POV select on open. `pov_character_id` is `null` when
 * no POV is set.
 */
export interface SceneMetadata {
  characters_present: string[];
  pov_character_id: string | null;
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

  providerTest: (providerId: string): Promise<string[]> =>
    invoke("provider_test", { providerId }),
  providerSetKey: (providerId: string, key: string): Promise<void> =>
    invoke("provider_set_key", { providerId, key }),

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
      },
    }),
  pillPin: (
    pill: import("../pill/types").Pill,
    sceneId: string,
    blockId: string,
    snippet: string,
  ): Promise<void> => invoke("pill_pin", { pill, sceneId, blockId, snippet }),
  pillDismiss: (pill_id: string): Promise<void> =>
    invoke("pill_dismiss", { pillId: pill_id }),
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
