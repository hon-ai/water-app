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
    fontSize: 11,
    fontWeight: 600,
    textTransform: "uppercase",
    letterSpacing: 0.6,
    color: "var(--water-fg-muted)",
    marginBottom: 6,
    fontFamily: "var(--water-font-sans)",
  };
  const fieldInput: React.CSSProperties = {
    width: "100%",
    padding: "10px 12px",
    border: "1px solid color-mix(in srgb, var(--water-fg-faint) 22%, transparent)",
    borderRadius: "var(--water-r-8)",
    background: "var(--water-bg-canvas)",
    color: "var(--water-fg-default)",
    fontFamily: "var(--water-font-sans)",
    fontSize: "var(--water-fs-body)",
    outline: "none",
    transition:
      "border-color var(--water-dur-tiny) var(--water-ease-out-soft), box-shadow var(--water-dur-tiny) var(--water-ease-out-soft)",
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
              className="water-button"
              onClick={handleBrowse}
              aria-label="Browse for folder"
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
            className="water-button water-button-ghost"
            onClick={onClose}
          >
            Cancel
          </button>
          <button
            type="submit"
            className="water-button water-button-primary"
            disabled={busy || !name.trim() || !parentDir.trim()}
          >
            {busy ? "Creating…" : "Create"}
          </button>
        </div>
      </form>
    </Sheet>
  );
}
