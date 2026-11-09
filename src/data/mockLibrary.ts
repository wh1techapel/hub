// The Library, Belajar, Jejak, and search for the in-memory backend (mock.ts). Like the rest of
// the mock, it follows the core's rules the UI can see in a simpler way: files are kept by name,
// thumbnails are drawn here, and search is a plain "contains".

import { HubError } from "./backend";
import type { Command } from "./backend";
import type { Drag, LearningDetail, Opened, Thumb } from "./api";
import type { ActivityItem } from "./generated/ActivityItem";
import type { Actor } from "./generated/Actor";
import type { Added } from "./generated/Added";
import type { Collection } from "./generated/Collection";
import type { CollectionInput } from "./generated/CollectionInput";
import type { CtfEvent } from "./generated/CtfEvent";
import type { CtfInput } from "./generated/CtfInput";
import type { DayMinutes } from "./generated/DayMinutes";
import type { Domain } from "./generated/Domain";
import type { DomainStat } from "./generated/DomainStat";
import type { Entity } from "./generated/Entity";
import type { FileCard } from "./generated/FileCard";
import type { FileCounts } from "./generated/FileCounts";
import type { FileEdit } from "./generated/FileEdit";
import type { FileKind } from "./generated/FileKind";
import type { FileQuery } from "./generated/FileQuery";
import type { LearningCard } from "./generated/LearningCard";
import type { LearningGlance } from "./generated/LearningGlance";
import type { LearningInput } from "./generated/LearningInput";
import type { LearningOverview } from "./generated/LearningOverview";
import type { LearningQuery } from "./generated/LearningQuery";
import type { LearningStatus } from "./generated/LearningStatus";
import type { LinkKind } from "./generated/LinkKind";
import type { LinkRef } from "./generated/LinkRef";
import type { LinkTarget } from "./generated/LinkTarget";
import type { Measure } from "./generated/Measure";
import type { Pigment } from "./generated/Pigment";
import type { SearchHit } from "./generated/SearchHit";
import type { Session } from "./generated/Session";
import type { SessionInput } from "./generated/SessionInput";
import type { Timer } from "./generated/Timer";
import type { Track } from "./generated/Track";
import type { TrackInput } from "./generated/TrackInput";
import type { TrailGroup } from "./generated/TrailGroup";
import type { TrailPage } from "./generated/TrailPage";
import type { TrailQuery } from "./generated/TrailQuery";
import type { Verb } from "./generated/Verb";
import type { WeekInsight } from "./generated/WeekInsight";

type Handler = (args: Record<string, unknown>) => unknown;

/** What the library needs from the rest of the mock. */
export interface LibraryDeps {
  newId: () => string;
  today: () => string;
  record: (actor: Actor, verb: Verb, entity: Entity, entityId: string | null, summary: string) => void;
  done: (entity: Entity, id?: string) => void;
  emitDrag: (drag: Drag) => void;
  linkTitle: (kind: LinkKind, id: string) => { title: string; pigment: Pigment | null } | null;
  projectTaskIds: (projectId: string) => string[];
  findProject: (name: string) => string | null;
  findTask: (title: string) => string | null;
  activity: () => ActivityItem[];
  /** Hits among tasks, projects, and events (the rest of the mock's store). */
  searchCore: (words: string[]) => SearchHit[];
  /** Tasks done between two instants, and projects that moved. */
  doneBetween: (from: string, to: string) => { tasks: number; projects: number };
  goalDays: () => number;
}

interface FileRec {
  id: string;
  name: string;
  kind: FileKind;
  extension: string;
  sizeBytes: number | null;
  path: string;
  state: FileCard["state"];
  vaulted: boolean;
  note: string;
  addedAt: string;
  deleted: boolean;
}

interface ItemRec {
  id: string;
  trackId: string;
  ctfEventId: string | null;
  title: string;
  kind: LearningCard["kind"];
  domain: Domain;
  difficulty: LearningCard["difficulty"];
  status: LearningStatus;
  url: string | null;
  points: number | null;
  progress: number | null;
  notes: string;
  startedAt: string | null;
  completedAt: string | null;
  deleted: boolean;
}

interface TrackRec extends Omit<Track, "items" | "done"> {
  deleted: boolean;
}

interface SessionRec {
  id: string;
  itemId: string;
  startedAt: string;
  durationMin: number;
  note: string;
}

const KINDS: [FileKind, string[]][] = [
  ["document", ["pdf", "doc", "docx", "txt", "md", "ppt", "pptx", "xls", "xlsx", "csv"]],
  ["image", ["png", "jpg", "jpeg", "gif", "webp", "svg"]],
  ["code", ["rs", "py", "js", "ts", "json", "sh", "ps1", "pcap", "pcapng", "html", "css"]],
  ["archive", ["zip", "rar", "7z", "iso", "ova", "vmdk"]],
  ["media", ["mp3", "mp4", "mkv", "wav"]],
];
const PROGRAMS = ["exe", "msi", "bat", "cmd", "ps1", "lnk", "vbs", "js"];

