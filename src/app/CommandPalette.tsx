// Ctrl+K: go anywhere, make something, or find anything by typing. Commands first, then pages and
// projects, then what the core's search found (tasks, files, learning items, events, notes).
// "Catat cepat" (and Tugas or Pengingat baru) turns the palette into the capture line in place.

import { useNavigate } from "@tanstack/react-router";
import {
  Bell,
  BookOpen,
  CalendarDays,
  CalendarPlus,
  CheckSquare,
  CornerDownLeft,
  File,
  FolderKanban,
  Folders,
  Lamp,
  ListPlus,
  Moon,
  NotebookPen,
  PenLine,
  Repeat,
  Timer,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";
import { useDeferredValue, useEffect, useRef, useState } from "react";
import type { KeyboardEvent } from "react";

import { CaptureForm } from "../components/CaptureForm";
import { useToast } from "../components/Toast";
import { useApi } from "../data/api";
import type { CaptureKind } from "../data/generated/CaptureKind";
import type { SearchHit } from "../data/generated/SearchHit";
import type { SearchKind } from "../data/generated/SearchKind";
import { useProjects, useSearch } from "../data/queries";
import { id } from "../strings/id";
import { matchPages, normalize } from "./nav";
import styles from "./palette.module.css";

interface Entry {
  key: string;
  group: string;
  label: string;
  icon: LucideIcon;
  keywords: string;
  /** A quiet line under a search hit (its project, folder, or date). */
  detail?: string;
  run: () => void;
}

const HIT_ICONS: Record<SearchKind, LucideIcon> = {
  task: CheckSquare,
  project: FolderKanban,
  file: File,
  collection: Folders,
  learning_item: BookOpen,
  event: CalendarDays,
  journal: NotebookPen,
};

export function CommandPalette({ onClose, startWith = null }: { onClose: () => void; startWith?: CaptureKind | null }) {
  const navigate = useNavigate();
  const api = useApi();
  const projects = useProjects();
  const toast = useToast();
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const [capture, setCapture] = useState<CaptureKind | null>(startWith);
  const deferred = useDeferredValue(query);
  const found = useSearch(deferred);
  const returnTo = useRef<Element | null>(document.activeElement);

  // Give focus back to where it was when the palette closes.
  useEffect(() => {
    const previous = returnTo.current;
    return () => {
      if (previous instanceof HTMLElement) previous.focus();
    };
  }, []);

  const leave = (then: () => void) => {
    onClose();
    then();
  };

  const actions: Entry[] = [
    {
      key: "capture",
      group: id.palette.groupActions,
      label: id.palette.capture,
      icon: PenLine,
      keywords: "catat cepat capture note",
      run: () => setCapture("task"),
    },
    {
      key: "task",
      group: id.palette.groupActions,
      label: id.palette.newTask,
      icon: ListPlus,
      keywords: "tugas baru todo task",
      run: () => setCapture("task"),
    },
    {
      key: "reminder",
      group: id.palette.groupActions,
      label: id.palette.newReminder,
      icon: Bell,
      keywords: "pengingat ingatkan reminder",
      run: () => setCapture("reminder"),
    },
    {
      key: "event",
      group: id.palette.groupActions,
      label: id.palette.newEvent,
      icon: CalendarPlus,
      keywords: "jadwal acara event kalender",
      run: () => leave(() => void navigate({ to: "/kalender", search: { new: "event" } })),
    },
    {
      key: "routine",
      group: id.palette.groupActions,
      label: id.palette.newRoutine,
      icon: Repeat,
      keywords: "rutinitas pelajaran kebiasaan routine",
      run: () => leave(() => void navigate({ to: "/kalender", search: { new: "routine" } })),
    },
    {
      key: "review",
      group: id.palette.groupActions,
      label: id.palette.review,
      icon: Moon,
      keywords: "tutup hari review malam",
      run: () => leave(() => void navigate({ to: "/", search: { review: true } })),
    },
    {
      key: "lentera",
      group: id.palette.groupActions,
      label: id.palette.lentera,
      icon: Lamp,
      keywords: "lentera tanya asisten bantuan ctrl l",
      run: () => leave(() => void api.openLentera().catch(() => undefined)),
    },
    {
      key: "focus",
      group: id.palette.groupActions,
      label: id.palette.focus,
      icon: Timer,
      keywords: "sesi fokus pomodoro timer belajar konsentrasi",
      run: () => leave(() => void api.startFocus({}).catch(() => undefined)),
    },
  ];
  const pageEntries: Entry[] = matchPages("").map((page) => ({
    key: page.path,
    group: id.palette.groupPages,
    label: page.label,
    icon: page.icon,
    keywords: page.keywords,
    run: () => leave(() => void navigate({ to: page.path })),
  }));
  const projectEntries: Entry[] = (projects.data ?? [])
    .filter((p) => p.status === "active" || p.status === "paused")
    .map((p) => ({
      key: `project-${p.id}`,
      group: id.palette.groupProjects,
      label: p.name,
      icon: FolderKanban,
      keywords: `${p.area?.name ?? ""} proyek`,
      run: () => leave(() => void navigate({ to: "/proyek/$projectId", params: { projectId: p.id } })),
    }));

  const words = normalize(query).split(/\s+/).filter(Boolean);
  const matches = (entry: Entry) => words.every((word) => normalize(`${entry.label} ${entry.keywords}`).includes(word));
  const open = (hit: SearchHit) => {
    switch (hit.kind) {
      case "task":
        return navigate({ to: "/proyek", search: { view: "anytime", task: hit.id } });
      case "project":
        return navigate({ to: "/proyek/$projectId", params: { projectId: hit.id } });
      case "file":
        return navigate({ to: "/file", search: { file: hit.id } });
      case "collection":
        return navigate({ to: "/file", search: { collection: hit.id } });
      case "learning_item":
        return navigate({ to: "/belajar", search: { item: hit.id } });
      case "event":
        return navigate({ to: "/kalender", search: { view: "day", date: hit.detail } });
      case "journal":
        // Notes live in Jejak; open it on its Catatan filter so the note is among few rows (F3).
        return navigate({ to: "/jejak", search: { group: "notes" } });
    }
  };
  const named = [...actions, ...pageEntries, ...projectEntries].filter(matches);
  const hits: Entry[] =
    words.length === 0
      ? []
      : (found.data ?? [])
          .filter((hit) => !(hit.kind === "project" && named.some((entry) => entry.key === `project-${hit.id}`)))
          .map((hit) => ({
            key: `hit-${hit.kind}-${hit.id}`,
            group: id.search.kinds[hit.kind],
            label: hit.title,
            icon: HIT_ICONS[hit.kind],
            keywords: "",
            detail: hit.detail,
            run: () => leave(() => void open(hit)),
          }));
  // With nothing typed, pages come first: the palette is mostly for going places.
  const results = words.length === 0 ? [...pageEntries, ...actions] : [...named, ...hits];
  const current = Math.min(active, Math.max(0, results.length - 1));

  const onKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    switch (event.key) {
      case "ArrowDown":
        event.preventDefault();
        setActive((current + 1) % Math.max(1, results.length));
        break;
      case "ArrowUp":
        event.preventDefault();
        setActive((current - 1 + results.length) % Math.max(1, results.length));
        break;
      case "Enter": {
        event.preventDefault();
        results[current]?.run();
        break;
      }
      case "Escape":
        event.preventDefault();
        onClose();
        break;
      case "Tab":
        // The input is the only stop inside the palette.
        event.preventDefault();
        break;
    }
  };

  if (capture) {
    return (
      <>
        <div className={styles.scrim} onMouseDown={onClose} />
        <div className={`${styles.panel} ${styles.capturePanel}`} role="dialog" aria-modal="true" aria-label={id.capture.title}>
          <CaptureForm
            initialKind={capture}
            autoFocus
            onSaved={(saved) => {
              toast({ text: id.capture.saved[saved.kind] });
              onClose();
            }}
            onCancel={() => setCapture(null)}
          />
        </div>
      </>
    );
  }

  return (
    <>
      <div className={styles.scrim} onMouseDown={onClose} />
      <div className={styles.panel} role="dialog" aria-modal="true" aria-label={id.palette.title}>
        <input
          className={styles.input}
          role="combobox"
          aria-expanded="true"
          aria-controls="palette-list"
          aria-activedescendant={results.length ? `palette-${current}` : undefined}
          aria-autocomplete="list"
          placeholder={id.palette.placeholder}
          value={query}
          autoFocus
          onChange={(event) => {
            setQuery(event.target.value);
            setActive(0);
          }}
          onKeyDown={onKeyDown}
        />
        <ul className={styles.list} id="palette-list" role="listbox" aria-label={id.palette.title}>
          {results.map((entry, index) => (
            <li
              key={entry.key}
              id={`palette-${index}`}
              role="option"
              aria-selected={index === current}
              className={styles.option}
              onMouseEnter={() => setActive(index)}
              onMouseDown={(event) => event.preventDefault()}
              onClick={entry.run}
            >
              <entry.icon size={17} strokeWidth={1.6} aria-hidden="true" />
              <span className={styles.label}>
                {entry.label}
                {entry.detail && <small>{entry.detail}</small>}
              </span>
              <small className={styles.group}>{entry.group}</small>
              {index === current && <CornerDownLeft className={styles.enter} size={14} aria-hidden="true" />}
            </li>
          ))}
          {results.length === 0 && <li className={styles.empty}>{id.palette.empty}</li>}
        </ul>
      </div>
    </>
  );
}
