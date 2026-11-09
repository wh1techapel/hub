// Tugas tanpa jam: this week's tasks that have no time block yet. Drag one onto the calendar to
// give it time, or press Jadwalkan to choose the time in a dialog.

import { CalendarPlus } from "lucide-react";
import type { PointerEvent } from "react";

import type { TaskCard } from "../../data/generated/TaskCard";
import { id } from "../../strings/id";
import { relativeDate, span } from "../../time/format";
import styles from "./calendar.module.css";

interface Props {
  tasks: readonly TaskCard[];
  today: Date;
  onSchedule: (task: TaskCard) => void;
  onDragStart: (event: PointerEvent<HTMLElement>, task: TaskCard) => void;
}

export function TaskTray({ tasks, today, onSchedule, onDragStart }: Props) {
  return (
    <aside className={styles.tray} aria-labelledby="tray-title">
      <h2 className="section-label" id="tray-title">
        {id.calendar.tray}
      </h2>
      <p className={styles.trayHint}>{id.calendar.trayHint}</p>
      {tasks.length === 0 ? (
        <p className={styles.trayHint}>{id.calendar.trayEmpty}</p>
      ) : (
        <ul className={styles.trayList}>
          {tasks.map((task) => (
            <li
              key={task.id}
              className={styles.trayItem}
              data-pigment={task.project?.pigment ?? "batu"}
              onPointerDown={(event) => onDragStart(event, task)}
            >
              <span className={styles.trayTitle}>{task.title}</span>
              <span className={styles.trayMeta}>
                {task.doDate && relativeDate(task.doDate, today)}
                {task.estimateMin ? ` · ${span(task.estimateMin)}` : ""}
              </span>
              <button
                type="button"
                className="icon-btn"
                aria-label={`${id.calendar.schedule}: ${task.title}`}
                title={id.calendar.schedule}
                onPointerDown={(event) => event.stopPropagation()}
                onClick={() => onSchedule(task)}
              >
                <CalendarPlus size={16} aria-hidden="true" />
              </button>
            </li>
          ))}
        </ul>
      )}
    </aside>
  );
}
