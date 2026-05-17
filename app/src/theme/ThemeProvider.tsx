import { createContext, useEffect, useMemo, useState } from "react";
import type { ReactNode } from "react";

export type Theme = "light" | "dark" | "auto";
export type EffectiveTheme = "light" | "dark";

export interface ThemeContextValue {
  theme: Theme;
  effective: EffectiveTheme;
  setTheme: (t: Theme) => void;
}

export const ThemeContext = createContext<ThemeContextValue | null>(null);

const STORAGE_KEY = "water:theme";

function readStored(): Theme {
  try {
    const v = localStorage.getItem(STORAGE_KEY);
    if (v === "light" || v === "dark" || v === "auto") return v;
  } catch {}
  return "auto";
}

function systemPrefersDark(): boolean {
  if (typeof window === "undefined" || !window.matchMedia) return false;
  return window.matchMedia("(prefers-color-scheme: dark)").matches;
}

export function ThemeProvider({ children }: { children: ReactNode }) {
  const [theme, setThemeState] = useState<Theme>(readStored);

  const effective: EffectiveTheme = useMemo(() => {
    if (theme === "auto") return systemPrefersDark() ? "dark" : "light";
    return theme;
  }, [theme]);

  useEffect(() => {
    if (theme === "auto") {
      document.documentElement.removeAttribute("data-theme");
    } else {
      document.documentElement.setAttribute("data-theme", theme);
    }
  }, [theme]);

  const setTheme = (t: Theme) => {
    setThemeState(t);
    try {
      localStorage.setItem(STORAGE_KEY, t);
    } catch {}
  };

  const value: ThemeContextValue = { theme, effective, setTheme };
  return <ThemeContext.Provider value={value}>{children}</ThemeContext.Provider>;
}
