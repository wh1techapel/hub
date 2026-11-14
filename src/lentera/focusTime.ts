// Sesi Fokus as words and numbers (Lentera CP02): the time left, how far the phase is, the
// capsule's label, and its one action. Pure: the window passes its own clock.

import type { FocusView } from "../data/generated/FocusView";
import { id } from "../strings/id";

/** Seconds left in the phase: none while waiting, frozen while paused. */
export function secondsLeft(view: FocusView, nowMs: number): number {
  if (view.phase === "waiting") return 0;
  const started = Date.parse(view.phaseStartedAt);
  const at = view.pausedAt ? Date.parse(view.pausedAt) : nowMs;
  return Math.max(0, Math.min(view.phaseSeconds, Math.ceil(view.phaseSeconds - (at - started) / 1000)));
}

/** How much of the phase is behind, 0 to 1 (a wait is whole). */
export function progress(view: FocusView, nowMs: number): number {
  if (view.phase === "waiting" || view.phaseSeconds === 0) return 1;
  return 1 - secondsLeft(view, nowMs) / view.phaseSeconds;
}

/** "18:42", "4:05", "1:00:00". */
export function countdown(seconds: number): string {
  const s = Math.max(0, Math.round(seconds));
  const hours = Math.floor(s / 3600);
  const minutes = Math.floor((s % 3600) / 60);
  const rest = String(s % 60).padStart(2, "0");
  return hours > 0 ? `${hours}:${String(minutes).padStart(2, "0")}:${rest}` : `${minutes}:${rest}`;
}

/** The capsule's words: "Fokus 18:42 · Laporan lab", "Dijeda 12:30", "Istirahat 4:05", "Istirahat selesai". */
export function focusLabel(view: FocusView, nowMs: number): string {
  if (view.phase === "waiting") return id.focus.phase.waiting ?? "";
  const time = countdown(secondsLeft(view, nowMs));
  const what = view.pausedAt ? id.focus.paused : (id.focus.phase[view.phase] ?? "");
  const title = view.phase === "focus" && view.title ? ` · ${view.title}` : "";
  return `${what} ${time}${title}`;
}

export type FocusActionKind = "pause" | "resume" | "skip" | "next";

/** The capsule's one action for where the session is. */
export function focusAction(view: FocusView): { kind: FocusActionKind; label: string } {
  if (view.phase === "waiting") return { kind: "next", label: id.focus.next };
  if (view.phase !== "focus") return { kind: "skip", label: id.focus.skip };
  return view.pausedAt ? { kind: "resume", label: id.focus.resume } : { kind: "pause", label: id.focus.pause };
}

/** What changed between two looks at the session, said once (never the countdown itself). */
export function focusNews(before: FocusView | null, after: FocusView | null): string | null {
  if (!before && after) return id.focus.said.started(Math.round(after.phaseSeconds / 60));
  if (before && !after) return id.focus.said.ended;
  if (!before || !after) return null;
  if (before.phase !== after.phase) {
    if (after.phase === "short_break") return id.focus.said.break(Math.round(after.phaseSeconds / 60));
    if (after.phase === "long_break") return id.focus.said.longBreak(Math.round(after.phaseSeconds / 60));
    if (after.phase === "waiting") return id.focus.said.over;
    return id.focus.said.started(Math.round(after.phaseSeconds / 60));
  }
  if (!before.pausedAt && after.pausedAt) return id.focus.said.paused;
  if (before.pausedAt && !after.pausedAt) return id.focus.said.resumed;
  return null;
}
