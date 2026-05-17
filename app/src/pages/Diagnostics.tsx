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

  return (
    <div style={{ padding: 24, fontFamily: "JetBrains Mono, ui-monospace, monospace" }}>
      <h2>diagnostics</h2>
      <pre style={{ background: "var(--water-bg-canvas)", padding: 12, borderRadius: "var(--water-r-16)" }}>
        {JSON.stringify(status, null, 2)}
      </pre>
      <h3>provider test</h3>
      <p>
        provider:{" "}
        <select value={selected} onChange={(e) => setSelected(e.target.value)}>
          {(status?.providers ?? []).map((p) => (
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
    </div>
  );
}
