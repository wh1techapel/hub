// Geometry of the Day Arc (docs/design.md, "Signature elements"): a shallow circular arc from
// wake time (left horizon) to sleep time (right horizon), in a 1000 x 190 drawing.

import type { CalendarItem } from "../../data/generated/CalendarItem";

export const VIEW = { width: 1000, height: 190 };

const LEFT = { x: 40, y: 160 };
const RIGHT = { x: 960, y: 160 };
const APEX_Y = 30;

/** The circle through both horizons and the apex. */
export const ARC = (() => {
  const half = (RIGHT.x - LEFT.x) / 2;
  const sag = LEFT.y - APEX_Y;
  const r = (half * half + sag * sag) / (2 * sag);
  const cx = (LEFT.x + RIGHT.x) / 2;
  const cy = APEX_Y + r;
  return {
    r,
    cx,
    cy,
    a0: Math.atan2(LEFT.y - cy, LEFT.x - cx),
    a1: Math.atan2(RIGHT.y - cy, RIGHT.x - cx),
  };
})();

/** Where a minute of the day falls on the arc, 0 (wake) to 1 (sleep). */
export function frac(minute: number, wake: number, sleep: number): number {
  if (sleep <= wake) return 0;
  return Math.min(1, Math.max(0, (minute - wake) / (sleep - wake)));
}

/** A point on the arc at `t`, `offset` pixels outward (negative: toward the centre). */
export function arcPoint(t: number, offset = 0): [number, number] {
  const a = ARC.a0 + (ARC.a1 - ARC.a0) * t;
  const r = ARC.r + offset;
  return [ARC.cx + r * Math.cos(a), ARC.cy + r * Math.sin(a)];
}

const fixed = (n: number) => n.toFixed(1);

export function arcPath(t0: number, t1: number): string {
  const [x0, y0] = arcPoint(t0);
  const [x1, y1] = arcPoint(t1);
  return `M${fixed(x0)} ${fixed(y0)} A${fixed(ARC.r)} ${fixed(ARC.r)} 0 0 1 ${fixed(x1)} ${fixed(y1)}`;
}

export interface Moment {
  current: CalendarItem | undefined;
  next: CalendarItem | undefined;
}

/** The event under way and the next one to start. All-day events are not moments. */
export function momentAt(events: readonly CalendarItem[], minute: number): Moment {
  const timed = events.filter((event) => !event.allDay).sort((a, b) => a.startMinute - b.startMinute);
  return {
    current: timed.find((event) => event.startMinute <= minute && minute < event.endMinute),
    next: timed.find((event) => event.startMinute > minute),
  };
}

export interface ArcLabel {
  x: number;
  y: number;
  text: string;
  strong: boolean;
}

/** Labels for the current and next events, outside the arc. A label steps further out when it
 *  would collide with the previous label or sit on the sun. */
export function eventLabels(moment: Moment, wake: number, sleep: number, sunT: number | null): ArcLabel[] {
  const sunX = sunT === null ? Number.NEGATIVE_INFINITY : arcPoint(sunT)[0];
  let lastX = Number.NEGATIVE_INFINITY;
  const labels: ArcLabel[] = [];
  for (const event of [moment.current, moment.next]) {
    if (!event) continue;
    const mid = (frac(event.startMinute, wake, sleep) + frac(event.endMinute, wake, sleep)) / 2;
    const midX = arcPoint(mid)[0];
    const near = Math.abs(midX - lastX) < 140 || Math.abs(midX - sunX) < 80;
    const [x, y] = arcPoint(mid, near ? 40 : 20);
    lastX = x;
    labels.push({ x, y, text: event.title, strong: event === moment.current });
  }
  return labels;
}
