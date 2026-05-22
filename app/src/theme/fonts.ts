/**
 * Curated manuscript-serif options + persistence.
 *
 * The writer picks from a short list; the choice gets applied to the
 * editor's prose surface by setting `--water-font-serif` on the
 * `<html>` element. UI sans + mono are left alone — they're product
 * chrome, not the manuscript itself.
 *
 * Persistence: `localStorage` for now (per-install, not per-project).
 * When a real `project_setting` k/v table lands the persistence layer
 * swaps out without touching the curation list or the Settings UI.
 *
 * Custom-font import is the obvious next step; the storage key
 * doesn't bake in any assumption about which family names are
 * allowed, so a future user-imported font can write its family name
 * here without schema changes.
 */

const STORAGE_KEY = "water:manuscript-serif";
const CUSTOM_FONTS_KEY = "water:custom-fonts";
/** Hard upper bound on cumulative custom-font storage. localStorage
 *  is ~5MB per origin; we leave headroom for other Water state by
 *  rejecting imports that would push the total past this cap. */
const CUSTOM_FONTS_CAP_BYTES = 3 * 1024 * 1024;

export interface CustomFont {
  /** Stable id used in the picker / persistence. Format:
   *  `custom-<sanitized-name>-<short-suffix>` to avoid colliding
   *  with the curated `FontOption` ids. */
  id: string;
  /** Display name shown in the picker. Derived from the file name. */
  label: string;
  /** CSS `font-family` value applied to `--water-font-serif`. Same
   *  shape as the curated options so they swap interchangeably. */
  family: string;
  /** Hint shown under the picker. For custom fonts this is just
   *  "Custom font — imported by you." */
  hint: string;
  /** Base64 data URL of the font file. Persisted in localStorage so
   *  the font is restored across restarts without disk I/O. */
  dataUrl: string;
  /** Approximate byte cost of the dataUrl, for the storage cap check. */
  bytes: number;
}

export interface FontOption {
  id: string;
  /** Display name shown in the picker. */
  label: string;
  /** CSS `font-family` value applied to `--water-font-serif`. */
  family: string;
  /** A short note for the option's flavor. */
  hint: string;
}

/**
 * Curated set. All entries either alias web-safe system serifs or
 * pile fallbacks so even an OS without the named face still gets a
 * pleasant serif (`Iowan Old Style → Charter → Georgia → serif`).
 *
 * Order is "most-recommended first" so the dropdown's first paint
 * doesn't surprise.
 */
export const FONT_OPTIONS: FontOption[] = [
  {
    id: "plex-serif",
    label: "IBM Plex Serif",
    family: '"IBM Plex Serif", "Iowan Old Style", Georgia, serif',
    hint: "The Water default. Clean modern serif, light on the page.",
  },
  {
    id: "iowan",
    label: "Iowan Old Style",
    family: '"Iowan Old Style", Charter, Georgia, serif',
    hint: "Native on Apple platforms. Generous x-height; reads warm.",
  },
  {
    id: "charter",
    label: "Charter",
    family: 'Charter, "Iowan Old Style", Georgia, serif',
    hint: "Bitstream's screen-tuned serif. Bookish without weight.",
  },
  {
    id: "georgia",
    label: "Georgia",
    family: 'Georgia, Cambria, "Times New Roman", serif',
    hint: "Universally available. The neutral choice.",
  },
  {
    id: "palatino",
    label: "Palatino",
    family: '"Palatino Linotype", Palatino, "Book Antiqua", serif',
    hint: "Wide, calligraphic. For longer scenes that breathe.",
  },
  {
    id: "system-serif",
    label: "System serif",
    family: "ui-serif, serif",
    hint: "Whatever your OS prefers. Most consistent across platforms.",
  },
];

const DEFAULT_ID = "plex-serif";

export function defaultFontOption(): FontOption {
  return FONT_OPTIONS.find((o) => o.id === DEFAULT_ID) ?? FONT_OPTIONS[0]!;
}

/**
 * Apply a font family to the manuscript by overriding the
 * `--water-font-serif` custom property on `<html>`. The override
 * cascades through every place the editor uses
 * `var(--water-font-serif)` (prose typography, scene titles, etc.).
 *
 * Safe to call before the DOM is ready (no-op when `document` is
 * undefined — keeps SSR / test environments stable).
 */
