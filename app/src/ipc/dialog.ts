import { open } from "@tauri-apps/plugin-dialog";

export const dialog = {
  /**
   * Open the native OS folder picker. Returns the chosen absolute path,
   * or null if the user cancelled or the plugin returned an unexpected shape.
   */
  pickFolder: async (): Promise<string | null> => {
    const result = await open({ directory: true, multiple: false });
    return typeof result === "string" ? result : null;
  },
};