function kindOf(extension: string, folder: boolean): FileKind {
  if (folder) return "folder";
  return KINDS.find(([, extensions]) => extensions.includes(extension))?.[0] ?? "other";
}

function fail(field: string, reason: string): never {
  throw new HubError({ kind: "invalid", field, reason, message: `invalid ${field}` });
}

function notFound(what: string): never {
  throw new HubError({ kind: "not_found", field: what, reason: null, message: `${what} not found` });
}

const iso = (date: Date) => date.toISOString().replace(/\.\d{3}Z$/, "Z");

function localDay(at: string): string {
  const d = new Date(at);
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

function addDays(date: string, days: number): string {
  const [y, m, d] = date.split("-").map(Number);
  const next = new Date(y ?? 0, (m ?? 1) - 1, (d ?? 1) + days);
  return localDay(next.toISOString());
}

function mondayOf(date: string): string {
  const [y, m, d] = date.split("-").map(Number);
  const day = new Date(y ?? 0, (m ?? 1) - 1, d ?? 1).getDay();
  return addDays(date, -((day + 6) % 7));
}

/** A small drawn picture per kind, so thumbnails show in the browser too. */
function drawn(kind: FileKind, name: string): Thumb {
  const hue = { document: 220, image: 160, code: 280, archive: 40, media: 340, folder: 30, other: 0 }[kind];
  const letter = (name.split(".").pop() ?? "?").slice(0, 4).toUpperCase();
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="96" height="96" viewBox="0 0 96 96"><rect x="18" y="10" width="60" height="76" rx="6" fill="hsl(${hue} 45% 88%)" stroke="hsl(${hue} 35% 55%)" stroke-width="2"/><text x="48" y="58" font-family="sans-serif" font-size="14" font-weight="700" text-anchor="middle" fill="hsl(${hue} 40% 35%)">${letter}</text></svg>`;
  return { image: `data:image/svg+xml;base64,${btoa(svg)}`, kind: kind === "image" ? "thumbnail" : "icon" };
}

export function libraryMock(deps: LibraryDeps, demo: boolean) {
  const files: FileRec[] = [];
  const links: { fileId: string; kind: LinkKind; id: string }[] = [];
  const collections: (Omit<Collection, "files"> & { deleted: boolean })[] = [];
  const memberships: { fileId: string; collectionId: string }[] = [];
  const tracks: TrackRec[] = [];
  const items: ItemRec[] = [];
  const sessions: SessionRec[] = [];
  const ctf: (Omit<CtfEvent, "challenges" | "solved" | "points"> & { deleted: boolean })[] = [];
  let timer: { itemId: string; startedAt: string } | null = null;
  const drops = new Map<number, string[]>();
  let nextDrop = 1;
  // The demo rows are there from the start: they leave no Jejak.
  let seeding = demo;
  const record: LibraryDeps["record"] = (...args) => {
    if (!seeding) deps.record(...args);
  };
  const titleOf = (kind: LinkKind, id: string): { title: string; pigment: Pigment | null } | null => {
    if (kind !== "learning_item") return deps.linkTitle(kind, id);
    const found = items.find((i) => i.id === id && !i.deleted);
    return found ? { title: found.title, pigment: tracks.find((t) => t.id === found.trackId)?.pigment ?? null } : null;
  };

  const live = () => files.filter((f) => !f.deleted);
  const file = (id: string) => live().find((f) => f.id === id) ?? notFound("file");

  function card(f: FileRec): FileCard {
    const refs: LinkRef[] = links
      .filter((l) => l.fileId === f.id)
      .flatMap((l) => {
        const found = titleOf(l.kind, l.id);
        return found ? [{ kind: l.kind, id: l.id, title: found.title, pigment: found.pigment }] : [];
      });
    const kept = memberships
      .filter((m) => m.fileId === f.id)
      .flatMap((m) => collections.filter((c) => c.id === m.collectionId && !c.deleted))
      .map(({ id, name, pigment }) => ({ id, name, pigment }));
    return {
      id: f.id,
      name: f.name,
      kind: f.kind,
      extension: f.extension,
      sizeBytes: f.sizeBytes,
      path: f.path,
      state: f.state,
      vaulted: f.vaulted,
      program: PROGRAMS.includes(f.extension),
      note: f.note,
      addedAt: f.addedAt,
      source: "owner",
      links: refs,
      collections: kept,
    };
  }

  function add(path: string, target: LinkTarget | null, removable = false): Added {
    const name = path.split(/[\\/]/).pop() ?? path;
    const folder = !name.includes(".");
    const extension = folder ? "" : (name.split(".").pop() ?? "").toLowerCase();
    const existing = files.find((f) => f.path.toLowerCase() === path.toLowerCase());
    let id: string;
    let existed = false;
    if (existing) {
      id = existing.id;
      existed = !existing.deleted;
      existing.deleted = false;
    } else {
      id = deps.newId();
      files.push({
        id,
        name,
        kind: kindOf(extension, folder),
        extension,
        sizeBytes: folder ? null : 2048 + name.length * 997,
        path,
        state: "ok",
        vaulted: false,
        note: "",
        addedAt: iso(new Date()),
        deleted: false,
      });
      record("owner", "created", "file", id, name);
    }
    if (target) attach(id, target);
    deps.done("file", id);
    return { id, name, existed, wantsCopy: removable && !folder };
  }

  function attach(id: string, target: LinkTarget) {
    if (!titleOf(target.kind, target.id)) fail("target", "unknown");
    if (links.some((l) => l.fileId === id && l.kind === target.kind && l.id === target.id)) return;
    links.push({ fileId: id, kind: target.kind, id: target.id });
    record("owner", "attached", "file", id, file(id).name);
  }

  function list(query: FileQuery): FileCard[] {
    const projectTasks = query.projectId ? deps.projectTaskIds(query.projectId) : [];
    return live()
      .filter((f) => !query.kind || f.kind === query.kind)
      .filter((f) => !query.state || f.state === query.state)
      .filter((f) => !query.collectionId || memberships.some((m) => m.fileId === f.id && m.collectionId === query.collectionId))
      .filter(
        (f) =>
          !query.projectId ||
          links.some(
            (l) =>
              l.fileId === f.id &&
              ((l.kind === "project" && l.id === query.projectId) || (l.kind === "task" && projectTasks.includes(l.id))),
          ),
      )
      .filter((f) => !query.target || links.some((l) => l.fileId === f.id && l.kind === query.target?.kind && l.id === query.target.id))
      .sort((a, b) => b.addedAt.localeCompare(a.addedAt) || b.id.localeCompare(a.id))
      .slice(0, query.limit ?? 500)
      .map(card);
  }

  // ---------- Belajar ----------

  const liveItems = () => items.filter((i) => !i.deleted && tracks.some((t) => t.id === i.trackId && !t.deleted));
  const item = (id: string) => liveItems().find((i) => i.id === id) ?? notFound("learning item");

  function learningCard(i: ItemRec): LearningCard {
    const track = tracks.find((t) => t.id === i.trackId);
    const own = sessions.filter((s) => s.itemId === i.id);
    return {
      id: i.id,
      trackId: i.trackId,
      trackName: track?.name ?? "",
      pigment: track?.pigment ?? "batu",
      ctfEventId: i.ctfEventId,
      title: i.title,
      kind: i.kind,
      domain: i.domain,
      difficulty: i.difficulty,
      status: i.status,
      url: i.url,
      points: i.points,
      progress: i.progress,
      notes: i.notes,
      minutes: own.reduce((sum, s) => sum + s.durationMin, 0),
      sessions: own.length,
      lastSessionAt:
        own
          .map((s) => s.startedAt)
          .sort()
          .pop() ?? null,
      startedAt: i.startedAt,
      completedAt: i.completedAt,
      files: links.filter((l) => l.kind === "learning_item" && l.id === i.id && live().some((f) => f.id === l.fileId)).length,
      source: "owner",
    };
  }

  function setStatus(i: ItemRec, status: LearningStatus) {
    const now = iso(new Date());
    if (status === "doing" || status === "done") i.startedAt ??= now;
    i.completedAt = status === "done" ? (i.completedAt ?? now) : null;
    if (status === "done") i.progress = 100;
    i.status = status;
  }

  function stop(): number | null {
    if (!timer) return null;
    const minutes = Math.min(480, Math.max(1, Math.ceil((Date.now() - new Date(timer.startedAt).getTime()) / 60_000)));
    sessions.push({ id: deps.newId(), itemId: timer.itemId, startedAt: timer.startedAt, durationMin: minutes, note: "" });
    deps.record("owner", "created", "learning_session", timer.itemId, item(timer.itemId).title);
    timer = null;
    return minutes;
  }

  function timerView(): Timer | null {
    if (!timer) return null;
    const i = items.find((x) => x.id === timer?.itemId);
    const track = tracks.find((t) => t.id === i?.trackId);
    return {
      itemId: timer.itemId,
      itemTitle: i?.title ?? "",
      trackName: track?.name ?? "",
      pigment: track?.pigment ?? "batu",
      startedAt: timer.startedAt,
    };
  }

  function byDay(from: string): Map<string, number> {
    const days = new Map<string, number>();
    for (const s of sessions) {
      if (!liveItems().some((i) => i.id === s.itemId)) continue;
      const day = localDay(s.startedAt);
      if (day >= from) days.set(day, (days.get(day) ?? 0) + s.durationMin);
    }
    return days;
  }

  function overview(): LearningOverview {
    const monday = mondayOf(deps.today());
    const first = addDays(monday, -7 * 19);
    const minutes = byDay(first);
    const span = (from: string, count: number): DayMinutes[] =>
      Array.from({ length: count }, (_, n) => {
        const date = addDays(from, n);
        return { date, minutes: minutes.get(date) ?? 0 };
      });
    const days = span(monday, 7);
    const domains = new Map<Domain, DomainStat>();
    for (const i of liveItems().filter((x) => x.status !== "dropped")) {
      const stat = domains.get(i.domain) ?? { domain: i.domain, minutes: 0, done: 0, items: 0 };
      stat.items += 1;
      stat.done += i.status === "done" ? 1 : 0;
      stat.minutes += learningCard(i).minutes;
      domains.set(i.domain, stat);
    }
    const order: Domain[] = ["network", "web", "linux", "windows", "crypto", "forensics", "osint", "reversing", "pwn", "other"];
    const doing = liveItems()
      .filter((i) => i.status === "doing")
      .sort((a, b) => (learningCard(b).lastSessionAt ?? "").localeCompare(learningCard(a).lastSessionAt ?? ""));
    const currentId = timer?.itemId ?? doing[0]?.id ?? null;
    return {
      weekStart: monday,
      goalDays: deps.goalDays(),
      activeDays: days.filter((d) => d.minutes > 0).length,
      minutesWeek: days.reduce((sum, d) => sum + d.minutes, 0),
      days,
      heatmap: span(first, 140),
      domains: order.flatMap((d) => domains.get(d) ?? []),
      current: currentId ? learningCard(item(currentId)) : null,
      timer: timerView(),
    };
  }

  function checkLearning(input: LearningInput) {
    if (!tracks.some((t) => t.id === input.trackId && !t.deleted)) fail("trackId", "unknown");
    if (!input.title.trim()) fail("title", "length");
    if (input.url && !/^https?:\/\//i.test(input.url)) fail("url", "format");
  }

  // ---------- Jejak ----------

  const groups: Record<TrailGroup, Entity[]> = {
    tasks: ["task", "focus"],
    projects: ["project", "milestone", "area"],
    calendar: ["event", "routine", "reminder"],
    files: ["file", "collection"],
    learning: ["track", "learning_item", "learning_session", "ctf_event"],
    notes: ["journal"],
    studio: ["studio_project", "studio_ledger", "studio_stack", "studio_prd", "studio_roadmap", "studio_checkpoint", "studio_task", "studio_report", "studio_request"],
  };

  function trail(query: TrailQuery): TrailPage {
    const limit = query.limit ?? 50;
    const rows = deps
      .activity()
      .filter((a) => a.entity !== "settings")
      .filter((a) => !query.actor || a.actor === query.actor)
      .filter((a) => !query.group || groups[query.group].includes(a.entity))
      .sort((a, b) => b.at.localeCompare(a.at) || b.id.localeCompare(a.id));
    const start = query.before ? rows.findIndex((a) => `${a.at}|${a.id}` === query.before) + 1 : 0;
    const page = rows.slice(start, start + limit);
    const last = page[page.length - 1];
    return { items: page, next: start + limit < rows.length && last ? `${last.at}|${last.id}` : null };
  }

  function week(): WeekInsight {
    const monday = mondayOf(deps.today());
    const weeks = Array.from({ length: 8 }, (_, n) => addDays(monday, -7 * (7 - n)));
    const measure = (values: number[]): Measure => {
      const before = values.slice(3, 7);
      const average = Math.round((before.reduce((a, b) => a + b, 0) / before.length) * 10) / 10;
      return { value: values[7] ?? 0, average, weeks: values };
    };
    const minutes = byDay(weeks[0] ?? monday);
    const perWeek = weeks.map((start) => {
      const end = addDays(start, 7);
      const from = new Date(`${start}T00:00`).toISOString();
      const to = new Date(`${end}T00:00`).toISOString();
      const days = Array.from({ length: 7 }, (_, n) => minutes.get(addDays(start, n)) ?? 0);
      const done = deps.doneBetween(from, to);
      return {
        tasks: done.tasks,
        projects: done.projects,
        minutes: days.reduce((a, b) => a + b, 0),
        active: days.filter((m) => m > 0).length,
        files: files.filter((f) => !f.deleted && f.addedAt >= from && f.addedAt < to).length,
      };
    });
    return {
      weekStart: monday,
      tasksDone: measure(perWeek.map((w) => w.tasks)),
      learningMinutes: measure(perWeek.map((w) => w.minutes)),
      activeDays: measure(perWeek.map((w) => w.active)),
      filesAdded: measure(perWeek.map((w) => w.files)),
      projectsMoved: measure(perWeek.map((w) => w.projects)),
    };
  }

  // ---------- demo ----------

  if (demo) {
    const today = deps.today();
    const track = (name: string, kind: Track["kind"], pigment: Pigment, url: string | null = null) => {
      const id = deps.newId();
      tracks.push({ id, name, kind, url, pigment, deleted: false });
      return id;
    };
    const thm = track("TryHackMe", "platform", "lumut", "https://tryhackme.com");
    const htb = track("Hack The Box", "platform", "toska", "https://app.hackthebox.com");
    const ctfTrack = track("CTF", "ctf", "terakota");
    const school = track("Lab Sekolah", "lab", "lazuardi");
    track("Homelab", "lab", "kunyit");
    const lks = deps.newId();
    ctf.push({
      id: lks,
      name: "LKS Cyber Security Kabupaten",
      startsOn: addDays(today, -12),
      endsOn: addDays(today, -12),
      team: null,
      rank: 3,
      participants: 24,
      url: null,
      notes: "",
      deleted: false,
    });
    const learn = (
      trackId: string,
      title: string,
      kind: ItemRec["kind"],
      domain: Domain,
      status: LearningStatus,
      extra: Partial<ItemRec> = {},
    ) => {
      const id = deps.newId();
      items.push({
        id,
        trackId,
        ctfEventId: null,
        title,
        kind,
        domain,
        difficulty: "easy",
        status,
        url: null,
        points: null,
        progress: status === "done" ? 100 : null,
        notes: "",
        startedAt: status === "todo" ? null : iso(new Date()),
        completedAt: status === "done" ? iso(new Date()) : null,
        deleted: false,
        ...extra,
      });
      return id;
    };
    const nmap = learn(thm, "Nmap Live Host Discovery", "room", "network", "doing", {
      url: "https://tryhackme.com/room/nmap01",
      progress: 60,
    });
    learn(thm, "Linux Fundamentals Part 1", "room", "linux", "done");
    learn(thm, "OWASP Top 10 - 2021", "room", "web", "todo");
    const lame = learn(htb, "Lame", "machine", "linux", "done");
    const vlan = learn(school, "Praktikum VLAN dan Trunking", "practical", "network", "doing");
    learn(ctfTrack, "baby-sqli", "challenge", "web", "done", { ctfEventId: lks, points: 100 });
    learn(ctfTrack, "pcap-hunt", "challenge", "forensics", "done", { ctfEventId: lks, points: 200 });
    learn(ctfTrack, "rev-101", "challenge", "reversing", "todo", { ctfEventId: lks, points: 300 });
    for (let back = 1; back <= 42; back++) {
      if (back % 7 === 3 || back % 5 === 0) continue;
      const itemId = back % 3 === 0 ? vlan : back > 20 ? lame : nmap;
      sessions.push({
        id: deps.newId(),
        itemId,
        startedAt: new Date(`${addDays(today, -back)}T19:00`).toISOString(),
        durationMin: 25 + ((back * 7) % 70),
        note: "",
      });
    }

    const folder = "C:\\Users\\Glenvio\\Documents\\Sekolah";
    const kept = [
      "Laporan Praktikum 5 - VLAN.docx",
      "topologi-vlan.png",
      "nmap-cheatsheet.pdf",
      "writeup-lame.md",
      "Ubuntu 24.04 lab.ova",
    ].map((name, n) => {
      const added = add(`${folder}\\${name}`, null);
      const rec = files.find((f) => f.id === added.id);
      if (rec) rec.addedAt = iso(new Date(Date.now() - (5 - n) * 3_600_000));
      return added.id;
    });
    const report = deps.findTask("Laporan Praktikum 5: VLAN dan Trunking");
    const lab = deps.findProject("Laporan Lab Jaringan");
    if (report) links.push({ fileId: kept[0] ?? "", kind: "task", id: report });
    if (lab) links.push({ fileId: kept[1] ?? "", kind: "project", id: lab });
    links.push({ fileId: kept[2] ?? "", kind: "learning_item", id: nmap }, { fileId: kept[3] ?? "", kind: "learning_item", id: lame });
    const sheets = deps.newId();
    const writeups = deps.newId();
    collections.push(
      { id: sheets, name: "Cheatsheet", pigment: "lumut", deleted: false },
      { id: writeups, name: "Write-up", pigment: "lavender", deleted: false },
    );
    memberships.push({ fileId: kept[2] ?? "", collectionId: sheets }, { fileId: kept[3] ?? "", collectionId: writeups });
    const lost = files.find((f) => f.id === kept[4]);
    if (lost) lost.state = "missing";
  }
  seeding = false;

  const handlers: Partial<Record<Command, Handler>> = {
    keep_drop: ({ dropId, target }) => {
      const names = drops.get(dropId as number) ?? notFound("drop");
      drops.delete(dropId as number);
      return names.map((name) =>
        add(`C:\\Users\\Glenvio\\Downloads\\${name}`, (target as LinkTarget | null) ?? null, name.startsWith("usb-")),
      );
    },
    pick_files: ({ target }) => [
      add(`C:\\Users\\Glenvio\\Documents\\dipilih-${files.length + 1}.pdf`, (target as LinkTarget | null) ?? null),
    ],
    relocate_file: ({ id }) => {
      const f = file(id as string);
      f.state = "ok";
      f.path = `C:\\Users\\Glenvio\\Documents\\${f.name}`;
      deps.record("owner", "moved", "file", f.id, f.name);
      deps.done("file", f.id);
      return true;
    },
    open_file: ({ id }): Opened => {
      const f = file(id as string);
      if (f.state === "missing" && !f.vaulted) notFound("path");
      return PROGRAMS.includes(f.extension) ? "revealed" : f.state === "missing" ? "vault" : "opened";
    },
    reveal_file: ({ id }) => {
      const f = file(id as string);
      if (f.state === "missing" && !f.vaulted) notFound("path");
      return undefined;
    },
    check_file: () => undefined,
    file_thumbnail: ({ id }) => {
      const f = files.find((x) => x.id === id);
      return f && f.state !== "missing" ? drawn(f.kind, f.name) : null;
    },
    copy_to_vault: ({ id }) => {
      const f = file(id as string);
      f.vaulted = true;
      deps.record("owner", "updated", "file", f.id, f.name);
      deps.done("file", f.id);
    },
    remove_vault_copy: ({ id }) => {
      const f = file(id as string);
      f.vaulted = false;
      deps.record("owner", "updated", "file", f.id, f.name);
      deps.done("file", f.id);
    },
    list_files: ({ query }) => list((query as FileQuery | undefined) ?? {}),
    file_counts: (): FileCounts => {
      const all = live();
      return {
        all: all.length,
        missing: all.filter((f) => f.state === "missing").length,
        kinds: (["document", "image", "code", "archive", "media", "folder", "other"] as FileKind[]).map((kind) => ({
          kind,
          count: all.filter((f) => f.kind === kind).length,
        })),
      };
    },
    get_file: ({ id }) => card(file(id as string)),
    update_file: ({ id, edit }) => {
      const f = file(id as string);
      const e = edit as FileEdit;
      if (!e.name.trim()) fail("name", "length");
      f.name = e.name.trim();
      f.note = e.note.trim();
      deps.record("owner", "updated", "file", f.id, f.name);
      deps.done("file", f.id);
    },
    delete_file: ({ id }) => {
      const f = file(id as string);
      f.deleted = true;
      deps.record("owner", "deleted", "file", f.id, f.name);
      deps.done("file", f.id);
    },
    restore_file: ({ id }) => {
      const f = files.find((x) => x.id === id) ?? notFound("file");
      f.deleted = false;
      deps.record("owner", "restored", "file", f.id, f.name);
      deps.done("file", f.id);
    },
    attach_file: ({ id, target }) => {
      attach(id as string, target as LinkTarget);
      deps.done("file", id as string);
    },
    detach_file: ({ id, target }) => {
      const t = target as LinkTarget;
      const at = links.findIndex((l) => l.fileId === id && l.kind === t.kind && l.id === t.id);
      if (at >= 0) {
        links.splice(at, 1);
        deps.record("owner", "detached", "file", id as string, file(id as string).name);
      }
      deps.done("file", id as string);
    },
    set_file_collections: ({ id, collectionIds }) => {
      const f = file(id as string);
      for (let n = memberships.length - 1; n >= 0; n--) if (memberships[n]?.fileId === f.id) memberships.splice(n, 1);
      for (const collectionId of collectionIds as string[]) {
        if (!collections.some((c) => c.id === collectionId && !c.deleted)) fail("collectionIds", "unknown");
        memberships.push({ fileId: f.id, collectionId });
      }
      deps.record("owner", "updated", "file", f.id, f.name);
      deps.done("file", f.id);
    },
    list_collections: (): Collection[] =>
      collections
        .filter((c) => !c.deleted)
        .map(({ id, name, pigment }) => ({
          id,
          name,
          pigment,
          files: memberships.filter((m) => m.collectionId === id && live().some((f) => f.id === m.fileId)).length,
        })),
    create_collection: ({ input }) => {
      const i = input as CollectionInput;
      if (!i.name.trim()) fail("name", "length");
      const id = deps.newId();
      collections.push({ id, name: i.name.trim(), pigment: i.pigment, deleted: false });
      deps.record("owner", "created", "collection", id, i.name.trim());
      deps.done("collection", id);
      return id;
    },
    update_collection: ({ id, input }) => {
      const c = collections.find((x) => x.id === id && !x.deleted) ?? notFound("collection");
      const i = input as CollectionInput;
      c.name = i.name.trim();
      c.pigment = i.pigment;
      deps.done("collection", c.id);
    },
    delete_collection: ({ id }) => {
      const c = collections.find((x) => x.id === id && !x.deleted) ?? notFound("collection");
      c.deleted = true;
      for (let n = memberships.length - 1; n >= 0; n--) if (memberships[n]?.collectionId === c.id) memberships.splice(n, 1);
      deps.record("owner", "deleted", "collection", c.id, c.name);
      deps.done("collection");
    },
    list_tracks: (): Track[] =>
      tracks
        .filter((t) => !t.deleted)
        .map((t) => ({
          id: t.id,
          name: t.name,
          kind: t.kind,
          url: t.url,
          pigment: t.pigment,
          items: liveItems().filter((i) => i.trackId === t.id).length,
          done: liveItems().filter((i) => i.trackId === t.id && i.status === "done").length,
        })),
    create_track: ({ input }) => {
      const i = input as TrackInput;
      if (!i.name.trim()) fail("name", "length");
      const id = deps.newId();
      tracks.push({ id, name: i.name.trim(), kind: i.kind, url: i.url ?? null, pigment: i.pigment, deleted: false });
      deps.record("owner", "created", "track", id, i.name.trim());
      deps.done("track", id);
      return id;
    },
    update_track: ({ id, input }) => {
      const t = tracks.find((x) => x.id === id && !x.deleted) ?? notFound("track");
      const i = input as TrackInput;
      Object.assign(t, { name: i.name.trim(), kind: i.kind, url: i.url ?? null, pigment: i.pigment });
      deps.done("track", t.id);
    },
    delete_track: ({ id }) => {
      const t = tracks.find((x) => x.id === id && !x.deleted) ?? notFound("track");
      t.deleted = true;
      deps.record("owner", "deleted", "track", t.id, t.name);
      deps.done("track");
    },
    restore_track: ({ id }) => {
      const t = tracks.find((x) => x.id === id) ?? notFound("track");
      t.deleted = false;
      deps.done("track");
    },
    list_learning: ({ query }) => {
      const q = (query as LearningQuery | undefined) ?? {};
      const rank = { doing: 0, todo: 1, done: 2, dropped: 3 };
      return liveItems()
        .filter((i) => !q.trackId || i.trackId === q.trackId)
        .filter((i) => !q.status || i.status === q.status)
        .filter((i) => !q.ctfEventId || i.ctfEventId === q.ctfEventId)
        .sort((a, b) => rank[a.status] - rank[b.status])
        .map(learningCard);
    },
    get_learning: ({ id }): LearningDetail => {
      const i = item(id as string);
      return {
        card: learningCard(i),
        sessions: sessions
          .filter((s) => s.itemId === i.id)
          .sort((a, b) => b.startedAt.localeCompare(a.startedAt))
          .map((s): Session => ({
            id: s.id,
            itemId: s.itemId,
            itemTitle: i.title,
            startedAt: s.startedAt,
            durationMin: s.durationMin,
            note: s.note,
            source: "owner",
          })),
        files: list({ target: { kind: "learning_item", id: i.id } }),
      };
    },
    create_learning: ({ input }) => {
      const i = input as LearningInput;
      checkLearning(i);
      const id = deps.newId();
      const status = i.status ?? "todo";
      items.push({
        id,
        trackId: i.trackId,
        ctfEventId: i.ctfEventId,
        title: i.title.trim(),
        kind: i.kind ?? "room",
        domain: i.domain ?? "other",
        difficulty: i.difficulty,
        status: "todo",
        url: i.url || null,
        points: i.points,
        progress: i.progress,
        notes: i.notes,
        startedAt: null,
        completedAt: null,
        deleted: false,
      });
      const rec = items.find((x) => x.id === id);
      if (rec) setStatus(rec, status);
      deps.record("owner", "created", "learning_item", id, i.title.trim());
      deps.done("learning_item", id);
      return id;
    },
    update_learning: ({ id, input }) => {
      const rec = item(id as string);
      const i = input as LearningInput;
      checkLearning(i);
      Object.assign(rec, {
        trackId: i.trackId,
        ctfEventId: i.ctfEventId,
        title: i.title.trim(),
        kind: i.kind ?? rec.kind,
        domain: i.domain ?? rec.domain,
        difficulty: i.difficulty,
        url: i.url || null,
        points: i.points,
        progress: i.progress,
        notes: i.notes,
      });
      if (i.status && i.status !== rec.status) setStatus(rec, i.status);
      deps.record("owner", "updated", "learning_item", rec.id, rec.title);
      deps.done("learning_item", rec.id);
    },
    set_learning_status: ({ id, status }) => {
      const rec = item(id as string);
      const was = rec.status;
      setStatus(rec, status as LearningStatus);
      deps.record("owner", status === "done" ? "completed" : was === "done" ? "reopened" : "updated", "learning_item", rec.id, rec.title);
      deps.done("learning_item", rec.id);
    },
    delete_learning: ({ id }) => {
      const rec = item(id as string);
      rec.deleted = true;
      if (timer?.itemId === rec.id) timer = null;
      deps.record("owner", "deleted", "learning_item", rec.id, rec.title);
      deps.done("learning_item", rec.id);
    },
    restore_learning: ({ id }) => {
      const rec = items.find((x) => x.id === id) ?? notFound("learning item");
      rec.deleted = false;
      deps.record("owner", "restored", "learning_item", rec.id, rec.title);
      deps.done("learning_item", rec.id);
    },
    start_timer: ({ itemId }) => {
      const rec = item(itemId as string);
      if (timer?.itemId === rec.id) return;
      stop();
      timer = { itemId: rec.id, startedAt: iso(new Date()) };
      if (rec.status === "todo") setStatus(rec, "doing");
      deps.done("learning_session");
    },
    stop_timer: () => {
      const minutes = stop();
      deps.done("learning_session");
      return minutes;
    },
    discard_timer: () => {
      timer = null;
      deps.done("learning_session");
    },
    add_session: ({ input }) => {
      const s = input as SessionInput;
      const rec = item(s.itemId);
      if (s.minutes <= 0 || s.minutes > 960) fail("minutes", "format");
      const startedAt = new Date(
        `${s.date}T${String(Math.floor(s.startMinute / 60)).padStart(2, "0")}:${String(s.startMinute % 60).padStart(2, "0")}`,
      ).toISOString();
      const id = deps.newId();
      sessions.push({ id, itemId: rec.id, startedAt, durationMin: s.minutes, note: s.note });
      deps.record("owner", "created", "learning_session", rec.id, rec.title);
      deps.done("learning_session");
      return id;
    },
    delete_session: ({ id }) => {
      const at = sessions.findIndex((s) => s.id === id);
      if (at < 0) notFound("session");
      sessions.splice(at, 1);
      deps.done("learning_session");
    },
    list_ctf: (): CtfEvent[] =>
      ctf
        .filter((c) => !c.deleted)
        .map((c) => {
          const own = liveItems().filter((i) => i.ctfEventId === c.id);
          const solved = own.filter((i) => i.status === "done");
          const { id, name, startsOn, endsOn, team, rank, participants, url, notes } = c;
          return {
            id,
            name,
            startsOn,
            endsOn,
            team,
            rank,
            participants,
            url,
            notes,
            challenges: own.length,
            solved: solved.length,
            points: solved.reduce((sum, i) => sum + (i.points ?? 0), 0),
          };
        }),
    create_ctf: ({ input }) => {
      const i = input as CtfInput;
      if (!i.name.trim()) fail("name", "length");
      if (i.startsOn && i.endsOn && i.endsOn < i.startsOn) fail("endsOn", "order");
      const id = deps.newId();
      ctf.push({
        id,
        name: i.name.trim(),
        startsOn: i.startsOn,
        endsOn: i.endsOn,
        team: i.team,
        rank: i.rank,
        participants: i.participants,
        url: i.url,
        notes: i.notes,
        deleted: false,
      });
      deps.record("owner", "created", "ctf_event", id, i.name.trim());
      deps.done("ctf_event", id);
      return id;
    },
    update_ctf: ({ id, input }) => {
      const c = ctf.find((x) => x.id === id && !x.deleted) ?? notFound("ctf event");
      const i = input as CtfInput;
      Object.assign(c, {
        name: i.name.trim(),
        startsOn: i.startsOn,
        endsOn: i.endsOn,
        team: i.team,
        rank: i.rank,
        participants: i.participants,
        url: i.url,
        notes: i.notes,
      });
      deps.done("ctf_event", c.id);
    },
    delete_ctf: ({ id }) => {
      const c = ctf.find((x) => x.id === id && !x.deleted) ?? notFound("ctf event");
      c.deleted = true;
      for (const i of items) if (i.ctfEventId === c.id) i.ctfEventId = null;
      deps.done("ctf_event");
    },
    open_link: () => undefined,
    get_learning_overview: () => overview(),
    get_trail: ({ query }) => trail((query as TrailQuery | undefined) ?? {}),
    get_week_insight: () => week(),
    search: ({ query }) => {
      const words = String(query).toLowerCase().split(/\s+/).filter(Boolean);
      if (words.length === 0) return [];
      const has = (text: string) => words.every((w) => text.toLowerCase().includes(w));
      const own: SearchHit[] = [
        ...live()
          .filter((f) => has(`${f.name} ${f.note}`))
          .map((f): SearchHit => ({ kind: "file", id: f.id, title: f.name, detail: f.path, pigment: null, projectId: null })),
        ...collections
          .filter((c) => !c.deleted && has(c.name))
          .map((c): SearchHit => ({ kind: "collection", id: c.id, title: c.name, detail: "", pigment: c.pigment, projectId: null })),
        ...liveItems()
          .filter((i) => has(`${i.title} ${i.notes}`))
          .map((i): SearchHit => {
            const card = learningCard(i);
            return { kind: "learning_item", id: i.id, title: i.title, detail: card.trackName, pigment: card.pigment, projectId: null };
          }),
      ];
      return [...deps.searchCore(words), ...own].slice(0, 20);
    },
  };

  return {
    handlers,
    /** Files dragged over the window, as Windows would announce them: enter, over, then drop. */
    dropFiles(names: string[], x: number, y: number) {
      const dropId = nextDrop++;
      drops.set(dropId, names);
      deps.emitDrag({ phase: "enter", x, y, dropId: null, count: names.length, names: names.slice(0, 3) });
      deps.emitDrag({ phase: "over", x, y, dropId: null, count: 0, names: [] });
      deps.emitDrag({ phase: "drop", x, y, dropId, count: names.length, names: names.slice(0, 3) });
    },
    recentFiles: (limit: number) => list({ limit }),
    projectFiles: (projectId: string) => list({ projectId }),
    taskFiles: (taskId: string) => list({ target: { kind: "task", id: taskId } }),
    glance(): LearningGlance {
      const view = overview();
      return {
        goalDays: view.goalDays,
        activeDays: view.activeDays,
        minutesWeek: view.minutesWeek,
        days: view.days,
        current: view.current,
        timer: view.timer,
      };
    },
  };
}
