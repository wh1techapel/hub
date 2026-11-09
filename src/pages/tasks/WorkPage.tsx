// Proyek & Tugas (R6): one place for the projects, by Ruang, and the task lists: Kotak Masuk,
// Mendatang, Semua, Selesai. It opens on the projects. A task opens in the panel beside its list
// (its address carries it, so ULTRON and the palette can open one). Tugas's Hari Ini list is gone
// (Hari Ini, the dashboard, is that list), and Pengingat moved to Kalender; their old addresses
// still lead there (router.tsx).

import { useNavigate, useSearch } from "@tanstack/react-router";
import { CalendarDays, CheckCheck, FolderKanban, Inbox, Layers } from "lucide-react";
import type { LucideIcon } from "lucide-react";

import { CaptureForm } from "../../components/CaptureForm";
import { TaskList } from "../../components/TaskList";
import { useToast } from "../../components/Toast";
import type { TaskCard } from "../../data/generated/TaskCard";
import type { TaskView } from "../../data/generated/TaskView";
import { useTaskCounts, useTasks } from "../../data/queries";
import { id } from "../../strings/id";
import { useNow } from "../../time/clock";
import { dateLabel, isoDate, relativeDate } from "../../time/format";
import { ProjectsOverview } from "../projects/ProjectsPage";
import { TaskPanel } from "./TaskPanel";
import styles from "./tasks.module.css";

/** The task lists this place shows (the core's `today` list is Hari Ini's, on the dashboard). */
export type ListView = Exclude<TaskView, "today">;
export type Place = "projects" | ListView;

const LISTS: { place: ListView; icon: LucideIcon }[] = [
  { place: "inbox", icon: Inbox },
  { place: "upcoming", icon: CalendarDays },
  { place: "anytime", icon: Layers },
  { place: "done", icon: CheckCheck },
];

const HAND_SORTED: ReadonlySet<Place> = new Set(["inbox", "anytime"]);

export function isPlace(value: unknown): value is Place {
  return value === "projects" || LISTS.some((p) => p.place === value);
}

export function WorkPage() {
  const now = useNow();
  const todayIso = isoDate(now);
  const navigate = useNavigate();
  const search: { view?: string; task?: string } = useSearch({ strict: false });
  const place: Place = isPlace(search.view) ? search.view : "projects";
  const selected = typeof search.task === "string" ? search.task : null;
  const counts = useTaskCounts(todayIso);
  const toast = useToast();

  const go = (next: { view?: Place; task?: string | null }) => {
    const view = next.view ?? place;
    const task = next.task === undefined ? selected : next.task;
    void navigate({ to: "/proyek", search: task ? { view, task } : { view } });
  };

  const countOf = (p: ListView): number | null => {
    if (!counts.data || p === "done") return null;
    return counts.data[p];
  };

  return (
    <div className={styles.page}>
      <nav className={styles.places} aria-label={id.tasks.title}>
        <h1 className={`display ${styles.title}`}>{id.tasks.title}</h1>
        <button
          type="button"
          className={styles.place}
          aria-current={place === "projects" ? "page" : undefined}
          onClick={() => go({ view: "projects", task: null })}
        >
          <FolderKanban size={17} strokeWidth={1.6} aria-hidden="true" />
          <span>{id.tasks.projects}</span>
        </button>
        <div className={styles.placeGroup} role="group" aria-labelledby="places-tasks">
          <p className={styles.placeLabel} id="places-tasks">
            {id.tasks.groupTasks}
          </p>
          {LISTS.map(({ place: p, icon: Icon }) => {
            const name = id.tasks.views[p];
            const count = countOf(p);
            return (
              <button
                key={p}
                type="button"
                className={styles.place}
                aria-label={count ? `${name} ${count}` : undefined}
                aria-current={p === place ? "page" : undefined}
                onClick={() => go({ view: p, task: null })}
              >
                <Icon size={17} strokeWidth={1.6} aria-hidden="true" />
                <span>{name}</span>
                {count ? <em>{count}</em> : null}
              </button>
            );
          })}
        </div>
      </nav>

      {place === "projects" ? (
        <section className={styles.main} aria-labelledby="place-title">
          <ProjectsOverview titleId="place-title" />
        </section>
      ) : (
        <section className={styles.main} aria-labelledby="place-title">
          <header className={styles.head}>
            <h2 className="display" id="place-title">
              {id.tasks.views[place]}
            </h2>
            <p>{id.tasks.hints[place]}</p>
          </header>
          {place !== "done" && (
            <div className={styles.add}>
              <CaptureForm
                kinds={["task"]}
                compact
                placeholder={id.tasks.addPlaceholder}
                onSaved={(saved) => toast({ text: id.tasks.created(saved.title) })}
              />
            </div>
          )}
          <TaskLists place={place} todayIso={todayIso} now={now} selected={selected} onOpen={(task) => go({ task: task.id })} />
          <p className={styles.keys}>{id.tasks.keyboardHint}</p>
        </section>
      )}

      {selected && place !== "projects" && (
        <TaskPanel taskId={selected} onClose={() => go({ task: null })} onOpen={(taskId) => go({ task: taskId })} />
      )}
    </div>
  );
}

function TaskLists({
  place,
  todayIso,
  now,
  selected,
  onOpen,
}: {
  place: ListView;
  todayIso: string;
  now: Date;
  selected: string | null;
  onOpen: (task: TaskCard) => void;
}) {
  const tasks = useTasks(place, todayIso);
  const list = tasks.data ?? [];
  if (tasks.isPending) return null;
  if (list.length === 0) return <p className={styles.empty}>{id.tasks.empty[place]}</p>;

  if (place === "upcoming") {
    // Grouped by day, in date order.
    const groups = new Map<string, TaskCard[]>();
    for (const task of list) {
      const day = task.doDate ?? task.dueDate ?? todayIso;
      groups.set(day, [...(groups.get(day) ?? []), task]);
    }
    return (
      <div className={styles.groups}>
        {[...groups.entries()].map(([day, items]) => (
          <section key={day} aria-label={relativeDate(day, now)}>
            <h3 className={styles.groupHead}>
              <span>{relativeDate(day, now)}</span>
              <small>{dateLabel(new Date(`${day}T00:00`))}</small>
            </h3>
            <TaskList tasks={items} label={relativeDate(day, now)} today={now} todayIso={todayIso} selectedId={selected} onOpen={onOpen} />
          </section>
        ))}
      </div>
    );
  }

  return (
    <TaskList
      tasks={list}
      label={id.tasks.views[place]}
      today={now}
      todayIso={todayIso}
      reorderable={HAND_SORTED.has(place)}
      showDoDate={place === "anytime"}
      quiet={place === "done"}
      selectedId={selected}
      onOpen={onOpen}
    />
  );
}
