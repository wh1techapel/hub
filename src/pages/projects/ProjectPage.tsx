// One project: its goal and status, how far it has come, its milestones, its tasks, and what
// happened to it. Files join in CP4.

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
              ? id.projects.progressOf(summary.progress.mode, summary.progress.done, summary.progress.total)
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
          {open.length === 0 && done.length === 0 && <p className={styles.quiet}>{id.projects.noTasks}</p>}
          {open.length > 0 && (
            <TaskList
              tasks={open}
              label={id.projects.tasks}
              today={now}
              todayIso={todayIso}
              reorderable
              showDoDate
              onOpen={(task) => void navigate({ to: "/proyek", search: { view: "anytime", task: task.id } })}
            />
          )}
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
          <section aria-labelledby="project-milestones" className={styles.block}>
            <h2 className="section-label" id="project-milestones">
              {id.projects.milestones}
            </h2>
            <p className={styles.hint}>{id.projects.milestonesHint}</p>
            <Milestones projectId={summary.id} milestones={detail.milestones} now={now} />
          </section>

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

function Milestones({ projectId, milestones, now }: { projectId: string; milestones: Milestone[]; now: Date }) {
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
    <>
      {milestones.length > 0 && (
        <ol className={styles.milestones}>
          {milestones.map((m) => (
            <li key={m.id} className={m.done ? styles.milestoneDone : undefined}>
              <label>
                <input type="checkbox" checked={m.done} onChange={(e) => void act(() => api.setMilestoneDone(m.id, e.target.checked))} />
                <span>{m.title}</span>
              </label>
              {m.dueDate && <small>{relativeDate(m.dueDate, now)}</small>}
              <button
                type="button"
                className="icon-btn"
                aria-label={`${id.common.delete} ${m.title}`}
                onClick={() => void act(() => api.deleteMilestone(m.id), { text: id.common.deleted(m.title) })}
              >
                <X size={14} aria-hidden="true" />
              </button>
            </li>
          ))}
        </ol>
      )}
      <div className={styles.milestoneAdd}>
        <input
          className="input"
          placeholder={id.projects.addMilestone}
          aria-label={id.projects.addMilestone}
          value={title}
          maxLength={200}
          onChange={(e) => setTitle(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") void add();
          }}
        />
        <input type="date" className="input" aria-label={id.projects.milestoneDate} value={due} onChange={(e) => setDue(e.target.value)} />
      </div>
    </>
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
