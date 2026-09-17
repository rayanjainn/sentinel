import { useEffect } from "react";

import { isModEvent } from "../lib/platform";
import { useSettings } from "../stores/settings";
import { NAV_ITEMS } from "./navigation";

/** Global shortcuts: Cmd/Ctrl+J toggles the agent panel, Cmd/Ctrl+1–6 switch views, Cmd/Ctrl+, settings. */
export function useShortcuts(): void {
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (!isModEvent(event) || event.altKey || event.shiftKey) return;
      const { togglePanel, setView } = useSettings.getState();
      if (event.key.toLowerCase() === "j") {
        event.preventDefault();
        togglePanel();
        return;
      }
      if (event.key === ",") {
        event.preventDefault();
        setView("settings");
        return;
      }
      const index = Number(event.key) - 1;
      const item = Number.isInteger(index) ? NAV_ITEMS[index] : undefined;
      if (item) {
        event.preventDefault();
        setView(item.id);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
}
