// Tasks with the ink check (docs/design.md, "Signature elements"): the ring fills with ink, the tick
// draws itself, a hand-drawn strike crosses the title. Every list in Hub uses this one.
//
// Keyboard: ↑↓ move between tasks, Space checks, Enter opens, Alt+↑↓ moves a task in a
// hand-sorted list, Delete removes it (with Urungkan). The mouse can drag the grip to reorder.

import { useFlip } from "../motion/motion";
import { GripVertical } from "lucide-react";
import { useRef, useState } from "react";
import type { KeyboardEvent, PointerEvent } from "react";

import { useApi } from "../data/api";
import type { TaskCard } from "../data/generated/TaskCard";
import { useSetTaskDone } from "../data/queries";
import { id } from "../strings/id";
import { clock, parseClock, relativeDate } from "../time/format";
import styles from "./taskList.module.css";
import { useAct, useToast } from "./Toast";

function dueLabel(task: TaskCard, today: Date, todayIso: string): { text: string; warn: boolean } | null {
  if (!task.dueDate) return null;
  if (task.overdue) return { text: id.task.overdue, warn: true };
  const time = task.dueTime ? ` ${clock(parseClock(task.dueTime) ?? 0)}` : "";
  if (task.dueDate === todayIso) return { text: `${id.task.dueToday}${time}`, warn: true };
  return { text: id.task.dueOn(`${relativeDate(task.dueDate, today)}${time}`), warn: false };
}

interface Props {
  tasks: readonly TaskCard[];
  label: string;
  today: Date;
  todayIso: string;
  quiet?: boolean;
  /** Drag and Alt+↑↓ reorder (hand-sorted lists only). */
  reorderable?: boolean;
  /** Show the do date beside each task (lists that mix days). */
  showDoDate?: boolean;
  selectedId?: string | null;
  onOpen?: (task: TaskCard) => void;
}

