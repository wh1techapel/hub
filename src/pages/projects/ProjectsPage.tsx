// Proyek: every active and paused project, grouped by Ruang, each with how far it has come and
// what to do next. Finished and archived ones wait below, folded. Since R6 it is the first view of
// Proyek & Tugas (WorkPage), not a page of its own.

import { Link } from "@tanstack/react-router";
import { useState } from "react";

import type { ProjectSummary } from "../../data/generated/ProjectSummary";
import { useAreas, useProjects } from "../../data/queries";
import { id } from "../../strings/id";
import { useNow } from "../../time/clock";
import { relativeDate } from "../../time/format";
import { AreasDialog } from "./AreasDialog";
import { blankProject, ProjectDialog } from "./ProjectDialog";
import { ProgressBar } from "./ProgressBar";
import styles from "./projects.module.css";

/** `titleId`: the id of its heading, which names the section around it. */
export function ProjectsOverview({ titleId }: { titleId: string }) {
  const projects = useProjects();
  const areas = useAreas();
  const now = useNow();
  const [creating, setCreating] = useState<string | null | false>(false);
  const [managing, setManaging] = useState(false);

  const list = projects.data ?? [];
  const live = list.filter((p) => p.status === "active" || p.status === "paused");
  const finished = list.filter((p) => p.status === "done" || p.status === "archived");

  // Ruang in their own order, then projects without one.
  const groups: { key: string; name: string; pigment: string | null; areaId: string | null; items: ProjectSummary[] }[] = (
    areas.data ?? []
  ).map((area) => ({
    key: area.id,
    name: area.name,
    pigment: area.pigment,
    areaId: area.id,
    items: live.filter((p) => p.area?.id === area.id),
  }));
  const loose = live.filter((p) => !p.area);
  if (loose.length > 0) groups.push({ key: "none", name: id.projects.noArea, pigment: null, areaId: null, items: loose });

  return (
    <div>
      <header className={styles.header}>
        <h2 className={`display ${styles.overviewTitle}`} id={titleId}>
          {id.projects.title}
        </h2>
        <div className={styles.actions}>
          <button type="button" className="btn btn-quiet" onClick={() => setManaging(true)}>
            {id.projects.areas}
          </button>
          <button type="button" className="btn btn-primary" onClick={() => setCreating(null)}>
            {id.projects.new}
          </button>
        </div>
      </header>

      {projects.isSuccess && list.length === 0 && <p className={styles.empty}>{id.projects.empty}</p>}

      {groups
        .filter((group) => group.items.length > 0)
        .map((group) => (
          <section key={group.key} className={styles.group} aria-labelledby={`area-${group.key}`}>
            <h3 className={styles.groupHead} id={`area-${group.key}`} data-pigment={group.pigment ?? undefined}>
              {group.pigment && <i className={styles.areaDot} aria-hidden="true" />}
              {group.name}
            </h3>
            <ul className={styles.cards}>
              {group.items.map((project) => (
                <ProjectRow key={project.id} project={project} now={now} />
              ))}
            </ul>
          </section>
        ))}

      {finished.length > 0 && (
        <details className={styles.finished}>
          <summary>
            {id.projects.finished} <em>{finished.length}</em>
          </summary>
          <ul className={styles.cards}>
            {finished.map((project) => (
              <ProjectRow key={project.id} project={project} now={now} />
            ))}
          </ul>
        </details>
      )}

      {creating !== false && <ProjectDialog initial={blankProject(creating)} onClose={() => setCreating(false)} />}
      {managing && <AreasDialog onClose={() => setManaging(false)} />}
    </div>
  );
}

function ProjectRow({ project, now }: { project: ProjectSummary; now: Date }) {
  const { progress } = project;
  return (
    <li className={styles.card} data-pigment={project.pigment} data-status={project.status}>
      <Link to="/proyek/$projectId" params={{ projectId: project.id }} className={styles.cardLink}>
        <span className={styles.cardName}>
          {project.name}
          {project.status !== "active" && <span className={styles.status}>{id.projects.status[project.status]}</span>}
          {project.source === "ultron" && <span className="ultron-mark">{id.task.byUltron}</span>}
        </span>
        {project.description && <span className={styles.cardGoal}>{project.description}</span>}
      </Link>
      <ProgressBar progress={progress} label={project.name} />
      <div className={styles.cardMeta}>
        <span>{progress.total > 0 ? id.projects.progressOf(progress.mode, progress.done, progress.total) : id.projects.noProgress}</span>
        {project.targetDate && <span>{id.projects.target(relativeDate(project.targetDate, now))}</span>}
        {project.openTasks > 0 && <span>{id.projects.openTasks(project.openTasks)}</span>}
      </div>
      {project.status === "active" && (
        <p className={styles.next}>
          <span className={styles.nextLabel}>{id.projects.next}</span>
          {project.nextStep ? project.nextStep.title : <span className={styles.nextNone}>{id.projects.noNext}</span>}
        </p>
      )}
    </li>
  );
}
