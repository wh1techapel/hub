// Studio in the demo backend: projects, the conversation, and a pretend Gemini that streams a
// short reply in pieces, asks one question with options, records an assumption, and on the
// second turn proposes moving to Klarifikasi. A message containing "gagal" fails as offline after
// a few words; one containing "panjang" streams a long answer; Hentikan stops it where it is.
// Enough for the browser preview and the page tests.

import { HubError } from "./backend";
import { prdMock } from "./mockPrd";
import { roadmapMock } from "./mockRoadmap";
import { stackMock } from "./mockStack";
import type { PrdEvent, ReplyEvent, RoadmapEvent, StackEvent } from "./api";
import type { Actor } from "./generated/Actor";
import type { Entity } from "./generated/Entity";
import type { LedgerEdit } from "./generated/LedgerEdit";
import type { LedgerInput } from "./generated/LedgerInput";
import type { LedgerItem } from "./generated/LedgerItem";
import type { StudioMessage } from "./generated/StudioMessage";
import type { StudioProject } from "./generated/StudioProject";
import type { StudioStage } from "./generated/StudioStage";
import type { Verb } from "./generated/Verb";

interface StudioDeps {
  newId: () => string;
  record: (actor: Actor, verb: Verb, entity: Entity, entityId: string | null, summary: string) => void;
  done: (entity: Entity, id?: string) => void;
  hasKey: () => boolean;
  model: () => string;
  /** A new Hub Proyek (roadmap approval), and an existing one's name (null when there is none). */
  createHubProject: (name: string, description: string) => string;
  hubProjectName: (id: string) => string | null;
}

/** The moves the owner may make by hand (advance_studio_stage), as in the core. */
const NEXT: Partial<Record<StudioStage, StudioStage>> = { explore: "clarify", clarify: "prd" };
/** Pieces arrive this many milliseconds apart: quick for tests, visible in the browser. */
const PIECE_MS = 15;

function fail(field: string, reason: string): never {
  throw new HubError({ kind: "invalid", field, reason, message: `invalid ${field}` });
}

export function nameFromIdea(idea: string): string {
  const line = idea.split("\n").map((l) => l.trim()).find((l) => l.length > 0) ?? "Ide baru";
  if (line.length <= 60) return line;
  const cut = line.slice(0, 60);
  const space = cut.lastIndexOf(" ");
  return `${(space > 30 ? cut.slice(0, space) : cut).replace(/[,.;: ]+$/, "")}…`;
}

