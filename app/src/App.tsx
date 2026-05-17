import { ThemeProvider } from "./theme/ThemeProvider";
import { useTheme } from "./theme/useTheme";

function ThemeToggle() {
  const { theme, effective, setTheme } = useTheme();
  return (
    <div className="water-theme-toggle">
      <span>theme: {theme} (effective: {effective})</span>{" "}
      <button onClick={() => setTheme("light")}>light</button>{" "}
      <button onClick={() => setTheme("dark")}>dark</button>{" "}
      <button onClick={() => setTheme("auto")}>auto</button>
    </div>
  );
}

export default function App() {
  return (
    <ThemeProvider>
      <main className="water-shell">
        <h1>Water</h1>
        <p>foundation milestone</p>
        <ThemeToggle />
      </main>
    </ThemeProvider>
  );
}
