// Sekarang: what is happening now or next, and the reminder still to come. The one raised surface
// on the page.

import { Bell } from "lucide-react";

import type { CalendarItem } from "../../data/generated/CalendarItem";
import type { DayReminder } from "../../data/generated/DayReminder";
import { id } from "../../strings/id";
import { clock, span } from "../../time/format";
import type { Moment } from "./arcGeometry";
import styles from "./today.module.css";

interface Props {
  moment: Moment;
  hasEvents: boolean;
  reminders: readonly DayReminder[];
  minute: number;
  sleep: number;
}

function EventLine({ event, minute }: { event: CalendarItem; minute: number }) {
  const running = event.startMinute <= minute && minute < event.endMinute;
  const progress = running ? (minute - event.startMinute) / (event.endMinute - event.startMinute) : 0;
  const where = event.location ? `, ${event.location}` : "";
  return (
    <>
      <div className={styles.nowEvent} data-pigment={event.pigment}>
        <span className={styles.rail} />
        <div>
          <h3>{event.title}</h3>
          <p>
            {id.today.until_time(clock(event.startMinute), clock(event.endMinute))}
            {where}
          </p>
        </div>
        <div className={styles.nowLeft}>
          <strong>{span(running ? event.endMinute - minute : event.startMinute - minute)}</strong>
          <span>{running ? id.today.left : id.today.until}</span>
        </div>
      </div>
      {running && (
        <div className={styles.bar} role="presentation">
          <i style={{ transform: `scaleX(${progress.toFixed(3)})` }} />
        </div>
      )}
    </>
  );
}

export function NowCard({ moment, hasEvents, reminders, minute, sleep }: Props) {
  const event = moment.current ?? moment.next;
  const reminder = reminders.find((r) => r.minute >= minute) ?? reminders[reminders.length - 1];
  return (
    <div className={styles.nowCard}>
      {event ? (
        <EventLine event={event} minute={minute} />
      ) : (
        <div className={styles.nowEvent} data-pigment="batu">
          <span className={styles.rail} />
          <div>
            <h3>{hasEvents ? id.today.noMore : id.today.emptyDay}</h3>
            <p>{id.today.freeUntil(clock(sleep))}</p>
          </div>
        </div>
      )}
      {reminder && (
        <>
          <div className={styles.nowSep} />
          <div className={styles.nowNext}>
            <Bell size={16} strokeWidth={1.6} aria-hidden="true" />
            <span>
              {reminder.minute >= minute ? id.today.reminderAhead : id.today.reminderPast} {clock(reminder.minute)}:{" "}
              {reminder.text}
            </span>
          </div>
        </>
      )}
    </div>
  );
}
