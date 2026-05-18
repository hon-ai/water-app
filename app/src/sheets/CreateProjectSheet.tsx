import { useState } from "react";
import { Folder } from "lucide-react";
import { Sheet } from "./Sheet";
import { ipc, type OpenProjectInfo } from "../ipc/commands";
import { dialog } from "../ipc/dialog";

interface Props {
  open: boolean;
  onClose: () => void;
  onCreated: (info: OpenProjectInfo) => void;
}

export function CreateProjectSheet({ open, onClose, onCreated }: Props) {
  const [name, setName] = useState("");
  const [parentDir, setParentDir] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleBrowse = async () => {
    const picked = await dialog.pickFolder();
    if (picked) setParentDir(picked);
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    setBusy(true);
    try {
      const info = await ipc.createProject(parentDir, name);
      onCreated(info);
      setName("");
      setParentDir("");
      onClose();
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  };

  const fieldLabel: React.CSSProperties = {
    display: "block",
    fontSize: "var(--water-fs-meta)",
    color: "var(--water-fg-muted)",
    marginBottom: 4,
    fontFamily: "var(--water-font-sans)",
  };
  const fieldInput: React.CSSProperties = {
    width: "100%",
    padding: "8px 12px",
    border: "none",
    borderRadius: "var(--water-r-8)",
    background: "var(--water-bg-canvas)",
    color: "var(--water-fg-default)",
    fontFamily: "var(--water-font-sans)",
    fontSize: "var(--water-fs-body)",
    outline: "none",
  };

  return (
    <Sheet open={open} onClose={onClose} title="Create new project">
      <form onSubmit={handleSubmit} style={{ display: "flex", flexDirection: "column", gap: 16 }}>
        <div>
          <label htmlFor="cp-name" style={fieldLabel}>
            Project name
          </label>
          <input
            id="cp-name"
            type="text"
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="e.g. Acceptance Test"
            required
            style={fieldInput}
          />
        </div>
        <div>
          <label htmlFor="cp-parent" style={fieldLabel}>
            Parent directory
          </label>
          <div style={{ display: "flex", gap: 8 }}>
            <input
              id="cp-parent"
              type="text"
              value={parentDir}
              onChange={(e) => setParentDir(e.target.value)}
              placeholder="C:\\Users\\you\\Desktop"
              required
              style={{ ...fieldInput, flex: 1 }}
            />
            <button
              type="button"
              onClick={handleBrowse}
              aria-label="Browse for folder"
              style={{
                padding: "8px 12px",
                display: "flex",
                alignItems: "center",
                gap: 6,
                border: "none",
                background: "transparent",
                color: "var(--water-fg-default)",
                cursor: "pointer",
                borderRadius: "var(--water-r-8)",
                boxShadow:
                  "inset 0 0 0 1px color-mix(in srgb, var(--water-fg-faint) 30%, transparent)",
                fontFamily: "var(--water-font-sans)",
                fontSize: "var(--water-fs-ui)",
              }}
            >
              <Folder size={14} strokeWidth={1.5} />
              Browse…
            </button>
          </div>
        </div>
        {error && (
          <pre style={{ color: "var(--water-hue-drift)", margin: 0, fontSize: "var(--water-fs-meta)" }}>
            {error}
          </pre>
        )}
        <div style={{ display: "flex", justifyContent: "flex-end", gap: 8, marginTop: 8 }}>
          <button
            type="button"
            onClick={onClose}
            style={{
              padding: "8px 14px",
              border: "none",
              background: "transparent",
              color: "var(--water-fg-muted)",
              cursor: "pointer",
              borderRadius: "var(--water-r-8)",
              fontFamily: "var(--water-font-sans)",
              fontSize: "var(--water-fs-ui)",
            }}
          >
            Cancel
          </button>
          <button
            type="submit"
            disabled={busy || !name.trim() || !parentDir.trim()}
            style={{
              padding: "8px 16px",
              border: "none",
              background: "color-mix(in srgb, var(--water-hue-flow) 60%, transparent)",
              color: "var(--water-fg-default)",
              cursor: "pointer",
              borderRadius: "var(--water-r-8)",
              fontFamily: "var(--water-font-sans)",
              fontSize: "var(--water-fs-ui)",
              opacity: busy || !name.trim() || !parentDir.trim() ? 0.5 : 1,
            }}
          >
            {busy ? "Creating…" : "Create"}
          </button>
        </div>
      </form>
    </Sheet>
  );
}