export function applyFont(family: string): void {
  if (typeof document === "undefined") return;
  document.documentElement.style.setProperty("--water-font-serif", family);
}

/**
 * Read the saved choice (or the default) and apply it. Called once
 * at app mount. Also registers any persisted custom fonts with the
 * browser's font registry so they're available globally.
 */
export function loadAndApplyFont(): FontOption | CustomFont {
  // Register all custom fonts up-front so the chosen one (if it's a
  // custom font) actually resolves when we apply the family below.
  registerAllCustomFonts();
  let id = DEFAULT_ID;
  try {
    const stored = typeof localStorage !== "undefined" ? localStorage.getItem(STORAGE_KEY) : null;
    if (stored) {
      if (FONT_OPTIONS.some((o) => o.id === stored)) id = stored;
      else if (loadCustomFonts().some((c) => c.id === stored)) id = stored;
    }
  } catch {
    /* swallow — localStorage can throw in private modes */
  }
  const all = allFontOptions();
  const opt = all.find((o) => o.id === id) ?? defaultFontOption();
  applyFont(opt.family);
  return opt;
}

/**
 * Persist + apply the writer's choice. Safe against storage errors.
 * Accepts both curated `FontOption` ids and custom-font ids.
 */
export function setFont(id: string): FontOption | CustomFont {
  const opt = allFontOptions().find((o) => o.id === id) ?? defaultFontOption();
  try {
    if (typeof localStorage !== "undefined") {
      localStorage.setItem(STORAGE_KEY, opt.id);
    }
  } catch {
    /* swallow */
  }
  applyFont(opt.family);
  return opt;
}

/**
 * Read the saved font id without applying. Used by the Settings UI
 * to highlight the active choice without firing a re-paint.
 */
export function currentFontId(): string {
  try {
    const stored = typeof localStorage !== "undefined" ? localStorage.getItem(STORAGE_KEY) : null;
    if (stored) {
      if (FONT_OPTIONS.some((o) => o.id === stored)) return stored;
      // Custom font id — only valid if it's actually loaded.
      if (loadCustomFonts().some((c) => c.id === stored)) return stored;
    }
  } catch {
    /* swallow */
  }
  return DEFAULT_ID;
}

/* ─────────── Custom font import ─────────── */

/** Read the persisted custom fonts. Quietly returns [] on any
 *  parse / storage error so a corrupted entry never blocks boot. */
export function loadCustomFonts(): CustomFont[] {
  try {
    if (typeof localStorage === "undefined") return [];
    const raw = localStorage.getItem(CUSTOM_FONTS_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw) as unknown;
    if (!Array.isArray(parsed)) return [];
    return parsed.filter(
      (e): e is CustomFont =>
        typeof e === "object" &&
        e !== null &&
        typeof (e as CustomFont).id === "string" &&
        typeof (e as CustomFont).label === "string" &&
        typeof (e as CustomFont).family === "string" &&
        typeof (e as CustomFont).dataUrl === "string",
    );
  } catch {
    return [];
  }
}

/** Combined list (curated + custom) used by the picker. */
export function allFontOptions(): Array<FontOption | CustomFont> {
  return [...FONT_OPTIONS, ...loadCustomFonts()];
}

/** Register a single custom font with the browser's font registry so
 *  it's available everywhere `var(--water-font-serif)` resolves. Uses
 *  the FontFace API rather than injecting a `<style>` tag so failures
 *  are catchable + the font is dropped from the registry when removed. */
