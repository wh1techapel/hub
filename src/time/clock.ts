// One clock for the whole UI: it ticks once a minute, on the minute, and stops while the window
// is hidden (docs/design.md, "Motion": nothing runs at rest). Components read it with useNow().
//
// In development builds `localStorage["hub:t"] = "21:05"` (then reload) pins the time of day, so
// every light and state can be looked at in the browser without waiting for it. Not a URL
// parameter: a query outside the hash confuses the router's active links.

import { useSyncExternalStore } from "react";

type Listener = () => void;

const listeners = new Set<Listener>();
let current = read();
let timer: ReturnType<typeof setTimeout> | undefined;

function pinnedMinutes(): number | null {
  if (!import.meta.env.DEV || import.meta.env.MODE === "test") return null;
  let pinned: string | null;
  try {
    pinned = localStorage.getItem("hub:t");
  } catch {
    return null;
  }
  const match = /^(\d{1,2}):(\d{2})$/.exec(pinned ?? "");
  if (!match) return null;
  return Number(match[1]) * 60 + Number(match[2]);
}

function read(): Date {
  const now = new Date();
  const pinned = pinnedMinutes();
  if (pinned === null) return now;
  return new Date(now.getFullYear(), now.getMonth(), now.getDate(), Math.floor(pinned / 60), pinned % 60);
}

/** Read the time again and tell everyone (also used after the window comes back). */
export function refreshClock(): void {
  const next = read();
  if (next.getTime() === current.getTime()) return;
  current = next;
  for (const listener of listeners) listener();
}

function schedule(): void {
  clearTimeout(timer);
  if (listeners.size === 0 || document.hidden) return;
  const untilNextMinute = 60_000 - (Date.now() % 60_000) + 50;
  timer = setTimeout(() => {
    refreshClock();
    schedule();
  }, untilNextMinute);
}

function onVisibility(): void {
  if (!document.hidden) refreshClock();
  schedule();
}

function subscribe(listener: Listener): () => void {
  if (listeners.size === 0) document.addEventListener("visibilitychange", onVisibility);
  listeners.add(listener);
  schedule();
  return () => {
    listeners.delete(listener);
    if (listeners.size === 0) {
      clearTimeout(timer);
      document.removeEventListener("visibilitychange", onVisibility);
    }
  };
}

/** The current time, to the minute. */
export function useNow(): Date {
  return useSyncExternalStore(subscribe, () => current);
}
