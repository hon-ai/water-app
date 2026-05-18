import { useEffect, useRef } from "react";
import { X } from "lucide-react";

interface Props {
  open: boolean;
  onClose: () => void;
  title: string;
  children: React.ReactNode;
}

export function Sheet({ open, onClose, title, children }: Props) {
  const dialogRef = useRef<HTMLDialogElement | null>(null);

  useEffect(() => {
    const el = dialogRef.current;
    if (!el) return;
    if (open && !el.open) el.showModal();
    if (!open && el.open) el.close();
  }, [open]);

  useEffect(() => {
    const el = dialogRef.current;
    if (!el) return;
    const handleCancel = (e: Event) => {
      e.preventDefault();
      onClose();
    };
    el.addEventListener("cancel", handleCancel);
    return () => el.removeEventListener("cancel", handleCancel);
  }, [onClose]);

  return (
    <dialog
      ref={dialogRef}
      onClick={(e) => {
        // Click outside the inner content closes the sheet.
        if (e.target === e.currentTarget) onClose();
      }}
      style={{
        margin: 0,
        marginLeft: "auto",
        height: "100vh",
        width: "min(420px, 90vw)",
        maxHeight: "100vh",
        padding: 0,
        border: "none",
        background: "var(--water-bg-paper)",
        color: "var(--water-fg-default)",
        boxShadow: "-12px 0 32px color-mix(in srgb, var(--water-fg-default) 8%, transparent)",
      }}
    >
      <header
        style={{
          display: "flex",
          alignItems: "center",
          gap: 12,
          padding: "16px 20px",
          borderBottom: "1px solid color-mix(in srgb, var(--water-fg-faint) 20%, transparent)",
        }}
      >
        <h2
          style={{
            margin: 0,
            flex: 1,
            fontFamily: "var(--water-font-serif)",
            fontSize: "var(--water-fs-title)",
            lineHeight: "var(--water-lh-title)",
            fontWeight: 500,
          }}
        >
          {title}
        </h2>
        <button
          type="button"
          aria-label="Close"
          onClick={onClose}
          style={{
            width: 32,
            height: 32,
            display: "grid",
            placeItems: "center",
            border: "none",
            background: "transparent",
            color: "var(--water-fg-muted)",
            cursor: "pointer",
            borderRadius: "var(--water-r-8)",
          }}
        >
          <X size={16} strokeWidth={1.5} />
        </button>
      </header>
      <div style={{ padding: 20, overflowY: "auto", height: "calc(100vh - 64px)" }}>{children}</div>
    </dialog>
  );
}
