// One task, all its fields. Every change is saved as it is made (no Save button to forget), and
// steps are tasks of their own, checked with the same ink.

import { CalendarPlus, Star, Trash2, X } from "lucide-react";
import { useEffect, useState } from "react";

import { Field, Segmented } from "../../components/Fields";
import { FileRows } from "../../components/FileRows";
import { TaskList } from "../../components/TaskList";
import { FocusButton } from "../../components/FocusButton";
import { useAct } from "../../components/Toast";
import { useApi } from "../../data/api";
import type { TaskDetail } from "../../data/generated/TaskDetail";
import type { TaskInput } from "../../data/generated/TaskInput";
import { useAreas, useProject, useProjects, useTask, useToday } from "../../data/queries";
import { id } from "../../strings/id";
import { useNow } from "../../time/clock";
import { clock, isoDate, minutesOf, relativeDate } from "../../time/format";
import { blankEvent, EventDialog } from "../calendar/EventDialog";
import styles from "./tasks.module.css";

/** Fokus holds at most three tasks a day (CONTEXT.md). */
const FOCUS_MAX = 3;

function inputOf(detail: TaskDetail): TaskInput {
  const c = detail.card;
  return {
    title: c.title,
    notes: detail.notes,
    priority: c.priority,
    doDate: c.doDate,
    dueDate: c.dueDate,
    dueTime: c.dueTime,
    estimateMin: c.estimateMin,
    projectId: c.project?.id ?? null,
    areaId: c.project ? null : (c.area?.id ?? null),
    milestoneId: c.milestoneId,
    parentId: null,
  };
}

export function TaskPanel({ taskId, onClose, onOpen }: { taskId: string; onClose: () => void; onOpen: (id: string) => void }) {
  const task = useTask(taskId);
  const now = useNow();

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !(event.target as HTMLElement | null)?.closest("[role=dialog]")) onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  return (
    <aside className={styles.panel} aria-label={id.tasks.detail}>
      <button type="button" className={`icon-btn ${styles.closePanel}`} aria-label={id.tasks.close} onClick={onClose}>
        <X size={18} aria-hidden="true" />
      </button>
      {task.data && <TaskEditor key={task.data.card.id} detail={task.data} now={now} onClose={onClose} onOpen={onOpen} />}
    </aside>
  );
}

