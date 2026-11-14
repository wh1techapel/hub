// Sesi Fokus in the browser and in tests (Lentera CP02): hub-core's focus.rs, short. A block ends in
// a break that starts by itself, a break ends in waiting, every few blocks the break is long, and a
// session left paused or waiting for half an hour ends. The real one also pauses for time Hub did
// not see (the computer asleep); a page in a browser has no such gap to find, so that rule is left
// out here. Each phase's end is announced as the scheduler would, with a timer.

import type { Actor } from "./generated/Actor";
import type { Entity } from "./generated/Entity";
import type { FocusPhase } from "./generated/FocusPhase";
import type { FocusStart } from "./generated/FocusStart";
import type { FocusView } from "./generated/FocusView";
import type { Settings } from "./generated/Settings";
import type { Verb } from "./generated/Verb";

export interface FocusDeps {
  record: (actor: Actor, verb: Verb, entity: Entity, entityId: string | null, summary: string) => void;
  done: (entity: Entity, id?: string) => void;
  settings: () => Settings;
  taskTitle: (id: string) => string | null;
  itemTitle: (id: string) => string | null;
  /** A block on a Belajar item becomes its learning session. */
  itemBlock: (itemId: string, startedAt: string, minutes: number) => void;
  /** A session on an item starts: Belajar's timer stops, and the item is "doing". */
  itemStarts: (itemId: string) => void;
  /** Any session starts: Belajar's timer stops. */
  stopTimer: () => void;
  fail: (field: string, reason: string) => never;
  notFound: (what: string) => never;
}

interface Run {
  taskId: string | null;
  itemId: string | null;
  title: string;
  phase: FocusPhase;
  phaseStartedAt: number;
  phaseSeconds: number;
  pausedAt: number | null;
  blocksDone: number;
  focusedSeconds: number;
  tucked: boolean;
  startedAt: number;
  lengths: { focus: number; short: number; long: number; every: number };
}

const IDLE_LIMIT_MS = 30 * 60_000;
const iso = (ms: number) => new Date(Math.floor(ms / 1000) * 1000).toISOString().replace(".000Z", "Z");

