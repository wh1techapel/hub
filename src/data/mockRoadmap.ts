// The roadmap in the demo backend: a pretend Gemini writes one checkpoint per Must and Should
// feature of the approved PRD (a project whose name holds "putus" fails as offline), the owner
// edits, adds, removes, and reorders the draft, picks a folder, and approves it with the core's
// rules in a smaller form. Folders holding "tidak-ada" do not exist; "tanpa-git" has no `.git`.

import { DOD_REPORT, DOD_TESTS } from "../pages/studio/roadmapRules";
import { HubError } from "./backend";
import type { Appended, RoadmapEvent } from "./api";
import type { ApproveRoadmap } from "./generated/ApproveRoadmap";
import type { Checkpoint } from "./generated/Checkpoint";
import type { CheckpointTask } from "./generated/CheckpointTask";
import type { FolderCheck } from "./generated/FolderCheck";
import type { PlanCheckpoint } from "./generated/PlanCheckpoint";
import type { Prd } from "./generated/Prd";
import type { PrdRef } from "./generated/PrdRef";
import type { Problem } from "./generated/Problem";
import type { Roadmap } from "./generated/Roadmap";
import type { RoadmapMeta } from "./generated/RoadmapMeta";
import type { ReportInput } from "./generated/ReportInput";
import type { RoadmapView } from "./generated/RoadmapView";
import type { StudioLink } from "./generated/StudioLink";
import type { StudioProject } from "./generated/StudioProject";
import type { StudioReport } from "./generated/StudioReport";
import type { StudioRequest } from "./generated/StudioRequest";
import type { StudioTaskStatus } from "./generated/StudioTaskStatus";

const PIECE_MS = 15;
/** Where the pretend folder dialog lands. */
export const PICKED_FOLDER = "C:\\Users\\kamu\\Projects\\absensi";

interface RoadmapDeps {
  newId: () => string;
  done: (id: string) => void;
  project: (id: string) => StudioProject;
  approvedPrd: (id: string) => { ref: PrdRef; prd: Prd } | null;
  createHubProject: (name: string, description: string) => string;
  hubProjectName: (id: string) => string | null;
  /** Another Studio project holds this Hub project. */
  linkedElsewhere: (hubId: string, studioId: string) => boolean;
  stopping: Set<string>;
}

interface Store {
  draft: Roadmap | null;
  approved: Roadmap | null;
  superseded: RoadmapMeta[];
  requests: StudioRequest[];
}

const ACTIVE = ["ready", "in_progress", "blocked", "in_review"];

/** The core's report rules, in the cases the demo meets. */
export function mockReportCheck(report: ReportInput, cp: Checkpoint): Problem[] {
  const problems: Problem[] = [];
  if (!report.summary.trim()) problems.push({ at: "summary", message: "wajib diisi" });
  const failed: string[] = [];
  for (const test of cp.requiredTests) {
    const found = report.tests.find((x) => x.name.trim().toLowerCase() === test.name.trim().toLowerCase());
    if (!found) problems.push({ at: "tests", message: `pengujian wajib "${test.name}" belum dilaporkan` });
    else if (found.result !== "passed") failed.push(test.name);
  }
  if (failed.length > 0 && report.notVerified.every((x) => !x.trim())) {
    problems.push({ at: "notVerified", message: `jelaskan kenapa ${failed.join(", ")} tidak lulus atau tidak dijalankan` });
  }
  return problems;
}

function fail(field: string, reason: string): never {
  throw new HubError({ kind: "invalid", field, reason, message: `invalid ${field}` });
}

const code = (index: number) => `CP-${String(index + 1).padStart(2, "0")}`;

export function folderCheck(path: string): FolderCheck {
  const trimmed = path.trim().replace(/[\\/]+$/, "");
  if (!/^[A-Za-z]:\\/.test(`${trimmed}\\`) || trimmed.includes("..")) fail("repoPath", "format");
  const exists = !trimmed.includes("tidak-ada");
  return { path: trimmed, exists, git: exists && !trimmed.includes("tanpa-git") };
}

