// One project: its goal and status, how far it has come (done tasks over tasks), its tasks grouped
// under their milestones, its files, and what happened to it.

import { Link, useNavigate, useParams } from "@tanstack/react-router";
import { ArrowLeft, Pencil, Trash2, X } from "lucide-react";
import { useState } from "react";

import { CaptureForm } from "../../components/CaptureForm";
import { FileRows } from "../../components/FileRows";
import { Segmented } from "../../components/Fields";
import { TaskList } from "../../components/TaskList";
import { useAct } from "../../components/Toast";
import { useApi } from "../../data/api";
import type { Milestone } from "../../data/generated/Milestone";
import type { ProjectDetail } from "../../data/generated/ProjectDetail";
import type { ProjectStatus } from "../../data/generated/ProjectStatus";
import type { TaskCard } from "../../data/generated/TaskCard";
import { failedForGood, useProject, useStudioLink } from "../../data/queries";
import { id, trailSentence } from "../../strings/id";
import { useNow } from "../../time/clock";
import { isoDate, relativeDate, whenLabel } from "../../time/format";
import { ProgressBar } from "./ProgressBar";
import { ProjectDialog } from "./ProjectDialog";
import styles from "./projects.module.css";

const STATUSES: ProjectStatus[] = ["active", "paused", "done", "archived"];

export function ProjectPage() {
  const params: { projectId?: string } = useParams({ strict: false });
  const project = useProject(params.projectId ?? "");

  if (failedForGood(project)) {
    return (
      <div className={styles.page}>
        <p className={styles.empty}>{id.projects.notFound}</p>
        <Link to="/proyek" className="btn btn-secondary">
          {id.projects.all}
        </Link>
      </div>
    );
  }
  if (!project.data) return null;
  return <ProjectView detail={project.data} />;
}

