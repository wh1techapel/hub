// Kalender: week (owner's default), day, and month. Drag to create, move, and resize; drag a task
// from the tray to give it time. Every drag has a keyboard path: N for a new event, Alt+arrows on
// an event, Jadwalkan on a tray task, and the time fields in every dialog. Since R6 Pengingat lives
// here too, as a fourth view (`?view=pengingat`, P).

import { useSound } from "../../sound/SoundProvider";
import { useNavigate, useSearch } from "@tanstack/react-router";
import { ChevronLeft, ChevronRight, Plus, Repeat } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import type { PointerEvent as ReactPointerEvent } from "react";

import { Segmented } from "../../components/Fields";
import { useAct } from "../../components/Toast";
import { useApi } from "../../data/api";
import type { CalendarItem } from "../../data/generated/CalendarItem";
import type { Routine } from "../../data/generated/Routine";
import type { RoutineInput } from "../../data/generated/RoutineInput";
import type { Slot } from "../../data/generated/Slot";
import type { TaskCard } from "../../data/generated/TaskCard";
import { useCalendar, useRoutines, useSettings, useTasks } from "../../data/queries";
import { id } from "../../strings/id";
import { useNow } from "../../time/clock";
import { dateLabel, isoDate, minutesOf, parseClock } from "../../time/format";
import styles from "./calendar.module.css";
import { blankEvent, EventDialog } from "./EventDialog";
import type { EventDraft } from "./EventDialog";
import { addDays, addMonths, dayCode, mondayOf, monthStart, parseDate, SNAP } from "./layout";
import { MonthGrid } from "./MonthGrid";
import { OccurrenceDialog } from "./OccurrenceDialog";
import { RemindersView } from "./RemindersView";
import { blankRoutine, RoutineDialog } from "./RoutineDialog";
import { TaskTray } from "./TaskTray";
import { TimeGrid } from "./TimeGrid";
import type { GridHandle } from "./TimeGrid";

export type CalendarView = "day" | "week" | "month";
/** What the page shows: a span of time, or the reminders. */
type Mode = CalendarView | "pengingat";

const monthFormat = new Intl.DateTimeFormat("id-ID", { month: "long", year: "numeric" });
const dayMonthFormat = new Intl.DateTimeFormat("id-ID", { day: "numeric", month: "long" });

function rangeLabel(view: CalendarView, from: string, anchor: string): string {
  if (view === "month") return monthFormat.format(parseDate(anchor));
  if (view === "day") return dateLabel(parseDate(anchor));
  const end = parseDate(addDays(from, 6));
  const start = parseDate(from);
  const sameMonth = start.getMonth() === end.getMonth();
  return `${sameMonth ? start.getDate() : dayMonthFormat.format(start)} sampai ${dayMonthFormat.format(end)} ${end.getFullYear()}`;
}

type Open =
  | { kind: "event"; draft: EventDraft; taskTitle?: string }
  | { kind: "occurrence"; item: CalendarItem }
  | { kind: "routine"; routine?: Routine; draft?: RoutineInput }
  | null;

