import { useEffect, useState } from "react";

import { resolveTheme, useSettings } from "../stores/settings";

/** The theme actually in effect ("system" resolved), for code that needs concrete colors. */
export function useResolvedTheme(): "dark" | "light" {
  const preference = useSettings((s) => s.theme);
  const [prefersDark, setPrefersDark] = useState(() => window.matchMedia("(prefers-color-scheme: dark)").matches);
  useEffect(() => {
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = () => setPrefersDark(mq.matches);
    mq.addEventListener("change", onChange);
    return () => mq.removeEventListener("change", onChange);
  }, []);
  return resolveTheme(preference, prefersDark);
}

/** Applies the theme preference as `data-theme` on <html>, following the OS when set to system. */
export function useThemeEffect(): void {
  const preference = useSettings((s) => s.theme);
  useEffect(() => {
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const apply = () => {
      document.documentElement.dataset.theme = resolveTheme(preference, mq.matches);
    };
    apply();
    if (preference !== "system") return;
    mq.addEventListener("change", apply);
    return () => mq.removeEventListener("change", apply);
  }, [preference]);
}