function TaskEditor({
  detail,
  now,
  onClose,
  onOpen,
}: {
  detail: TaskDetail;
  now: Date;
  onClose: () => void;
  onOpen: (id: string) => void;
}) {
  const api = useApi();
  const act = useAct();
  const projects = useProjects();
  const areas = useAreas();
  const [value, setValue] = useState<TaskInput>(() => inputOf(detail));
  const project = useProject(value.projectId ?? "", value.projectId !== null);
  const milestones = (value.projectId && project.data?.milestones) || [];
  const [step, setStep] = useState("");
  const [scheduling, setScheduling] = useState(false);
  const todayIso = isoDate(now);
  const today = useToday(todayIso);
  const focusIds = today.data?.focus.map((t) => t.id) ?? [];
  const inFocus = focusIds.includes(detail.card.id);
  const focusFull = !inFocus && focusIds.length >= FOCUS_MAX;

  const [typing, setTyping] = useState(false);
  const [seen, setSeen] = useState(detail);
  // A change from elsewhere (ULTRON, another window) replaces what is shown, unless the title or
  // the notes are being typed in. Adjusted while rendering, as React suggests for props.
  if (detail !== seen) {
    setSeen(detail);
    if (!typing) setValue(inputOf(detail));
  }

  const save = (patch: Partial<TaskInput>) => {
    const next = { ...value, ...patch };
    setValue(next);
    if (!next.title.trim()) return;
    void act(() => api.updateTask(detail.card.id, next));
  };

  const addStep = async () => {
    const title = step.trim();
    if (!title) return;
    setStep("");
    await act(() =>
      api.createTask({
        title,
        notes: "",
        priority: 0,
        doDate: null,
        dueDate: null,
        dueTime: null,
        estimateMin: null,
        projectId: value.projectId,
        areaId: null,
        milestoneId: null,
        parentId: detail.card.id,
      }),
    );
  };

  const remove = async () => {
    const taskId = detail.card.id;
    await act(() => api.deleteTask(taskId), {
      text: id.common.deleted(detail.card.title),
      action: {
        label: id.common.undo,
        run: () => void act(() => api.restoreTask(taskId)),
      },
    });
    onClose();
  };

  const start = Math.min(1380, Math.ceil((minutesOf(now) + 1) / 15) * 15);

  return (
    <div className={styles.editor}>
      <label className="sr-only" htmlFor="task-title">
        {id.tasks.titleField}
      </label>
      <textarea
        id="task-title"
        className={styles.titleInput}
        rows={2}
        value={value.title}
        maxLength={300}
        onChange={(e) => setValue({ ...value, title: e.target.value.replace(/\n/g, " ") })}
        onFocus={() => setTyping(true)}
        onBlur={() => {
          setTyping(false);
          if (value.title.trim() && value.title !== detail.card.title) save({});
        }}
        onKeyDown={(e) => {
          if (e.key === "Enter") {
            e.preventDefault();
            e.currentTarget.blur();
          }
        }}
      />
      {detail.card.source === "ultron" && <span className="ultron-mark">{id.task.byUltron}</span>}

      {detail.card.parentId === null && detail.card.status !== "done" && (
        <div className={styles.focusRow}>
          <button
            type="button"
            className={`btn ${inFocus ? "btn-secondary" : "btn-quiet"}`}
            aria-pressed={inFocus}
            disabled={focusFull}
            onClick={() =>
              void act(() => api.setFocus(todayIso, inFocus ? focusIds.filter((x) => x !== detail.card.id) : [...focusIds, detail.card.id]))
            }
          >
            <Star size={16} aria-hidden="true" fill={inFocus ? "currentColor" : "none"} />
            {inFocus ? id.tasks.focusRemove : id.tasks.focusAdd}
          </button>
          {focusFull && <span>{id.tasks.focusFull}</span>}
          <FocusButton taskId={detail.card.id} className="btn btn-quiet" />
        </div>
      )}

      <div className="row">
        <Field label={id.tasks.doDate} htmlFor="task-do">
          <input
            id="task-do"
            type="date"
            className="input"
            value={value.doDate ?? ""}
            onChange={(e) => save({ doDate: e.target.value || null })}
          />
        </Field>
        <Field label={id.tasks.dueDate} htmlFor="task-due">
          <input
            id="task-due"
            type="date"
            className="input"
            value={value.dueDate ?? ""}
            onChange={(e) =>
              save({
                dueDate: e.target.value || null,
                dueTime: e.target.value ? value.dueTime : null,
              })
            }
          />
        </Field>
        <Field label={id.tasks.dueTime} htmlFor="task-due-time">
          <input
            id="task-due-time"
            type="time"
            className="input"
            disabled={!value.dueDate}
            value={value.dueTime ?? ""}
            onChange={(e) => save({ dueTime: e.target.value || null })}
          />
        </Field>
      </div>

      <Field label={id.tasks.priority}>
        <Segmented
          label={id.tasks.priority}
          options={id.tasks.priorities.map((text, level) => [level, text] as const)}
          value={value.priority}
          onChange={(priority) => save({ priority })}
        />
      </Field>

      <div className="row">
        <Field label={id.tasks.project} htmlFor="task-project">
          <select
            id="task-project"
            className="input"
            value={value.projectId ?? ""}
            onChange={(e) =>
              save({
                projectId: e.target.value || null,
                areaId: e.target.value ? null : value.areaId,
                // A milestone belongs to its project: another project starts without one.
                milestoneId: null,
              })
            }
          >
            <option value="">{id.tasks.noProject}</option>
            {(projects.data ?? [])
              .filter((p) => p.status === "active" || p.id === value.projectId)
              .map((p) => (
                <option key={p.id} value={p.id}>
                  {p.name}
                </option>
              ))}
          </select>
        </Field>
        {value.projectId && detail.card.parentId === null && milestones.length > 0 && (
          <Field label={id.tasks.milestone} htmlFor="task-milestone">
            <select
              id="task-milestone"
              className="input"
              value={value.milestoneId ?? ""}
              onChange={(e) => save({ milestoneId: e.target.value || null })}
            >
              <option value="">{id.tasks.noMilestone}</option>
              {milestones.map((m) => (
                <option key={m.id} value={m.id}>
                  {m.title}
                </option>
              ))}
            </select>
          </Field>
        )}
        {!value.projectId && (
          <Field label={id.tasks.area} htmlFor="task-area">
            <select id="task-area" className="input" value={value.areaId ?? ""} onChange={(e) => save({ areaId: e.target.value || null })}>
              <option value="">{id.tasks.noArea}</option>
              {(areas.data ?? []).map((a) => (
                <option key={a.id} value={a.id}>
                  {a.name}
                </option>
              ))}
            </select>
          </Field>
        )}
        <Field label={id.tasks.estimate} htmlFor="task-estimate">
          <select
            id="task-estimate"
            className="input"
            value={value.estimateMin ?? ""}
            onChange={(e) =>
              save({
                estimateMin: e.target.value ? Number(e.target.value) : null,
              })
            }
          >
            {id.tasks.estimates.map(([minutes, text]) => (
              <option key={text} value={minutes ?? ""}>
                {text}
              </option>
            ))}
          </select>
        </Field>
      </div>

      <Field label={id.tasks.notes} htmlFor="task-notes">
        <textarea
          id="task-notes"
          className="input"
          value={value.notes}
          maxLength={10000}
          onChange={(e) => setValue({ ...value, notes: e.target.value })}
          onFocus={() => setTyping(true)}
          onBlur={() => {
            setTyping(false);
            if (value.notes !== detail.notes) save({});
          }}
        />
      </Field>

      {detail.card.parentId === null && (
        <section aria-labelledby="steps-title" className={styles.steps}>
          <h3 className="section-label" id="steps-title">
            {id.tasks.steps}
          </h3>
          {detail.subtasks.length > 0 && (
            <TaskList
              tasks={detail.subtasks}
              label={id.tasks.steps}
              today={now}
              todayIso={todayIso}
              quiet
              reorderable
              onOpen={(t) => onOpen(t.id)}
            />
          )}
          <input
            className="input"
            placeholder={id.tasks.addStep}
            aria-label={id.tasks.addStep}
            value={step}
            maxLength={300}
            onChange={(e) => setStep(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") void addStep();
            }}
          />
        </section>
      )}

      <section aria-labelledby="task-files" className={styles.steps}>
        <h3 className="section-label" id="task-files">
          {id.files.section}
        </h3>
        <FileRows files={detail.files} target={{ kind: "task", id: detail.card.id }} label={detail.card.title} />
      </section>

      <section aria-labelledby="blocks-title" className={styles.steps}>
        <h3 className="section-label" id="blocks-title">
          {id.tasks.blocks}
        </h3>
        {detail.blocks.map((block) => {
          const start = new Date(block.start);
          const end = new Date(block.end);
          return (
            <p key={block.eventId} className={styles.block}>
              {relativeDate(isoDate(start), now)}, {clock(minutesOf(start))} sampai {clock(minutesOf(end))}
            </p>
          );
        })}
        <button type="button" className="btn btn-secondary" onClick={() => setScheduling(true)}>
          <CalendarPlus size={16} aria-hidden="true" />
          {id.tasks.schedule}
        </button>
      </section>

      <button type="button" className="btn btn-danger" onClick={() => void remove()}>
        <Trash2 size={16} aria-hidden="true" />
        {id.tasks.deleteTask}
      </button>

      {scheduling && (
        <EventDialog
          taskTitle={detail.card.title}
          onClose={() => setScheduling(false)}
          draft={{
            ...blankEvent(value.doDate ?? todayIso, start, start + (value.estimateMin ?? 60)),
            title: detail.card.title,
            pigment: detail.card.project?.pigment ?? "batu",
            projectId: detail.card.project?.id ?? null,
            taskId: detail.card.id,
          }}
        />
      )}
    </div>
  );
}
