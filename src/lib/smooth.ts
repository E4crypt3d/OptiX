import { useEffect, useRef, useState } from "react";

/** True when the user prefers reduced motion: snap values instead of easing. */
export const REDUCED_MOTION =
  typeof window !== "undefined" &&
  window.matchMedia?.("(prefers-reduced-motion: reduce)").matches;

/**
 * `useSmoothValue(target)` returns a value that glides toward `target` at
 * 60 fps instead of jumping. Targets arrive at 1 Hz from the dashboard poll;
 * this interpolates between them so percentages and rates move continuously.
 *
 * The easing is exponential (`alpha` from elapsed time, frame-rate
 * independent) and converges to within 0.05 before stopping. The very first
 * target snaps in immediately, and `null` yields `null` (rendered as "—").
 * Only the calling (leaf) component re-renders per frame.
 */
export function useSmoothValue(target: number | null | undefined, tauMs = 400): number | null {
  const [value, setValue] = useState<number | null>(target ?? null);
  const valueRef = useRef<number | null>(value);
  const rafRef = useRef(0);

  useEffect(() => {
    const current = valueRef.current;
    if (target == null) return;
    if (current == null || REDUCED_MOTION || Math.abs(target - current) < 0.05) {
      valueRef.current = target;
      setValue(target);
      return;
    }
    const start = performance.now();
    const step = (now: number) => {
      const alpha = 1 - Math.exp(-Math.min(100, now - start) / tauMs);
      const next = valueRef.current! + (target - valueRef.current!) * alpha;
      valueRef.current = next;
      setValue(next);
      if (Math.abs(target - next) > 0.05) {
        rafRef.current = requestAnimationFrame(step);
      } else {
        valueRef.current = target;
        setValue(target);
      }
    };
    rafRef.current = requestAnimationFrame(step);
    return () => cancelAnimationFrame(rafRef.current);
  }, [target, tauMs]);

  return value;
}
