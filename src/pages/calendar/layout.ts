// Calendar arithmetic that has nothing to do with drawing: weeks, months, snapping, and placing
// overlapping items side by side.

import type { Day } from "../../data/generated/Day";
import { isoDate } from "../../time/format";

export const SNAP = 15;
export const DAY_MINUTES = 1440;

export function parseDate(text: string): Date {
  const [y, m, d] = text.split("-").map(Number);
  return new Date(y ?? 2000, (m ?? 1) - 1, d ?? 1);
}

export function addDays(text: string, days: number): string {
  const date = parseDate(text);
  date.setDate(date.getDate() + days);
  return isoDate(date);
}

/** The Monday of the week holding `text` (Indonesian weeks start on Monday). */
export function mondayOf(text: string): string {
  const date = parseDate(text);
  return addDays(text, -((date.getDay() + 6) % 7));
}

/** The first day shown in a month view: the Monday on or before the 1st. */
export function monthStart(text: string): string {
  return mondayOf(`${text.slice(0, 8)}01`);
}

export function addMonths(text: string, months: number): string {
  const date = parseDate(`${text.slice(0, 8)}01`);
  date.setMonth(date.getMonth() + months);
  return isoDate(date);
}

const CODES: Day[] = ["su", "mo", "tu", "we", "th", "fr", "sa"];
export function dayCode(text: string): Day {
  return CODES[parseDate(text).getDay()] ?? "mo";
}

export function snap(minute: number, step = SNAP): number {
  return Math.round(minute / step) * step;
}

export function clampMinute(minute: number): number {
  return Math.max(0, Math.min(DAY_MINUTES, minute));
}

export interface Placed {
  key: string;
  start: number;
  end: number;
}

/** Side-by-side columns for overlapping items: each gets a column and the width of its cluster. */
export function layoutDay(items: readonly Placed[]): Map<string, { column: number; columns: number }> {
  const sorted = [...items].sort((a, b) => a.start - b.start || b.end - a.end);
  const out = new Map<string, { column: number; columns: number }>();
  let cluster: { key: string; column: number }[] = [];
  let columnEnds: number[] = [];
  let clusterEnd = -1;
  const flush = () => {
    for (const item of cluster) out.set(item.key, { column: item.column, columns: columnEnds.length });
    cluster = [];
    columnEnds = [];
  };
  for (const item of sorted) {
    const end = Math.max(item.end, item.start + 15);
    if (item.start >= clusterEnd) {
      flush();
      clusterEnd = end;
    } else {
      clusterEnd = Math.max(clusterEnd, end);
    }
    let column = columnEnds.findIndex((columnEnd) => columnEnd <= item.start);
    if (column < 0) {
      column = columnEnds.length;
      columnEnds.push(end);
    } else {
      columnEnds[column] = end;
    }
    cluster.push({ key: item.key, column });
  }
  flush();
  return out;
}