export function focusMock(deps: FocusDeps, secondsPerMinute = 60) {
  let run: Run | null = null;
  let timer: ReturnType<typeof setTimeout> | undefined;

  const endsAt = (r: Run) => (r.pausedAt === null && r.phase !== "waiting" ? r.phaseStartedAt + r.phaseSeconds * 1000 : null);
  const idleEnd = (r: Run) => (r.pausedAt !== null ? r.pausedAt + IDLE_LIMIT_MS : r.phase === "waiting" ? r.phaseStartedAt + IDLE_LIMIT_MS : null);

  function keep(r: Run, started: number, seconds: number, completed: boolean) {
    const minutes = Math.max(1, Math.round(seconds / 60));
    if (r.itemId && deps.itemTitle(r.itemId) !== null) deps.itemBlock(r.itemId, iso(started), minutes);
    else if (completed) deps.record("owner", "completed", "focus_session", null, r.title);
  }

  function end(r: Run, actor: Actor, at: number) {
    if (r.phase === "focus") {
      const until = r.pausedAt ?? at;
      const seconds = Math.min(r.phaseSeconds, Math.max(0, Math.floor((until - r.phaseStartedAt) / 1000)));
      if (seconds >= 60) keep(r, r.phaseStartedAt, seconds, false);
    }
    deps.record(actor, "closed", "focus_session", null, r.title);
    run = null;
  }

  /** Bring the session up to now; true when anything moved. */
  function settle(now = Date.now()): boolean {
    if (!run) return false;
    const r = run;
    let moved = false;
    for (let due = endsAt(r); due !== null && now >= due; due = endsAt(r)) {
      moved = true;
      if (r.phase === "focus") {
        keep(r, r.phaseStartedAt, r.phaseSeconds, true);
        r.blocksDone += 1;
        r.focusedSeconds += r.phaseSeconds;
        const long = r.blocksDone % r.lengths.every === 0;
        r.phase = long ? "long_break" : "short_break";
        r.phaseSeconds = long ? r.lengths.long : r.lengths.short;
      } else {
        r.phase = "waiting";
        r.phaseSeconds = 0;
      }
      r.phaseStartedAt = due;
      r.tucked = false;
    }
    const idle = idleEnd(r);
    if (idle !== null && now >= idle) {
      end(r, "hub", idle);
      moved = true;
    }
    return moved;
  }

  /** As the scheduler: wake at the next end, move on, and say so. */
  function schedule() {
    clearTimeout(timer);
    if (!run) return;
    const next = endsAt(run) ?? idleEnd(run);
    if (next === null) return;
    timer = setTimeout(
      () => {
        if (settle()) deps.done("focus_session");
        schedule();
      },
      Math.max(0, next - Date.now()) + 20,
    );
  }

  function view(): FocusView | null {
    if (!run) return null;
    const ends = endsAt(run);
    return {
      phase: run.phase,
      title: run.title,
      taskId: run.taskId,
      itemId: run.itemId,
      phaseStartedAt: iso(run.phaseStartedAt),
      phaseSeconds: run.phaseSeconds,
      pausedAt: run.pausedAt === null ? null : iso(run.pausedAt),
      endsAt: ends === null ? null : iso(ends),
      blocksDone: run.blocksDone,
      longBreakEvery: run.lengths.every,
      focusedSeconds: run.focusedSeconds,
      tucked: run.tucked,
      startedAt: iso(run.startedAt),
    };
  }

  function act(change: (r: Run, now: number) => void) {
    const now = Date.now();
    settle(now);
    if (!run) deps.notFound("focus session");
    change(run, now);
    deps.done("focus_session");
    schedule();
  }

  const handlers = {
    get_focus: () => {
      if (settle()) deps.done("focus_session");
      return view();
    },
    start_focus: ({ input }: Record<string, unknown>) => {
      const start = input as FocusStart;
      if (start.taskId && start.itemId) deps.fail("itemId", "format");
      const title = start.taskId
        ? (deps.taskTitle(start.taskId) ?? deps.notFound("task"))
        : start.itemId
          ? (deps.itemTitle(start.itemId) ?? deps.notFound("learning item"))
          : "";
      const now = Date.now();
      settle(now);
      if (run) end(run, "owner", now);
      if (start.itemId) deps.itemStarts(start.itemId);
      else deps.stopTimer();
      const s = deps.settings();
      const unit = secondsPerMinute;
      const lengths = { focus: s.focusMinutes * unit, short: s.breakMinutes * unit, long: s.longBreakMinutes * unit, every: s.longBreakEvery };
      run = {
        taskId: start.taskId ?? null,
        itemId: start.itemId ?? null,
        title,
        phase: "focus",
        phaseStartedAt: now,
        phaseSeconds: lengths.focus,
        pausedAt: null,
        blocksDone: 0,
        focusedSeconds: 0,
        tucked: false,
        startedAt: now,
        lengths,
      };
      deps.done("focus_session");
      schedule();
      return view();
    },
    pause_focus: () =>
      act((r, now) => {
        if (r.phase === "focus" && r.pausedAt === null) r.pausedAt = now;
      }),
    resume_focus: () =>
      act((r, now) => {
        if (r.pausedAt !== null) {
          r.phaseStartedAt += now - r.pausedAt;
          r.pausedAt = null;
        }
      }),
    next_focus: () =>
      act((r, now) => {
        if (r.phase !== "focus") {
          r.phase = "focus";
          r.phaseStartedAt = now;
          r.phaseSeconds = r.lengths.focus;
          r.pausedAt = null;
          r.tucked = false;
        }
      }),
    tuck_focus: ({ tucked }: Record<string, unknown>) =>
      act((r) => {
        r.tucked = tucked === true;
      }),
    stop_focus: () => {
      const now = Date.now();
      settle(now);
      if (run) {
        end(run, "owner", now);
        deps.done("focus_session");
      }
      schedule();
    },
  };

  return {
    handlers,
    /** Belajar's own timer started: the session ends (one clock at a time). */
    stop() {
      if (!run) return;
      end(run, "owner", Date.now());
      deps.done("focus_session");
      schedule();
    },
  };
}
