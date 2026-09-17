import { useEffect } from "react";

import { resolveTheme, useSettings } from "../stores/settings";

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
