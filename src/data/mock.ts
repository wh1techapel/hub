// An in-memory backend for the browser (`npm run dev`) and for tests. It follows the core's rules
// where the UI can see them (lists, focus, routines on their days, reminders firing, Tutup Hari)
// in a simpler way, holds a demo week like `hub-seed`, and announces every write. The quick
// capture reading here is a small stand-in: the real one is hub-core's capture.rs.

import { HubError } from "./backend";
import type { ClaudeSignals } from "./generated/ClaudeSignals";
import type { Backend, Command, HubEvent } from "./backend";
import type { ActivityItem } from "./generated/ActivityItem";
import type { Actor } from "./generated/Actor";
import type { Area } from "./generated/Area";
import type { CalendarDay } from "./generated/CalendarDay";
import type { CalendarItem } from "./generated/CalendarItem";
import type { CaptureKind } from "./generated/CaptureKind";
import type { Change } from "./generated/Change";
import type { CheckReport } from "./generated/CheckReport";
import type { CloseInput } from "./generated/CloseInput";
import type { Day } from "./generated/Day";
import type { DayReview } from "./generated/DayReview";
import type { Entity } from "./generated/Entity";
import type { EventInput } from "./generated/EventInput";
import type { Milestone } from "./generated/Milestone";
import type { Pigment } from "./generated/Pigment";
import type { ProjectDetail } from "./generated/ProjectDetail";
import type { ProjectInput } from "./generated/ProjectInput";
import type { ProjectStatus } from "./generated/ProjectStatus";
import type { ProjectSummary } from "./generated/ProjectSummary";
import type { Reading } from "./generated/Reading";
import type { ReminderInput } from "./generated/ReminderInput";
import type { ReminderItem } from "./generated/ReminderItem";
import type { ReminderState } from "./generated/ReminderState";
import type { Routine } from "./generated/Routine";
import type { RoutineInput } from "./generated/RoutineInput";
import type { Settings } from "./generated/Settings";
import type { SettingsPatch } from "./generated/SettingsPatch";
import type { Slot } from "./generated/Slot";
import type { Source } from "./generated/Source";
import type { StudioProject } from "./generated/StudioProject";
import type { TaskCard } from "./generated/TaskCard";
import type { TaskInput } from "./generated/TaskInput";
import type { TaskStatus } from "./generated/TaskStatus";
import type { TaskView } from "./generated/TaskView";
import type { TodayView } from "./generated/TodayView";
import type { Verb } from "./generated/Verb";
import { isoDate, minutesOf, parseClock } from "../time/format";
import type { Drag } from "./api";
import type { SearchHit } from "./generated/SearchHit";
import { focusMock } from "./mockFocus";
import { lenteraMock } from "./mockLentera";
import { libraryMock } from "./mockLibrary";
import { studioMock } from "./mockStudio";

export const defaultSettings: Settings = {
  ownerName: "Glenvio",
  wakeTime: "05:00",
  sleepTime: "22:30",
  themeMode: "auto",
  ink: "biru",
  autostart: true,
  timeZone: null,
  closeNudge: true,
  closeNudgeTime: "21:00",
  learningGoalDays: 4,
  apiEnabled: true,
  apiPort: 47731,
  claudeEnabled: false,
  geminiModel: "gemini-3.8-flash",
  devKnows: "",
  devLearning: "",
  devCost: "free",
  devDeploy: "",
  soundEnabled: true,
  soundVolume: 30,
  focusMinutes: 25,
  breakMinutes: 5,
  longBreakMinutes: 15,
  longBreakEvery: 4,
};

interface TaskRec {
  id: string;
  title: string;
  notes: string;
  status: TaskStatus;
  priority: number;
  doDate: string | null;
  dueDate: string | null;
  dueTime: string | null;
  estimateMin: number | null;
  projectId: string | null;
  areaId: string | null;
  parentId: string | null;
  milestoneId: string | null;
  order: number;
  completedAt: string | null;
  source: Source;
  deleted: boolean;
}

interface EventRec {
  id: string;
  title: string;
  location: string;
  notes: string;
  allDay: boolean;
  date: string;
  endDate: string;
  startMinute: number;
  endMinute: number;
  pigment: Pigment;
  projectId: string | null;
  taskId: string | null;
  source: Source;
  deleted: boolean;
}

interface ReminderRec {
  id: string;
  text: string;
  date: string;
  minute: number;
  state: ReminderState;
  firedAt: string | null;
  taskId: string | null;
  source: Source;
}

interface ProjectRec {
  id: string;
  name: string;
  description: string;
  areaId: string | null;
  pigment: Pigment;
  status: ProjectStatus;
  targetDate: string | null;
  source: Source;
  deleted: boolean;
}

/** A milestone as stored: its own tick only counts while it has no tasks (like the core). */
interface MilestoneRec {
  id: string;
  projectId: string;
  title: string;
  dueDate: string | null;
  ticked: boolean;
}

type Exception = { kind: "skipped" } | { kind: "moved"; slot: Slot };

interface Store {
  settings: Settings;
  areas: Area[];
  projects: ProjectRec[];
  milestones: MilestoneRec[];
  tasks: TaskRec[];
  events: EventRec[];
  routines: Routine[];
  exceptions: Map<string, Exception>;
  reminders: ReminderRec[];
  focus: Map<string, string[]>;
  closed: Set<string>;
  activity: ActivityItem[];
}

let counter = 0;
const newId = () => `00000000-0000-7000-8000-${String(++counter).padStart(12, "0")}`;

function addDays(date: string, days: number): string {
  const [y, m, d] = date.split("-").map(Number);
  return isoDate(new Date(y ?? 2026, (m ?? 1) - 1, (d ?? 1) + days));
}

const WEEKDAYS: Day[] = ["su", "mo", "tu", "we", "th", "fr", "sa"];
function weekdayOf(date: string): Day {
  const [y, m, d] = date.split("-").map(Number);
  return WEEKDAYS[new Date(y ?? 2026, (m ?? 1) - 1, d ?? 1).getDay()] ?? "mo";
}
function daysBetween(a: string, b: string): number {
  const [ya, ma, da] = a.split("-").map(Number);
  const [yb, mb, db] = b.split("-").map(Number);
  return Math.round((Date.UTC(yb ?? 0, (mb ?? 1) - 1, db ?? 1) - Date.UTC(ya ?? 0, (ma ?? 1) - 1, da ?? 1)) / 86_400_000);
}

function occursOn(routine: Routine, date: string): boolean {
  if (date < routine.startsOn || (routine.endsOn !== null && date > routine.endsOn)) return false;
  const { freq, interval, byDay } = routine.rule;
  const step = Math.max(1, interval);
  if (freq === "daily") return daysBetween(routine.startsOn, date) % step === 0;
  if (freq === "monthly") return date.slice(8) === routine.startsOn.slice(8);
  const days = byDay.length ? byDay : [weekdayOf(routine.startsOn)];
  const weeks = Math.floor(daysBetween(routine.startsOn, date) / 7);
  return days.includes(weekdayOf(date)) && weeks % step === 0;
}

