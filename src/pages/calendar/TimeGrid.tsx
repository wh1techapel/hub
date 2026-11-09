// The day and week views: one column per day, an hour scale, all-day items on top. A drag on empty
// space makes a new event; a drag on an item moves it (a routine's occurrence moves alone); the
// bottom edge resizes. Every item is also a button: Enter opens it, Alt+arrows move it.

import { Bell } from "lucide-react";
import { forwardRef, useEffect, useImperativeHandle, useLayoutEffect, useRef, useState } from "react";
import type { KeyboardEvent, PointerEvent } from "react";

import type { CalendarDay } from "../../data/generated/CalendarDay";
import type { CalendarItem } from "../../data/generated/CalendarItem";
import type { Slot } from "../../data/generated/Slot";
import { id } from "../../strings/id";
import { clock, span } from "../../time/format";
import styles from "./calendar.module.css";
import { addDays, clampMinute, dayCode, DAY_MINUTES, layoutDay, parseDate, SNAP, snap } from "./layout";

/** Pixels per minute: 48 px an hour. */
export const PPM = 0.8;
const DRAG_THRESHOLD = 4;

export interface GridHandle {
  /** The day and snapped minute under a screen point, for drops from outside (the task tray). */
  slotAt: (clientX: number, clientY: number) => { date: string; minute: number } | null;
  /** Show where a drop from outside would land. */
  preview: (slot: Slot | null) => void;
}

interface Props {
  days: readonly CalendarDay[];
  todayIso: string;
  nowMinute: number;
  wake: number;
  onCreate: (slot: Slot) => void;
  onOpen: (item: CalendarItem, date: string) => void;
  onMove: (item: CalendarItem, date: string, slot: Slot) => void;
}

type Drag =
  | { mode: "create"; date: string; from: number; to: number; x: number; y: number; moved: boolean }
  | {
      mode: "move" | "resize";
      item: CalendarItem;
      fromDate: string;
      date: string;
      start: number;
      end: number;
      grab: number;
      x: number;
      y: number;
      moved: boolean;
    };

function itemLabel(item: CalendarItem): string {
  const when = item.allDay ? id.calendar.allDay : `${clock(item.startMinute)} sampai ${clock(item.endMinute)}`;
  return `${item.title}, ${when}${item.location ? `, ${item.location}` : ""}`;
}

