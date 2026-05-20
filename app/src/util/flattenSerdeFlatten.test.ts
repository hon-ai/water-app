import { describe, it, expect } from "vitest";
import { flattenSerdeFlatten } from "./flattenSerdeFlatten";

describe("flattenSerdeFlatten", () => {
  it("flattens section keys into dotted paths", () => {
    const source = {
      id: "01J",
      name: "Aren",
      schema_version: "lsm-v2.1",
      main: { full_name: "Aren Vale", role: "scribe" },
      lists: { themes: ["memory", "obligation"] },
    };
    expect(flattenSerdeFlatten(source)).toEqual({
      "main.full_name": "Aren Vale",
      "main.role": "scribe",
      "lists.themes": ["memory", "obligation"],
    });
  });

  it("skips metadata keys", () => {
    expect(flattenSerdeFlatten({ id: "X", name: "Y", schema_version: "v" })).toEqual({});
  });

  it("respects custom metadataKeys", () => {
    expect(
      flattenSerdeFlatten(
        { id: "X", custom_meta: "skip", main: { a: 1 } },
        new Set(["id", "custom_meta"]),
      ),
    ).toEqual({ "main.a": 1 });
  });

  it("skips aliases array at metadata level", () => {
    expect(
      flattenSerdeFlatten({
        id: "X",
        aliases: ["a", "b"],
        main: { type: "library" },
      }),
    ).toEqual({ "main.type": "library" });
  });
});