export function studioMock(deps: StudioDeps, initial: StudioProject[]) {
  const projects: (StudioProject & { deleted: boolean })[] = initial.map((p) => ({ ...p, deleted: false }));
  const messages = new Map<string, StudioMessage[]>();
  const ledger = new Map<string, LedgerItem[]>();
  const stopping = new Set<string>();
  const now = () => new Date().toISOString();

  const live = (id: unknown) => {
    const found = projects.find((p) => p.id === id && !p.deleted);
    if (!found) throw new HubError({ kind: "not_found", field: "studioProject", reason: null, message: "not found" });
    return found;
  };
  const listOf = <T>(map: Map<string, T[]>, id: string) => {
    const list = map.get(id) ?? [];
    map.set(id, list);
    return list;
  };
  const touch = (p: StudioProject) => {
    p.updatedAt = now();
  };

  const addLedger = (projectId: string, input: LedgerInput, by: "owner" | "gemini", messageId: string | null) => {
    const list = listOf(ledger, projectId);
    if (!input.topic.trim()) fail("topic", "length");
    if (!input.statement.trim()) fail("statement", "length");
    if (input.replacesId) {
      const old = list.find((i) => i.id === input.replacesId && i.status === "active");
      if (!old) fail("replacesId", "unknown");
      old.status = "retired";
    }
    const item: LedgerItem = {
      id: deps.newId(),
      kind: input.kind,
      topic: input.topic.trim(),
      statement: input.statement.trim(),
      rationale: input.rationale.trim(),
      status: "active",
      statedBy: by,
      messageId,
      replacesId: input.replacesId ?? null,
      area: input.area ?? null,
      createdAt: now(),
      updatedAt: now(),
    };
    list.push(item);
    deps.record(by === "owner" ? "owner" : "gemini", "created", "studio_ledger", item.id, item.topic);
    deps.done("studio_ledger", item.id);
    return item.id;
  };

  /** The pretend reply: streamed in pieces, then parts, then the end. */
  const reply = async (projectId: string, onEvent: (event: ReplyEvent) => void) => {
    const project = live(projectId);
    const list = listOf(messages, projectId);
    if (list.some((m) => m.state === "streaming")) fail("reply", "too_many");
    const asked = list.filter((m) => m.role === "owner");
    const last = asked[asked.length - 1]?.text ?? "";
    const message: StudioMessage = {
      id: deps.newId(),
      role: "model",
      stage: project.stage,
      text: "",
      parts: {},
      state: "streaming",
      errorKind: null,
      model: deps.model(),
      inputTokens: null,
      outputTokens: null,
      createdAt: now(),
    };
    list.push(message);
    deps.done("studio_message", message.id);
    onEvent({ kind: "started", messageId: message.id });

    const failing = last.includes("gagal");
    // "kuota" fails as Google's quota does, unless the model was changed since (L2).
    const quota = last.includes("kuota") && deps.model() === "gemini-3.8-flash";
    // "sibuk": the model is in high demand at Google (service), unless it was changed since.
    const busy = last.includes("sibuk") && deps.model() === "gemini-3.8-flash";
    // "panjang" asks for a long answer, long enough to stop halfway.
    const long = last.includes("panjang");
    const second = asked.length >= 2;
    const pieces = long
      ? Array.from({ length: 60 }, (_, i) => `Bagian ${i + 1}. `)
      : failing || quota || busy
      ? ["Sebentar, ", "aku sedang "]
      : second
        ? ["Terima kasih, ", "itu memperjelas pengguna dan masalahnya. ", "Menurutku kita sudah bisa memilih solusinya."]
        : ["Aku tangkap idenya: ", `**${project.name}**. `, "Sebelum lanjut, aku perlu tahu siapa yang paling sering memakainya."];
    for (const piece of pieces) {
      await new Promise((resolve) => setTimeout(resolve, PIECE_MS));
      if (stopping.has(projectId)) break;
      message.text += piece;
      onEvent({ kind: "delta", text: piece });
    }
    const end = (state: StudioMessage["state"], errorKind: string | null) => {
      message.state = state;
      message.errorKind = errorKind;
      touch(project);
      stopping.delete(projectId);
      deps.done("studio_message", message.id);
      onEvent({ kind: "finished", state, errorKind });
    };
    if (stopping.has(projectId)) {
      end("stopped", null);
      return;
    }
    if (failing) {
      end("failed", "offline");
      return;
    }
    if (quota) {
      end("failed", "quota");
      return;
    }
    if (busy) {
      end("failed", "service");
      return;
    }

    if (second) {
      // Stage 1 is one conversation (R2): the only proposal is to move on to the PRD.
      message.parts = { stage: { next: "prd", readyBecause: "Masalah dan pengguna sudah jelas.", stillOpen: ["Perangkat yang dipakai"] } };
    } else {
      addLedger(projectId, { kind: "assumption", topic: "Pengguna", statement: "Penggunanya siswa di lab sekolah.", rationale: "", area: "users" }, "gemini", message.id);
      addLedger(projectId, { kind: "decision", topic: "Masalah utama", statement: "Absen manual memakan waktu di awal praktikum.", rationale: "", area: "problem" }, "gemini", message.id);
      // One set of questions, grouped by part of the idea (R2).
      message.parts = {
        questions: [
          {
            text: "Siapa yang paling sering memakainya?",
            why: "Menentukan alur utama dan perangkatnya.",
            area: "users",
            multiple: false,
            options: [
              { label: "Siswa", detail: "", recommended: true },
              { label: "Guru", detail: "", recommended: false },
            ],
          },
          {
            text: "Fitur mana yang wajib ada di versi pertama?",
            why: "Yang lain bisa menyusul.",
            area: "features",
            multiple: true,
            options: [
              { label: "Absen dengan kartu", detail: "", recommended: true },
              { label: "Rekap mingguan", detail: "", recommended: false },
              { label: "Notifikasi ke wali kelas", detail: "", recommended: false },
            ],
          },
          {
            text: "Perangkat apa yang ada di lab?",
            why: "Menentukan platform.",
            area: "constraints",
            multiple: false,
            options: [
              { label: "PC lab", detail: "", recommended: true },
              { label: "HP siswa", detail: "", recommended: false },
            ],
          },
        ],
      };
    }
    onEvent({ kind: "parts", parts: message.parts });
    message.inputTokens = 120;
    message.outputTokens = 40;
    end("complete", null);
  };

  const handlers = {
    list_studio_projects: () =>
      projects.filter((p) => !p.deleted).sort((a, b) => (a.updatedAt < b.updatedAt ? 1 : -1)),
    start_studio_project: ({ idea }: Record<string, unknown>) => {
      const text = String(idea).trim();
      if (!text) fail("name", "length");
      const at = now();
      const project = {
        id: deps.newId(),
        name: nameFromIdea(text),
        idea: text.slice(0, 2000),
        stage: "explore" as StudioStage,
        projectId: null,
        projectName: null,
        repoPath: null,
        claudeAccess: "none" as const,
        waiting: 0,
        stackStatus: null,
        createdAt: at,
        updatedAt: at,
        deleted: false,
      };
      projects.push(project);
      deps.record("owner", "created", "studio_project", project.id, project.name);
      deps.done("studio_project", project.id);
      return project.id;
    },
    get_studio_project: ({ id }: Record<string, unknown>) => {
      const project: StudioProject & { deleted?: boolean } = { ...live(id) };
      delete project.deleted;
      return project;
    },
    rename_studio_project: ({ id, name }: Record<string, unknown>) => {
      const project = live(id);
      const text = String(name).trim();
      if (!text || text.length > 120) fail("name", "length");
      project.name = text;
      touch(project);
      deps.record("owner", "updated", "studio_project", project.id, project.name);
      deps.done("studio_project", project.id);
      return undefined;
    },
    delete_studio_project: ({ id }: Record<string, unknown>) => {
      const project = live(id);
      project.deleted = true;
      deps.record("owner", "deleted", "studio_project", project.id, project.name);
      deps.done("studio_project", project.id);
      return undefined;
    },
    advance_studio_stage: ({ id, to }: Record<string, unknown>) => {
      const project = live(id);
      if (NEXT[project.stage] !== to || (to !== "clarify" && to !== "prd")) fail("stage", "order");
      project.stage = to;
      touch(project);
      deps.record("owner", "moved", "studio_project", project.id, project.name);
      deps.done("studio_project", project.id);
      return undefined;
    },
    studio_messages: ({ projectId }: Record<string, unknown>) => {
      live(projectId);
      return listOf(messages, String(projectId));
    },
    studio_ledger: ({ projectId }: Record<string, unknown>) => {
      live(projectId);
      const order = ["decision", "assumption", "question", "constraint"];
      return [...listOf(ledger, String(projectId))].sort((a, b) => order.indexOf(a.kind) - order.indexOf(b.kind));
    },
    add_studio_ledger: ({ projectId, input }: Record<string, unknown>) => {
      live(projectId);
      return addLedger(String(projectId), input as LedgerInput, "owner", null);
    },
    edit_studio_ledger: ({ id, edit }: Record<string, unknown>) => {
      const change = edit as LedgerEdit;
      const item = [...ledger.values()].flat().find((i) => i.id === id);
      if (!item) throw new HubError({ kind: "not_found", field: "ledgerItem", reason: null, message: "not found" });
      if (change.status === "confirmed" && item.kind !== "assumption") fail("status", "format");
      if (change.status === "answered" && item.kind !== "question") fail("status", "format");
      if (change.statement !== undefined && !change.statement.trim()) fail("statement", "length");
      if (change.topic) item.topic = change.topic.trim();
      if (change.statement) item.statement = change.statement.trim();
      if (change.rationale !== undefined) item.rationale = change.rationale.trim();
      if (change.status) item.status = change.status;
      if (change.area) item.area = change.area;
      item.updatedAt = now();
      deps.record("owner", "updated", "studio_ledger", item.id, item.topic);
      deps.done("studio_ledger", item.id);
      return undefined;
    },
    stop_studio_reply: ({ projectId }: Record<string, unknown>) => {
      stopping.add(String(projectId));
      return undefined;
    },
  };

  const streams = {
    send_studio_message: async ({ projectId, text }: Record<string, unknown>, onEvent: (event: ReplyEvent) => void) => {
      if (!deps.hasKey()) fail("gemini", "no_key");
      const project = live(projectId);
      const body = String(text).trim();
      if (!body) fail("text", "length");
      listOf(messages, project.id).push({
        id: deps.newId(),
        role: "owner",
        stage: project.stage,
        text: body,
        parts: {},
        state: "complete",
        errorKind: null,
        model: null,
        inputTokens: null,
        outputTokens: null,
        createdAt: now(),
      });
      touch(project);
      deps.done("studio_message");
      await reply(project.id, onEvent);
      return undefined;
    },
    retry_studio_reply: async ({ projectId }: Record<string, unknown>, onEvent: (event: ReplyEvent) => void) => {
      if (!deps.hasKey()) fail("gemini", "no_key");
      await reply(String(projectId), onEvent);
      return undefined;
    },
  };

  const stack = stackMock({
    hasKey: deps.hasKey,
    stage: (id) => live(id).stage,
    setStatus: (id, status) => {
      const project = live(id);
      project.stackStatus = status;
      touch(project);
      deps.record("owner", status === "approved" ? "approved" : "updated", "studio_stack", id, project.name);
      deps.done("studio_stack", id);
      deps.done("studio_project", id);
    },
    stopping,
  });
  const prd = prdMock({
    newId: deps.newId,
    done: (id) => deps.done("studio_prd", id),
    projectName: (id) => live(id).name,
    stopping,
    stackLines: (id) => stack.technologies(id),
  });
  const reachPrd = (id: string) => {
    const project = live(id);
    if (project.stage === "explore" || project.stage === "clarify") project.stage = "prd";
    deps.done("studio_project", id);
  };
  const roadmap = roadmapMock({
    newId: deps.newId,
    done: (id) => {
      deps.done("studio_roadmap", id);
      deps.done("studio_project", id);
    },
    project: (id) => live(id),
    approvedPrd: (id) => {
      const approved = prd.handlers.studio_prd({ projectId: id }).approved;
      if (!approved?.number) return null;
      const body = prd.handlers.get_prd_version({ id: approved.id });
      // The rule of hub-core roadmap::proyek_name: the PRD's product name, else the project's.
      const proyekName = body.identity.name.trim() || live(id).name;
      return { ref: { id: approved.id, number: approved.number, proyekName }, prd: body };
    },
    createHubProject: deps.createHubProject,
    hubProjectName: deps.hubProjectName,
    linkedElsewhere: (hubId, studioId) => projects.some((p) => !p.deleted && p.id !== studioId && p.projectId === hubId),
    stopping,
  });
  return {
    handlers: { ...handlers, ...prd.handlers, ...roadmap.handlers, ...stack.handlers },
    streams: {
      ...streams,
      recommend_studio_stack: (args: Record<string, unknown>, onEvent: (event: StackEvent) => void) => stack.streams.recommend_studio_stack(args, onEvent),
      generate_roadmap: async (args: Record<string, unknown>, onEvent: (event: RoadmapEvent) => void) => {
        if (!deps.hasKey()) fail("gemini", "no_key");
        return roadmap.streams.generate_roadmap(args, onEvent);
      },
      generate_prd: async (args: Record<string, unknown>, onEvent: (event: PrdEvent) => void) => {
        if (!deps.hasKey()) fail("gemini", "no_key");
        // The PRD follows the approved tech stack (R3).
        stack.requireForPrd(String(args.projectId));
        reachPrd(String(args.projectId));
        return prd.streams.generate_prd(args, onEvent);
      },
    },
  };
}
