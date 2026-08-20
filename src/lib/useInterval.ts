import { useEffect, useRef } from "react";

/**
 * `useInterval(fn, delayMs)` runs `fn` every `delayMs` while the document is
 * visible. The interval automatically pauses when the window is hidden or
 * minimized and resumes when it becomes visible again, so background polling
 * (telemetry invokes, DB samples, process refreshes) doesn't keep consuming
 * CPU, IPC, and battery while the user isn't looking at the app.
 *
 * `fn` is read from a ref, so it always sees the latest closure without
 * restarting the timer. `delayMs === null` disables the interval.
 */
export function useInterval(fn: () => void, delayMs: number | null): void {
  const fnRef = useRef(fn);
  fnRef.current = fn;

  useEffect(() => {
    if (delayMs === null) return;

    let handle = 0;
    let running = !document.hidden;
    let active = true;

    const run = () => {
      if (document.hidden) return;
      fnRef.current();
    };

    const schedule = () => {
      if (!running || !active) return;
      window.clearInterval(handle);
      handle = window.setInterval(run, delayMs);
    };

    const onVisibility = () => {
      const visible = !document.hidden;
      if (visible === running) return;
      running = visible;
      if (visible) {
        // Poll immediately on returning to the window, then resume cadence.
        run();
        schedule();
      } else {
        window.clearInterval(handle);
      }
    };

    document.addEventListener("visibilitychange", onVisibility);
    schedule();

    return () => {
      active = false;
      document.removeEventListener("visibilitychange", onVisibility);
      window.clearInterval(handle);
    };
  }, [delayMs]);
}