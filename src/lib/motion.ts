// Motion presets from docs/DESIGN.md plus visibility helpers used to pause ambient animation.
import { useEffect, useState, type RefObject } from "react";
import type { Transition } from "motion/react";

export const springInteraction: Transition = { type: "spring", stiffness: 380, damping: 34 };
export const springPanel: Transition = { type: "spring", stiffness: 260, damping: 30 };
export const tweenNumber: Transition = { duration: 0.3, ease: [0.16, 1, 0.3, 1] };
export const instant: Transition = { duration: 0 };

export function usePrefersReducedMotion(): boolean {
  const [reduced, setReduced] = useState(() =>
    typeof window === "undefined" || !window.matchMedia
      ? false
      : window.matchMedia("(prefers-reduced-motion: reduce)").matches,
  );
  useEffect(() => {
    if (!window.matchMedia) return;
    const mq = window.matchMedia("(prefers-reduced-motion: reduce)");
    const onChange = () => setReduced(mq.matches);
    mq.addEventListener("change", onChange);
    return () => mq.removeEventListener("change", onChange);
  }, []);
  return reduced;
}

export function usePageVisible(): boolean {
  const [visible, setVisible] = useState(() =>
    typeof document === "undefined" ? true : document.visibilityState !== "hidden",
  );
  useEffect(() => {
    const onChange = () => setVisible(document.visibilityState !== "hidden");
    document.addEventListener("visibilitychange", onChange);
    return () => document.removeEventListener("visibilitychange", onChange);
  }, []);
  return visible;
}

/** True while the element intersects the viewport and the window is visible. */
export function useOnScreen(ref: RefObject<Element | null>): boolean {
  const pageVisible = usePageVisible();
  const [intersecting, setIntersecting] = useState(true);
  useEffect(() => {
    const el = ref.current;
    if (!el || typeof IntersectionObserver === "undefined") return;
    const io = new IntersectionObserver((entries) => {
      const entry = entries[entries.length - 1];
      if (entry) setIntersecting(entry.isIntersecting);
    });
    io.observe(el);
    return () => io.disconnect();
  }, [ref]);
  return pageVisible && intersecting;
}

export function useElementSize<T extends Element>(ref: RefObject<T | null>): { width: number; height: number } {
  const [size, setSize] = useState({ width: 0, height: 0 });
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const update = (w: number, h: number) =>
      setSize((prev) => (prev.width === w && prev.height === h ? prev : { width: w, height: h }));
    const rect = el.getBoundingClientRect();
    update(Math.round(rect.width), Math.round(rect.height));
    if (typeof ResizeObserver === "undefined") return;
    const ro = new ResizeObserver((entries) => {
      const box = entries[0]?.contentRect;
      if (box) update(Math.round(box.width), Math.round(box.height));
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, [ref]);
  return size;
}