function ProjectView({ detail }: { detail: ProjectDetail }) {
  const api = useApi();
  const act = useAct();
  const now = useNow();
  const navigate = useNavigate();
  const [editing, setEditing] = useState(false);
  const [confirming, setConfirming] = useState(false);
  const { summary } = detail;
  const todayIso = isoDate(now);
  const open = detail.tasks.filter((t) => t.status !== "done");
  const done = detail.tasks.filter((t) => t.status === "done");
  const grouped = new Set(detail.milestones.map((m) => m.id));
  const loose = open.filter((t) => t.milestoneId === null || !grouped.has(t.milestoneId));
  const hasMilestones = detail.milestones.length > 0;
  const openTask = (task: TaskCard) => void navigate({ to: "/proyek", search: { view: "anytime", task: task.id } });

  const remove = async () => {
    const result = await act(() => api.deleteProject(summary.id), { text: id.common.deleted(summary.name) });
    if (result.ok) void navigate({ to: "/proyek" });
  };

  return (
    <div className={styles.page} data-pigment={summary.pigment}>
      <Link to="/proyek" className={styles.back}>
        <ArrowLeft size={16} aria-hidden="true" />
        {id.projects.all}
      </Link>

      <header className={styles.hero}>
        <div className={styles.heroText}>
          {summary.area && (
            <span className="chip" data-pigment={summary.area.pigment}>
              {summary.area.name}
            </span>
          )}
          <h1 className={`display ${styles.projectName}`}>{summary.name}</h1>
          {summary.description && <p className={styles.goal}>{summary.description}</p>}
          <StudioLine projectId={summary.id} />
        </div>
        <div className={styles.heroSide}>
          <button type="button" className="btn btn-quiet" onClick={() => setEditing(true)}>
            <Pencil size={15} aria-hidden="true" />
            {id.common.edit}
          </button>
        </div>
      </header>

      <div className={styles.standing}>
        <div className={styles.percent}>
          <span className="display">{summary.progress.percent}</span>
          <small>%</small>
        </div>
        <div className={styles.standingBody}>
          <ProgressBar progress={summary.progress} label={id.projects.progressLabel(summary.name)} large />
          <p>
            {summary.progress.total > 0
              ? id.projects.progressOf(summary.progress.done, summary.progress.total)
              : id.projects.noProgress}
            {summary.targetDate && ` · ${id.projects.target(relativeDate(summary.targetDate, now))}`}
          </p>
        </div>
        <Segmented
          label={id.projects.statusLabel}
          options={STATUSES.map((status) => [status, id.projects.status[status]] as const)}
          value={summary.status}
          onChange={(status) => void act(() => api.setProjectStatus(summary.id, status))}
        />
      </div>

      <div className={styles.columns}>
        <section aria-labelledby="project-tasks" className={styles.block}>
          <h2 className="section-label" id="project-tasks">
            {id.projects.tasks}
          </h2>
          <div className={styles.addTask}>
            <CaptureForm kinds={["task"]} compact projectId={summary.id} placeholder={id.projects.addTask} />
          </div>
          {open.length === 0 && done.length === 0 && !hasMilestones && <p className={styles.quiet}>{id.projects.noTasks}</p>}
          {loose.length > 0 && (
            <div className={styles.group}>
              {hasMilestones && <h3 className={styles.groupTitle}>{id.projects.noMilestone}</h3>}
              <TaskList
                tasks={loose}
                label={hasMilestones ? id.projects.noMilestone : id.projects.tasks}
                today={now}
                todayIso={todayIso}
                reorderable
                showDoDate
                onOpen={openTask}
              />
            </div>
          )}
          {detail.milestones.map((m) => (
            <MilestoneGroup
              key={m.id}
              projectId={summary.id}
              milestone={m}
              tasks={open.filter((t) => t.milestoneId === m.id)}
              now={now}
              todayIso={todayIso}
              onOpen={openTask}
            />
          ))}
          <MilestoneAdd projectId={summary.id} />
          {done.length > 0 && (
            <details className={styles.doneTasks}>
              <summary>{id.projects.doneTasks(done.length)}</summary>
              <TaskList
                tasks={done}
                label={id.projects.doneTasks(done.length)}
                today={now}
                todayIso={todayIso}
                quiet
                onOpen={(task) => void navigate({ to: "/proyek", search: { view: "done", task: task.id } })}
              />
            </details>
          )}
        </section>

        <aside className={styles.side}>
          <section aria-labelledby="project-files" className={styles.block}>
            <h2 className="section-label" id="project-files">
              {id.files.section}
            </h2>
            <FileRows files={detail.files} target={{ kind: "project", id: summary.id }} label={summary.name} />
          </section>

          <section aria-labelledby="project-history" className={styles.block}>
            <h2 className="section-label" id="project-history">
              {id.projects.history}
            </h2>
            {detail.activity.length === 0 ? (
              <p className={styles.quiet}>{id.projects.historyEmpty}</p>
            ) : (
              <ul className={styles.history}>
                {detail.activity.map((item) => (
                  <li key={item.id} className={item.actor === "ultron" ? styles.byUltron : undefined}>
                    {trailSentence(item)}
                    <time dateTime={item.at}>{whenLabel(item.at, now)}</time>
                  </li>
                ))}
              </ul>
            )}
          </section>

          <div className={styles.danger}>
            {confirming ? (
              <>
                <p>{id.projects.deleteHint}</p>
                <div className={styles.actions}>
                  <button type="button" className="btn btn-quiet" onClick={() => setConfirming(false)}>
                    {id.common.cancel}
                  </button>
                  <button type="button" className="btn btn-danger" onClick={() => void remove()}>
                    {id.projects.deleteProject}
                  </button>
                </div>
              </>
            ) : (
              <button type="button" className="btn btn-quiet" onClick={() => setConfirming(true)}>
                <Trash2 size={15} aria-hidden="true" />
                {id.projects.deleteProject}
              </button>
            )}
          </div>
        </aside>
      </div>

      {editing && (
        <ProjectDialog
          projectId={summary.id}
          initial={{
            name: summary.name,
            description: summary.description,
            areaId: summary.area?.id ?? null,
            pigment: summary.pigment,
            targetDate: summary.targetDate,
          }}
          onClose={() => setEditing(false)}
        />
      )}
    </div>
  );
}

/**
 * A milestone and its open tasks, like a heading in the task list. The diamond fills once every
 * task in it is done; a milestone without tasks is ticked by hand on the diamond itself.
 */
