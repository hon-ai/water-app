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
  pillPin: (pill_id: string): Promise<void> =>
    invoke("pill_pin", { pillId: pill_id }),
  pillDismiss: (pill_id: string): Promise<void> =>
    invoke("pill_dismiss", { pillId: pill_id }),
};
