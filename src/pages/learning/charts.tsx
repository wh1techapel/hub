// Belajar's small pictures: the week as seven dots, twenty weeks as a heatmap, and the domains as
// bars. Colour is never the only carrier: every cell and bar says its value in words too.

import { useRef, useState } from "react";
import type { CSSProperties, KeyboardEvent } from "react";

import type { DayMinutes } from "../../data/generated/DayMinutes";
import type { DomainStat } from "../../data/generated/DomainStat";
import { id } from "../../strings/id";
import { dateLabel, span } from "../../time/format";
import styles from "./learning.module.css";

const DAY_CODES = ["mo", "tu", "we", "th", "fr", "sa", "su"] as const;

const dateOf = (text: string) => new Date(`${text}T00:00`);

/** Monday to Sunday; a filled dot is a day with learning, today has a ring. */
export function WeekDots({ days, todayIso }: { days: readonly DayMinutes[]; todayIso: string }) {
  return (
    <ol className={styles.weekDots}>
      {days.map((day, index) => {
        const code = DAY_CODES[index] ?? "mo";
        const name = id.weekdays.long[code];
        const label = day.minutes > 0 ? id.learning.dayActive(name, span(day.minutes)) : id.learning.dayRest(name);
        return (
          <li
            key={day.date}
            className={day.minutes > 0 ? styles.active : undefined}
            aria-current={day.date === todayIso ? "date" : undefined}
            title={label}
          >
            <i aria-hidden="true" />
            <span aria-hidden="true">{id.weekdays.short[code].slice(0, 1)}</span>
            <span className="sr-only">{label}</span>
          </li>
        );
      })}
    </ol>
  );
}

/** Minutes to one of five steps of ink. */
function level(minutes: number): number {
  if (minutes <= 0) return 0;
  if (minutes < 30) return 1;
  if (minutes < 60) return 2;
  if (minutes < 120) return 3;
  return 4;
}

/** Twenty weeks, a column each, Monday at the top. Arrow keys walk the days; each says its time. */
export function Heatmap({ cells, todayIso }: { cells: readonly DayMinutes[]; todayIso: string }) {
  const weeks = Math.ceil(cells.length / 7);
  const todayIndex = Math.max(
    0,
    cells.findIndex((c) => c.date === todayIso),
  );
  const [active, setActive] = useState(todayIndex);
  const grid = useRef<HTMLDivElement>(null);
  const days = cells.filter((c) => c.minutes > 0).length;
  const total = cells.reduce((sum, c) => sum + c.minutes, 0);

  const move = (event: KeyboardEvent<HTMLDivElement>) => {
    const steps: Record<string, number> = {
      ArrowUp: -1,
      ArrowDown: 1,
      ArrowLeft: -7,
      ArrowRight: 7,
      Home: -active,
      End: todayIndex - active,
    };
    const step = steps[event.key];
    if (step === undefined) return;
    event.preventDefault();
    const next = Math.min(cells.length - 1, Math.max(0, active + step));
    setActive(next);
    grid.current?.querySelector<HTMLElement>(`[data-cell="${next}"]`)?.focus();
  };

  // Month names above the first week that starts in that month.
  const months = Array.from({ length: weeks }, (_, week) => {
    const first = cells[week * 7];
    const previous = week > 0 ? cells[(week - 1) * 7] : undefined;
    if (!first) return "";
    const month = dateOf(first.date).getMonth();
    return !previous || dateOf(previous.date).getMonth() !== month
      ? new Intl.DateTimeFormat("id-ID", { month: "short" }).format(dateOf(first.date))
      : "";
  });

  return (
    <figure className={styles.heatmap}>
      <figcaption className={styles.heatmapSummary}>{id.learning.heatmapSummary(days, span(total))}</figcaption>
      <div className={styles.months} style={{ "--weeks": weeks } as CSSProperties} aria-hidden="true">
        {months.map((month, week) => (
          <span key={week}>{month}</span>
        ))}
      </div>
      <div
        ref={grid}
        className={styles.cells}
        role="grid"
        aria-label={id.learning.heatmap}
        aria-readonly="true"
        style={{ "--weeks": weeks } as CSSProperties}
        onKeyDown={move}
      >
        {Array.from({ length: 7 }, (_, row) => (
          <div key={row} role="row" className={styles.cellRow}>
            {Array.from({ length: weeks }, (_, week) => {
              const index = week * 7 + row;
              const cell = cells[index];
              if (!cell) return <span key={week} role="gridcell" />;
              const future = cell.date > todayIso;
              const label = id.learning.heatmapCell(
                dateLabel(dateOf(cell.date)),
                cell.minutes > 0 ? span(cell.minutes) : id.learning.noTime,
              );
              return (
                <span
                  key={week}
                  role="gridcell"
                  data-cell={index}
                  data-level={future ? "future" : level(cell.minutes)}
                  tabIndex={index === active ? 0 : -1}
                  aria-label={label}
                  title={future ? undefined : label}
                  className={styles.cell}
                  onFocus={() => setActive(index)}
                />
              );
            })}
          </div>
        ))}
      </div>
      <div className={styles.legend} aria-hidden="true">
        <span>{id.learning.heatmapLess}</span>
        {[0, 1, 2, 3, 4].map((step) => (
          <span key={step} className={styles.cell} data-level={step} />
        ))}
        <span>{id.learning.heatmapMore}</span>
      </div>
    </figure>
  );
}

export function DomainBars({ domains }: { domains: readonly DomainStat[] }) {
  if (domains.length === 0) return <p className={styles.quiet}>{id.learning.domainsEmpty}</p>;
  const most = Math.max(1, ...domains.map((d) => d.minutes));
  return (
    <ul className={styles.domains}>
      {domains.map((domain) => (
        <li key={domain.domain}>
          <span className={styles.domainName}>{id.learning.domains[domain.domain]}</span>
          <span className={styles.domainBar} aria-hidden="true">
            <i style={{ transform: `scaleX(${domain.minutes / most})` }} />
          </span>
          <span className={styles.domainValue}>
            {domain.minutes > 0 ? span(domain.minutes) : id.learning.noTime}
            <small>{id.learning.domainLine(domain.done, domain.items)}</small>
          </span>
        </li>
      ))}
    </ul>
  );
}
