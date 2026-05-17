import { useCallback, useEffect, useState } from "react";
import { ipc, type DiagnosticsStatus } from "../ipc/commands";

export function Diagnostics() {
  const [status, setStatus] = useState<DiagnosticsStatus | null>(null);
  const [selected, setSelected] = useState("canned");
  const [variants, setVariants] = useState<string[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    const s = await ipc.diagnosticsStatus();
    setStatus(s);
  }, []);

  useEffect(() => {
    refresh().catch(() => {});
    const t = window.setInterval(() => {
      refresh().catch(() => {});
    }, 3000);
    return () => window.clearInterval(t);
  }, [refresh]);

  const test = async () => {
    setError(null);
    setVariants(null);
    try {
      const v = await ipc.providerTest(selected);
      setVariants(v);
    } catch (e) {
      setError(String(e));
    }
    refresh();
  };

  const providerIds = (status?.provider_health ?? []).map((p) => p.id);
  const fallbackIds = ["canned", "anthropic", "openai", "ollama", "llamacpp"];
  const dropdownIds = providerIds.length > 0 ? providerIds : fallbackIds;

  return (
    <div
      style={{
        padding: 24,
        fontFamily: "JetBrains Mono, ui-monospace, monospace",
        display: "flex",
        flexDirection: "column",
        gap: 16,
      }}
    >
      <h2>diagnostics</h2>

      <section>
        <h3>project</h3>
        <div>open: {status?.has_open_project ? "yes" : "no"}</div>
        <div>root: {status?.project_root ?? "—"}</div>
      </section>

      <section>
        <h3>sidecar</h3>
        {status?.sidecar ? (
          <>
            <div>base_url: {status.sidecar.base_url}</div>
            <div>status: {status.sidecar.status}</div>
            {status.sidecar.last_status_detail && (
              <div>detail: {status.sidecar.last_status_detail}</div>
            )}
          </>
        ) : (
          <div>not running</div>
        )}
      </section>

      <section>
        <h3>router</h3>
        <div>primary: {status?.router_primary_id ?? "(none)"}</div>
        {status?.provider_health && status.provider_health.length > 0 && (
          <ul>
            {status.provider_health.map((p) => (
              <li key={p.id}>
                {p.id}: {p.ok ? "ok" : `fail (${p.error ?? "unknown"})`}
              </li>
            ))}
          </ul>
        )}
      </section>

      <section>
        <h3>provider test</h3>
        <p>
          provider:{" "}
          <select value={selected} onChange={(e) => setSelected(e.target.value)}>
            {dropdownIds.map((p) => (
              <option key={p}>{p}</option>
            ))}
          </select>{" "}
          <button onClick={test}>test round-trip</button>
        </p>
        {error && <pre style={{ color: "var(--water-hue-drift)" }}>{error}</pre>}
        {variants && (
          <ul>
            {variants.map((v, i) => (
              <li key={i}>{v}</li>
            ))}
          </ul>
        )}
      </section>

      <details>
        <summary>raw json</summary>
        <pre
          style={{
            background: "var(--water-bg-canvas)",
            padding: 12,
            borderRadius: "var(--water-r-16)",
          }}
        >
          {JSON.stringify(status, null, 2)}
        </pre>
      </details>
    </div>
  );
}