export function CalendarPage() {
  const now = useNow();
  const todayIso = isoDate(now);
  const nowMinute = minutesOf(now);
  const api = useApi();
  const act = useAct();
  const sound = useSound();
  const navigate = useNavigate();
  const search: { view?: string; date?: string; new?: string } = useSearch({ strict: false });
  const mode: Mode = search.view === "day" || search.view === "month" || search.view === "pengingat" ? search.view : "week";
  // The span of time behind Pengingat stays the week, so leaving it lands where the owner was.
  const view: CalendarView = mode === "pengingat" ? "week" : mode;
  const anchor = search.date && /^\d{4}-\d{2}-\d{2}$/.test(search.date) ? search.date : todayIso;
  const from = view === "week" ? mondayOf(anchor) : view === "month" ? monthStart(anchor) : anchor;
  const count = view === "week" ? 7 : view === "month" ? 42 : 1;
  const calendar = useCalendar(from, count);
  const routines = useRoutines();
  const settings = useSettings();
  const todayTasks = useTasks("today", todayIso);
  const upcoming = useTasks("upcoming", todayIso);
  const grid = useRef<GridHandle>(null);
  const [open, setOpen] = useState<Open>(null);
  const [trayDrag, setTrayDrag] = useState<{ task: TaskCard; x: number; y: number } | null>(null);
  const wake = parseClock(settings.data?.wakeTime ?? "05:00") ?? 300;

  const go = useCallback(
    (next: { view?: Mode; date?: string }) => {
      void navigate({ to: "/kalender", search: { view: next.view ?? mode, date: next.date ?? anchor } });
    },
    [navigate, mode, anchor],
  );

  const step = (direction: number) => {
    if (view === "month") go({ date: addMonths(anchor, direction) });
    else go({ date: addDays(anchor, direction * (view === "week" ? 7 : 1)) });
  };

  const nextQuarter = Math.min(1440 - 60, Math.ceil((nowMinute + 1) / SNAP) * SNAP);
  const newEvent = useCallback(() => {
    const date = anchor;
    setOpen({ kind: "event", draft: blankEvent(date, date === todayIso ? nextQuarter : 9 * 60) });
  }, [anchor, todayIso, nextQuarter]);

  // ?new=event or ?new=routine (from the command palette) opens a sheet; closing it drops the
  // parameter, so going back does not open it again.
  const requested: Open =
    search.new === "event"
      ? { kind: "event", draft: blankEvent(anchor, anchor === todayIso ? nextQuarter : 9 * 60) }
      : search.new === "routine"
        ? { kind: "routine", draft: blankRoutine(anchor, 7 * 60, dayCode(anchor)) }
        : null;
  const shown = open ?? requested;
  const close = () => {
    setOpen(null);
    if (search.new) void navigate({ to: "/kalender", search: { view, date: anchor }, replace: true });
  };

  // Single keys, unless the owner is typing or a dialog is open.
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (shown || event.ctrlKey || event.metaKey || event.altKey) return;
      const target = event.target as HTMLElement | null;
      if (target && (target.closest("input, textarea, select, [role=dialog]") || target.isContentEditable)) return;
      const key = event.key.toLowerCase();
      const actions: Record<string, () => void> = {
        n: newEvent,
        t: () => go({ date: todayIso }),
        d: () => go({ view: "day" }),
        w: () => go({ view: "week" }),
        m: () => go({ view: "month" }),
        p: () => go({ view: "pengingat" }),
        arrowleft: () => step(-1),
        arrowright: () => step(1),
      };
      // Pengingat is a list, not a span of time: no stepping through days there.
      const action = mode === "pengingat" && (key === "t" || key === "arrowleft" || key === "arrowright") ? undefined : actions[key];
      if (action) {
        event.preventDefault();
        action();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  const onOpen = async (item: CalendarItem) => {
    if (item.kind === "routine") {
      setOpen({ kind: "occurrence", item });
      return;
    }
    if (!item.eventId) return;
    const eventId = item.eventId;
    const read = await act(() => api.getEvent(eventId));
    if (read.ok)
      setOpen({ kind: "event", draft: { ...read.value, id: eventId }, taskTitle: item.kind === "block" ? item.title : undefined });
  };

  const onMove = async (item: CalendarItem, fromDate: string, slot: Slot) => {
    sound.play("drop");
    if (item.kind === "routine" && item.routineId && item.ruleDate) {
      const routineId = item.routineId;
      const ruleDate = item.ruleDate;
      await act(() => api.moveOccurrence(routineId, ruleDate, slot), {
        text: id.calendar.movedOnce,
        action: { label: id.common.undo, run: () => void act(() => api.restoreOccurrence(routineId, ruleDate)) },
      });
      return;
    }
    if (!item.eventId) return;
    const eventId = item.eventId;
    const back: Slot = { date: fromDate, startMinute: item.startMinute, endMinute: item.endMinute };
    await act(() => api.rescheduleEvent(eventId, slot), {
      text: id.calendar.moved,
      action: { label: id.common.undo, run: () => void act(() => api.rescheduleEvent(eventId, back)) },
    });
  };

  const scheduleTask = (task: TaskCard, date: string, start: number) => {
    sound.play("drop");
    const end = Math.min(start + (task.estimateMin ?? 60), 1440 + 360);
    void act(
      () =>
        api.createEvent({
          ...blankEvent(date, start, end),
          title: task.title,
          pigment: task.project?.pigment ?? "batu",
          projectId: task.project?.id ?? null,
          taskId: task.id,
        }),
      { text: id.common.saved },
    );
  };

  // Dragging a task from the tray onto the grid.
  const startTrayDrag = (event: ReactPointerEvent<HTMLElement>, task: TaskCard) => {
    if (event.button !== 0) return;
    event.preventDefault();
    setTrayDrag({ task, x: event.clientX, y: event.clientY });
  };
  useEffect(() => {
    if (!trayDrag) return;
    const length = trayDrag.task.estimateMin ?? 60;
    const onMovePointer = (event: PointerEvent) => {
      setTrayDrag((d) => (d ? { ...d, x: event.clientX, y: event.clientY } : d));
      const slot = grid.current?.slotAt(event.clientX, event.clientY);
      grid.current?.preview(slot ? { date: slot.date, startMinute: slot.minute, endMinute: slot.minute + length } : null);
    };
    const onUp = (event: PointerEvent) => {
      const slot = grid.current?.slotAt(event.clientX, event.clientY);
      grid.current?.preview(null);
      if (slot) scheduleTask(trayDrag.task, slot.date, slot.minute);
      setTrayDrag(null);
    };
    window.addEventListener("pointermove", onMovePointer);
    window.addEventListener("pointerup", onUp, { once: true });
    return () => {
      window.removeEventListener("pointermove", onMovePointer);
      window.removeEventListener("pointerup", onUp);
    };
    // The drag's task is fixed for its lifetime.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [trayDrag?.task.id]);

  const last = addDays(from, count - 1);
  const trayTasks = [...(todayTasks.data ?? []), ...(upcoming.data ?? [])].filter(
    (task, index, all) =>
      task.status !== "done" &&
      !task.scheduled &&
      (task.doDate ?? task.dueDate ?? todayIso) <= last &&
      all.findIndex((t) => t.id === task.id) === index,
  );

  const reminders = mode === "pengingat";

  return (
    <div className={styles.page}>
      <header className={styles.header}>
        <h1 className={`display ${styles.title}`}>{reminders ? id.calendar.reminders : rangeLabel(view, from, anchor)}</h1>
        <div className={styles.controls}>
          {!reminders && (
            <>
              <button type="button" className="btn btn-secondary" onClick={() => go({ date: todayIso })}>
                {id.calendar.today}
              </button>
              <button type="button" className="icon-btn" aria-label={id.calendar.prev} onClick={() => step(-1)}>
                <ChevronLeft size={18} aria-hidden="true" />
              </button>
              <button type="button" className="icon-btn" aria-label={id.calendar.next} onClick={() => step(1)}>
                <ChevronRight size={18} aria-hidden="true" />
              </button>
            </>
          )}
          <Segmented
            label={id.calendar.title}
            options={[
              ["day", id.calendar.views.day],
              ["week", id.calendar.views.week],
              ["month", id.calendar.views.month],
              ["pengingat", id.calendar.reminders],
            ]}
            value={mode}
            onChange={(next) => go({ view: next })}
          />
          <button
            type="button"
            className="btn btn-secondary"
            onClick={() => setOpen({ kind: "routine", draft: blankRoutine(anchor, 7 * 60, dayCode(anchor)) })}
          >
            <Repeat size={16} aria-hidden="true" />
            {id.calendar.routine}
          </button>
          <button type="button" className="btn btn-primary" onClick={newEvent}>
            <Plus size={16} aria-hidden="true" />
            {id.calendar.new}
          </button>
        </div>
      </header>
      {!reminders && <p className={styles.keys}>{id.calendar.keyboardHint}</p>}

      {reminders ? (
        <RemindersView />
      ) : view === "month" ? (
        <MonthGrid
          days={calendar.data ?? []}
          month={parseDate(anchor).getMonth()}
          todayIso={todayIso}
          onOpenDay={(date) => go({ view: "day", date })}
        />
      ) : (
        <div className={styles.withTray}>
          <TimeGrid
            ref={grid}
            days={calendar.data ?? []}
            todayIso={todayIso}
            nowMinute={nowMinute}
            wake={wake}
            onCreate={(slot) => setOpen({ kind: "event", draft: blankEvent(slot.date, slot.startMinute, slot.endMinute) })}
            onOpen={(item) => void onOpen(item)}
            onMove={(item, date, slot) => void onMove(item, date, slot)}
          />
          <TaskTray
            tasks={trayTasks}
            today={now}
            onDragStart={startTrayDrag}
            onSchedule={(task) =>
              setOpen({
                kind: "event",
                taskTitle: task.title,
                draft: {
                  ...blankEvent(
                    anchor,
                    anchor === todayIso ? nextQuarter : 16 * 60,
                    (anchor === todayIso ? nextQuarter : 16 * 60) + (task.estimateMin ?? 60),
                  ),
                  title: task.title,
                  pigment: task.project?.pigment ?? "batu",
                  projectId: task.project?.id ?? null,
                  taskId: task.id,
                },
              })
            }
          />
        </div>
      )}

      {trayDrag && (
        <div className={styles.trayGhost} style={{ left: trayDrag.x + 12, top: trayDrag.y + 12 }} aria-hidden="true">
          {trayDrag.task.title}
        </div>
      )}

      {shown?.kind === "event" && <EventDialog draft={shown.draft} taskTitle={shown.taskTitle} onClose={close} />}
      {shown?.kind === "occurrence" && (
        <OccurrenceDialog
          item={shown.item}
          today={now}
          onClose={() => setOpen(null)}
          onEditRoutine={(routineId) => setOpen({ kind: "routine", routine: routines.data?.find((r) => r.id === routineId) })}
        />
      )}
      {shown?.kind === "routine" && <RoutineDialog routine={shown.routine} draft={shown.draft} onClose={close} />}
    </div>
  );
}
