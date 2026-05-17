import type { Config } from "tailwindcss";

export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      borderRadius: {
        none: "0",
        sm: "8px",
        DEFAULT: "16px",
        md: "16px",
        lg: "24px",
        xl: "32px",
        full: "9999px"
      },
      fontFamily: {
        sans: ["Inter", "system-ui", "sans-serif"],
        serif: ["'Source Serif Pro'", "Georgia", "serif"],
        mono: ["'JetBrains Mono'", "ui-monospace", "monospace"]
      }
    }
  },
  plugins: []
} satisfies Config;