/** The demo week, like hub-seed, around `now`. */
function demoStore(now: Date): Store {
  const today = isoDate(now);
  const monday = addDays(today, -((now.getDay() + 6) % 7));
  const store: Store = {
    settings: { ...defaultSettings },
    areas: [],
    projects: [],
    milestones: [],
    tasks: [],
    events: [],
    routines: [],
    exceptions: new Map(),
    reminders: [],
    focus: new Map(),
    closed: new Set(),
    activity: [],
  };
  const area = (name: string, pigment: Pigment) => {
    const a: Area = { id: newId(), name, pigment, openProjects: 0 };
    store.areas.push(a);
    return a.id;
  };
  const school = area("Sekolah", "lazuardi");
  const cyber = area("Belajar Cyber", "lavender");
  area("Pribadi", "lumut");
  const own = area("Proyek Pribadi", "kunyit");
  const project = (name: string, pigment: Pigment, areaId: string, targetDate: string | null = null) => {
    const p: ProjectRec = {
      id: newId(),
      name,
      description: "",
      areaId,
      pigment,
      status: "active",
      targetDate,
      source: "owner",
      deleted: false,
    };
    store.projects.push(p);
    return p.id;
  };
  const lab = project("Laporan Lab Jaringan", "toska", school, addDays(today, 1));
  const hub = project("Hub", "kunyit", own);
  const learning = project("Jalur Jr Penetration Tester", "lavender", cyber);
  const kik = project("Presentasi KIK", "lazuardi", school, addDays(today, 6));
  const topology = newId();
  const written = newId();
  store.milestones.push(
    { id: topology, projectId: lab, title: "Topologi dan konfigurasi", dueDate: null, ticked: false },
    { id: written, projectId: lab, title: "Laporan ditulis", dueDate: addDays(today, 1), ticked: false },
  );
  const lesson = (
    title: string,
    byDay: Day[],
    start: string,
    minutes: number,
    location: string,
    pigment: Pigment,
    kind: Routine["kind"] = "lesson",
  ) =>
    store.routines.push({
      id: newId(),
      title,
      kind,
      rule: { freq: "weekly", interval: 1, byDay },
      startTime: start,
      durationMin: minutes,
      startsOn: monday,
      endsOn: null,
      location,
      pigment,
      projectId: null,
      source: "owner",
    });
  lesson("Matematika", ["mo", "th"], "07:00", 90, "Ruang 12", "lazuardi");
  lesson("Bahasa Inggris", ["mo", "we", "th"], "08:30", 90, "Ruang 12", "lazuardi");
  lesson("Praktikum Jaringan", ["tu", "th"], "10:15", 135, "Lab TJKT 2", "toska");
  lesson("Produktif TJKT", ["mo", "we", "th", "fr"], "13:00", 90, "Lab TJKT 2", "toska");
  lesson("Belajar Cyber", ["mo", "we", "th", "fr"], "19:00", 90, "TryHackMe", "lavender", "study");

  const task = (title: string, projectId: string | null, extra: Partial<TaskRec> = {}) => {
    const t: TaskRec = {
      id: newId(),
      title,
      notes: "",
      status: "todo",
      priority: 0,
      doDate: null,
      dueDate: null,
      dueTime: null,
      estimateMin: null,
      projectId,
      areaId: null,
      parentId: null,
      milestoneId: null,
      order: store.tasks.length + 1,
      completedAt: null,
      source: "owner",
      deleted: false,
      ...extra,
    };
    store.tasks.push(t);
    return t.id;
  };
  task("Rangkai topologi VLAN di Packet Tracer", lab, {
    milestoneId: topology,
    status: "done",
    completedAt: new Date(now.getTime() - 7_200_000).toISOString(),
  });
  const report = task("Laporan Praktikum 5: VLAN dan Trunking", lab, {
    milestoneId: written,
    dueDate: addDays(today, 1),
    dueTime: "23:59",
    priority: 2,
    estimateMin: 90,
  });
  const nmap = task("THM: Nmap Live Host Discovery", learning, { doDate: today, priority: 1, estimateMin: 45 });
  const design = task("Hub: kunci arah desain", hub, {
    doDate: today,
    status: "done",
    completedAt: new Date(now.getTime() - 3_600_000).toISOString(),
  });
  task("Revisi slide presentasi KIK", kik, { doDate: today });
  const cable = task("Bawa kabel UTP ke lab besok", kik, { doDate: today, source: "ultron" });
  task("Latihan soal subnetting /27", learning, { doDate: addDays(today, 2) });
  task("Ide: dashboard homelab", null);
  task("Topologi VLAN", lab, { parentId: report, status: "done", completedAt: new Date(now.getTime() - 7_200_000).toISOString() });
  task("Konfigurasi trunk", lab, { parentId: report });
  store.focus.set(today, [report, nmap, design]);
  store.events.push(
    {
      id: newId(),
      title: "Kerjakan Laporan Lab",
      location: "",
      notes: "",
      allDay: false,
      date: today,
      endDate: today,
      startMinute: 960,
      endMinute: 1020,
      pigment: "kunyit",
      projectId: lab,
      taskId: report,
      source: "owner",
      deleted: false,
    },
    {
      id: newId(),
      title: "Rapat OSIS",
      location: "Aula",
      notes: "",
      allDay: false,
      date: addDays(today, 1),
      endDate: addDays(today, 1),
      startMinute: 900,
      endMinute: 990,
      pigment: "terakota",
      projectId: null,
      taskId: null,
      source: "owner",
      deleted: false,
    },
  );
  store.reminders.push({
    id: newId(),
    text: "Kumpulkan laporan di Classroom",
    date: today,
    minute: 1260,
    state: "scheduled",
    firedAt: null,
    taskId: report,
    source: "owner",
  });
  const at = new Date(now.getFullYear(), now.getMonth(), now.getDate(), 11, 20).toISOString();
  store.activity.push(
    { id: newId(), at, actor: "owner", verb: "completed", entity: "task", entityId: design, summary: "Hub: kunci arah desain" },
    { id: newId(), at, actor: "ultron", verb: "created", entity: "task", entityId: cable, summary: "Bawa kabel UTP ke lab besok" },
  );
  return store;
}

function emptyStore(): Store {
  return {
    settings: { ...defaultSettings },
    areas: [],
    projects: [],
    milestones: [],
    tasks: [],
    events: [],
    routines: [],
    exceptions: new Map(),
    reminders: [],
    focus: new Map(),
    closed: new Set(),
    activity: [],
  };
}

export interface MockBackend extends Backend {
  /** Announce a change as if another client (ULTRON) made it. */
  emit: (change: Change) => void;
  /** Add a task as another client would. */
  addTask: (title: string, actor: Actor, extra?: Partial<TaskRec>) => string;
  /** Fire due reminders now, as the scheduler would. */
  fireDue: () => void;
  /** Drag files from "Explorer" and drop them at a point of the page. */
  dropFiles: (names: string[], x: number, y: number) => void;
  /** Send a native event (a page to go to, the capture window shown again). */
  send: (event: Exclude<HubEvent, "hub://changed">, payload: unknown) => void;
  /** Every call, as "command json". */
  calls: string[];
  /** What the bridge reported (R5), for any project and for all, announced as Hub would. */
  setClaudeSignals: (project: ClaudeSignals, all?: ClaudeSignals) => void;
}

function fail(field: string, reason: string): never {
  throw new HubError({ kind: "invalid", field, reason, message: `invalid ${field}` });
}

