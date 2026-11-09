// U2: Studio's side of the sidebar, shown while Studio is open (the way Pengaturan's parts are,
// P1): back to the Hub page left, Ide baru, a search over the projects, and the projects
// themselves as a history (those waiting for the owner first, then the most recent). It folds into
// a rail of icons; the rail keeps a count of what waits, on the button that opens it again.

import { Link, useRouterState } from "@tanstack/react-router";
import { ArrowLeft, PanelLeftClose, PanelLeftOpen, Plus, Search, Settings } from "lucide-react";
import { useState } from "react";

import type { StudioProject } from "../data/generated/StudioProject";
import { id } from "../strings/id";
import { normalize } from "./nav";
import styles from "./shell.module.css";

const t = id.studio.side;

/** The projects that match `query` (name or idea), waiting ones first, each part newest first. */
export function historyOf(projects: readonly StudioProject[], query: string): { waiting: StudioProject[]; recent: StudioProject[] } {
  const words = normalize(query).split(/\s+/).filter(Boolean);
  const matches = projects
    .filter((project) => words.every((word) => normalize(`${project.name} ${project.idea}`).includes(word)))
    .sort((a, b) => b.updatedAt.localeCompare(a.updatedAt));
  return { waiting: matches.filter((project) => project.waiting > 0), recent: matches.filter((project) => project.waiting === 0) };
}

export function StudioSide({
  projects,
  expanded,
  onBack,
  onToggle,
  onSearch,
}: {
  projects: readonly StudioProject[];
  /** Whether the history shows in full (not folded into the rail). */
  expanded: boolean;
  onBack: () => void;
  onToggle: () => void;
  /** From the rail: open the history and put the cursor in the search. */
  onSearch: () => void;
}) {
  const pathname = useRouterState({ select: (state) => state.location.pathname });
  const [query, setQuery] = useState("");
  const { waiting, recent } = historyOf(projects, query);
  const waitingCount = projects.reduce((sum, project) => sum + project.waiting, 0);
  const toggleLabel = expanded ? t.fold : t.unfold;

  return (
    <>
      <div className={styles.studioTop}>
        <button type="button" className={styles.item} title={t.backTitle} onClick={onBack}>
          <ArrowLeft size={18} strokeWidth={1.6} aria-hidden="true" />
          <span>{t.back}</span>
        </button>
        <button
          type="button"
          className={`${styles.item} ${styles.fold}`}
          aria-expanded={expanded}
          aria-controls="studio-history"
          aria-label={toggleLabel}
          title={toggleLabel}
          onClick={onToggle}
        >
          {expanded ? <PanelLeftClose size={18} strokeWidth={1.6} aria-hidden="true" /> : <PanelLeftOpen size={18} strokeWidth={1.6} aria-hidden="true" />}
          {!expanded && waitingCount > 0 && (
            <span className={styles.badge} aria-label={id.nav.waiting(waitingCount)}>
              {waitingCount}
            </span>
          )}
        </button>
      </div>

      <Link
        to="/studio"
        className={styles.capture}
        activeOptions={{ exact: true, includeSearch: false }}
        title={t.newIdea}
        onClick={() => {
          // Already on Studio's home: the idea field is where a new idea starts.
          if (pathname === "/studio") requestAnimationFrame(() => document.getElementById("studio-idea")?.focus());
        }}
      >
        <Plus size={17} strokeWidth={1.7} aria-hidden="true" />
        <span>{t.newIdea}</span>
      </Link>

      <button type="button" className={`${styles.item} ${styles.railOnly}`} aria-label={t.searchOpen} title={t.searchOpen} onClick={onSearch}>
        <Search size={18} strokeWidth={1.6} aria-hidden="true" />
      </button>
      <div className={`${styles.searchBox} ${styles.wideOnly}`}>
        <Search size={15} strokeWidth={1.6} aria-hidden="true" />
        <input
          id="studio-search"
          type="search"
          className={styles.search}
          value={query}
          placeholder={t.search}
          aria-label={t.searchOpen}
          autoComplete="off"
          spellCheck={false}
          onChange={(event) => setQuery(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Escape" && query) {
              event.preventDefault();
              event.stopPropagation();
              setQuery("");
            }
          }}
        />
      </div>

      <nav id="studio-history" className={`${styles.history} ${styles.wideOnly}`} aria-label={t.label}>
        {projects.length === 0 && <p className={styles.historyEmpty}>{t.empty}</p>}
        {projects.length > 0 && waiting.length + recent.length === 0 && <p className={styles.historyEmpty}>{t.noMatch}</p>}
        {[
          { key: "waiting", label: t.waiting, items: waiting },
          { key: "recent", label: t.recent, items: recent },
        ].map(
          (group) =>
            group.items.length > 0 && (
              <section key={group.key} className={styles.historyGroup} aria-labelledby={`studio-history-${group.key}`}>
                <p className={styles.paneLabel} id={`studio-history-${group.key}`}>
                  {group.label}
                </p>
                <ul className={styles.historyList}>
                  {group.items.map((project) => (
                    <li key={project.id}>
                      <Link
                        to="/studio/$projectId"
                        params={{ projectId: project.id }}
                        className={styles.item}
                        activeProps={{ className: `${styles.item} ${styles.active}` }}
                        title={project.name}
                      >
                        <span className={styles.historyName}>{project.name}</span>
                        {project.waiting > 0 && (
                          <span className={styles.badge} aria-label={id.nav.waiting(project.waiting)}>
                            {project.waiting}
                          </span>
                        )}
                      </Link>
                    </li>
                  ))}
                </ul>
              </section>
            ),
        )}
      </nav>

      <div className={styles.foot}>
        <Link to="/pengaturan/$bagian" params={{ bagian: "ai" }} className={styles.item} title={id.nav.settings}>
          <Settings size={18} strokeWidth={1.6} aria-hidden="true" />
          <span>{id.nav.settings}</span>
        </Link>
      </div>
    </>
  );
}