function registerFontFace(font: CustomFont): void {
  if (typeof document === "undefined" || !("fonts" in document)) return;
  // Strip outer quotes from the family for FontFace registration (the
  // FontFace constructor wants the bare family name; the `family`
  // string we store already wraps it in quotes for CSS use).
  const familyName = font.family.replace(/^["']|["']$/g, "").split(",")[0]!.trim().replace(/^["']|["']$/g, "");
  // Avoid double-registering on re-boot.
  for (const f of document.fonts) {
    if (f.family === familyName) {
      return;
    }
  }
  try {
    const face = new FontFace(familyName, `url(${font.dataUrl})`);
    document.fonts.add(face);
    void face.load();
  } catch {
    /* swallow — bad data URL, unsupported format, etc. */
  }
}

/** Boot-time: register every persisted custom font with the browser. */
export function registerAllCustomFonts(): void {
  for (const f of loadCustomFonts()) registerFontFace(f);
}

/**
 * Import a user-supplied font file. Reads the file as base64,
 * persists it to localStorage, registers the FontFace with the
 * browser, and returns the new `CustomFont` so the caller can flip
 * the picker to it.
 *
 * Throws on:
 *  - Unrecognized extension (must be `.ttf` / `.otf` / `.woff` / `.woff2`).
 *  - Adding the file would push cumulative custom-font storage past
 *    `CUSTOM_FONTS_CAP_BYTES` (rejection is cleaner than letting
 *    localStorage throw QuotaExceededError mid-write).
 *  - A font with the same family-name is already imported.
 */
export async function importCustomFont(file: File): Promise<CustomFont> {
  const ext = file.name.split(".").pop()?.toLowerCase() ?? "";
  const allowedExts = new Set(["ttf", "otf", "woff", "woff2"]);
  if (!allowedExts.has(ext)) {
    throw new Error(
      `Unsupported font format ".${ext}". Use .ttf, .otf, .woff, or .woff2.`,
    );
  }
  const baseName = file.name.replace(/\.[^.]+$/, "").trim();
  if (baseName.length === 0) throw new Error("Font file has no name.");

  // Read as data URL (base64). Fonts are small enough that this fits
  // in memory comfortably; localStorage cap is checked next.
  const dataUrl: string = await new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result ?? ""));
    reader.onerror = () => reject(reader.error ?? new Error("read failed"));
    reader.readAsDataURL(file);
  });
  if (!dataUrl.startsWith("data:")) {
    throw new Error("Could not read font file.");
  }

  const existing = loadCustomFonts();
  // Family-name collision check.
  if (existing.some((e) => e.label.toLowerCase() === baseName.toLowerCase())) {
    throw new Error(
      `A font named "${baseName}" is already imported. Rename the file and try again.`,
    );
  }
  const bytes = dataUrl.length;
  const totalAfter = existing.reduce((sum, e) => sum + (e.bytes ?? 0), 0) + bytes;
  if (totalAfter > CUSTOM_FONTS_CAP_BYTES) {
    const usedMb = (totalAfter / 1024 / 1024).toFixed(1);
    const capMb = (CUSTOM_FONTS_CAP_BYTES / 1024 / 1024).toFixed(1);
    throw new Error(
      `Importing this font would exceed the ${capMb} MB custom-font cap (currently ${usedMb} MB after import). Remove a font first.`,
    );
  }

  // Mint a stable id. Sanitize the base name to ASCII alphanumeric +
  // hyphens so it slots into localStorage keys / CSS selectors cleanly.
  const slug = baseName.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "") || "font";
  const suffix = Math.random().toString(36).slice(2, 6);
  const font: CustomFont = {
    id: `custom-${slug}-${suffix}`,
    label: baseName,
    family: `"${baseName}", "IBM Plex Serif", "Iowan Old Style", Georgia, serif`,
    hint: "Custom font — imported by you.",
    dataUrl,
    bytes,
  };

  const next = [...existing, font];
  try {
    localStorage.setItem(CUSTOM_FONTS_KEY, JSON.stringify(next));
  } catch (e) {
    throw new Error(
      `Couldn't save the font: ${e instanceof Error ? e.message : String(e)}`,
    );
  }
  registerFontFace(font);
  return font;
}

/** Remove an imported font. No-op if `id` isn't a custom font. */
export function removeCustomFont(id: string): void {
  const existing = loadCustomFonts();
  const next = existing.filter((e) => e.id !== id);
  if (next.length === existing.length) return;
  try {
    localStorage.setItem(CUSTOM_FONTS_KEY, JSON.stringify(next));
  } catch {
    /* swallow */
  }
  // We don't unregister the FontFace here — there's no reliable way to
  // remove a face from `document.fonts` by name in all browsers. The
  // font stays loaded for this session; next boot it won't be
  // registered again. Acceptable for a "remove" action that triggers
  // an obvious next-session reset.
}
