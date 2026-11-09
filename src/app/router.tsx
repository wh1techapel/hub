// Every page has an address, so ULTRON (CP5) and the palette can open it. Hash history: the app is
// served from files, where a path that is not index.html would not load.

import { createHashHistory, createRootRoute, createRoute, createRouter, lazyRouteComponent, redirect } from "@tanstack/react-router";

import type { Actor } from "../data/generated/Actor";
import type { FileKind } from "../data/generated/FileKind";
import type { TrailGroup } from "../data/generated/TrailGroup";
import { CalendarPage } from "../pages/calendar/CalendarPage";
import { FilesPage } from "../pages/files/FilesPage";
import type { FilesSearch } from "../pages/files/FilesPage";
import { LearningPage } from "../pages/learning/LearningPage";
import type { LearningSearch } from "../pages/learning/LearningPage";
import { ProjectPage } from "../pages/projects/ProjectPage";
import { FIRST_PART, isSettingsSlug } from "../pages/settings/parts";
import { SettingsPage } from "../pages/settings/SettingsPage";
import { isPlace, WorkPage } from "../pages/tasks/WorkPage";
import type { Place } from "../pages/tasks/WorkPage";
import { TodayPage } from "../pages/today/TodayPage";
import { TrailPage } from "../pages/trail/TrailPage";
import type { TrailSearch } from "../pages/trail/TrailPage";
import { Shell } from "./Shell";

const root = createRootRoute({ component: Shell });

// Search parameters are read defensively: an address can come from ULTRON or a notification.
const text = (value: unknown): string | undefined =>
  typeof value === "string" && value.length > 0 && value.length <= 64 ? value : undefined;
const pick = <T extends string>(value: unknown, options: readonly T[]): T | undefined => options.find((option) => option === value);
const flag = (value: unknown): true | undefined => (value === true || value === 1 || value === "1" || value === "true" ? true : undefined);

export interface TodaySearch {
  review?: true;
}
export interface CalendarSearch {
  /** `pengingat`: the reminders (R6), beside the day, week, and month. */
  view?: "day" | "week" | "month" | "pengingat";
  date?: string;
  new?: "event" | "routine";
}
/** Proyek & Tugas: the projects (no view) or a task list, and the task open beside it. */
export interface WorkSearch {
  view?: Place;
  task?: string;
}
/** The old Tugas address (before R6), read only to send it on. */
export interface TasksSearch {
  view?: string;
  task?: string;
}

const todaySearch = (search: Record<string, unknown>): TodaySearch => (flag(search.review) ? { review: true } : {});
const calendarSearch = (search: Record<string, unknown>): CalendarSearch => ({
  view: pick(search.view, ["day", "week", "month", "pengingat"] as const),
  date: text(search.date),
  new: pick(search.new, ["event", "routine"] as const),
});
const tasksSearch = (search: Record<string, unknown>): TasksSearch => ({ view: text(search.view), task: text(search.task) });
const workSearch = (search: Record<string, unknown>): WorkSearch => ({
  view: isPlace(search.view) && search.view !== "projects" ? search.view : undefined,
  task: text(search.task),
});

/** Where an old Tugas address (a bookmark, ULTRON, a link from before R6) leads now: its list in
 * Proyek & Tugas; Hari Ini's list to Hari Ini itself (a task named in it opens among all open
 * ones); Pengingat to Kalender. */
export function tugasTarget(search: TasksSearch): { to: "/" } | { to: "/kalender"; search: CalendarSearch } | { to: "/proyek"; search: WorkSearch } {
  if (search.view === "reminders") return { to: "/kalender", search: { view: "pengingat" } };
  if (search.view === "today" && !search.task) return { to: "/" };
  const view = isPlace(search.view) && search.view !== "projects" ? search.view : "anytime";
  return { to: "/proyek", search: search.task ? { view, task: search.task } : { view } };
}
const KINDS: readonly FileKind[] = ["document", "image", "code", "archive", "media", "folder", "other"];
const filesSearch = (search: Record<string, unknown>): FilesSearch => ({
  kind: pick(search.kind, KINDS),
  collection: text(search.collection),
  project: text(search.project),
  missing: flag(search.missing),
  file: text(search.file),
  view: pick(search.view, ["grid", "list"] as const),
});
const learningSearch = (search: Record<string, unknown>): LearningSearch => ({ track: text(search.track), item: text(search.item) });
const ACTORS: readonly Actor[] = ["owner", "ultron", "hub"];
const GROUPS: readonly TrailGroup[] = ["tasks", "projects", "calendar", "files", "learning", "notes", "studio"];
const trailSearch = (search: Record<string, unknown>): TrailSearch => ({
  actor: pick(search.actor, ACTORS),
  group: pick(search.group, GROUPS),
});

const routeTree = root.addChildren([
  createRoute({ getParentRoute: () => root, path: "/", component: TodayPage, validateSearch: todaySearch }),
  createRoute({ getParentRoute: () => root, path: "/kalender", component: CalendarPage, validateSearch: calendarSearch }),
  createRoute({
    getParentRoute: () => root,
    path: "/tugas",
    validateSearch: tasksSearch,
    beforeLoad: ({ search }) => {
      // The router's own way to send an address on: a redirect is thrown, not an Error.
      // eslint-disable-next-line @typescript-eslint/only-throw-error
      throw redirect({ ...tugasTarget(search), replace: true });
    },
  }),
  createRoute({ getParentRoute: () => root, path: "/proyek", component: WorkPage, validateSearch: workSearch }),
  createRoute({ getParentRoute: () => root, path: "/proyek/$projectId", component: ProjectPage }),
  // Studio is loaded when first opened: its code stays out of the start of the app.
  createRoute({ getParentRoute: () => root, path: "/studio", component: lazyRouteComponent(() => import("../pages/studio/StudioPage"), "StudioPage") }),
  createRoute({
    getParentRoute: () => root,
    path: "/studio/$projectId",
    component: lazyRouteComponent(() => import("../pages/studio/StudioProjectPage"), "StudioProjectPage"),
  }),
  createRoute({ getParentRoute: () => root, path: "/belajar", component: LearningPage, validateSearch: learningSearch }),
  createRoute({ getParentRoute: () => root, path: "/file", component: FilesPage, validateSearch: filesSearch }),
  createRoute({ getParentRoute: () => root, path: "/jejak", component: TrailPage, validateSearch: trailSearch }),
  // P1: Pengaturan in parts. `/pengaturan` alone (and a part that does not exist) opens the first.
  createRoute({
    getParentRoute: () => root,
    path: "/pengaturan",
    beforeLoad: () => {
      // eslint-disable-next-line @typescript-eslint/only-throw-error
      throw redirect({ to: "/pengaturan/$bagian", params: { bagian: FIRST_PART }, replace: true });
    },
  }),
  createRoute({
    getParentRoute: () => root,
    path: "/pengaturan/$bagian",
    component: SettingsPage,
    beforeLoad: ({ params }: { params: { bagian: string } }) => {
      if (!isSettingsSlug(params.bagian)) {
        // eslint-disable-next-line @typescript-eslint/only-throw-error
        throw redirect({ to: "/pengaturan/$bagian", params: { bagian: FIRST_PART }, replace: true });
      }
    },
  }),
]);

export function createAppRouter(history = createHashHistory()) {
  return createRouter({ routeTree, history });
}

export type AppRouter = ReturnType<typeof createAppRouter>;

declare module "@tanstack/react-router" {
  interface Register {
    router: AppRouter;
  }
}
