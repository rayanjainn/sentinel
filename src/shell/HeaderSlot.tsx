// Views place their toolbar in the unified title-bar header through a portal.
import { createContext, useContext, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";

const SlotContext = createContext<{ node: HTMLElement | null; setNode: (node: HTMLElement | null) => void }>({
  node: null,
  setNode: () => undefined,
});

export function HeaderSlotProvider({ children }: { children: ReactNode }) {
  const [node, setNode] = useState<HTMLElement | null>(null);
  return <SlotContext.Provider value={{ node, setNode }}>{children}</SlotContext.Provider>;
}

export function useHeaderSlotTarget() {
  return useContext(SlotContext).setNode;
}

export function HeaderToolbar({ children }: { children: ReactNode }) {
  const { node } = useContext(SlotContext);
  return node ? createPortal(children, node) : null;
}
