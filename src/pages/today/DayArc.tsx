// Busur Hari: the day from wake to sleep, today's events on it, the sun at the current minute (a
// moon outside waking hours). Redrawn once a minute by the shared clock; nothing animates at rest.

import { useState } from "react";

import type { CalendarItem } from "../../data/generated/CalendarItem";
import type { DayReminder } from "../../data/generated/DayReminder";
import { id } from "../../strings/id";
import { clock, span } from "../../time/format";
import { arcPath, arcPoint, eventLabels, frac, momentAt, VIEW } from "./arcGeometry";
import styles from "./today.module.css";

interface Props {
  events: readonly CalendarItem[];
  reminders: readonly DayReminder[];
  /** Minutes from local midnight. */
  minute: number;
  wake: number;
  sleep: number;
  /** Tutup Hari is done: the ink runs to the right horizon and the moon rises there. */
  closed?: boolean;
}

export function DayArc({ events, reminders, minute, wake, sleep, closed = false }: Props) {
  const asleep = minute >= sleep;
  const beforeWake = minute < wake && !closed;
  const night = asleep || beforeWake || closed;
  const t = closed ? 1 : frac(minute, wake, sleep);
  // The ink runs only when the day is closed while the arc is on screen, not on every visit after.
  const [openAtFirst] = useState(!closed);
  const draw = closed && openAtFirst;
  const moment = momentAt(events, minute);
  const labels = eventLabels(moment, wake, sleep, night ? null : t);
  const timed = events.filter((event) => !event.allDay);
  const [lx, ly] = arcPoint(0);
  const [rx, ry] = arcPoint(1);
  const [nx, ny] = arcPoint(frac(12 * 60, wake, sleep), -22);
  const showNoon = night || Math.abs(minute - 720) > 75;
  const [sx, sy] = arcPoint(t);
  const [mx, my] = beforeWake ? [lx, ly] : [rx, ry];

  const first = timed[0];
  let caption: string;
  if (closed) caption = id.arc.closed;
  else if (asleep) caption = id.arc.asleep;
  else if (beforeWake) caption = first ? id.arc.startsAt("today", clock(first.startMinute), first.title) : id.arc.startsFree("today");
  else {
    const left = id.arc.dayLeft(span(sleep - minute));
    caption = moment.next
      ? `${left} · ${id.arc.next(moment.next.title, clock(moment.next.startMinute), span(moment.next.startMinute - minute))}`
      : `${left} · ${id.arc.noMore}`;
  }

  return (
    <figure className={styles.arcWrap}>
      <svg
        className={styles.arc}
        viewBox={`0 0 ${VIEW.width} ${VIEW.height}`}
        role="img"
        aria-label={`${id.arc.label}: ${id.arc.summary(timed.length, clock(minute))}`}
      >
        <path d={arcPath(0, 1)} className={styles.arcFuture} />
        {/* The whole arc, revealed up to now by its dash; closing the day lets the ink run on. */}
        <path d={arcPath(0, 1)} pathLength={1} className={styles.arcPast} strokeDasharray="1 1" strokeDashoffset={1 - t} />
        {closed && <path d={arcPath(0, 1)} pathLength={1} className={`${styles.arcInk} ${draw ? styles.arcInkDraw : ""}`} />}
        {timed.map((event) => (
          <g key={event.key} data-pigment={event.pigment}>
            <path
              d={arcPath(frac(event.startMinute, wake, sleep) + 0.003, frac(event.endMinute, wake, sleep) - 0.003)}
              className={styles.arcEvent}
              opacity={event.endMinute <= minute ? 0.45 : 1}
            />
          </g>
        ))}
        {reminders.map((reminder) => {
          const [x, y] = arcPoint(frac(reminder.minute, wake, sleep));
          return <circle key={reminder.id} cx={x} cy={y} r={4} className={styles.arcReminder} />;
        })}
        {labels.map((label) => (
          <text key={label.text} x={label.x} y={label.y} textAnchor="middle" className={label.strong ? styles.arcStrong : undefined}>
            {label.text}
          </text>
        ))}
        <text x={lx} y={ly + 26} textAnchor="middle">
          {clock(wake)}
        </text>
        <text x={rx} y={ry + 26} textAnchor="middle">
          {clock(sleep)}
        </text>
        {showNoon && (
          <text x={nx} y={ny} textAnchor="middle">
            {clock(12 * 60)}
          </text>
        )}
        {night ? (
          <g className={`${styles.moon} ${draw ? styles.moonRise : ""}`}>
            <circle cx={mx} cy={my - 26} r={13} className={styles.moonHalo} />
            <path d={`M${mx + 6} ${my - 21}a8 8 0 1 1 -5 -12a6.5 6.5 0 0 0 5 12z`} className={styles.moonBody} />
          </g>
        ) : (
          <g>
            <circle cx={sx} cy={sy} r={16} className={styles.sunHalo} />
            <circle cx={sx} cy={sy} r={7.5} className={styles.sun} />
            <text x={sx} y={sy + 34} textAnchor="middle" className={styles.arcStrong}>
              {clock(minute)}
            </text>
          </g>
        )}
      </svg>
      <figcaption className={styles.arcCaption}>{caption}</figcaption>
    </figure>
  );
}
