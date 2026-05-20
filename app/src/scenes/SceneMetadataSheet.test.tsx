import { beforeEach, describe, expect, it, vi } from "vitest";

// jsdom does not implement HTMLDialogElement; stub the surface our Sheet
// primitive uses (mirrors Sheet.test.tsx + CharacterIntakeSheet.test.tsx).
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

// Mock the singleton `ipc` object. The factory must be self-contained
// (no closure references) so vitest hoisting is safe.
vi.mock("../ipc/commands", () => ({
  ipc: {
    characterList: vi.fn(),
    sceneReadMetadata: vi.fn(),
    characterLinkToScene: vi.fn(),
    characterUnlinkFromScene: vi.fn(),
    characterSetPov: vi.fn(),
    worldSegmentList: vi.fn(),
    worldEntryList: vi.fn(),
    sceneSetLocation: vi.fn(),
  },
}));

import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { SceneMetadataSheet } from "./SceneMetadataSheet";
import { ipc } from "../ipc/commands";
import type { CharacterIndexEntry, SceneMetadata } from "../ipc/commands";
import { publishAutosuggest } from "./sceneMetadataChannel";

const mockChars: CharacterIndexEntry[] = [
  {
    id: "c1",
    full_name: "Marcus",
    role: null,
    hue_token: "--water-hue-character-1",
    completion: 80,
  },
  {
    id: "c2",
    full_name: "Talia",
    role: null,
    hue_token: "--water-hue-character-2",
    completion: 80,
  },
];

// Default scene: Marcus is present + POV; Talia is unlinked. `location`
// is `null` here — M4 T11 added the field as a required-but-nullable
// property of `SceneMetadata`; this default fixture doesn't exercise
// the location pill so `null` is fine.
const mockMeta: SceneMetadata = {
  characters_present: ["c1"],
  pov_character_id: "c1",
  location: null,
};

function mockIpcDefaults() {
  (ipc.characterList as ReturnType<typeof vi.fn>).mockReset();
  (ipc.sceneReadMetadata as ReturnType<typeof vi.fn>).mockReset();
  (ipc.characterLinkToScene as ReturnType<typeof vi.fn>).mockReset();
  (ipc.characterUnlinkFromScene as ReturnType<typeof vi.fn>).mockReset();
  (ipc.characterSetPov as ReturnType<typeof vi.fn>).mockReset();
  (ipc.worldSegmentList as ReturnType<typeof vi.fn>).mockReset();
  (ipc.worldEntryList as ReturnType<typeof vi.fn>).mockReset();
  (ipc.sceneSetLocation as ReturnType<typeof vi.fn>).mockReset();
  (ipc.characterList as ReturnType<typeof vi.fn>).mockResolvedValue(mockChars);
  (ipc.sceneReadMetadata as ReturnType<typeof vi.fn>).mockResolvedValue(
    mockMeta,
  );
  (ipc.characterLinkToScene as ReturnType<typeof vi.fn>).mockResolvedValue(
    undefined,
  );
  (ipc.characterUnlinkFromScene as ReturnType<typeof vi.fn>).mockResolvedValue(
    undefined,
  );
  (ipc.characterSetPov as ReturnType<typeof vi.fn>).mockResolvedValue(
    undefined,
  );
  // Default world fixtures: a single `locations` segment with two entries.
  // Tests that need a "no locations segment" or empty-entry case override.
  (ipc.worldSegmentList as ReturnType<typeof vi.fn>).mockResolvedValue([
    {
      id: "seg-loc",
      slug: "locations",
      name: "Locations",
      ordering: 1,
      is_collection: true,
      hue_token: "--water-hue-world-1",
      hidden: false,
      has_template_override: false,
    },
    {
      id: "seg-concept",
      slug: "concept",
      name: "Concept",
      ordering: 0,
      is_collection: false,
      hue_token: "--water-hue-world-2",
      hidden: false,
      has_template_override: false,
    },
  ]);
  (ipc.worldEntryList as ReturnType<typeof vi.fn>).mockResolvedValue([
    { id: "loc-1", segment_id: "seg-loc", name: "The Pell Library", preview: "" },
    { id: "loc-2", segment_id: "seg-loc", name: "Aren Square", preview: "" },
  ]);
  (ipc.sceneSetLocation as ReturnType<typeof vi.fn>).mockResolvedValue(undefined);
}

