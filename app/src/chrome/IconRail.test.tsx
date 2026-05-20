import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { IconRail } from "./IconRail";

describe("IconRail", () => {
  it("renders all nav items when a project is open", () => {
    render(
      <IconRail
        active="scenes"
        onSelect={() => {}}
        onOpenSettings={() => {}}
        projectOpen={true}
      />,
    );
    // app mark is not interactive — assert the other four are buttons
    expect(screen.getByRole("button", { name: /scenes/i })).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /characters/i }),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /world/i })).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /settings/i }),
    ).toBeInTheDocument();
  });

  it("hides scenes/characters/world when no project is open, keeps Settings", () => {
    render(
      <IconRail
        active="scenes"
        onSelect={() => {}}
        onOpenSettings={() => {}}
        projectOpen={false}
      />,
    );
    expect(
      screen.queryByRole("button", { name: /scenes/i }),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /characters/i }),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /world/i }),
    ).not.toBeInTheDocument();
    // Settings stays — it's accessible whether or not a project is open.
    expect(
      screen.getByRole("button", { name: /settings/i }),
    ).toBeInTheDocument();
  });

  it("marks the active nav with data-active=true", () => {
    render(
      <IconRail
        active="characters"
        onSelect={() => {}}
        onOpenSettings={() => {}}
        projectOpen={true}
      />,
    );
    expect(
      screen.getByRole("button", { name: /characters/i }),
    ).toHaveAttribute("data-active", "true");
    expect(screen.getByRole("button", { name: /scenes/i })).toHaveAttribute(
      "data-active",
      "false",
    );
  });

  it("fires onSelect with the right target id", () => {
    const onSelect = vi.fn();
    render(
      <IconRail
        active="scenes"
        onSelect={onSelect}
        onOpenSettings={() => {}}
        projectOpen={true}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: /characters/i }));
    expect(onSelect).toHaveBeenCalledWith("characters");
  });

  it("fires onOpenSettings when the gear is clicked", () => {
    const onOpenSettings = vi.fn();
    render(
      <IconRail
        active="scenes"
        onSelect={() => {}}
        onOpenSettings={onOpenSettings}
        projectOpen={true}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: /settings/i }));
    expect(onOpenSettings).toHaveBeenCalledOnce();
  });
});