/** The core's rules, in the cases the demo meets. */
export function mockRoadmapCheck(checkpoints: PlanCheckpoint[], prd: Prd): Problem[] {
  const problems: Problem[] = [];
  if (checkpoints.length === 0) problems.push({ at: "checkpoints", message: "belum ada checkpoint" });
  const earlier = new Set<string>();
  const covered = new Map<string, number>();
  checkpoints.forEach((cp, i) => {
    const at = code(i);
    if (!cp.title.trim()) problems.push({ at: `${at}.title`, message: "wajib diisi" });
    if (!cp.goal.trim()) problems.push({ at: `${at}.goal`, message: "wajib diisi" });
    for (const d of cp.dependsOn) if (!earlier.has(d)) problems.push({ at: `${at}.dependsOn`, message: `bergantung pada ${d} yang bukan checkpoint sebelumnya` });
    if (cp.tasks.length === 0) problems.push({ at: `${at}.tasks`, message: "butuh paling sedikit satu tugas" });
    if (cp.acceptance.length === 0) problems.push({ at: `${at}.acceptance`, message: "butuh paling sedikit satu kriteria penerimaan" });
    if (cp.requiredTests.length === 0) problems.push({ at: `${at}.requiredTests`, message: "butuh paling sedikit satu pengujian wajib" });
    for (const f of cp.features) {
      if (!prd.features.some((x) => x.id === f)) problems.push({ at: `${at}.features`, message: `${f} tidak ada di PRD yang disetujui` });
      covered.set(f, (covered.get(f) ?? 0) + 1);
    }
    earlier.add(at);
  });
  for (const f of prd.features) {
    const n = covered.get(f.id) ?? 0;
    if (f.priority === "must" && n === 0) problems.push({ at: f.id, message: `${f.id} (must) belum masuk checkpoint mana pun` });
    if (n > 1) problems.push({ at: f.id, message: `${f.id} masuk lebih dari satu checkpoint` });
  }
  return problems;
}