export function createMockBackend(
  options: {
    empty?: boolean;
    settings?: Partial<Settings>;
    geminiKey?: string | null;
    studioProjects?: StudioProject[];
    claudeCli?: boolean;
    claudeRegistered?: boolean;
    /** Sesi Fokus: how long a minute of Pengaturan lasts (1 makes a 25-minute block 25 s). */
    focusSecondsPerMinute?: number;
  } = {},
): MockBackend {
  const store = options.empty ? emptyStore() : demoStore(new Date());
  store.settings = { ...store.settings, ...options.settings };
  const listeners = new Set<(change: Change) => void>();
  const others = new Map<string, Set<(payload: unknown) => void>>();
  const calls: string[] = [];
  // Gemini: the key as Credential Manager would hold it (never returned), and Studio's projects.
  let geminiKey: string | null = options.geminiKey ?? null;

  const emit = (change: Change) => {
    for (const listener of listeners) listener(change);
  };
  const today = () => isoDate(new Date());
  const minuteNow = () => minutesOf(new Date());
  const record = (actor: Actor, verb: Verb, entity: Entity, entityId: string | null, summary: string) =>
    store.activity.unshift({ id: newId(), at: new Date().toISOString(), actor, verb, entity, entityId, summary });
  const done = (entity: Entity, id?: string) => emit({ entity, ids: id ? [id] : [] });
  const studio = studioMock(
    {
      newId,
      record,
      done,
      hasKey: () => geminiKey !== null,
      model: () => store.settings.geminiModel,
      createHubProject: (name, description) => {
        const p: ProjectRec = { id: newId(), name, description, areaId: null, pigment: "batu", status: "active", targetDate: null, source: "owner", deleted: false };
        store.projects.push(p);
        record("owner", "created", "project", p.id, p.name);
        done("project", p.id);
        return p.id;
      },
      hubProjectName: (id) => store.projects.find((p) => p.id === id && !p.deleted)?.name ?? null,
    },
    options.studioProjects ?? [],
  );

  const project = (id: string | null) => store.projects.find((p) => p.id === id && !p.deleted) ?? null;
  const area = (id: string | null) => store.areas.find((a) => a.id === id) ?? null;
  const live = () => store.tasks.filter((t) => !t.deleted);
  const open = (t: TaskRec) => t.status === "todo" || t.status === "doing";
  const doneToday = (t: TaskRec) => t.status === "done" && t.completedAt !== null && isoDate(new Date(t.completedAt)) === today();

  function card(t: TaskRec): TaskCard {
    const p = project(t.projectId);
    const a = area(t.areaId ?? p?.areaId ?? null);
    const steps = live().filter((s) => s.parentId === t.id && s.status !== "cancelled");
    return {
      id: t.id,
      title: t.title,
      status: t.status,
      priority: t.priority,
      doDate: t.doDate,
      dueDate: t.dueDate,
      dueTime: t.dueTime,
      estimateMin: t.estimateMin,
      overdue: open(t) && t.dueDate !== null && t.dueDate < today(),
      project: p ? { id: p.id, name: p.name, pigment: p.pigment } : null,
      area: a ? { id: a.id, name: a.name, pigment: a.pigment } : null,
      parentId: t.parentId,
      milestoneId: t.milestoneId,
      subtasksDone: steps.filter((s) => s.status === "done").length,
      subtasksTotal: steps.length,
      scheduled: store.events.some((e) => !e.deleted && e.taskId === t.id),
      completedAt: t.completedAt,
      source: t.source,
    };
  }

  function listTasks(view: TaskView): TaskCard[] {
    const d = today();
    const top = live()
      .filter((t) => t.parentId === null)
      .sort((a, b) => a.order - b.order);
    let picked: TaskRec[];
    switch (view) {
      case "inbox":
        picked = top.filter((t) => open(t) && !t.projectId && !t.areaId && !t.doDate && !t.dueDate);
        break;
      case "today":
        picked = top.filter(
          (t) => (open(t) && ((t.doDate !== null && t.doDate <= d) || (t.dueDate !== null && t.dueDate <= d))) || doneToday(t),
        );
        break;
      case "upcoming":
        picked = top
          .filter((t) => open(t) && ((t.doDate !== null && t.doDate > d) || (t.doDate === null && t.dueDate !== null && t.dueDate > d)))
          .sort((a, b) => (a.doDate ?? a.dueDate ?? "").localeCompare(b.doDate ?? b.dueDate ?? "") || a.order - b.order);
        break;
      case "anytime":
        picked = top.filter(open);
        break;
      case "done":
        picked = top.filter((t) => t.status === "done").sort((a, b) => (b.completedAt ?? "").localeCompare(a.completedAt ?? ""));
        break;
    }
    return picked.map(card);
  }

  function taskInput(input: TaskInput, isStep = input.parentId !== null) {
    const title = input.title.trim();
    if (!title || title.length > 300) fail("title", "length");
    if (input.dueTime && !input.dueDate) fail("dueTime", "order");
    if (input.milestoneId) {
      // Like the core: one of the task's own project's milestones, and never on a step.
      const m = store.milestones.find((x) => x.id === input.milestoneId);
      if (!m || isStep || m.projectId !== input.projectId) fail("milestoneId", "unknown");
    }
    return { ...input, title, milestoneId: input.milestoneId ?? null };
  }

  /** A milestone's tasks: top-level, not cancelled, still in its project. */
  function milestoneTasks(m: MilestoneRec): TaskRec[] {
    return live().filter((t) => t.milestoneId === m.id && t.projectId === m.projectId && t.parentId === null && t.status !== "cancelled");
  }

  function milestoneView(m: MilestoneRec): Milestone {
    const tasks = milestoneTasks(m);
    const tasksDone = tasks.filter((t) => t.status === "done").length;
    return {
      id: m.id,
      title: m.title,
      dueDate: m.dueDate,
      done: tasks.length > 0 ? tasksDone === tasks.length : m.ticked,
      tasksDone,
      tasksTotal: tasks.length,
    };
  }

  function focusOf(date: string): TaskCard[] {
    return (store.focus.get(date) ?? [])
      .map((id) => live().find((t) => t.id === id))
      .filter((t): t is TaskRec => t !== undefined)
      .map(card);
  }

  function suggest(date: string): TaskCard[] {
    const chosen = store.focus.get(date) ?? [];
    return live()
      .filter((t) => t.parentId === null && open(t) && !chosen.includes(t.id))
      .filter((t) => (t.doDate !== null && t.doDate <= date) || (t.dueDate !== null && t.dueDate <= addDays(date, 2)) || t.priority >= 2)
      .sort(
        (a, b) =>
          Number(b.dueDate !== null && b.dueDate < date) - Number(a.dueDate !== null && a.dueDate < date) ||
          (a.dueDate ?? "9").localeCompare(b.dueDate ?? "9") ||
          b.priority - a.priority,
      )
      .slice(0, 3)
      .map(card);
  }

  function calendarDay(date: string): CalendarDay {
    const items: CalendarItem[] = [];
    for (const e of store.events.filter((x) => !x.deleted)) {
      const base = {
        eventId: e.id,
        routineId: null,
        ruleDate: null,
        taskId: e.taskId,
        taskDone: e.taskId !== null && store.tasks.some((t) => t.id === e.taskId && t.status === "done"),
        title: e.title,
        location: e.location,
        moved: false,
        pigment: e.pigment,
        source: e.source,
        kind: e.taskId ? ("block" as const) : ("event" as const),
      };
      if (e.allDay && e.date <= date && e.endDate >= date) {
        items.push({
          ...base,
          key: e.id,
          allDay: true,
          startMinute: 0,
          endMinute: 1440,
          continuesBefore: e.date < date,
          continuesAfter: e.endDate > date,
        });
      } else if (!e.allDay && e.date === date) {
        items.push({
          ...base,
          key: e.id,
          allDay: false,
          startMinute: e.startMinute,
          endMinute: Math.min(1440, e.endMinute),
          continuesBefore: false,
          continuesAfter: e.endMinute > 1440,
        });
      } else if (!e.allDay && addDays(e.date, 1) === date && e.endMinute > 1440) {
        items.push({
          ...base,
          key: e.id,
          allDay: false,
          startMinute: 0,
          endMinute: e.endMinute - 1440,
          continuesBefore: true,
          continuesAfter: false,
        });
      }
    }
    for (const r of store.routines) {
      const start = parseClock(r.startTime) ?? 0;
      const routineItem = (ruleDate: string, startMinute: number, endMinute: number, moved: boolean): CalendarItem => ({
        key: `${r.id}:${ruleDate}`,
        kind: "routine",
        eventId: null,
        routineId: r.id,
        ruleDate,
        taskId: null,
        taskDone: false,
        title: r.title,
        location: r.location,
        allDay: false,
        startMinute,
        endMinute: Math.min(1440, endMinute),
        continuesBefore: false,
        continuesAfter: endMinute > 1440,
        moved,
        pigment: r.pigment,
        source: r.source,
      });
      if (occursOn(r, date)) {
        const exception = store.exceptions.get(`${r.id}:${date}`);
        if (!exception) items.push(routineItem(date, start, start + r.durationMin, false));
      }
      for (const [key, exception] of store.exceptions) {
        if (exception.kind === "moved" && key.startsWith(`${r.id}:`) && exception.slot.date === date) {
          items.push(routineItem(key.slice(r.id.length + 1), exception.slot.startMinute, exception.slot.endMinute, true));
        }
      }
    }
    items.sort((a, b) => Number(b.allDay) - Number(a.allDay) || a.startMinute - b.startMinute || a.title.localeCompare(b.title));
    const wake = parseClock(store.settings.wakeTime) ?? 300;
    const sleep = parseClock(store.settings.sleepTime) ?? 1350;
    const scheduled = items
      .filter((i) => !i.allDay)
      .reduce((sum, i) => sum + Math.max(0, Math.min(i.endMinute, sleep) - Math.max(i.startMinute, wake)), 0);
    const planned = live()
      .filter((t) => t.parentId === null && open(t) && t.doDate === date && !store.events.some((e) => !e.deleted && e.taskId === t.id))
      .reduce((sum, t) => sum + (t.estimateMin ?? 0), 0);
    const waking = sleep - wake;
    return {
      date,
      items,
      reminders: store.reminders
        .filter((r) => r.date === date && ["scheduled", "snoozed", "fired"].includes(r.state))
        .sort((a, b) => a.minute - b.minute)
        .map((r) => ({ id: r.id, text: r.text, minute: r.minute, state: r.state })),
      load: { scheduledMin: scheduled, plannedMin: planned, wakingMin: waking, over: (scheduled + planned) * 100 > waking * 85 },
      closed: store.closed.has(date),
    };
  }

  function reminderItem(r: ReminderRec): ReminderItem {
    const [y, m, d] = r.date.split("-").map(Number);
    const due = new Date(y ?? 2026, (m ?? 1) - 1, d ?? 1, Math.floor(r.minute / 60), r.minute % 60).toISOString();
    return { id: r.id, text: r.text, dueAt: due, state: r.state, firedAt: r.firedAt, taskId: r.taskId, source: r.source };
  }

  function summary(p: ProjectRec): ProjectSummary {
    const tasks = live().filter((t) => t.projectId === p.id && t.parentId === null && t.status !== "cancelled");
    const doneCount = tasks.filter((t) => t.status === "done").length;
    const total = tasks.length;
    const next = tasks.filter(open).sort((a, b) => a.order - b.order)[0];
    const a = area(p.areaId);
    return {
      id: p.id,
      name: p.name,
      description: p.description,
      pigment: p.pigment,
      area: a ? { id: a.id, name: a.name, pigment: a.pigment } : null,
      status: p.status,
      targetDate: p.targetDate,
      progress: {
        done: doneCount,
        total,
        percent: total ? Math.floor((doneCount * 100) / total) : 0,
      },
      nextStep: next ? { id: next.id, title: next.title } : null,
      openTasks: tasks.filter(open).length,
      source: p.source,
    };
  }

  function projectInput(input: ProjectInput) {
    const name = input.name.trim();
    if (!name || name.length > 120) fail("name", "length");
    return { ...input, name };
  }

  function readCapture(text: string, kind: CaptureKind): Reading {
    // A small stand-in for hub-core's reader: "besok", "hari ini", "lusa", "HH.MM"/"HH:MM", "#tag", "!".
    const d = today();
    let date: string | null = null;
    let time: string | null = null;
    let priority = 0;
    let projectRef: Reading["project"] = null;
    let unknownTag: string | null = null;
    const words: string[] = [];
    for (const word of text.split(/\s+/).filter(Boolean)) {
      const w = word.toLowerCase();
      if (w === "besok" || w === "tomorrow") date = addDays(d, 1);
      else if (w === "lusa") date = addDays(d, 2);
      else if (w === "today") date = d;
      else if (/^\d{1,2}[.:]\d{2}$/.test(w)) time = w.replace(".", ":").padStart(5, "0");
      else if (/^!{1,3}$/.test(w)) priority = w.length;
      else if (w.startsWith("#") && w.length > 1) {
        const p = store.projects.find((x) => x.name.toLowerCase().replace(/\s/g, "").includes(w.slice(1)));
        if (p) projectRef = { id: p.id, name: p.name, pigment: p.pigment };
        else unknownTag = w.slice(1);
      } else words.push(word);
    }
    const title = words
      .join(" ")
      .replace(/\bhari ini\b/i, "")
      .trim();
    if (time && !date) date = (parseClock(time) ?? 0) > minuteNow() ? d : addDays(d, 1);
    const missing = !title ? "title" : kind === "reminder" && !time ? "time" : kind === "event" && !date ? "date" : null;
    return {
      kind,
      title,
      date: kind === "note" ? (date ?? d) : date,
      time,
      endTime:
        kind === "event" && time ? `${String(Math.min(23, (parseClock(time) ?? 0) / 60 + 1) | 0).padStart(2, "0")}:${time.slice(3)}` : null,
      endNextDay: false,
      due: kind === "task" && time !== null,
      priority: kind === "task" ? priority : 0,
      project: projectRef,
      unknownTag,
      estimateMin: null,
      location: null,
      missing,
    };
  }

  const library = libraryMock(
    {
      newId,
      today,
      record,
      done,
      emitDrag: (drag: Drag) => {
        for (const listener of others.get("hub://drag") ?? []) listener(drag);
      },
      linkTitle: (kind, id) => {
        if (kind === "project") {
          const p = project(id);
          return p ? { title: p.name, pigment: p.pigment } : null;
        }
        if (kind === "task") {
          const t = live().find((x) => x.id === id);
          return t ? { title: t.title, pigment: project(t.projectId)?.pigment ?? null } : null;
        }
        if (kind === "event") {
          const e = store.events.find((x) => x.id === id && !x.deleted);
          return e ? { title: e.title, pigment: e.pigment } : null;
        }
        return null;
      },
      projectTaskIds: (projectId) => live().filter((t) => t.projectId === projectId).map((t) => t.id),
      findProject: (name) => store.projects.find((p) => p.name === name)?.id ?? null,
      findTask: (title) => store.tasks.find((t) => t.title === title)?.id ?? null,
      activity: () => store.activity,
      searchCore: (words) => {
        const has = (text: string) => words.every((w) => text.toLowerCase().includes(w));
        return [
          ...live()
            .filter((t) => has(`${t.title} ${t.notes}`))
            .map((t): SearchHit => ({ kind: "task", id: t.id, title: t.title, detail: project(t.projectId)?.name ?? "", pigment: project(t.projectId)?.pigment ?? null, projectId: t.projectId })),
          ...store.projects
            .filter((p) => !p.deleted && has(`${p.name} ${p.description}`))
            .map((p): SearchHit => ({ kind: "project", id: p.id, title: p.name, detail: area(p.areaId)?.name ?? "", pigment: p.pigment, projectId: null })),
          ...store.events
            .filter((e) => !e.deleted && has(`${e.title} ${e.location}`))
            .map((e): SearchHit => ({ kind: "event", id: e.id, title: e.title, detail: e.date, pigment: e.pigment, projectId: null })),
        ];
      },
      doneBetween: (from, to) => {
        const finished = live().filter((t) => t.status === "done" && t.completedAt !== null && t.completedAt >= from && t.completedAt < to);
        return { tasks: finished.length, projects: new Set(finished.flatMap((t) => (t.projectId ? [t.projectId] : []))).size };
      },
      goalDays: () => store.settings.learningGoalDays,
      stopFocus: () => focus.stop(),
    },
    !options.empty,
  );

  // Sesi Fokus (Lentera CP02).
  const focus = focusMock(
    {
      record,
      done,
      settings: () => store.settings,
      taskTitle: (id) => live().find((t) => t.id === id)?.title ?? null,
      itemTitle: (id) => library.itemTitle(id),
      itemBlock: (itemId, startedAt, minutes) => library.focusBlock(itemId, startedAt, minutes),
      itemStarts: (itemId) => library.focusStarts(itemId),
      stopTimer: () => library.stopTimer(),
      fail,
      notFound: (what) => {
        throw new HubError({ kind: "not_found", field: what, reason: null, message: `${what} not found` });
      },
    },
    options.focusSecondsPerMinute,
  );

  // Lentera's brain (CP03): a pretend Gemini that proposes, Simpan through the handlers below.
  const lentera = lenteraMock({
    hasKey: () => geminiKey !== null,
    today,
    addDays,
    call: (command, args) => handlers[command]?.(args),
  });

  const apiStatus = () => ({
    enabled: store.settings.apiEnabled,
    running: store.settings.apiEnabled,
    port: store.settings.apiPort,
    problem: null,
    lastUsedAt: store.settings.apiEnabled ? new Date(Date.now() - 5 * 60_000).toISOString() : null,
    listeners: store.settings.apiEnabled ? 1 : 0,
    claudeEnabled: store.settings.claudeEnabled,
  });

  // Claude Code in the demo: the bridge is there; the `claude` command too unless the options say
  // otherwise. Pasang writes the skill and, with the command, registers; Uji jembatan passes while
  // it is switched on. `claudeRegistered` is the bridge registered by hand, without the skill.
  const claude = { cli: options.claudeCli ?? true, registered: options.claudeRegistered ?? false, skill: false };
  const claudeStatus = () => ({
    enabled: store.settings.claudeEnabled,
    bridge: "C:\\Users\\kamu\\AppData\\Local\\Programs\\Hub\\hub-mcp.exe",
    cli: claude.cli ? "C:\\Users\\kamu\\.local\\bin\\claude.exe" : null,
    registration: claude.cli ? (claude.registered ? "ours" : "missing") : "unknown",
    skill: claude.skill ? "ours" : "missing",
    skillPath: "C:\\Users\\kamu\\.claude\\skills\\hub-connect",
    dataDir: "C:\\Users\\kamu\\AppData\\Local\\Hub",
    commands: ['claude mcp add --scope user hub -- "C:\\Users\\kamu\\AppData\\Local\\Programs\\Hub\\hub-mcp.exe" --data-dir "C:\\Users\\kamu\\AppData\\Local\\Hub"'],
  });
  // What the bridge reported (R5): nothing until a test says so (setClaudeSignals).
  const none: ClaudeSignals = { session: null, open: 0, activity: null };
  const reported = { project: none, all: none };

  const handlers: Partial<Record<Command, (args: Record<string, unknown>) => unknown>> = {
    ...library.handlers,
    ...focus.handlers,
    ...(lentera.handlers as Partial<Record<Command, (args: Record<string, unknown>) => unknown>>),
    take_lentera_open: () => true,
    focus_lentera: () => undefined,
    ...(studio.handlers as Partial<Record<Command, (args: Record<string, unknown>) => unknown>>),
    get_settings: () => store.settings,
    update_settings: ({ patch }) => {
      const p = patch as SettingsPatch;
      if (p.ownerName !== undefined) {
        const name = p.ownerName.trim();
        if (!name || name.length > 40) fail("ownerName", "length");
        p.ownerName = name;
      }
      const wake = parseClock(p.wakeTime ?? store.settings.wakeTime) ?? 0;
      const sleep = parseClock(p.sleepTime ?? store.settings.sleepTime) ?? 0;
      if (sleep - wake < 360) fail("sleepTime", "order");
      if (p.apiPort !== undefined && p.apiPort < 1024) fail("apiPort", "format");
      store.settings = { ...store.settings, ...p };
      done("settings");
      return store.settings;
    },
    get_info: () => ({ version: "1.0.1", dataDir: "C:\\Users\\kamu\\AppData\\Local\\Hub", schemaVersion: 2, lastBackup: null }),
    get_shell: () => ({ hotkeyOk: true, hotkey: "Ctrl+Alt+H", lenteraHotkeyOk: true, lenteraHotkey: "Ctrl+Alt+L" }),
    get_api_status: () => apiStatus(),
    claude_code_status: () => claudeStatus(),
    install_claude_code: () => {
      claude.skill = true;
      if (claude.cli) claude.registered = true;
      if (!store.settings.claudeEnabled) {
        store.settings = { ...store.settings, claudeEnabled: true };
        done("settings");
      }
      return claudeStatus();
    },
    remove_claude_code: () => {
      claude.registered = false;
      claude.skill = false;
      return claudeStatus();
    },
    test_claude_bridge: () =>
      store.settings.claudeEnabled
        ? [
            { step: "bridge", ok: true, detail: "hub-mcp.exe" },
            { step: "hub", ok: true, detail: "1.0.1" },
            { step: "token", ok: true, detail: "1 proyek terlihat oleh Claude" },
          ]
        : [
            { step: "bridge", ok: true, detail: "hub-mcp.exe" },
            { step: "hub", ok: true, detail: "" },
            { step: "token", ok: false, detail: "not_allowed" },
          ],
    rotate_claude_token: () => claudeStatus(),
    claude_signals: ({ projectId }) => (projectId ? reported.project : reported.all),
    gemini_status: () => ({ hasKey: geminiKey !== null, model: store.settings.geminiModel }),
    gemini_save_key: ({ key }) => {
      const text = String(key).trim();
      if (text.length < 20 || text.length > 200) fail("key", "length");
      if (!/^[\x21-\x7e]+$/.test(text)) fail("key", "format");
      geminiKey = text;
      record("owner", "updated", "settings", null, "gemini_key");
      done("settings");
      return undefined;
    },
    gemini_delete_key: () => {
      geminiKey = null;
      record("owner", "deleted", "settings", null, "gemini_key");
      done("settings");
      return undefined;
    },
    // The demo pretends: a key with "invalid" in it is refused, "quota" has run out.
    gemini_check: (): CheckReport => {
      const model = store.settings.geminiModel;
      const info = { id: model, displayName: "Gemini 3.8 Flash", inputTokenLimit: 1048576, outputTokenLimit: 65536, generates: true };
      const none = { model: null, detail: null, code: null, retryAfter: null };
      if (geminiKey === null) return { outcome: "no_key", ...none };
      if (geminiKey.includes("invalid")) return { outcome: "key_invalid", ...none, detail: "API key not valid.", code: "authentication" };
      if (geminiKey.includes("quota")) return { outcome: "quota", ...none, model: info, detail: "Daily limit reached", code: "quota_exceeded", retryAfter: 30 };
      return { outcome: "connected", ...none, model: info };
    },
    gemini_models: () =>
      geminiKey === null
        ? []
        : [
            { id: "gemini-3.8-flash", displayName: "Gemini 3.8 Flash", inputTokenLimit: 1048576, outputTokenLimit: 65536, generates: true },
            { id: "gemini-3.5-flash-lite", displayName: "Gemini 3.5 Flash-Lite", inputTokenLimit: 1048576, outputTokenLimit: 65536, generates: true },
            { id: "gemini-3.8-pro-preview", displayName: "Gemini 3.8 Pro Preview", inputTokenLimit: 1048576, outputTokenLimit: 65536, generates: true },
            // Google lists speech and image models too; Studio offers text models only (L2).
            { id: "gemini-3.8-flash-preview-tts", displayName: "Gemini 3.8 Flash Preview TTS", inputTokenLimit: 8192, outputTokenLimit: 16384, generates: true },
            { id: "gemini-3.8-flash-image", displayName: "Gemini 3.8 Flash Image", inputTokenLimit: 32768, outputTokenLimit: 32768, generates: true },
            // Older versions too, as a real key lists them (the menu groups them by family).
            { id: "gemini-3.7-flash", displayName: "Gemini 3.7 Flash", inputTokenLimit: 1048576, outputTokenLimit: 65536, generates: true },
            { id: "gemini-2.5-pro", displayName: "Gemini 2.5 Pro", inputTokenLimit: 1048576, outputTokenLimit: 65536, generates: true },
          ],
    export_data: () => `C:\\Users\\kamu\\Documents\\Hub-ekspor-${isoDate(new Date())}.json`,
    rotate_api_token: () => {
      done("settings");
      return apiStatus();
    },
    window_ready: () => undefined,
    set_caption: () => undefined,
    hide_capture: () => undefined,
    open_lentera: () => undefined,
    set_lentera_shape: () => undefined,
    hide_lentera: () => undefined,
    return_lentera_focus: () => undefined,
    take_start_path: () => null,
    get_today: (): TodayView => {
      const d = today();
      const day = calendarDay(d);
      const focus = focusOf(d);
      return {
        date: d,
        items: day.items,
        reminders: day.reminders,
        load: day.load,
        focus,
        suggestions: focus.length ? [] : suggest(d),
        also: listTasks("today")
          .filter((t) => !focus.some((f) => f.id === t.id))
          .slice(0, 8),
        recent: store.activity.filter((a) => a.entity !== "settings").slice(0, 5),
        learning: library.glance(),
        files: library.recentFiles(4),
        alerts: store.reminders.filter((r) => r.state === "fired").map(reminderItem),
        closed: day.closed,
      };
    },
    list_tasks: ({ view }) => listTasks(view as TaskView),
    task_counts: () => ({
      inbox: listTasks("inbox").length,
      today: listTasks("today").filter((t) => t.status !== "done").length,
      upcoming: listTasks("upcoming").length,
      anytime: listTasks("anytime").length,
    }),
    get_task: ({ id }) => {
      const t = live().find((x) => x.id === id);
      if (!t) throw new HubError({ kind: "not_found", field: "task", reason: null, message: "task not found" });
      return {
        card: card(t),
        notes: t.notes,
        subtasks: live()
          .filter((s) => s.parentId === t.id)
          .sort((a, b) => a.order - b.order)
          .map(card),
        blocks: store.events
          .filter((e) => !e.deleted && e.taskId === t.id)
          .map((e) => ({ eventId: e.id, start: `${e.date}T00:00:00Z`, end: `${e.date}T00:00:00Z` })),
        createdAt: new Date().toISOString(),
        files: library.taskFiles(t.id),
      };
    },
    create_task: ({ input }) => {
      const i = taskInput(input as TaskInput);
      const t: TaskRec = {
        id: newId(),
        title: i.title,
        notes: i.notes,
        status: "todo",
        priority: i.priority,
        doDate: i.doDate,
        dueDate: i.dueDate,
        dueTime: i.dueTime,
        estimateMin: i.estimateMin,
        projectId: i.projectId,
        areaId: i.areaId,
        parentId: i.parentId,
        milestoneId: i.milestoneId,
        order: Math.max(0, ...store.tasks.map((x) => x.order)) + 1,
        completedAt: null,
        source: "owner",
        deleted: false,
      };
      store.tasks.push(t);
      record("owner", "created", "task", t.id, t.title);
      done("task", t.id);
      return t.id;
    },
    update_task: ({ id, input }) => {
      const t = live().find((x) => x.id === id) ?? fail("id", "unknown");
      const i = taskInput(input as TaskInput, t.parentId !== null);
      Object.assign(t, {
        title: i.title,
        notes: i.notes,
        priority: i.priority,
        doDate: i.doDate,
        dueDate: i.dueDate,
        dueTime: i.dueTime,
        estimateMin: i.estimateMin,
        projectId: i.projectId,
        areaId: i.areaId,
        milestoneId: i.milestoneId,
      });
      record("owner", "updated", "task", t.id, t.title);
      done("task", t.id);
    },
    complete_task: ({ id }) => setDone(id as string, true),
    reopen_task: ({ id }) => setDone(id as string, false),
    delete_task: ({ id }) => {
      for (const t of store.tasks.filter((x) => x.id === id || x.parentId === id)) t.deleted = true;
      done("task", id as string);
    },
    restore_task: ({ id }) => {
      for (const t of store.tasks.filter((x) => x.id === id || x.parentId === id)) t.deleted = false;
      done("task", id as string);
    },
    reorder_task: ({ id, before, after }) => {
      const t = live().find((x) => x.id === id) ?? fail("id", "unknown");
      const low = live().find((x) => x.id === before)?.order;
      const high = live().find((x) => x.id === after)?.order;
      t.order =
        low !== undefined && high !== undefined
          ? (low + high) / 2
          : low !== undefined
            ? low + 0.5
            : high !== undefined
              ? high - 0.5
              : t.order;
      done("task", t.id);
    },
    set_task_do_date: ({ id, date }) => {
      const t = live().find((x) => x.id === id) ?? fail("id", "unknown");
      t.doDate = date as string | null;
      done("task", t.id);
    },
    set_focus: ({ date, taskIds }) => {
      const ids = taskIds as string[];
      if (ids.length > 3) fail("taskIds", "too_many");
      store.focus.set(date as string, ids);
      done("focus");
    },
    list_projects: () => store.projects.filter((p) => !p.deleted).map(summary),
    get_project: ({ id }): ProjectDetail => {
      const p = project(id as string) ?? fail("id", "unknown");
      return {
        summary: summary(p),
        milestones: store.milestones.filter((m) => m.projectId === p.id).map(milestoneView),
        tasks: [
          ...live()
            .filter((t) => t.projectId === p.id && t.parentId === null && open(t))
            .sort((a, b) => a.order - b.order),
          ...live().filter((t) => t.projectId === p.id && t.parentId === null && t.status === "done"),
        ].map(card),
        // Like the core: what happened to the project, its milestones, and its tasks.
        activity: store.activity
          .filter((item) => {
            const owned = [
              p.id,
              ...store.tasks.filter((t) => t.projectId === p.id).map((t) => t.id),
              ...store.milestones.filter((m) => m.projectId === p.id).map((m) => m.id),
            ];
            return item.entityId !== null && owned.includes(item.entityId);
          })
          .slice(0, 10),
        files: library.projectFiles(p.id),
      };
    },
    create_project: ({ input }) => {
      const i = projectInput(input as ProjectInput);
      const p: ProjectRec = {
        id: newId(),
        name: i.name,
        description: i.description,
        areaId: i.areaId,
        pigment: i.pigment,
        status: "active",
        targetDate: i.targetDate,
        source: "owner",
        deleted: false,
      };
      store.projects.push(p);
      record("owner", "created", "project", p.id, p.name);
      done("project", p.id);
      return p.id;
    },
    update_project: ({ id, input }) => {
      const p = project(id as string) ?? fail("id", "unknown");
      const i = projectInput(input as ProjectInput);
      Object.assign(p, { name: i.name, description: i.description, areaId: i.areaId, pigment: i.pigment, targetDate: i.targetDate });
      done("project", p.id);
    },
    set_project_status: ({ id, status }) => {
      const p = project(id as string) ?? fail("id", "unknown");
      p.status = status as ProjectStatus;
      done("project", p.id);
    },
    delete_project: ({ id }) => {
      const p = project(id as string) ?? fail("id", "unknown");
      p.deleted = true;
      for (const t of store.tasks.filter((x) => x.projectId === p.id)) Object.assign(t, { projectId: null, milestoneId: null });
      done("project");
    },
    add_milestone: ({ projectId, title, dueDate }) => {
      const m: MilestoneRec = {
        id: newId(),
        projectId: projectId as string,
        title: (title as string).trim(),
        dueDate: dueDate as string | null,
        ticked: false,
      };
      if (!m.title) fail("title", "length");
      store.milestones.push(m);
      done("project", m.projectId);
      return m.id;
    },
    update_milestone: ({ id, title, dueDate }) => {
      const m = store.milestones.find((x) => x.id === id) ?? fail("id", "unknown");
      Object.assign(m, { title, dueDate });
      done("project", m.projectId);
    },
    set_milestone_done: ({ id, done: isDone }) => {
      const m = store.milestones.find((x) => x.id === id) ?? fail("id", "unknown");
      if (milestoneTasks(m).length > 0) fail("done", "derived");
      m.ticked = isDone as boolean;
      done("project", m.projectId);
    },
    delete_milestone: ({ id }) => {
      const index = store.milestones.findIndex((x) => x.id === id);
      const [m] = store.milestones.splice(index, 1);
      for (const t of store.tasks.filter((x) => x.milestoneId === m?.id)) t.milestoneId = null;
      done("project", m?.projectId);
    },
    list_areas: () =>
      store.areas.map((a) => ({
        ...a,
        openProjects: store.projects.filter((p) => !p.deleted && p.areaId === a.id && p.status === "active").length,
      })),
    create_area: ({ input }) => {
      const a: Area = {
        id: newId(),
        name: (input as { name: string }).name.trim(),
        pigment: (input as { pigment: Pigment }).pigment,
        openProjects: 0,
      };
      if (!a.name) fail("name", "length");
      store.areas.push(a);
      done("area", a.id);
      return a.id;
    },
    update_area: ({ id, input }) => {
      const a = area(id as string) ?? fail("id", "unknown");
      Object.assign(a, input);
      done("area", a.id);
    },
    delete_area: ({ id }) => {
      store.areas = store.areas.filter((a) => a.id !== id);
      for (const p of store.projects.filter((x) => x.areaId === id)) p.areaId = null;
      done("area");
    },
    get_calendar: ({ from, days }) => Array.from({ length: days as number }, (_, n) => calendarDay(addDays(from as string, n))),
    get_event: ({ id: eventId }): EventInput => {
      const e = store.events.find((x) => x.id === eventId && !x.deleted) ?? fail("id", "unknown");
      return {
        title: e.title,
        location: e.location,
        notes: e.notes,
        allDay: e.allDay,
        date: e.date,
        endDate: e.allDay ? e.endDate : null,
        startMinute: e.startMinute,
        endMinute: e.endMinute,
        pigment: e.pigment,
        projectId: e.projectId,
        taskId: e.taskId,
      };
    },
    create_event: ({ input }) => {
      const i = input as EventInput;
      if (!i.title.trim()) fail("title", "length");
      if (!i.allDay && i.endMinute <= i.startMinute) fail("endMinute", "order");
      const e: EventRec = {
        id: newId(),
        title: i.title.trim(),
        location: i.location,
        notes: i.notes,
        allDay: i.allDay,
        date: i.date,
        endDate: i.endDate ?? i.date,
        startMinute: i.startMinute,
        endMinute: i.endMinute,
        pigment: i.pigment,
        projectId: i.projectId,
        taskId: i.taskId,
        source: "owner",
        deleted: false,
      };
      store.events.push(e);
      record("owner", "created", "event", e.id, e.title);
      done("event", e.id);
      return e.id;
    },
    update_event: ({ id, input }) => {
      const e = store.events.find((x) => x.id === id && !x.deleted) ?? fail("id", "unknown");
      const i = input as EventInput;
      Object.assign(e, {
        title: i.title,
        location: i.location,
        notes: i.notes,
        allDay: i.allDay,
        date: i.date,
        endDate: i.endDate ?? i.date,
        startMinute: i.startMinute,
        endMinute: i.endMinute,
        pigment: i.pigment,
        projectId: i.projectId,
        taskId: i.taskId,
      });
      done("event", e.id);
    },
    reschedule_event: ({ id, slot }) => {
      const e = store.events.find((x) => x.id === id && !x.deleted) ?? fail("id", "unknown");
      const s = slot as Slot;
      if (s.endMinute <= s.startMinute) fail("endMinute", "order");
      Object.assign(e, { date: s.date, endDate: s.date, startMinute: s.startMinute, endMinute: s.endMinute });
      record("owner", "moved", "event", e.id, e.title);
      done("event", e.id);
    },
    delete_event: ({ id }) => {
      const e = store.events.find((x) => x.id === id) ?? fail("id", "unknown");
      e.deleted = true;
      done("event", e.id);
    },
    restore_event: ({ id }) => {
      const e = store.events.find((x) => x.id === id) ?? fail("id", "unknown");
      e.deleted = false;
      done("event", e.id);
    },
    list_routines: () => store.routines,
    create_routine: ({ input }) => {
      const i = input as RoutineInput;
      if (!i.title.trim()) fail("title", "length");
      const r: Routine = {
        id: newId(),
        title: i.title.trim(),
        kind: i.kind,
        rule: i.rule,
        startTime: i.startTime,
        durationMin: i.durationMin,
        startsOn: i.startsOn,
        endsOn: i.endsOn,
        location: i.location,
        pigment: i.pigment,
        projectId: i.projectId,
        source: "owner",
      };
      store.routines.push(r);
      done("routine", r.id);
      return r.id;
    },
    update_routine: ({ id, input }) => {
      const r = store.routines.find((x) => x.id === id) ?? fail("id", "unknown");
      Object.assign(r, input);
      done("routine", r.id);
    },
    delete_routine: ({ id }) => {
      store.routines = store.routines.filter((r) => r.id !== id);
      done("routine");
    },
    skip_occurrence: ({ routineId, date }) => {
      store.exceptions.set(`${routineId as string}:${date as string}`, { kind: "skipped" });
      done("routine", routineId as string);
    },
    move_occurrence: ({ routineId, date, slot }) => {
      store.exceptions.set(`${routineId as string}:${date as string}`, { kind: "moved", slot: slot as Slot });
      done("routine", routineId as string);
    },
    restore_occurrence: ({ routineId, date }) => {
      store.exceptions.delete(`${routineId as string}:${date as string}`);
      done("routine", routineId as string);
    },
    list_reminders: () => store.reminders.map(reminderItem),
    create_reminder: ({ input }) => {
      const i = input as ReminderInput;
      if (!i.text.trim()) fail("text", "length");
      const r: ReminderRec = {
        id: newId(),
        text: i.text.trim(),
        date: i.date,
        minute: i.minute,
        state: "scheduled",
        firedAt: null,
        taskId: i.taskId,
        source: "owner",
      };
      store.reminders.push(r);
      done("reminder", r.id);
      return r.id;
    },
    update_reminder: ({ id, input }) => {
      const r = store.reminders.find((x) => x.id === id) ?? fail("id", "unknown");
      const i = input as ReminderInput;
      Object.assign(r, { text: i.text, date: i.date, minute: i.minute, state: "scheduled" });
      done("reminder", r.id);
    },
    snooze_reminder: ({ id, minutes }) => {
      const r = store.reminders.find((x) => x.id === id) ?? fail("id", "unknown");
      const at = minuteNow() + (minutes as number);
      Object.assign(r, { state: "snoozed", date: at >= 1440 ? addDays(today(), 1) : today(), minute: at % 1440 });
      done("reminder", r.id);
    },
    done_reminder: ({ id }) => {
      const r = store.reminders.find((x) => x.id === id) ?? fail("id", "unknown");
      r.state = "done";
      done("reminder", r.id);
    },
    cancel_reminder: ({ id }) => {
      const r = store.reminders.find((x) => x.id === id) ?? fail("id", "unknown");
      r.state = "cancelled";
      done("reminder", r.id);
    },
    read_capture: ({ text, kind }) => readCapture(text as string, kind as CaptureKind),
    save_capture: ({ text, kind }) => {
      const reading = readCapture(text as string, kind as CaptureKind);
      if (reading.missing) fail(reading.missing, "format");
      let id: string;
      if (reading.kind === "task") {
        id = handlers.create_task?.({
          input: {
            title: reading.title,
            notes: "",
            priority: reading.priority,
            doDate: reading.due ? null : reading.date,
            dueDate: reading.due ? reading.date : null,
            dueTime: reading.due ? reading.time : null,
            estimateMin: null,
            projectId: reading.project?.id ?? null,
            areaId: null,
            parentId: null,
          },
        }) as string;
      } else if (reading.kind === "reminder") {
        id = handlers.create_reminder?.({
          input: { text: reading.title, date: reading.date, minute: parseClock(reading.time ?? "00:00") ?? 0, taskId: null },
        }) as string;
      } else if (reading.kind === "event") {
        const start = reading.time ? (parseClock(reading.time) ?? 0) : 0;
        id = handlers.create_event?.({
          input: {
            title: reading.title,
            location: "",
            notes: "",
            allDay: !reading.time,
            date: reading.date,
            endDate: null,
            startMinute: start,
            endMinute: start + 60,
            pigment: "batu",
            projectId: null,
            taskId: null,
          },
        }) as string;
      } else {
        id = newId();
        record("owner", "created", "journal", id, reading.title);
        done("journal", id);
      }
      return { kind: reading.kind, id, title: reading.title, date: reading.date };
    },
    get_review: ({ date }): DayReview => {
      const d = (date as string | null) ?? today();
      const focusIds = store.focus.get(d) ?? [];
      const slipped = live()
        .filter((t) => t.parentId === null && open(t) && (focusIds.includes(t.id) || (t.doDate !== null && t.doDate <= d)))
        .sort((a, b) => Number(!focusIds.includes(a.id)) - Number(!focusIds.includes(b.id)) || a.order - b.order)
        .map(card);
      const now = minuteNow();
      return {
        date: d,
        tomorrow: addDays(d, 1),
        closed: store.closed.has(d),
        done: [
          ...live()
            .filter((t) => t.parentId === null && doneToday(t))
            .map((t) => ({ title: t.title, kind: "task" as const })),
          ...calendarDay(d)
            .items.filter((i) => !i.allDay && i.kind !== "block" && i.endMinute <= now)
            .map((i) => ({ title: i.title, kind: i.kind === "routine" ? ("routine" as const) : ("event" as const) })),
        ],
        slipped,
        suggestions: suggest(addDays(d, 1)),
        tomorrowFocus: focusOf(addDays(d, 1)),
      };
    },
    close_day: ({ input }) => {
      const i = input as CloseInput;
      if (store.closed.has(i.date)) fail("date", "order");
      for (const slip of i.slips) {
        const t = live().find((x) => x.id === slip.taskId) ?? fail("taskId", "unknown");
        if (slip.destination.to === "tomorrow") t.doDate = addDays(i.date, 1);
        else if (slip.destination.to === "date") t.doDate = slip.destination.date;
        else if (slip.destination.to === "let_go") t.doDate = null;
      }
      if (i.tomorrowFocus.length) store.focus.set(addDays(i.date, 1), i.tomorrowFocus);
      store.closed.add(i.date);
      record("owner", "closed", "focus", null, i.date);
      done("focus");
    },
  };

  function setDone(id: string, isDone: boolean) {
    const t = live().find((x) => x.id === id);
    if (!t) throw new HubError({ kind: "not_found", field: "task", reason: null, message: "task not found" });
    if ((t.status === "done") === isDone) return;
    t.status = isDone ? "done" : "todo";
    t.completedAt = isDone ? new Date().toISOString() : null;
    record("owner", isDone ? "completed" : "reopened", "task", t.id, t.title);
    done("task", t.id);
  }

  return {
    calls,
    call: <T>(command: Command, args: Record<string, unknown> = {}): Promise<T> => {
      calls.push(Object.keys(args).length ? `${command} ${JSON.stringify(args)}` : command);
      const handler = handlers[command];
      if (!handler) return Promise.reject(new HubError({ kind: "internal", field: null, reason: null, message: `mock: ${command}` }));
      try {
        return Promise.resolve(structuredClone(handler(args)) as T);
      } catch (error) {
        return Promise.reject(error instanceof Error ? error : new Error(String(error)));
      }
    },
    stream: async <T>(command: Command, args: Record<string, unknown>, onEvent: (event: unknown) => void): Promise<T> => {
      calls.push(`${command} ${JSON.stringify(args)}`);
      const streams = { ...studio.streams, ...lentera.streams };
      const handler = (streams as Partial<Record<Command, (args: Record<string, unknown>, onEvent: (event: never) => void) => Promise<unknown>>>)[command];
      if (!handler) throw new HubError({ kind: "internal", field: null, reason: null, message: `mock: ${command}` });
      return (await handler(args, (event) => onEvent(structuredClone(event)))) as T;
    },
    onChanged: (listener) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    on: (event, listener) => {
      const set = others.get(event) ?? new Set();
      set.add(listener);
      others.set(event, set);
      return () => set.delete(listener);
    },
    emit,
    setClaudeSignals: (project: ClaudeSignals, all?: ClaudeSignals) => {
      reported.project = project;
      reported.all = all ?? project;
      emit({ entity: "claude_session", ids: [] });
    },
    dropFiles: (names, x, y) => library.dropFiles(names, x, y),
    send: (event, payload) => {
      for (const listener of others.get(event) ?? []) listener(payload);
    },
    addTask: (title, actor, extra = {}) => {
      const t: TaskRec = {
        id: newId(),
        title,
        notes: "",
        status: "todo",
        priority: 0,
        doDate: today(),
        dueDate: null,
        dueTime: null,
        estimateMin: null,
        projectId: null,
        areaId: null,
        parentId: null,
        milestoneId: null,
        order: Math.max(0, ...store.tasks.map((x) => x.order)) + 1,
        completedAt: null,
        source: actor === "ultron" ? "ultron" : "owner",
        deleted: false,
        ...extra,
      };
      store.tasks.push(t);
      record(actor, "created", "task", t.id, t.title);
      return t.id;
    },
    fireDue: () => {
      const d = today();
      const now = minuteNow();
      for (const r of store.reminders) {
        if ((r.state === "scheduled" || r.state === "snoozed") && (r.date < d || (r.date === d && r.minute <= now))) {
          r.state = "fired";
          r.firedAt = new Date().toISOString();
        }
      }
      done("reminder");
    },
  };
}
