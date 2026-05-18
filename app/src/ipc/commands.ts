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
};