export function TaskList({
  tasks,
  label,
  today,
  todayIso,
  quiet = false,
  reorderable = false,
  showDoDate = false,
  selectedId = null,
  onOpen,
}: Props) {
  const api = useApi();
  const act = useAct();
  const toast = useToast();
  const setDone = useSetTaskDone();
  const list = useRef<HTMLUListElement>(null);
  useFlip(list);
  const [order, setOrder] = useState<string[] | null>(null);
  const [drag, setDrag] = useState<{ id: string; over: number } | null>(null);

  // A local order wins until the core's own list has caught up with it.
  const ids = tasks.map((t) => t.id);
  const local = order && order.length === ids.length && order.every((x) => ids.includes(x)) && order.join() !== ids.join() ? order : null;
  const shown = local ? local.map((x) => tasks.find((t) => t.id === x)).filter((t): t is TaskCard => t !== undefined) : tasks;

  const toggle = (task: TaskCard, done: boolean) => {
    setDone.mutate(
      { id: task.id, done },
      {
        onSuccess: () =>
          toast({
            text: done ? id.task.done(task.title) : id.task.reopened(task.title),
            action: { label: id.task.undo, run: () => setDone.mutate({ id: task.id, done: !done }) },
          }),
        onError: () => toast({ text: id.task.failed }),
      },
    );
  };

  const move = (taskId: string, to: number) => {
    const from = shown.findIndex((t) => t.id === taskId);
    if (from < 0 || to === from || to < 0 || to > shown.length) return;
    const next = shown.map((t) => t.id).filter((x) => x !== taskId);
    const index = to > from ? to - 1 : to;
    next.splice(index, 0, taskId);
    setOrder(next);
    const before = next[index - 1] ?? null;
    const after = next[index + 1] ?? null;
    void act(() => api.reorderTask(taskId, before, after));
  };

  const remove = (task: TaskCard) => {
    void act(() => api.deleteTask(task.id), {
      text: id.common.deleted(task.title),
      action: { label: id.common.undo, run: () => void act(() => api.restoreTask(task.id)) },
    });
  };

  const focusRow = (index: number) => {
    const rows = list.current?.querySelectorAll<HTMLElement>("[data-row]");
    rows?.[Math.max(0, Math.min(index, rows.length - 1))]?.focus();
  };

  const onKeyDown = (event: KeyboardEvent<HTMLElement>, task: TaskCard, index: number) => {
    if (event.target !== event.currentTarget) return;
    if (event.altKey && reorderable && (event.key === "ArrowUp" || event.key === "ArrowDown")) {
      event.preventDefault();
      move(task.id, event.key === "ArrowUp" ? index - 1 : index + 2);
      requestAnimationFrame(() => focusRow(event.key === "ArrowUp" ? index - 1 : index + 1));
      return;
    }
    switch (event.key) {
      case "ArrowDown":
        event.preventDefault();
        focusRow(index + 1);
        break;
      case "ArrowUp":
        event.preventDefault();
        focusRow(index - 1);
        break;
      case " ":
        event.preventDefault();
        toggle(task, task.status !== "done");
        break;
      case "Enter":
        event.preventDefault();
        onOpen?.(task);
        break;
      case "Delete":
      case "Backspace":
        event.preventDefault();
        remove(task);
        requestAnimationFrame(() => focusRow(index));
        break;
    }
  };

  // Dragging the grip: an insertion line follows the pointer; the drop moves one task.
  const startDrag = (event: PointerEvent<HTMLButtonElement>, task: TaskCard) => {
    if (!reorderable || event.button !== 0) return;
    event.preventDefault();
    event.currentTarget.setPointerCapture(event.pointerId);
    setDrag({ id: task.id, over: shown.findIndex((t) => t.id === task.id) });
  };
  const overIndex = (y: number) => {
    const rows = [...(list.current?.querySelectorAll<HTMLElement>("[data-task]") ?? [])];
    const index = rows.findIndex((row) => {
      const box = row.getBoundingClientRect();
      return y < box.top + box.height / 2;
    });
    return index < 0 ? rows.length : index;
  };
  const moveDrag = (event: PointerEvent<HTMLButtonElement>) => {
    if (!drag) return;
    const over = overIndex(event.clientY);
    if (over !== drag.over) setDrag({ ...drag, over });
  };
  // The drop point is read from the release itself: the last move may not have been rendered yet.
  const endDrag = (event: PointerEvent<HTMLButtonElement>) => {
    if (!drag) return;
    move(drag.id, overIndex(event.clientY));
    setDrag(null);
  };

  return (
    <ul ref={list} className={`${styles.tasks} ${quiet ? styles.quiet : ""}`} aria-label={label}>
      {shown.map((task, index) => {
        const done = task.status === "done";
        const due = dueLabel(task, today, todayIso);
        const doDate = showDoDate && task.doDate && task.doDate !== todayIso ? relativeDate(task.doDate, today) : null;
        return (
          <li
            key={task.id}
            data-flip={task.id}
            data-task={task.id}
            className={`${styles.task} ${done ? styles.done : ""} ${selectedId === task.id ? styles.selected : ""} ${drag?.id === task.id ? styles.dragging : ""}`}
          >
            {drag && drag.over === index && drag.id !== task.id && <span className={styles.insert} aria-hidden="true" />}
            {reorderable && (
              <button
                type="button"
                className={styles.grip}
                aria-label={id.task.drag(task.title)}
                tabIndex={-1}
                onPointerDown={(event) => startDrag(event, task)}
                onPointerMove={moveDrag}
                onPointerUp={endDrag}
                onPointerCancel={() => setDrag(null)}
              >
                <GripVertical size={14} aria-hidden="true" />
              </button>
            )}
            <button
              type="button"
              className={styles.check}
              aria-pressed={done}
              aria-label={done ? id.task.markOpen(task.title) : id.task.markDone(task.title)}
              tabIndex={-1}
              onClick={() => toggle(task, !done)}
            >
              <svg viewBox="0 0 16 16" aria-hidden="true">
                <path d="M3.5 8.5 6.5 11.5 12.5 5" pathLength={1} />
              </svg>
            </button>
            <div
              className={styles.body}
              data-row
              role="button"
              tabIndex={0}
              aria-label={task.title}
              aria-current={selectedId === task.id ? "true" : undefined}
              onClick={() => onOpen?.(task)}
              onKeyDown={(event) => onKeyDown(event, task, index)}
            >
              <span className={styles.title}>
                {task.title}
                <svg className={styles.strike} viewBox="0 0 100 8" preserveAspectRatio="none" aria-hidden="true">
                  <path d="M0 5 C 18 3, 36 6, 55 4 S 88 5, 100 3.5" pathLength={1} />
                </svg>
              </span>
              <span className={styles.meta}>
                {task.project && (
                  <span className="chip" data-pigment={task.project.pigment}>
                    {task.project.name}
                  </span>
                )}
                {!task.project && task.area && (
                  <span className="chip" data-pigment={task.area.pigment}>
                    {task.area.name}
                  </span>
                )}
                {doDate && <span>{doDate}</span>}
                {due && <span className={due.warn ? styles.warn : undefined}>{due.text}</span>}
                {task.subtasksTotal > 0 && <span>{id.task.steps(task.subtasksDone, task.subtasksTotal)}</span>}
                {task.priority > 0 && <span className={styles.priority}>{"!".repeat(task.priority)}</span>}
                {task.scheduled && <span>{id.task.scheduled}</span>}
                {task.source === "ultron" && <span className="ultron-mark">{id.task.byUltron}</span>}
              </span>
            </div>
          </li>
        );
      })}
      {drag && drag.over === shown.length && <li className={styles.insertEnd} aria-hidden="true" />}
    </ul>
  );
}
