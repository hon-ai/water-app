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

export interface DiagnosticsStatus {
  has_open_project: boolean;
  project_root: string | null;
  providers: string[];
  router_configured: boolean;
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

  providerTest: (providerId: string): Promise<string[]> =>
    invoke("provider_test", { providerId }),
  providerSetKey: (providerId: string, key: string): Promise<void> =>
    invoke("provider_set_key", { providerId, key }),

  diagnosticsStatus: (): Promise<DiagnosticsStatus> =>
    invoke("diagnostics_status"),
};
