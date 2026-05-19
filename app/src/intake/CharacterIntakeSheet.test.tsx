import { beforeEach, describe, expect, it, vi } from "vitest";

// jsdom does not implement HTMLDialogElement; stub the minimum surface
// our Sheet primitive uses. Mirrors Sheet.test.tsx.
beforeEach(() => {
  if (!HTMLDialogElement.prototype.showModal) {
    HTMLDialogElement.prototype.showModal = function () {
      (this as unknown as { open: boolean }).open = true;
    };
  }
  if (!HTMLDialogElement.prototype.close) {
    HTMLDialogElement.prototype.close = function () {
      (this as unknown as { open: boolean }).open = false;
    };
  }
});

// Mock the IPC singleton. Factory is self-contained so vitest hoisting is
// safe — no closure references to outer variables.
vi.mock("../ipc/commands", () => ({
  ipc: {
    intakeSchema: vi.fn(),
    characterRead: vi.fn(),
    characterUpdateField: vi.fn(),
  },
}));

import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { CharacterIntakeSheet } from "./CharacterIntakeSheet";
import { ipc } from "../ipc/commands";
import type {
  CharacterFile,
  CharacterIndexEntry,
  IntakeSchemaSection,
} from "../ipc/commands";

const mockSchema: IntakeSchemaSection[] = [
  {
    section: "main",
    fields: [
      {
        id: "main.full_name",
        section: "main",
        label: "Full name",
        prompt_question: "Full name?",
        helper: null,
        examples: [],
        kind: { type: "short_text" },
        optional_skip: false,
      },
    ],
  },
];

const mockFile: CharacterFile = {
  id: "c1",
  name: "",
  schema_version: "lsm-v2.1",
  // `main` lives at the top level because the Rust side flattens via
  // `#[serde(flatten)]`. Leaf is empty so the resume index starts at 0.
  main: { full_name: "" },
};

const mockIndexEntry: CharacterIndexEntry = {
  id: "c1",
  full_name: "Marcus Vale",
  role: null,
  hue_token: "character-1",
  completion: 100,
};

function mockIpcDefaults() {
  (ipc.intakeSchema as ReturnType<typeof vi.fn>).mockReset();
  (ipc.characterRead as ReturnType<typeof vi.fn>).mockReset();
  (ipc.characterUpdateField as ReturnType<typeof vi.fn>).mockReset();
  (ipc.intakeSchema as ReturnType<typeof vi.fn>).mockResolvedValue(mockSchema);
  (ipc.characterRead as ReturnType<typeof vi.fn>).mockResolvedValue(mockFile);
  (ipc.characterUpdateField as ReturnType<typeof vi.fn>).mockResolvedValue(
    mockIndexEntry,
  );
}

describe("CharacterIntakeSheet", () => {
  it("loads schema + values on open", async () => {
    mockIpcDefaults();
    render(
      <CharacterIntakeSheet
        characterId="c1"
        open={true}
        onClose={vi.fn()}
        onCompleted={vi.fn()}
      />,
    );
    await screen.findByRole("heading", { name: /full name\?/i });
    expect(ipc.intakeSchema).toHaveBeenCalledWith("lsm-v2.1");
    expect(ipc.characterRead).toHaveBeenCalledWith("c1");
  });

  it("calls characterUpdateField per answer", async () => {
    mockIpcDefaults();
    render(
      <CharacterIntakeSheet
        characterId="c1"
        open={true}
        onClose={vi.fn()}
        onCompleted={vi.fn()}
      />,
    );
    const input = await screen.findByRole("textbox");
    fireEvent.change(input, { target: { value: "Marcus Vale" } });
    fireEvent.keyDown(input, { key: "Enter" });
    await waitFor(() => {
      // Dotted-path fieldId — matches IntakeField.id, which is what
      // ConversationalIntake forwards through onAnswer.
      expect(ipc.characterUpdateField).toHaveBeenCalledWith(
        "c1",
        "main.full_name",
        "Marcus Vale",
      );
    });
  });

  it("calls onCompleted and onClose after the last field", async () => {
    mockIpcDefaults();
    const onCompleted = vi.fn();
    const onClose = vi.fn();
    render(
      <CharacterIntakeSheet
        characterId="c1"
        open={true}
        onClose={onClose}
        onCompleted={onCompleted}
      />,
    );
    const input = await screen.findByRole("textbox");
    fireEvent.change(input, { target: { value: "Marcus Vale" } });
    fireEvent.keyDown(input, { key: "Enter" });
    await waitFor(() => {
      expect(onCompleted).toHaveBeenCalled();
      expect(onClose).toHaveBeenCalled();
    });
  });

  it("does not race when characterId changes mid-load", async () => {
    mockIpcDefaults();
    // Make the first characterRead hang; the second resolves immediately
    // with a different character. If the cancellation guard is missing,
    // c1's stale read would overwrite c2's already-mounted state.
    let resolveFirst: ((file: CharacterFile) => void) | null = null;
    const firstPromise = new Promise<CharacterFile>((resolve) => {
      resolveFirst = resolve;
    });
    // c2's full_name must be empty so the intake renders the input
    // (otherwise it auto-completes on mount with a single-field schema).
    const c2File: CharacterFile = {
      id: "c2",
      name: "",
      schema_version: "lsm-v2.1",
      main: { full_name: "" },
    };
    (ipc.characterRead as ReturnType<typeof vi.fn>)
      .mockReset()
      .mockImplementationOnce(() => firstPromise)
      .mockResolvedValueOnce(c2File);
    // intakeSchema is called for both mounts; keep it resolved.
    (ipc.intakeSchema as ReturnType<typeof vi.fn>).mockResolvedValue(
      mockSchema,
    );

    const { rerender } = render(
      <CharacterIntakeSheet
        characterId="c1"
        open={true}
        onClose={vi.fn()}
        onCompleted={vi.fn()}
      />,
    );
    // Switch to c2 BEFORE c1's read resolves.
    rerender(
      <CharacterIntakeSheet
        characterId="c2"
        open={true}
        onClose={vi.fn()}
        onCompleted={vi.fn()}
      />,
    );

    // Wait for c2 to land on screen. Empty input means the cancellation
    // guard correctly dropped c1's pending load and used c2's values.
    await waitFor(() => {
      expect(screen.getByRole("textbox")).toHaveValue("");
    });

    // Now resolve c1's stale read with a populated full_name. If the
    // cancellation guard is missing, c1's data would clobber c2's
    // already-rendered (empty) state, and the input would now read
    // "Marcus".
    resolveFirst!({
      id: "c1",
      name: "Marcus",
      schema_version: "lsm-v2.1",
      main: { full_name: "Marcus" },
    });

    // Give the microtask queue a chance to process the late resolve.
    await new Promise((r) => setTimeout(r, 10));
    expect(screen.getByRole("textbox")).toHaveValue("");
  });

  // NOTE: a write-path race regression test ("characterId changes
  // mid-write") was attempted but is structurally hard to surface as
  // an observable failure: the poisoned `values` state never reaches
  // `ConversationalIntake`'s display layer (T15's `confirmedValues`
  // takes over after mount, and a sheet re-open re-reads from disk).
  // The `characterIdRef`-based guard in the component (M3 T16
  // reviewer finding I-1) is defensive against future code paths
  // that DO observe the parent's `values` after mount. Phase G can
  // revisit if a more direct assertion path emerges.
});
