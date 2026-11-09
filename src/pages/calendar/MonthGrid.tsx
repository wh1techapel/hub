// The month at a glance: six weeks, the first three things of each day, a day opens in Day view.

import type { CalendarDay } from "../../data/generated/CalendarDay";
import { id } from "../../strings/id";
import { clock } from "../../time/format";
import styles from "./calendar.module.css";
import { parseDate } from "./layout";

const SHOWN = 3;
const HEADS = ["mo", "tu", "we", "th", "fr", "sa", "su"] as const;

export function MonthGrid({
  days,
  month,
  todayIso,
  onOpenDay,
}: {
  days: readonly CalendarDay[];
  month: number;
  todayIso: string;
  onOpenDay: (date: string) => void;
}) {
  return (
    <div className={styles.month}>
      {HEADS.map((day) => (
        <div key={day} className={styles.monthHead}>
          {id.weekdays.short[day]}
        </div>
      ))}
      {days.map((day) => {
        const date = parseDate(day.date);
        const outside = date.getMonth() !== month;
        const more = day.items.length - SHOWN;
        return (
          <button
            key={day.date}
            type="button"
            className={`${styles.monthCell} ${outside ? styles.outside : ""} ${day.date === todayIso ? styles.isToday : ""}`}
            onClick={() => onOpenDay(day.date)}
            aria-label={`${date.getDate()}, ${day.items.length} jadwal`}
          >
            <span className={styles.monthNumber}>{date.getDate()}</span>
            {day.items.slice(0, SHOWN).map((item) => (
              <span key={item.key} className={styles.monthItem} data-pigment={item.pigment}>
                <i aria-hidden="true" />
                {item.allDay ? item.title : `${clock(item.startMinute)} ${item.title}`}
              </span>
            ))}
            {more > 0 && <span className={styles.monthMore}>{id.common.more(more)}</span>}
            {day.load.over && <span className={styles.monthOver} aria-hidden="true" />}
          </button>
        );
      })}
    </div>
  );
}