export function roadmapMock(deps: RoadmapDeps) {
  const stores = new Map<string, Store>();
  const now = () => new Date().toISOString();
  const store = (id: string) => {
    const found = stores.get(id) ?? { draft: null, approved: null, superseded: [], requests: [] };
    stores.set(id, found);
    return found;
  };

  const planOf = (cp: Checkpoint): PlanCheckpoint => ({
    code: cp.code,
    title: cp.title,
    goal: cp.goal,
    description: cp.description,
    features: [...cp.features],
    tasks: cp.tasks
      .filter((t) => t.parentId === null)
      .map((t) => ({
        code: t.code,
        title: t.title,
        detail: t.detail,
        feature: t.featureId ?? "",
        subtasks: cp.tasks.filter((s) => s.parentId === t.id).map((s) => s.title),
      })),
    dependsOn: [...cp.dependsOn],
    difficulty: cp.difficulty,
    risks: [...cp.risks],
    acceptance: [...cp.acceptance],
    requiredTests: cp.requiredTests.map((t) => ({ ...t })),
    deliverables: [...cp.deliverables],
    doneWhen: [...cp.doneWhen],
  });

  /** A checkpoint as the core keeps it: trimmed, codes assigned, the two DoD lines present. */
  const build = (plan: PlanCheckpoint, index: number, id: string, by: CheckpointTask["changedBy"]): Checkpoint => {
    const c = code(index);
    const at = now();
    const tasks: CheckpointTask[] = [];
    plan.tasks.forEach((t, j) => {
      const taskCode = `${c}.T${String(j + 1).padStart(2, "0")}`;
      const parent: CheckpointTask = {
        id: deps.newId(),
        parentId: null,
        code: taskCode,
        featureId: t.feature.trim() || null,
        title: t.title.trim(),
        detail: t.detail.trim(),
        status: "todo",
        note: "",
        changedBy: by,
        updatedAt: at,
      };
      tasks.push(parent);
      t.subtasks.forEach((s, k) => tasks.push({ ...parent, id: deps.newId(), parentId: parent.id, code: `${taskCode}.${k + 1}`, title: s.trim(), detail: "" }));
    });
    const doneWhen = plan.doneWhen.map((d) => d.trim()).filter(Boolean);
    for (const line of [DOD_TESTS, DOD_REPORT]) if (!doneWhen.includes(line)) doneWhen.push(line);
    return {
      id,
      code: c,
      position: index + 1,
      title: plan.title.trim(),
      goal: plan.goal.trim(),
      description: plan.description.trim(),
      features: [...new Set(plan.features.map((f) => f.trim()).filter(Boolean))],
      dependsOn: plan.dependsOn.map((d) => d.trim()).filter(Boolean),
      difficulty: plan.difficulty,
      risks: plan.risks.map((r) => r.trim()).filter(Boolean),
      acceptance: plan.acceptance.map((r) => r.trim()).filter(Boolean),
      requiredTests: plan.requiredTests.map((t) => ({ name: t.name.trim(), kind: t.kind, how: t.how.trim() })),
      deliverables: plan.deliverables.map((r) => r.trim()).filter(Boolean),
      doneWhen,
      status: "planned",
      readyAt: null,
      startedAt: null,
      submittedAt: null,
      completedAt: null,
      updatedAt: at,
      tasks,
    };
  };

  /** Codes follow the order; dependencies are renamed with them and dropped when gone. */
  const renumber = (roadmap: Roadmap, order: Checkpoint[]) => {
    const renamed = new Map(order.map((cp, i) => [cp.code, code(i)]));
    roadmap.checkpoints = order.map((cp, i) => {
      const next = code(i);
      return {
        ...cp,
        code: next,
        position: i + 1,
        dependsOn: cp.dependsOn.flatMap((d) => (renamed.has(d) ? [renamed.get(d) as string] : [])),
        tasks: cp.tasks.map((t) => ({ ...t, code: next + t.code.slice(cp.code.length) })),
      };
    });
  };

  const view = (projectId: string): RoadmapView => {
    deps.project(projectId);
    const s = store(projectId);
    const source = deps.approvedPrd(projectId);
    const problems = s.draft
      ? source
        ? mockRoadmapCheck(s.draft.checkpoints.map(planOf), source.prd)
        : [{ at: "prd", message: "belum ada PRD yang disetujui" }]
      : [];
    return {
      prd: source?.ref ?? null,
      draft: s.draft,
      problems,
      draftPrdChanged: s.draft !== null && source !== null && s.draft.meta.prdVersionId !== source.ref.id,
      approved: s.approved,
      locked: s.approved?.checkpoints.some((cp) => cp.status !== "planned" && cp.status !== "ready") ?? false,
      superseded: s.superseded,
    };
  };

  const meta = (ref: PrdRef, madeBy: RoadmapMeta["madeBy"]): RoadmapMeta => {
    const at = now();
    return { id: deps.newId(), status: "draft", prdVersionId: ref.id, prdNumber: ref.number, madeBy, createdAt: at, updatedAt: at, approvedAt: null };
  };

  const draftOf = (checkpointId: unknown) => {
    for (const [projectId, s] of stores) {
      const index = s.draft?.checkpoints.findIndex((cp) => cp.id === checkpointId) ?? -1;
      if (s.draft && index >= 0) return { projectId, draft: s.draft, index };
      if (s.approved?.checkpoints.some((cp) => cp.id === checkpointId)) fail("roadmap", "order");
    }
    throw new HubError({ kind: "not_found", field: "checkpoint", reason: null, message: "not found" });
  };

  const reports = new Map<string, StudioReport[]>();
  /** A checkpoint of an approved roadmap, with its project. */
  const working = (checkpointId: unknown) => {
    for (const [projectId, s] of stores) {
      const cp = s.approved?.checkpoints.find((c) => c.id === checkpointId);
      if (cp && s.approved) return { projectId, store: s, roadmap: s.approved, cp };
    }
    throw new HubError({ kind: "not_found", field: "checkpoint", reason: null, message: "not found" });
  };
  const moveTo = (projectId: string, cp: Checkpoint, to: Checkpoint["status"]) => {
    const at = now();
    if (to === "ready") cp.readyAt = at;
    if (to === "in_progress" && cp.status === "ready") cp.startedAt = at;
    if (to === "in_review") cp.submittedAt = at;
    if (to === "completed") cp.completedAt = at;
    cp.status = to;
    cp.updatedAt = at;
    deps.done(projectId);
  };
  const expect = (cp: Checkpoint, status: Checkpoint["status"]) => {
    if (cp.status !== status) fail("status", "order");
  };
  const request = (s: Store, cp: Checkpoint, kind: StudioRequest["kind"], text: string) => {
    const at = now();
    s.requests.push({ id: deps.newId(), checkpointId: cp.id, kind, text, options: [], status: "open", answer: null, madeBy: "owner", answeredBy: null, createdAt: at, answeredAt: null });
  };
  const close = (s: Store, cp: Checkpoint, kind: StudioRequest["kind"], answer: string) => {
    for (const r of s.requests) {
      if (r.checkpointId === cp.id && r.kind === kind && r.status === "open") Object.assign(r, { status: "answered", answer, answeredBy: "owner", answeredAt: now() });
    }
  };
  const promptOf = (projectId: string, prdNumber: number | null, cp: Checkpoint, project: StudioProject) => {
    const name = project.name.split(/\s+/).join(" ");
    return [
      `Kerjakan proyek Hub Studio "${name}" (id ${projectId}) di folder ${project.repoPath ?? ""}.`,
      "",
      `1. Jalankan /hub-connect ${projectId} ${cp.code}.`,
      `2. Baca PRD versi ${prdNumber ?? "?"} yang sudah disetujui dan checkpoint ${cp.code} (${cp.title}) lewat Hub. Jangan mengandalkan salinan lain dari dokumen itu.`,
      `3. Kerjakan hanya ${cp.code}. Periksa kode yang ada sebelum mengubahnya; jika ada konflik atau informasi kurang, tanyakan lewat hub_ask_owner.`,
      "4. Perbarui status tugas di Hub saat mulai dan selesai.",
      "5. Jalankan semua pengujian wajib checkpoint ini. Laporkan hasil yang benar-benar dijalankan lewat hub_submit_report, termasuk yang gagal atau tidak bisa dijalankan.",
      "6. Setelah laporan terkirim, berhenti. Jangan memulai checkpoint berikutnya sebelum aku menyetujuinya di Hub.",
      "",
    ].join("\n");
  };

  const work = {
    studio_requests: ({ projectId, openOnly }: Record<string, unknown>) => store(String(projectId)).requests.filter((r) => !openOnly || r.status === "open"),
    checkpoint_reports: ({ id }: Record<string, unknown>) => [...(reports.get(String(id)) ?? [])].reverse(),
    make_checkpoint_ready: ({ id }: Record<string, unknown>) => {
      const { projectId, roadmap, cp } = working(id);
      expect(cp, "planned");
      if (roadmap.checkpoints.some((c) => ACTIVE.includes(c.status))) fail("active", "too_many");
      for (const d of cp.dependsOn) if (roadmap.checkpoints.find((c) => c.code === d)?.status !== "completed") fail("dependsOn", "order");
      moveTo(projectId, cp, "ready");
      return undefined;
    },
    start_checkpoint: ({ id }: Record<string, unknown>) => {
      const { projectId, cp } = working(id);
      expect(cp, "ready");
      moveTo(projectId, cp, "in_progress");
      return undefined;
    },
    block_checkpoint: ({ id, reason }: Record<string, unknown>) => {
      const { projectId, store: s, cp } = working(id);
      expect(cp, "in_progress");
      if (!String(reason).trim()) fail("reason", "length");
      request(s, cp, "blocker", String(reason).trim());
      moveTo(projectId, cp, "blocked");
      return undefined;
    },
    resume_checkpoint: ({ id, answer }: Record<string, unknown>) => {
      const { projectId, store: s, cp } = working(id);
      expect(cp, "blocked");
      if (typeof answer === "string" && answer.trim()) close(s, cp, "blocker", answer.trim());
      if (s.requests.some((r) => r.checkpointId === cp.id && r.kind === "blocker" && r.status === "open")) fail("blocker", "order");
      moveTo(projectId, cp, "in_progress");
      return undefined;
    },
    answer_studio_request: ({ id, answer }: Record<string, unknown>) => {
      for (const [projectId, s] of stores) {
        const r = s.requests.find((x) => x.id === id);
        if (!r) continue;
        if (r.status !== "open" || (r.kind !== "question" && r.kind !== "blocker")) fail("request", "order");
        if (!String(answer).trim()) fail("answer", "length");
        Object.assign(r, { status: "answered", answer: String(answer).trim(), answeredBy: "owner", answeredAt: now() });
        deps.done(projectId);
        return undefined;
      }
      throw new HubError({ kind: "not_found", field: "request", reason: null, message: "not found" });
    },
    check_checkpoint_report: ({ id, report }: Record<string, unknown>) => mockReportCheck(report as ReportInput, working(id).cp),
    submit_checkpoint_report: ({ id, report }: Record<string, unknown>) => {
      const { projectId, store: s, cp } = working(id);
      expect(cp, "in_progress");
      const body = report as ReportInput;
      if (mockReportCheck(body, cp).length > 0) fail("report", "format");
      const list = reports.get(cp.id) ?? [];
      list.push({ id: deps.newId(), checkpointId: cp.id, kind: "submission", body: structuredClone(body), madeBy: "owner", createdAt: now() });
      reports.set(cp.id, list);
      close(s, cp, "changes", "Laporan baru dikirim.");
      request(s, cp, "review", `Laporan ${cp.code} menunggu tinjauanmu.`);
      moveTo(projectId, cp, "in_review");
      return undefined;
    },
    approve_checkpoint: ({ id }: Record<string, unknown>) => {
      const { projectId, store: s, cp } = working(id);
      expect(cp, "in_review");
      close(s, cp, "review", "Disetujui.");
      moveTo(projectId, cp, "completed");
      return undefined;
    },
    request_checkpoint_changes: ({ id, note }: Record<string, unknown>) => {
      const { projectId, store: s, cp } = working(id);
      expect(cp, "in_review");
      if (!String(note).trim()) fail("note", "length");
      close(s, cp, "review", "Diminta perubahan.");
      request(s, cp, "changes", String(note).trim());
      moveTo(projectId, cp, "in_progress");
      return undefined;
    },
    set_studio_task_status: ({ id, status, note }: Record<string, unknown>) => {
      for (const [projectId, s] of stores) {
        for (const cp of s.approved?.checkpoints ?? []) {
          const task = cp.tasks.find((x) => x.id === id);
          if (!task) continue;
          if (cp.status !== "in_progress" && cp.status !== "blocked") fail("status", "order");
          Object.assign(task, { status: status as StudioTaskStatus, changedBy: "owner", updatedAt: now() });
          if (typeof note === "string") task.note = note.trim();
          deps.done(projectId);
          return undefined;
        }
      }
      throw new HubError({ kind: "not_found", field: "task", reason: null, message: "not found" });
    },
    implementation_prompt: ({ id }: Record<string, unknown>) => {
      const { projectId, roadmap, cp } = working(id);
      if (cp.status === "planned" || cp.status === "completed") fail("status", "order");
      const project = deps.project(projectId);
      if (!project.repoPath) fail("repoPath", "unknown");
      return promptOf(projectId, roadmap.meta.prdNumber, cp, project);
    },
    // "Mandiri" (R4), in a shorter form than the core's: the rules, the checkpoint, and the plan.
    standalone_prompt: ({ id }: Record<string, unknown>) => {
      const { roadmap, cp, projectId } = working(id);
      if (cp.status === "planned" || cp.status === "completed") fail("status", "order");
      const project = deps.project(projectId);
      if (!project.repoPath) fail("repoPath", "unknown");
      const plan = roadmap.checkpoints.map((c) => `- ${c.code} ${c.title}: ${c.status}${c.id === cp.id ? " (yang dikerjakan sekarang)" : ""}`);
      return [
        `Kerjakan proyek "${project.name}" di folder ${project.repoPath}, checkpoint ${cp.code} saja. Prompt ini lengkap: semua yang kamu butuhkan ada di bawah, tanpa Hub dan tanpa MCP.`,
        "",
        "## Aturan kerja",
        `- Kerjakan hanya ${cp.code}. Jangan memulai checkpoint lain.`,
        "- Setelah selesai, tulis laporan, lalu berhenti dan tunggu persetujuanku.",
        "",
        `## Checkpoint ${cp.code}: ${cp.title}`,
        `Tujuan: ${cp.goal}`,
        "",
        "## Rencana checkpoint (konteks)",
        ...plan,
        "",
        `## PRD versi ${roadmap.meta.prdNumber ?? "?"} (disetujui)`,
      ].join("\n");
    },
    studio_link: ({ projectId }: Record<string, unknown>): StudioLink | null => {
      for (const [studioId, s] of stores) {
        let linked: StudioProject;
        try {
          linked = deps.project(studioId);
        } catch {
          continue;
        }
        if (linked.projectId !== projectId) continue;
        const list = s.approved?.checkpoints ?? [];
        const active = list.find((c) => ACTIVE.includes(c.status)) ?? null;
        return {
          studioProjectId: studioId,
          activeCode: active?.code ?? null,
          activeStatus: active?.status ?? null,
          completed: list.filter((c) => c.status === "completed").length,
          total: list.length,
        };
      }
      return null;
    },
  };

  const handlers = {
    ...work,
    studio_roadmap: ({ projectId }: Record<string, unknown>) => view(String(projectId)),
    update_roadmap_checkpoint: ({ id, value }: Record<string, unknown>) => {
      const { projectId, draft, index } = draftOf(id);
      const plan = value as PlanCheckpoint;
      if (!plan.title.trim()) fail("title", "length");
      draft.checkpoints[index] = build(plan, index, String(id), "owner");
      deps.done(projectId);
      return undefined;
    },
    add_roadmap_checkpoint: ({ projectId, after }: Record<string, unknown>) => {
      const s = store(String(projectId));
      if (!s.draft) throw new HubError({ kind: "not_found", field: "roadmap", reason: null, message: "no draft" });
      const at = after ? s.draft.checkpoints.findIndex((cp) => cp.id === after) + 1 : s.draft.checkpoints.length;
      const blank = build({ code: "", title: "Checkpoint baru", goal: "", description: "", features: [], tasks: [], dependsOn: [], difficulty: "medium", risks: [], acceptance: [], requiredTests: [], deliverables: [], doneWhen: [] }, s.draft.checkpoints.length, deps.newId(), "owner");
      blank.code = "CP-new";
      const order = [...s.draft.checkpoints];
      order.splice(at, 0, blank);
      renumber(s.draft, order);
      deps.done(String(projectId));
      return blank.id;
    },
    remove_roadmap_checkpoint: ({ id }: Record<string, unknown>) => {
      const { projectId, draft, index } = draftOf(id);
      renumber(draft, draft.checkpoints.filter((_, i) => i !== index));
      deps.done(projectId);
      return undefined;
    },
    move_roadmap_checkpoint: ({ id, to }: Record<string, unknown>) => {
      const { projectId, draft, index } = draftOf(id);
      const target = Number(to);
      if (target < 1 || target > draft.checkpoints.length) fail("to", "order");
      const order = [...draft.checkpoints];
      const [moved] = order.splice(index, 1);
      order.splice(target - 1, 0, moved as Checkpoint);
      renumber(draft, order);
      deps.done(projectId);
      return undefined;
    },
    edit_roadmap: ({ projectId }: Record<string, unknown>) => {
      const id = String(projectId);
      const s = store(id);
      if (s.draft) return undefined;
      if (!s.approved) throw new HubError({ kind: "not_found", field: "roadmap", reason: null, message: "none" });
      if (view(id).locked) fail("roadmap", "order");
      const source = deps.approvedPrd(id);
      if (!source) fail("prd", "unknown");
      s.draft = { meta: meta(source.ref, "owner"), checkpoints: s.approved.checkpoints.map((cp, i) => build(planOf(cp), i, deps.newId(), "owner")) };
      deps.done(id);
      return undefined;
    },
    discard_roadmap_draft: ({ projectId }: Record<string, unknown>) => {
      store(String(projectId)).draft = null;
      deps.done(String(projectId));
      return undefined;
    },
    append_roadmap_checkpoint: ({ projectId, value }: Record<string, unknown>): Appended => {
      const id = String(projectId);
      const s = store(id);
      const source = deps.approvedPrd(id);
      if (!s.approved || !source) throw new HubError({ kind: "not_found", field: "roadmap", reason: null, message: "none" });
      const cp = build(value as PlanCheckpoint, s.approved.checkpoints.length, deps.newId(), "owner");
      const problems = mockRoadmapCheck([...s.approved.checkpoints.map(planOf), planOf(cp)], source.prd).filter((p) => p.at.startsWith(cp.code));
      if (problems.length > 0) return { id: null, problems };
      s.approved.checkpoints.push(cp);
      deps.done(id);
      return { id: cp.id, problems: [] };
    },
    pick_repo_folder: (): FolderCheck => folderCheck(PICKED_FOLDER),
    check_repo_folder: ({ path }: Record<string, unknown>): FolderCheck => folderCheck(String(path)),
    approve_roadmap: ({ projectId, input }: Record<string, unknown>) => {
      const id = String(projectId);
      const s = store(id);
      const request = input as ApproveRoadmap;
      const folder = folderCheck(request.repoPath);
      if (!folder.exists) fail("repoPath", "unknown");
      if (!s.draft) throw new HubError({ kind: "not_found", field: "roadmap", reason: null, message: "no draft" });
      const source = deps.approvedPrd(id);
      if (!source) fail("prd", "unknown");
      if (mockRoadmapCheck(s.draft.checkpoints.map(planOf), source.prd).length > 0) fail("roadmap", "format");
      if (view(id).locked) fail("roadmap", "order");
      const project = deps.project(id);
      let hubId: string;
      if (request.link.kind === "new") {
        hubId = deps.createHubProject(source.ref.proyekName, source.prd.identity.summary);
      } else {
        hubId = request.link.projectId;
        if (deps.hubProjectName(hubId) === null) fail("projectId", "unknown");
        if (deps.linkedElsewhere(hubId, id)) fail("projectId", "order");
      }
      if (s.approved) s.superseded.unshift({ ...s.approved.meta, status: "superseded" });
      const at = now();
      s.approved = { ...s.draft, meta: { ...s.draft.meta, status: "approved", prdVersionId: source.ref.id, prdNumber: source.ref.number, approvedAt: at, updatedAt: at } };
      s.draft = null;
      Object.assign(project, { projectId: hubId, projectName: deps.hubProjectName(hubId), repoPath: folder.path, claudeAccess: request.claudeAccess, stage: "build", updatedAt: at });
      deps.done(id);
      return undefined;
    },
    set_repo_path: ({ projectId, path }: Record<string, unknown>) => {
      const folder = folderCheck(String(path));
      if (!folder.exists) fail("repoPath", "unknown");
      deps.project(String(projectId)).repoPath = folder.path;
      deps.done(String(projectId));
      return undefined;
    },
    set_claude_access: ({ projectId, access }: Record<string, unknown>) => {
      const project = deps.project(String(projectId));
      if (access !== "none" && !project.repoPath) fail("repoPath", "unknown");
      project.claudeAccess = access as StudioProject["claudeAccess"];
      deps.done(String(projectId));
      return undefined;
    },
  };

  const streams = {
    generate_roadmap: async ({ projectId }: Record<string, unknown>, onEvent: (event: RoadmapEvent) => void) => {
      const id = String(projectId);
      const project = deps.project(id);
      const source = deps.approvedPrd(id);
      if (!source) fail("prd", "unknown");
      if (view(id).locked) fail("gemini", "locked");
      onEvent({ kind: "step", repair: false });
      await new Promise((resolve) => setTimeout(resolve, PIECE_MS * 3));
      if (deps.stopping.has(id)) {
        deps.stopping.delete(id);
        onEvent({ kind: "finished", state: "stopped", errorKind: null, problems: 0 });
        return undefined;
      }
      if (project.name.includes("putus")) {
        onEvent({ kind: "finished", state: "failed", errorKind: "offline", problems: 0 });
        return undefined;
      }
      const plans: PlanCheckpoint[] = source.prd.features
        .filter((f) => f.priority === "must" || f.priority === "should")
        .map((f, i) => ({
          code: code(i),
          title: f.name,
          goal: f.description,
          description: "",
          features: [f.id],
          tasks: [
            { code: "", title: `Bangun ${f.name.toLowerCase()}`, detail: "", feature: f.id, subtasks: ["Siapkan data contoh"] },
            { code: "", title: `Uji ${f.name.toLowerCase()}`, detail: "", feature: f.id, subtasks: [] },
          ],
          dependsOn: i === 0 ? [] : [code(i - 1)],
          difficulty: i === 0 ? "medium" : "low",
          risks: i === 0 ? ["Pembaca kartu belum dipilih"] : [],
          acceptance: f.acceptance.map((a) => `${a.when}: ${a.then}`),
          requiredTests: [{ name: `Uji ${f.id}`, kind: "unit", how: "npm test" }],
          deliverables: [f.name],
          doneWhen: ["Didemonstrasikan ke guru"],
        }));
      store(id).draft = { meta: meta(source.ref, "gemini"), checkpoints: plans.map((p, i) => build(p, i, deps.newId(), "gemini")) };
      if (project.stage !== "build") project.stage = "roadmap";
      deps.done(id);
      onEvent({ kind: "finished", state: "complete", errorKind: null, problems: 0 });
      return undefined;
    },
  };

  return { handlers, streams };
}
