// How Hub writes times and dates: 24-hour with a dot ("07.00", as Indonesian writes it), spans
// as "1 j 12 m", dates in Indonesian.

import { id } from "../strings/id";

const pad = (n: number) => String(n).padStart(2, "0");

/** Minutes from midnight as "07.00". */
export function clock(minutes: number): string {
  const m = ((Math.round(minutes) % 1440) + 1440) % 1440;
  return `${pad(Math.floor(m / 60))}.${pad(m % 60)}`;
}

/** A duration in minutes as "1 j 12 m", "25 m", "2 j". */
export function span(minutes: number): string {
  const total = Math.max(0, Math.round(minutes));
  const h = Math.floor(total / 60);
  const m = total % 60;
  if (!h) return `${m} m`;
  return m ? `${h} j ${m} m` : `${h} j`;
}

/** "05:00" (as stored) to minutes from midnight; null when malformed. */
export function parseClock(text: string): number | null {
  const match = /^(\d{2}):(\d{2})$/.exec(text);
  if (!match) return null;
  const h = Number(match[1]);
  const m = Number(match[2]);
  if (h > 23 || m > 59) return null;
  return h * 60 + m;
}

export function minutesOf(date: Date): number {
  return date.getHours() * 60 + date.getMinutes();
}

/** The local date as stored: "2026-10-22". */
export function isoDate(date: Date): string {
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
}

const dayFormat = new Intl.DateTimeFormat("id-ID", { weekday: "long", day: "numeric", month: "long" });
const weekdayFormat = new Intl.DateTimeFormat("id-ID", { weekday: "long" });
const shortFormat = new Intl.DateTimeFormat("id-ID", { day: "numeric", month: "short" });

/** "Kamis, 22 Oktober". */
export function dateLabel(date: Date): string {
  return dayFormat.format(date);
}

function fromIso(text: string): Date | null {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(text);
  if (!match) return null;
  return new Date(Number(match[1]), Number(match[2]) - 1, Number(match[3]));
}

/** A stored date relative to today: "hari ini", "besok", a weekday within a week, else "25 Okt". */
export function relativeDate(text: string, today: Date): string {
  const date = fromIso(text);
  if (!date) return text;
  const start = new Date(today.getFullYear(), today.getMonth(), today.getDate());
  const days = Math.round((date.getTime() - start.getTime()) / 86_400_000);
  if (days === 0) return "hari ini";
  if (days === 1) return "besok";
  if (days === -1) return "kemarin";
  if (days > 1 && days < 7) return weekdayFormat.format(date);
  return shortFormat.format(date);
}

/** When something in Jejak happened: "11.20" today, "Rabu" this week, else "18 Okt". */
export function whenLabel(instant: string, now: Date): string {
  const at = new Date(instant);
  if (Number.isNaN(at.getTime())) return "";
  if (isoDate(at) === isoDate(now)) return clock(minutesOf(at));
  const days = (now.getTime() - at.getTime()) / 86_400_000;
  if (days < 6) return weekdayFormat.format(at);
  return shortFormat.format(at);
}

/** Selamat pagi until 11.00, siang until 15.00, sore until 18.30, then malam (until dawn). */
export function greeting(minutes: number, wake: number): string {
  if (minutes >= wake - 30 && minutes < 660) return id.greeting.pagi;
  if (minutes >= 660 && minutes < 900) return id.greeting.siang;
  if (minutes >= 900 && minutes < 1110) return id.greeting.sore;
  return id.greeting.malam;
}