describe("SceneMetadataSheet", () => {
  it("loads scene meta and characters on open", async () => {
    mockIpcDefaults();
    render(
      <SceneMetadataSheet sceneId="s1" open={true} onClose={vi.fn()} />,
    );
    // Wait for the load to land — at first render only "Loading…" shows.
    const marcusCheckbox = (await screen.findByLabelText(
      "Marcus",
    )) as HTMLInputElement;
    const taliaCheckbox = screen.getByLabelText("Talia") as HTMLInputElement;
    expect(marcusCheckbox.checked).toBe(true);
    expect(taliaCheckbox.checked).toBe(false);
    expect(ipc.characterList).toHaveBeenCalled();
    expect(ipc.sceneReadMetadata).toHaveBeenCalledWith("s1");
  });

  it("toggling an unlinked checkbox calls characterLinkToScene", async () => {
    mockIpcDefaults();
    render(
      <SceneMetadataSheet sceneId="s1" open={true} onClose={vi.fn()} />,
    );
    const taliaBox = await screen.findByLabelText("Talia");
    fireEvent.click(taliaBox);
    await waitFor(() => {
      expect(ipc.characterLinkToScene).toHaveBeenCalledWith("s1", "c2");
    });
    // Toggling unlinked should NEVER call unlink.
    expect(ipc.characterUnlinkFromScene).not.toHaveBeenCalled();
  });

  it("toggling a linked checkbox calls characterUnlinkFromScene", async () => {
    mockIpcDefaults();
    render(
      <SceneMetadataSheet sceneId="s1" open={true} onClose={vi.fn()} />,
    );
    const marcusBox = await screen.findByLabelText("Marcus");
    fireEvent.click(marcusBox);
    await waitFor(() => {
      expect(ipc.characterUnlinkFromScene).toHaveBeenCalledWith("s1", "c1");
    });
    expect(ipc.characterLinkToScene).not.toHaveBeenCalled();
  });

  it("POV select only shows linked characters", async () => {
    mockIpcDefaults();
    render(
      <SceneMetadataSheet sceneId="s1" open={true} onClose={vi.fn()} />,
    );
    await screen.findByLabelText("Marcus");
    const select = screen.getByLabelText("POV character") as HTMLSelectElement;
    const options = Array.from(select.querySelectorAll("option")).map(
      (o) => o.textContent,
    );
    // "— none —" plus only linked characters.
    expect(options).toContain("Marcus"); // linked
    expect(options).not.toContain("Talia"); // not linked
  });

  it("displays autosuggest chips after publish, hiding already-linked", async () => {
    mockIpcDefaults();
    render(
      <SceneMetadataSheet sceneId="s1" open={true} onClose={vi.fn()} />,
    );
    await screen.findByLabelText("Marcus");
    // Publish suggestions for both characters; Marcus is already linked
    // and must be filtered out, Talia should show.
    publishAutosuggest("s1", [
      { character_id: "c1", full_name: "Marcus", mention_count: 5 },
      { character_id: "c2", full_name: "Talia", mention_count: 3 },
    ]);
    await screen.findByText(/Talia \(×3\)/);
    expect(screen.queryByText(/Marcus \(×5\)/)).not.toBeInTheDocument();
  });

  it("dismissing a chip hides it but does not call unlink", async () => {
    mockIpcDefaults();
    render(
      <SceneMetadataSheet sceneId="s1" open={true} onClose={vi.fn()} />,
    );
    await screen.findByLabelText("Marcus");
    publishAutosuggest("s1", [
      { character_id: "c2", full_name: "Talia", mention_count: 3 },
    ]);
    const dismissBtn = await screen.findByRole("button", {
      name: /Dismiss Talia/,
    });
    fireEvent.click(dismissBtn);
    await waitFor(() => {
      expect(screen.queryByText(/Talia \(×3\)/)).not.toBeInTheDocument();
    });
    // Dismiss is a local UI action — it MUST NOT touch the IPC surface.
    expect(ipc.characterUnlinkFromScene).not.toHaveBeenCalled();
    expect(ipc.characterLinkToScene).not.toHaveBeenCalled();
  });

  it("clicking a chip's primary button links the character and reloads", async () => {
    mockIpcDefaults();
    render(
      <SceneMetadataSheet sceneId="s1" open={true} onClose={vi.fn()} />,
    );
    await screen.findByLabelText("Marcus");
    publishAutosuggest("s1", [
      { character_id: "c2", full_name: "Talia", mention_count: 3 },
    ]);
    const chipBtn = await screen.findByRole("button", {
      name: /Talia \(×3\)/,
    });
    // After link, the sheet calls reload() — make the next
    // sceneReadMetadata reflect Talia being added so we can observe the
    // chip disappearing (because she's now in `alreadyLinkedIds`).
    (ipc.sceneReadMetadata as ReturnType<typeof vi.fn>).mockResolvedValueOnce({
      characters_present: ["c1", "c2"],
      pov_character_id: "c1",
      location: null,
    } satisfies SceneMetadata);
    fireEvent.click(chipBtn);
    await waitFor(() => {
      expect(ipc.characterLinkToScene).toHaveBeenCalledWith("s1", "c2");
    });
  });

  it("renders the location dropdown populated from the locations segment", async () => {
    mockIpcDefaults();
    render(
      <SceneMetadataSheet sceneId="s1" open={true} onClose={vi.fn()} />,
    );
    const select = (await screen.findByLabelText(
      "Location",
    )) as HTMLSelectElement;
    const options = Array.from(select.querySelectorAll("option")).map(
      (o) => o.textContent,
    );
    expect(options).toContain("— none —");
    expect(options).toContain("The Pell Library");
    expect(options).toContain("Aren Square");
    // The dropdown must source from the `locations` segment, not any
    // single-doc segment — `worldEntryList` must be called with seg-loc.
    expect(ipc.worldEntryList).toHaveBeenCalledWith("seg-loc");
    expect(ipc.worldEntryList).not.toHaveBeenCalledWith("seg-concept");
  });

  it("selecting a location calls sceneSetLocation and reloads", async () => {
    mockIpcDefaults();
    render(
      <SceneMetadataSheet sceneId="s1" open={true} onClose={vi.fn()} />,
    );
    const select = (await screen.findByLabelText(
      "Location",
    )) as HTMLSelectElement;
    fireEvent.change(select, { target: { value: "loc-1" } });
    await waitFor(() => {
      expect(ipc.sceneSetLocation).toHaveBeenCalledWith({
        sceneId: "s1",
        locationId: "loc-1",
      });
    });
    // After the set, the sheet reloads metadata.
    await waitFor(() => {
      expect(ipc.sceneReadMetadata).toHaveBeenCalledTimes(2);
    });
  });

  it("clearing the selection calls sceneSetLocation with null", async () => {
    mockIpcDefaults();
    (ipc.sceneReadMetadata as ReturnType<typeof vi.fn>).mockResolvedValue({
      characters_present: ["c1"],
      pov_character_id: "c1",
      location: { id: "loc-1", name: "The Pell Library", segment_slug: "locations" },
    } satisfies SceneMetadata);
    render(
      <SceneMetadataSheet sceneId="s1" open={true} onClose={vi.fn()} />,
    );
    const select = (await screen.findByLabelText(
      "Location",
    )) as HTMLSelectElement;
    // Confirm it loaded with loc-1 selected.
    expect(select.value).toBe("loc-1");
    fireEvent.change(select, { target: { value: "" } });
    await waitFor(() => {
      expect(ipc.sceneSetLocation).toHaveBeenCalledWith({
        sceneId: "s1",
        locationId: null,
      });
    });
  });

  it("hides the location selector when there is no locations segment", async () => {
    mockIpcDefaults();
    (ipc.worldSegmentList as ReturnType<typeof vi.fn>).mockResolvedValue([
      {
        id: "seg-concept",
        slug: "concept",
        name: "Concept",
        ordering: 0,
        is_collection: false,
        hue_token: "--water-hue-world-2",
        hidden: false,
        has_template_override: false,
      },
    ]);
    render(
      <SceneMetadataSheet sceneId="s1" open={true} onClose={vi.fn()} />,
    );
    await screen.findByLabelText("Marcus");
    expect(screen.queryByLabelText("Location")).not.toBeInTheDocument();
    // We never asked for entries because there's no locations segment to ask about.
    expect(ipc.worldEntryList).not.toHaveBeenCalled();
  });

  it("ignores autosuggest publishes for a different sceneId", async () => {
    mockIpcDefaults();
    render(
      <SceneMetadataSheet sceneId="s1" open={true} onClose={vi.fn()} />,
    );
    await screen.findByLabelText("Marcus");
    // Publish for a different scene — the sheet must filter by sceneId.
    publishAutosuggest("s-other", [
      { character_id: "c2", full_name: "Talia", mention_count: 3 },
    ]);
    // Give the channel + microtask queue a chance.
    await new Promise((r) => setTimeout(r, 10));
    expect(screen.queryByText(/Talia \(×3\)/)).not.toBeInTheDocument();
  });
});
