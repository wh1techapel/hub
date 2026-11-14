import { describe, expect, it } from "vitest";

import type { FocusView } from "../data/generated/FocusView";
import { countdown, focusAction, focusLabel, focusNews, progress, secondsLeft } from "./focusTime";

const T0 = Date.parse("2026-11-10T02:00:00Z");

function view(extra: Partial<FocusView> = {}): FocusView {
  return {
    phase: "focus",
    title: "Laporan lab",
    taskId: null,
    itemId: null,
    phaseStartedAt: "2026-11-10T02:00:00Z",
    phaseSeconds: 1500,
    pausedAt: null,
    endsAt: "2026-11-10T02:25:00Z",
    blocksDone: 0,
    longBreakEvery: 4,
    focusedSeconds: 0,
    tucked: false,
    startedAt: "2026-11-10T02:00:00Z",
    ...extra,
  };
}

describe("Sesi Fokus in words", () => {
  it("counts down from the phase's start, and stands still while paused", () => {
    expect(secondsLeft(view(), T0)).toBe(1500);
    expect(secondsLeft(view(), T0 + 77_500)).toBe(1423);
    expect(secondsLeft(view(), T0 + 2_000_000)).toBe(0);
    const paused = view({ pausedAt: "2026-11-10T02:10:00Z", endsAt: null });
    expect(secondsLeft(paused, T0 + 3_600_000)).toBe(900);
    expect(progress(paused, T0)).toBeCloseTo(0.4);
    expect(progress(view({ phase: "waiting", phaseSeconds: 0, endsAt: null }), T0)).toBe(1);
  });

  it("reads the time the way a clock does", () => {
    expect(countdown(1500)).toBe("25:00");
    expect(countdown(245)).toBe("4:05");
    expect(countdown(5400)).toBe("1:30:00");
    expect(countdown(-3)).toBe("0:00");
  });

  it("labels each place in the session, the task only while working", () => {
    expect(focusLabel(view(), T0 + 78_000)).toBe("Fokus 23:42 · Laporan lab");
    expect(focusLabel(view({ title: "" }), T0)).toBe("Fokus 25:00");
    expect(focusLabel(view({ pausedAt: "2026-11-10T02:10:00Z", endsAt: null }), T0)).toBe("Dijeda 15:00 · Laporan lab");
    expect(focusLabel(view({ phase: "short_break", phaseSeconds: 300 }), T0 + 55_000)).toBe("Istirahat 4:05");
    expect(focusLabel(view({ phase: "long_break", phaseSeconds: 900 }), T0)).toBe("Istirahat panjang 15:00");
    expect(focusLabel(view({ phase: "waiting", phaseSeconds: 0, endsAt: null }), T0)).toBe("Istirahat selesai");
  });

  it("offers one action for where the session is", () => {
    expect(focusAction(view()).kind).toBe("pause");
    expect(focusAction(view({ pausedAt: "2026-11-10T02:10:00Z" })).kind).toBe("resume");
    expect(focusAction(view({ phase: "short_break" })).kind).toBe("skip");
    expect(focusAction(view({ phase: "waiting" }))).toEqual({ kind: "next", label: "Lanjut fokus" });
  });

  it("says what changed, never the countdown", () => {
    expect(focusNews(null, view())).toBe("Sesi fokus dimulai, 25 menit.");
    expect(focusNews(view(), view({ phaseStartedAt: "2026-11-10T02:00:01Z" }))).toBeNull();
    expect(focusNews(view(), view({ phase: "short_break", phaseSeconds: 300 }))).toBe("Blok selesai. Istirahat 5 menit.");
    expect(focusNews(view(), view({ phase: "long_break", phaseSeconds: 900 }))).toBe("Blok selesai. Istirahat panjang 15 menit.");
    expect(focusNews(view({ phase: "short_break" }), view({ phase: "waiting" }))).toBe("Istirahat selesai. Lanjut fokus saat kamu siap.");
    expect(focusNews(view(), view({ pausedAt: "2026-11-10T02:10:00Z" }))).toBe("Sesi fokus dijeda.");
    expect(focusNews(view({ pausedAt: "2026-11-10T02:10:00Z" }), view())).toBe("Sesi fokus dilanjutkan.");
    expect(focusNews(view(), null)).toBe("Sesi fokus selesai.");
    expect(focusNews(null, null)).toBeNull();
  });
});