export const TimeGrid = forwardRef<GridHandle, Props>(function TimeGrid(
  { days, todayIso, nowMinute, wake, onCreate, onOpen, onMove },
  ref,
) {
  const scroller = useRef<HTMLDivElement>(null);
  const columns = useRef<HTMLDivElement>(null);
  const [drag, setDrag] = useState<Drag | null>(null);
  const [external, setExternal] = useState<Slot | null>(null);
  const dates = days.map((d) => d.date);

  // Start at the waking hours (or around now, on today's week).
  useLayoutEffect(() => {
    const showsToday = dates.includes(todayIso);
    const minute = showsToday ? Math.max(0, nowMinute - 120) : Math.max(0, wake - 60);
    if (scroller.current) scroller.current.scrollTop = minute * PPM;
    // Only when the range changes, not every minute.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [dates.join()]);

  const pointAt = (clientX: number, clientY: number) => {
    const box = columns.current?.getBoundingClientRect();
    if (!box || dates.length === 0) return null;
    const index = Math.floor(((clientX - box.left) / box.width) * dates.length);
    const date = dates[Math.max(0, Math.min(dates.length - 1, index))];
    if (!date) return null;
    return {
      date,
      minute: clampMinute((clientY - box.top) / PPM),
      inside: clientX >= box.left && clientX <= box.right && clientY >= box.top && clientY <= box.bottom,
    };
  };

  useImperativeHandle(ref, () => ({
    slotAt: (clientX, clientY) => {
      const point = pointAt(clientX, clientY);
      return point?.inside ? { date: point.date, minute: Math.min(snap(point.minute), DAY_MINUTES - SNAP) } : null;
    },
    preview: setExternal,
  }));

  // ---------- pointer ----------

  const beginCreate = (event: PointerEvent<HTMLDivElement>, date: string) => {
    if (event.button !== 0 || event.target !== event.currentTarget) return;
    const point = pointAt(event.clientX, event.clientY);
    if (!point) return;
    event.currentTarget.setPointerCapture(event.pointerId);
    const minute = Math.min(snap(point.minute - SNAP / 2), DAY_MINUTES - SNAP);
    setDrag({ mode: "create", date, from: minute, to: minute + SNAP * 4, x: event.clientX, y: event.clientY, moved: false });
  };

  const beginItem = (event: PointerEvent<HTMLElement>, item: CalendarItem, date: string, mode: "move" | "resize") => {
    if (event.button !== 0 || item.allDay) return;
    event.stopPropagation();
    event.currentTarget.setPointerCapture(event.pointerId);
    const point = pointAt(event.clientX, event.clientY);
    setDrag({
      mode,
      item,
      fromDate: date,
      date,
      start: item.startMinute,
      end: item.endMinute,
      grab: point ? point.minute - item.startMinute : 0,
      x: event.clientX,
      y: event.clientY,
      moved: false,
    });
  };

  const onPointerMove = (event: PointerEvent<HTMLElement>) => {
    if (!drag) return;
    const moved = drag.moved || Math.hypot(event.clientX - drag.x, event.clientY - drag.y) > DRAG_THRESHOLD;
    const point = pointAt(event.clientX, event.clientY);
    if (!point) return;
    if (drag.mode === "create") {
      const to = Math.max(drag.from + SNAP, Math.min(DAY_MINUTES, snap(point.minute)));
      setDrag({ ...drag, to, moved });
    } else if (drag.mode === "move") {
      const length = drag.end - drag.start;
      const start = Math.max(0, Math.min(DAY_MINUTES - SNAP, snap(point.minute - drag.grab)));
      setDrag({ ...drag, date: point.date, start, end: start + length, moved });
    } else {
      const end = Math.max(drag.start + SNAP, Math.min(DAY_MINUTES, snap(point.minute)));
      setDrag({ ...drag, end, moved });
    }
  };

  const onPointerUp = () => {
    if (!drag) return;
    setDrag(null);
    if (drag.mode === "create") {
      onCreate({ date: drag.date, startMinute: drag.from, endMinute: drag.moved ? drag.to : drag.from + 60 });
    } else if (!drag.moved) {
      onOpen(drag.item, drag.fromDate);
    } else if (drag.date !== drag.fromDate || drag.start !== drag.item.startMinute || drag.end !== drag.item.endMinute) {
      onMove(drag.item, drag.fromDate, { date: drag.date, startMinute: drag.start, endMinute: drag.end });
    }
  };

  const onItemKey = (event: KeyboardEvent<HTMLElement>, item: CalendarItem, date: string) => {
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      onOpen(item, date);
      return;
    }
    if (!event.altKey || item.allDay) return;
    const step: Record<string, [number, number]> = { ArrowUp: [0, -SNAP], ArrowDown: [0, SNAP], ArrowLeft: [-1, 0], ArrowRight: [1, 0] };
    const move = step[event.key];
    if (!move) return;
    event.preventDefault();
    const [days, minutes] = move;
    const start = Math.max(0, Math.min(DAY_MINUTES - SNAP, item.startMinute + minutes));
    onMove(item, date, { date: addDays(date, days), startMinute: start, endMinute: start + (item.endMinute - item.startMinute) });
  };

  // Escape cancels a drag.
  useEffect(() => {
    if (!drag) return;
    const onKey = (event: globalThis.KeyboardEvent) => {
      if (event.key === "Escape") setDrag(null);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [drag]);

  // ---------- drawing ----------

  const hours = Array.from({ length: 24 }, (_, h) => h);
  const allDay = days.map((day) => day.items.filter((item) => item.allDay));
  const hasAllDay = allDay.some((items) => items.length > 0);

  return (
    <div className={styles.grid} style={{ ["--days" as string]: days.length }}>
      <div className={styles.gridHead}>
        <div className={styles.gutter} />
        {days.map((day) => {
          const date = parseDate(day.date);
          const load = day.load.wakingMin ? Math.min(1, (day.load.scheduledMin + day.load.plannedMin) / day.load.wakingMin) : 0;
          return (
            <div key={day.date} className={`${styles.dayHead} ${day.date === todayIso ? styles.isToday : ""}`}>
              <span className={styles.dayName}>{id.weekdays.short[dayCode(day.date)]}</span>
              <span className={styles.dayNumber}>{date.getDate()}</span>
              <span
                className={`${styles.load} ${day.load.over ? styles.over : ""}`}
                title={id.calendar.over(span(day.load.scheduledMin + day.load.plannedMin), span(day.load.wakingMin))}
                aria-label={
                  day.load.over ? id.calendar.over(span(day.load.scheduledMin + day.load.plannedMin), span(day.load.wakingMin)) : undefined
                }
              >
                <i style={{ transform: `scaleX(${load.toFixed(3)})` }} />
              </span>
            </div>
          );
        })}
      </div>
      {hasAllDay && (
        <div className={styles.allDayRow}>
          <div className={styles.gutterLabel}>{id.calendar.allDay}</div>
          {days.map((day, index) => (
            <div key={day.date} className={styles.allDayCell}>
              {(allDay[index] ?? []).map((item) => (
                <button
                  key={item.key}
                  type="button"
                  data-pigment={item.pigment}
                  className={styles.allDayItem}
                  aria-label={itemLabel(item)}
                  onClick={() => onOpen(item, day.date)}
                >
                  {item.title}
                </button>
              ))}
            </div>
          ))}
        </div>
      )}
      <div ref={scroller} className={styles.scroller}>
        <div className={styles.body} style={{ height: DAY_MINUTES * PPM }}>
          <div className={styles.hours} aria-hidden="true">
            {hours.map((h) => (
              <span key={h} style={{ top: h * 60 * PPM }}>
                {h === 0 ? "" : clock(h * 60)}
              </span>
            ))}
          </div>
          <div ref={columns} className={styles.columns} onPointerMove={onPointerMove} onPointerUp={onPointerUp}>
            {days.map((day) => {
              const timed = day.items.filter((item) => !item.allDay);
              const placed = layoutDay(timed.map((item) => ({ key: item.key, start: item.startMinute, end: item.endMinute })));
              return (
                <div key={day.date} className={styles.column} data-date={day.date} onPointerDown={(event) => beginCreate(event, day.date)}>
                  {timed.map((item) => {
                    const place = placed.get(item.key) ?? { column: 0, columns: 1 };
                    const hidden = drag && drag.mode !== "create" && drag.item.key === item.key && drag.moved;
                    return (
                      <button
                        key={item.key}
                        type="button"
                        data-pigment={item.pigment}
                        className={`${styles.item} ${styles[item.kind]} ${item.taskDone ? styles.taskDone : ""} ${hidden ? styles.ghosted : ""}`}
                        style={{
                          top: item.startMinute * PPM,
                          height: Math.max(18, (item.endMinute - item.startMinute) * PPM - 2),
                          left: `calc(${(place.column / place.columns) * 100}% + 2px)`,
                          width: `calc(${100 / place.columns}% - 4px)`,
                        }}
                        aria-label={itemLabel(item)}
                        onPointerDown={(event) => beginItem(event, item, day.date, "move")}
                        onKeyDown={(event) => onItemKey(event, item, day.date)}
                      >
                        <strong>{item.title}</strong>
                        {item.endMinute - item.startMinute >= 45 && (
                          <span>
                            {clock(item.startMinute)}
                            {item.location ? ` · ${item.location}` : ""}
                          </span>
                        )}
                        <i
                          className={styles.resize}
                          aria-hidden="true"
                          onPointerDown={(event) => beginItem(event, item, day.date, "resize")}
                        />
                      </button>
                    );
                  })}
                  {day.reminders.map((reminder) => (
                    <span
                      key={reminder.id}
                      className={styles.reminder}
                      style={{ top: reminder.minute * PPM }}
                      title={`${clock(reminder.minute)} ${reminder.text}`}
                    >
                      <Bell size={11} aria-hidden="true" />
                      <span className="sr-only">{`${id.reminders.title} ${clock(reminder.minute)}: ${reminder.text}`}</span>
                    </span>
                  ))}
                  {day.date === todayIso && <span className={styles.nowLine} style={{ top: nowMinute * PPM }} aria-hidden="true" />}
                  {drag?.mode === "create" && drag.date === day.date && (
                    <span className={styles.selection} style={{ top: drag.from * PPM, height: (drag.to - drag.from) * PPM }}>
                      {clock(drag.from)} sampai {clock(drag.to)}
                    </span>
                  )}
                  {drag && drag.mode !== "create" && drag.moved && drag.date === day.date && (
                    <span
                      data-pigment={drag.item.pigment}
                      className={`${styles.item} ${styles.dragGhost}`}
                      style={{ top: drag.start * PPM, height: (drag.end - drag.start) * PPM - 2, left: 2, right: 2 }}
                    >
                      <strong>{drag.item.title}</strong>
                      <span>
                        {clock(drag.start)} sampai {clock(drag.end)}
                      </span>
                    </span>
                  )}
                  {external && external.date === day.date && (
                    <span
                      className={styles.selection}
                      style={{ top: external.startMinute * PPM, height: (external.endMinute - external.startMinute) * PPM }}
                    >
                      {clock(external.startMinute)}
                    </span>
                  )}
                </div>
              );
            })}
          </div>
        </div>
      </div>
    </div>
  );
});
