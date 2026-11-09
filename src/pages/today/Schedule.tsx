// Jadwal: today's events and reminders in time order; the one under way is ringed in ink.

import { Bell } from "lucide-react";

import type { CalendarItem } from "../../data/generated/CalendarItem";
import type { DayReminder } from "../../data/generated/DayReminder";
import { id } from "../../strings/id";
import { clock } from "../../time/format";
import styles from "./today.module.css";

type Row = { kind: "event"; at: number; event: CalendarItem } | { kind: "reminder"; at: number; reminder: DayReminder };

export function Schedule({ events, reminders, minute }: { events: readonly CalendarItem[]; reminders: readonly DayReminder[]; minute: number }) {
  const rows: Row[] = [
    ...events.map((event): Row => ({ kind: "event", at: event.allDay ? -1 : event.startMinute, event })),
    ...reminders.map((reminder): Row => ({ kind: "reminder", at: reminder.minute, reminder })),
  ].sort((a, b) => a.at - b.at);

  if (rows.length === 0) return <p className={styles.empty}>{id.today.scheduleEmpty}</p>;

  return (
    <ol className={styles.schedule}>
      {rows.map((row) => {
        if (row.kind === "reminder") {
          return (
            <li key={row.reminder.id} className={`${styles.slot} ${styles.reminderSlot}`}>
              <time>{clock(row.reminder.minute)}</time>
              <div className={styles.block}>
                <Bell size={15} strokeWidth={1.6} aria-hidden="true" />
                {row.reminder.text}
              </div>
            </li>
          );
        }
        const { event } = row;
        const state = event.allDay
          ? ""
          : minute >= event.endMinute
            ? styles.past
            : minute >= event.startMinute
              ? styles.nowSlot
              : "";
        return (
          <li key={event.key} className={`${styles.slot} ${state}`} data-pigment={event.pigment}>
            <time>{event.allDay ? "" : clock(event.startMinute)}</time>
            <div className={styles.block}>
              <strong>{event.title}</strong>
              {!event.allDay && <span>{id.today.until_time(clock(event.startMinute), clock(event.endMinute))}</span>}
            </div>
          </li>
        );
      })}
    </ol>
  );
}
