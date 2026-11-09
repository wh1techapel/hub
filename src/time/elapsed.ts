// Time since a start, as Belajar's timer and the PRD's writing show it (L3 moved it here from
// LearningPage so both read the same clock).

import { useEffect, useState } from "react";

/** Seconds since a start, ticking once a second while shown (not while the window is hidden). */
export function useElapsed(startedAt: string | null): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (!startedAt) return;
    const tick = setInterval(() => {
      if (document.visibilityState === "visible") setNow(Date.now());
    }, 1000);
    return () => clearInterval(tick);
  }, [startedAt]);
  return startedAt ? Math.max(0, Math.floor((now - new Date(startedAt).getTime()) / 1000)) : 0;
}

/** Seconds as a stopwatch: "04:09", "1:02:03". */
export function clockOf(seconds: number): string {
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  const s = seconds % 60;
  const two = (n: number) => String(n).padStart(2, "0");
  return h ? `${h}:${two(m)}:${two(s)}` : `${two(m)}:${two(s)}`;
}
