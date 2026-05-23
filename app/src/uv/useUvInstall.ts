import { useCallback, useEffect, useRef, useState } from "react";
import { ipc } from "../ipc/commands";
import { onWaterEvent } from "../ipc/events";

/**
 * Tracks the state of the in-app `uv` installer. Used by both the
 * modal prompt (shown on every reload while uv is missing) and the
 * Settings Sheet's "Python sidecar" row.
 *
 * Lifecycle:
 *   - `idle` on mount; `installed: null` until the initial probe
 *     resolves, then `true`/`false`.
 *   - `installing` after `install()` resolves and stays until the
 *     `uv:install:done` event arrives.
 *   - `done` (success) drops the "Restart Water" affordance into the
 *     caller's UI — the host can decide whether to render it.
 *   - `failed` carries a short error sentence.
 *
 * Log lines are accumulated unbounded for the duration of the
 * session; they're cheap (the installer emits dozens, not thousands)
 * and a transcript helps when a tester needs to copy-paste a failure
 * into a bug report.
 */
export type UvInstallStatus = "idle" | "installing" | "done" | "failed";

export interface UvInstallState {
  installed: boolean | null;
  path: string | null;
  status: UvInstallStatus;
  logs: Array<{ line: string; stream: "stdout" | "stderr" }>;
  error: string | null;
  install: () => Promise<void>;
  restart: () => Promise<void>;
  recheck: () => Promise<void>;
}

export function useUvInstall(): UvInstallState {
  const [installed, setInstalled] = useState<boolean | null>(null);
  const [path, setPath] = useState<string | null>(null);
  const [status, setStatus] = useState<UvInstallStatus>("idle");
  const [logs, setLogs] = useState<
    Array<{ line: string; stream: "stdout" | "stderr" }>
  >([]);
  const [error, setError] = useState<string | null>(null);

  // Hold a stable ref so the event listeners don't need to be
  // re-bound when `setLogs` identity changes — listeners are
  // registered once on mount.
  const mountedRef = useRef(true);
  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
    };
  }, []);

  // Initial probe.
  useEffect(() => {
    void (async () => {
      try {
        const s = await ipc.checkUvInstalled();
        if (!mountedRef.current) return;
        setInstalled(s.installed);
        setPath(s.path);
      } catch {
        if (!mountedRef.current) return;
        setInstalled(false);
        setPath(null);
      }
    })();
  }, []);

  // Subscribe to install stream + terminal event. We catch the
  // listen rejection because in test environments (jsdom) the Tauri
  // IPC bridge isn't installed and `onWaterEvent` rejects — leaving
  // it uncaught poisons every test that mounts something using
  // this hook.
  useEffect(() => {
    const safe = <K extends Parameters<typeof onWaterEvent>[0]>(
      name: K,
      cb: Parameters<typeof onWaterEvent<K>>[1],
    ) =>
      onWaterEvent(name, cb).catch(() => () => {
        /* no-op unlisten */
      });
    const pLog = safe("uv:install:log", (p) => {
      if (!mountedRef.current) return;
      setLogs((prev) => [...prev, { line: p.line, stream: p.stream }]);
    });
    const pDone = safe("uv:install:done", (p) => {
      if (!mountedRef.current) return;
      if (p.success) {
        setStatus("done");
        setInstalled(true);
        setPath(p.path);
        setError(null);
      } else {
        setStatus("failed");
        setError(p.error ?? "Installer failed");
      }
    });
    return () => {
      void pLog.then((un) => un());
      void pDone.then((un) => un());
    };
  }, []);

  const install = useCallback(async () => {
    setStatus("installing");
    setLogs([]);
    setError(null);
    try {
      await ipc.installUv();
    } catch (e) {
      setStatus("failed");
      setError(String(e));
    }
  }, []);

  const restart = useCallback(async () => {
    try {
      await ipc.restartApp();
    } catch {
      // The IPC call may never resolve (the process terminates) so
      // swallow the rejection to avoid an unhandled-promise warning.
    }
  }, []);

  const recheck = useCallback(async () => {
    try {
      const s = await ipc.checkUvInstalled();
      setInstalled(s.installed);
      setPath(s.path);
    } catch {
      /* silent — the writer can hit the button again */
    }
  }, []);

  return {
    installed,
    path,
    status,
    logs,
    error,
    install,
    restart,
    recheck,
  };
}