function MilestoneGroup({
  projectId,
  milestone: m,
  tasks,
  now,
  todayIso,
  onOpen,
}: {
  projectId: string;
  milestone: Milestone;
  tasks: TaskCard[];
  now: Date;
  todayIso: string;
  onOpen: (task: TaskCard) => void;
}) {
  const api = useApi();
  const act = useAct();
  const headingId = `milestone-${m.id}`;
  const late = !m.done && m.dueDate !== null && m.dueDate < todayIso;
  const diamond = <i className={styles.diamond} data-done={m.done} aria-hidden="true" />;

  return (
    <section className={`${styles.group} ${styles.milestone}`} data-done={m.done} aria-labelledby={headingId}>
      <div className={styles.milestoneHead}>
        {m.tasksTotal === 0 ? (
          <label className={styles.mark}>
            <input
              type="checkbox"
              className="sr-only"
              checked={m.done}
              aria-labelledby={headingId}
              onChange={(e) => void act(() => api.setMilestoneDone(m.id, e.target.checked))}
            />
            {diamond}
          </label>
        ) : (
          <span className={styles.mark}>{diamond}</span>
        )}
        <div className={styles.milestoneText}>
          <h3 id={headingId} className={styles.groupTitle}>
            {m.title}
          </h3>
          <p className={styles.milestoneMeta}>
            {m.dueDate && <span className={late ? styles.late : undefined}>{relativeDate(m.dueDate, now)}</span>}
            <span>{id.projects.milestoneTasks(m.tasksDone, m.tasksTotal)}</span>
          </p>
        </div>
        <button
          type="button"
          className="icon-btn"
          aria-label={`${id.common.delete} ${m.title}`}
          onClick={() => void act(() => api.deleteMilestone(m.id), { text: id.common.deleted(m.title) })}
        >
          <X size={14} aria-hidden="true" />
        </button>
      </div>
      {tasks.length > 0 && (
        <TaskList tasks={tasks} label={m.title} today={now} todayIso={todayIso} reorderable showDoDate onOpen={onOpen} />
      )}
      <div className={styles.groupAdd}>
        <CaptureForm
          kinds={["task"]}
          compact
          projectId={projectId}
          milestoneId={m.id}
          label={id.projects.addTaskTo(m.title)}
          placeholder={id.projects.addTaskTo(m.title)}
        />
      </div>
    </section>
  );
}

/** A new milestone, with an optional date, and what a milestone is for. */
function MilestoneAdd({ projectId }: { projectId: string }) {
  const api = useApi();
  const act = useAct();
  const [title, setTitle] = useState("");
  const [due, setDue] = useState("");

  const add = async () => {
    const text = title.trim();
    if (!text) return;
    const created = await act(() => api.addMilestone(projectId, text, due || null));
    if (created.ok) {
      setTitle("");
      setDue("");
    }
  };

  return (
    <div className={styles.milestoneNew}>
      <div className={styles.milestoneAdd}>
        <input
          className="input"
          placeholder={id.projects.addMilestone}
          aria-label={id.projects.addMilestone}
          aria-describedby="milestone-hint"
          value={title}
          maxLength={200}
          onChange={(e) => setTitle(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") void add();
          }}
        />
        <input type="date" className="input" aria-label={id.projects.milestoneDate} value={due} onChange={(e) => setDue(e.target.value)} />
      </div>
      <p id="milestone-hint" className={styles.hint}>
        {id.projects.milestonesHint}
      </p>
    </div>
  );
}

/** "Dibangun di Studio: CP-03 sedang ditinjau", when this Proyek is built in Studio. */
function StudioLine({ projectId }: { projectId: string }) {
  const link = useStudioLink(projectId);
  const data = link.data;
  if (!data) return null;
  const t = id.projects;
  const state =
    data.activeCode && data.activeStatus ? (t.studioStates[data.activeStatus]?.(data.activeCode) ?? data.activeCode) : t.studioDone(data.completed, data.total);
  return (
    <p className={styles.studioLine}>
      <Link to="/studio/$projectId" params={{ projectId: data.studioProjectId }}>
        {t.studioLine(state)}
      </Link>
    </p>
  );
}
