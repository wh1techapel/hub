// Jejak: this week against the owner's four-week average (plain sentences, small sparklines),
// then everything that changed, by day, filtered by who did it and what it was about.

import { useNavigate, useSearch } from "@tanstack/react-router";

import type { Actor } from "../../data/generated/Actor";
import type { ActivityItem } from "../../data/generated/ActivityItem";
import type { Measure } from "../../data/generated/Measure";
import type { TrailGroup } from "../../data/generated/TrailGroup";
import type { WeekInsight } from "../../data/generated/WeekInsight";
import { useTrail, useWeekInsight } from "../../data/queries";
import { id, trailSentence } from "../../strings/id";
import { useNow } from "../../time/clock";
import { clock, dateLabel, isoDate, minutesOf, relativeDate, span } from "../../time/format";
import styles from "./trail.module.css";

export interface TrailSearch {
  actor?: Actor;
  group?: TrailGroup;
}

const GROUPS: TrailGroup[] = ["tasks", "projects", "calendar", "files", "learning", "notes", "studio"];
const ACTORS: Actor[] = ["owner", "ultron", "hub"];
type MeasureKey = keyof Omit<WeekInsight, "weekStart">;
const MEASURES: { key: MeasureKey; format: (n: number) => string }[] = [
  { key: "tasksDone", format: String },
  { key: "learningMinutes", format: (n) => span(n) },
  { key: "activeDays", format: String },
  { key: "filesAdded", format: String },
  { key: "projectsMoved", format: String },
];

/** Eight weeks as bars; the last (this week) in ink. Decorative: the sentence beside it says it. */
function Sparkline({ weeks }: { weeks: readonly number[] }) {
  const most = Math.max(1, ...weeks);
  return (
    <svg className={styles.spark} viewBox={`0 0 ${weeks.length * 8} 24`} aria-hidden="true" preserveAspectRatio="none">
      {weeks.map((value, index) => {
        const height = Math.max(1.5, (value / most) * 22);
        return (
          <rect
            key={index}
            x={index * 8 + 1}
            y={24 - height}
            width={6}
            height={height}
            rx={1.5}
            className={index === weeks.length - 1 ? styles.now : undefined}
          />
        );
      })}
    </svg>
  );
}

function compare(measure: Measure): string {
  if (measure.average === 0 && measure.value === 0) return id.trail.same;
  const ratio = measure.average === 0 ? 2 : measure.value / measure.average;
  if (ratio >= 1.15) return id.trail.above;
  if (ratio <= 0.85) return id.trail.below;
  return id.trail.same;
}

function Insight({ insight }: { insight: WeekInsight }) {
  return (
    <ul className={styles.measures} aria-label={id.trail.week}>
      {MEASURES.map(({ key, format }) => {
        const measure = insight[key];
        const average = format(Math.round(measure.average * 10) / 10).replace(".", ",");
        return (
          <li key={key}>
            <span className={styles.measureLabel}>{id.trail.measures[key]}</span>
            <span className={styles.measureRow}>
              <strong className={styles.measureValue}>{format(measure.value)}</strong>
              <Sparkline weeks={measure.weeks} />
            </span>
            <span className={styles.measureNote}>
              {compare(measure)}, {id.trail.average(average)}
            </span>
          </li>
        );
      })}
    </ul>
  );
}

/** Rows grouped by local day, newest first. */
function byDay(items: readonly ActivityItem[]): [string, ActivityItem[]][] {
  const days = new Map<string, ActivityItem[]>();
  for (const item of items) {
    const day = isoDate(new Date(item.at));
    days.set(day, [...(days.get(day) ?? []), item]);
  }
  return [...days.entries()];
}

export function TrailPage() {
  const search: TrailSearch = useSearch({ strict: false });
  const navigate = useNavigate();
  const now = useNow();
  const insight = useWeekInsight();
  const trail = useTrail({ actor: search.actor, group: search.group, limit: 60 });
  const items = trail.data?.pages.flatMap((page) => page.items) ?? [];
  const go = (next: TrailSearch) => void navigate({ to: "/jejak", search: { ...search, ...next } });

  return (
    <div className={styles.page}>
      <header className={styles.header}>
        <h1 className={`display ${styles.title}`}>{id.trail.title}</h1>
        <p>{id.trail.hint}</p>
      </header>

      <section className={styles.week} aria-labelledby="week-title">
        <div className={styles.weekHead}>
          <h2 className="section-label" id="week-title">
            {id.trail.week}
          </h2>
          <span>{id.trail.weekHint}</span>
        </div>
        {insight.data && <Insight insight={insight.data} />}
      </section>

      <div className={styles.filters} role="group" aria-label={id.trail.filters}>
        <div className={styles.chips}>
          <button type="button" className={styles.chip} aria-pressed={!search.actor} onClick={() => go({ actor: undefined })}>
            {id.trail.everyone}
          </button>
          {ACTORS.map((actor) => (
            <button
              key={actor}
              type="button"
              className={`${styles.chip} ${actor === "ultron" ? styles.ultronChip : ""}`}
              aria-pressed={search.actor === actor}
              onClick={() => go({ actor })}
            >
              {id.trail.actor[actor]}
            </button>
          ))}
        </div>
        <div className={styles.chips}>
          <button type="button" className={styles.chip} aria-pressed={!search.group} onClick={() => go({ group: undefined })}>
            {id.trail.groups.all}
          </button>
          {GROUPS.map((group) => (
            <button key={group} type="button" className={styles.chip} aria-pressed={search.group === group} onClick={() => go({ group })}>
              {id.trail.groups[group]}
            </button>
          ))}
        </div>
      </div>

      {trail.isSuccess && items.length === 0 && <p className={styles.empty}>{id.trail.empty}</p>}
      <div className={styles.days}>
        {byDay(items).map(([day, rows]) => (
          <section key={day} aria-labelledby={`day-${day}`} className={styles.day}>
            <h2 className={styles.dayHead} id={`day-${day}`}>
              <span>{relativeDate(day, now)}</span>
              <small>{dateLabel(new Date(`${day}T00:00`))}</small>
            </h2>
            <ol className={styles.rows}>
              {rows.map((item) => (
                <li key={item.id} className={item.actor === "ultron" ? styles.byUltron : item.actor === "hub" ? styles.byHub : undefined}>
                  <time dateTime={item.at}>{clock(minutesOf(new Date(item.at)))}</time>
                  <span>{trailSentence(item)}</span>
                </li>
              ))}
            </ol>
          </section>
        ))}
      </div>
      {trail.hasNextPage ? (
        <button type="button" className="btn btn-secondary" disabled={trail.isFetchingNextPage} onClick={() => void trail.fetchNextPage()}>
          {id.trail.more}
        </button>
      ) : (
        items.length > 0 && <p className={styles.end}>{id.trail.end}</p>
      )}
    </div>
  );
}
