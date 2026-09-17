import { animate, useMotionValue } from "motion/react";
import { useEffect, useRef, useState } from "react";

import { tweenNumber, usePrefersReducedMotion } from "../lib/motion";

/**
 * Tweens a live number (~300 ms ease-out) by writing text directly from a motion value, so a
 * changing value never re-renders React per frame.
 */
export function AnimatedNumber({
  value,
  format,
  className,
}: {
  value: number;
  format: (value: number) => string;
  className?: string;
}) {
  const ref = useRef<HTMLSpanElement>(null);
  const motionValue = useMotionValue(value);
  const reduced = usePrefersReducedMotion();
  const formatRef = useRef(format);
  const [initialText] = useState(() => format(value));

  useEffect(() => {
    formatRef.current = format;
    if (ref.current) ref.current.textContent = format(motionValue.get());
  }, [format, motionValue]);

  useEffect(
    () =>
      motionValue.on("change", (v) => {
        if (ref.current) ref.current.textContent = formatRef.current(v);
      }),
    [motionValue],
  );

  useEffect(() => {
    if (!Number.isFinite(value)) {
      if (ref.current) ref.current.textContent = formatRef.current(value);
      return;
    }
    if (reduced || !Number.isFinite(motionValue.get())) {
      motionValue.jump(value);
      if (ref.current) ref.current.textContent = formatRef.current(value);
      return;
    }
    const controls = animate(motionValue, value, tweenNumber);
    return () => controls.stop();
  }, [value, reduced, motionValue]);

  return (
    <span ref={ref} className={className}>
      {initialText}
    </span>
  );
}
