// The PRD in the demo backend: a pretend Gemini writes a small PRD in three parts (a project whose
// name holds "gagal" fails as offline), the owner edits sections, Gemini proposes revisions, and
// versions are saved and approved with the core's rules in a smaller form.

import { HubError } from "./backend";
import type { PrdEvent, PrdProposal } from "./api";
import type { Prd } from "./generated/Prd";
import type { PrdSection } from "./generated/PrdSection";
import type { PrdVersion } from "./generated/PrdVersion";
import type { PrdView } from "./generated/PrdView";
import type { Problem } from "./generated/Problem";

const PIECE_MS = 15;

export function samplePrd(name: string): Prd {
  return {
    identity: { name, summary: "Absensi praktikum dengan kartu pelajar.", owner: "Glenvio" },
    background: {
      problem: "Absen manual di lab memakan sekitar 10 menit setiap praktikum.",
      context: "Lab TJKT, 36 siswa per kelas.",
      targetUsers: ["Siswa TJKT", "Guru praktikum"],
      unmetNeed: "Absen yang cepat dan tercatat rapi.",
      evidence: [{ statement: "Pengamatan pemilik selama satu semester", source: "owner" }],
      assumptions: [{ id: "ASM-01", statement: "Setiap siswa punya kartu pelajar dengan NFC", validate: "Tanya bagian kesiswaan" }],
    },
    goals: {
      primary: "Absen satu kelas selesai dalam 2 menit.",
      benefits: ["Praktikum mulai tepat waktu"],
      differentiator: "Memakai kartu yang sudah dimiliki siswa.",
      metrics: [{ metric: "Waktu absen per kelas", target: "di bawah 2 menit", how: "Dicatat guru selama dua minggu" }],
      outOfScope: ["Penilaian praktikum"],
    },
    experience: {
      personas: [{ name: "Siswa", need: "Absen tanpa antre", context: "Datang bersamaan" }],
      journeys: [],
      mainFlows: [{ name: "Absen masuk", steps: ["Siswa menempelkan kartu", "Layar menampilkan nama", "Kehadiran tercatat"] }],
      alternateFlows: [{ name: "Kartu tertinggal", steps: ["Guru mencatat manual"] }],
      emptyStates: ["Belum ada kelas hari ini"],
      errorStates: ["Kartu tidak dikenal"],
      accessibility: ["Suara konfirmasi selain warna"],
    },
    // U5b: the user flow as a diagram.
    flow: {
      title: "Absen masuk praktikum",
      nodes: [
        { id: "N1", kind: "start", label: "Siswa masuk lab", feature: "" },
        { id: "N2", kind: "screen", label: "Layar absen", feature: "FEAT-001" },
        { id: "N3", kind: "decision", label: "Kartu terdaftar?", feature: "" },
        { id: "N4", kind: "action", label: "Sistem mencatat hadir", feature: "FEAT-001" },
        { id: "N5", kind: "screen", label: "Pesan kartu tidak dikenal", feature: "" },
        { id: "N6", kind: "action", label: "Guru melihat rekap", feature: "FEAT-002" },
        { id: "N7", kind: "end", label: "Praktikum dimulai", feature: "" },
      ],
      edges: [
        { from: "N1", to: "N2", label: "" },
        { from: "N2", to: "N3", label: "tempel kartu" },
        { from: "N3", to: "N4", label: "Ya" },
        { from: "N3", to: "N5", label: "Tidak" },
        { from: "N5", to: "N2", label: "coba lagi" },
        { from: "N4", to: "N6", label: "" },
        { from: "N6", to: "N7", label: "" },
      ],
    },
    features: [
      {
        id: "FEAT-001",
        name: "Absen dengan kartu",
        description: "Siswa menempelkan kartu ke pembaca.",
        rationale: "Masalah utamanya waktu absen.",
        priority: "must",
        dependsOn: [],
        story: { asA: "siswa", want: "absen dengan menempelkan kartu", soThat: "praktikum mulai tepat waktu" },
        acceptance: [{ id: "AC-001-1", given: "siswa terdaftar", when: "menempelkan kartu", then: "kehadiran tercatat dalam 2 detik" }],
        tests: ["Kartu terdaftar tercatat", "Kartu asing ditolak"],
      },
      {
        id: "FEAT-002",
        name: "Rekap untuk guru",
        description: "Guru melihat siapa yang hadir.",
        rationale: "Guru butuh daftar hadir.",
        priority: "should",
        dependsOn: ["FEAT-001"],
        story: { asA: "guru", want: "melihat rekap kehadiran", soThat: "tidak perlu memanggil nama" },
        acceptance: [{ id: "AC-002-1", given: "absen sudah berjalan", when: "guru membuka rekap", then: "daftar hadir tampil lengkap" }],
        tests: ["Rekap sesuai kartu yang ditempel"],
      },
    ],
    nfr: [{ category: "performance", requirement: "Absen tercatat cepat", measure: "di bawah 2 detik" }],
    technical: { applicable: false, architecture: "", components: [], dataModel: "", integrations: [], technologies: [], dependencies: [], constraints: [], risks: [], fromStack: false },
    security: [
      { id: "SEC-01", area: "data", requirement: "Rekap kehadiran hanya bisa dilihat guru.", verify: "Uji akses dengan akun siswa." },
      { id: "SEC-02", area: "secrets", requirement: "Kunci API tidak tersimpan di repo.", verify: "Periksa .gitignore dan riwayat commit." },
    ],
    delivery: {
      mvp: ["FEAT-001", "FEAT-002"],
      later: [],
      order: ["FEAT-001", "FEAT-002"],
      testing: "Uji di satu kelas selama satu minggu.",
      release: "Dipakai satu kelas dulu.",
      risks: [{ risk: "Pembaca kartu rusak", impact: "Absen kembali manual", mitigation: "Siapkan daftar manual" }],
      questions: [{ id: "Q-01", question: "Pembaca kartu mana yang dipakai?", owner: "Guru" }],
    },
  };
}

/** The core's rules, in the few cases the demo meets. */
export function mockCheck(prd: Prd): Problem[] {
  const problems: Problem[] = [];
  if (!prd.identity.name.trim()) problems.push({ at: "identity.name", message: "wajib diisi" });
  if (!prd.goals.primary.trim()) problems.push({ at: "goals.primary", message: "wajib diisi" });
  if (prd.goals.metrics.length === 0) problems.push({ at: "goals.metrics", message: "sebutkan paling sedikit satu indikator keberhasilan dengan targetnya" });
  const ids = new Set(prd.features.map((f) => f.id));
  for (const f of prd.features) {
    if ((f.priority === "must" || f.priority === "should") && f.acceptance.length === 0) problems.push({ at: f.id, message: "fitur must dan should butuh kriteria penerimaan" });
    if (f.priority === "must" && !prd.delivery.mvp.includes(f.id)) problems.push({ at: "delivery.mvp", message: `${f.id} (must) belum masuk MVP` });
  }
  for (const id of prd.delivery.mvp) if (!ids.has(id)) problems.push({ at: "delivery.mvp", message: `${id} tidak ada di daftar fitur` });
  if (prd.security.length === 0) problems.push({ at: "security", message: "tulis paling sedikit satu persyaratan keamanan" });
  const seen = new Set<string>();
  for (const s of prd.security) {
    if (!/^SEC-\d{2}$/.test(s.id)) problems.push({ at: s.id, message: "Persyaratan keamanan harus berbentuk SEC-00" });
    else if (seen.has(s.id)) problems.push({ at: s.id, message: `Persyaratan keamanan ${s.id} dipakai dua kali` });
    seen.add(s.id);
    if (!s.requirement.trim()) problems.push({ at: `${s.id}.requirement`, message: "wajib diisi" });
  }
  return problems;
}

interface Store {
  draft: Prd | null;
  draftMeta: PrdVersion | null;
  versions: { meta: PrdVersion; body: Prd }[];
}

/** The PRD with its technologies taken from the approved stack (hub-core `prd::with_stack`). */
function withStack(prd: Prd, lines: string[]): Prd {
  return { ...prd, technical: { ...prd.technical, applicable: true, technologies: [...lines], fromStack: true } };
}

export function prdMock(deps: {
  newId: () => string;
  done: (id: string) => void;
  projectName: (id: string) => string;
  stopping: Set<string>;
  /** The approved stack's technologies, or null without one (R4). */
  stackLines: (id: string) => string[] | null;
}) {
  const stores = new Map<string, Store>();
  const now = () => new Date().toISOString();
  const store = (id: string) => {
    const found = stores.get(id) ?? { draft: null, draftMeta: null, versions: [] };
    stores.set(id, found);
    return found;
  };
  const same = (a: Prd, b: Prd) => JSON.stringify(a) === JSON.stringify(b);
  const freeze = (s: Store, status: PrdVersion["status"], note: string, madeBy: PrdVersion["madeBy"]) => {
    const at = now();
    const meta: PrdVersion = {
      id: deps.newId(),
      number: s.versions.length + 1,
      status,
      changeNote: note,
      madeBy,
      createdAt: at,
      updatedAt: at,
      approvedAt: status === "approved" ? at : null,
    };
    s.versions.unshift({ meta, body: structuredClone(s.draft as Prd) });
    return meta.number ?? 0;
  };
  const view = (projectId: string): PrdView => {
    const s = store(projectId);
    const latest = s.versions[0]?.body ?? null;
    return {
      draft: s.draft,
      draftMeta: s.draftMeta,
      problems: s.draft ? mockCheck(s.draft) : [],
      unsaved: s.draft !== null && (latest === null || !same(latest, s.draft)),
      approved: s.versions.find((v) => v.meta.status === "approved")?.meta ?? null,
      versions: s.versions.map((v) => v.meta),
      stack: deps.stackLines(projectId),
    };
  };
  const draftOf = (projectId: string) => {
    const s = store(projectId);
    if (!s.draft) throw new HubError({ kind: "not_found", field: "prd", reason: null, message: "no prd" });
    return s;
  };
  const setSection = (prd: Prd, section: PrdSection, value: unknown): Prd => ({ ...prd, [section]: structuredClone(value) });

  const handlers = {
    studio_prd: ({ projectId }: Record<string, unknown>) => view(String(projectId)),
    apply_stack_to_prd: ({ projectId }: Record<string, unknown>) => {
      const id = String(projectId);
      const lines = deps.stackLines(id);
      if (!lines) throw new HubError({ kind: "invalid", field: "stack", reason: "order", message: "no approved stack" });
      const s = draftOf(id);
      s.draft = withStack(s.draft as Prd, lines);
      deps.done(id);
      return undefined;
    },
    get_prd_version: ({ id }: Record<string, unknown>) => {
      for (const s of stores.values()) {
        const found = s.versions.find((v) => v.meta.id === id);
        if (found) return found.body;
      }
      throw new HubError({ kind: "not_found", field: "prdVersion", reason: null, message: "not found" });
    },
    update_prd_section: ({ projectId, section, value }: Record<string, unknown>) => {
      const s = draftOf(String(projectId));
      s.draft = setSection(s.draft as Prd, section as PrdSection, value);
      if (s.draftMeta) s.draftMeta = { ...s.draftMeta, madeBy: "owner", updatedAt: now() };
      deps.done(String(projectId));
      return undefined;
    },
    revise_prd_section: ({ projectId, section, ask }: Record<string, unknown>): PrdProposal => {
      const s = draftOf(String(projectId));
      const current = structuredClone((s.draft as Prd)[section as PrdSection]) as unknown;
      let value = current;
      if (section === "goals" && typeof current === "object" && current !== null) {
        value = { ...(current as Record<string, unknown>), primary: `${(current as { primary: string }).primary} (${String(ask)})` };
      }
      const next = setSection(s.draft as Prd, section as PrdSection, value);
      return { section: section as PrdSection, value, changeNote: `Tujuan utama disesuaikan: ${String(ask)}.`, impacts: [], problems: mockCheck(next) };
    },
    accept_prd_revision: ({ projectId, section, value }: Record<string, unknown>) => {
      const s = draftOf(String(projectId));
      s.draft = setSection(s.draft as Prd, section as PrdSection, value);
      if (s.draftMeta) s.draftMeta = { ...s.draftMeta, madeBy: "gemini", updatedAt: now() };
      deps.done(String(projectId));
      return undefined;
    },
    save_prd_version: ({ projectId, note }: Record<string, unknown>) => {
      const s = draftOf(String(projectId));
      const number = freeze(s, "saved", String(note).trim(), s.draftMeta?.madeBy ?? "owner");
      deps.done(String(projectId));
      return number;
    },
    approve_prd: ({ projectId }: Record<string, unknown>) => {
      const s = draftOf(String(projectId));
      if (mockCheck(s.draft as Prd).length > 0) throw new HubError({ kind: "invalid", field: "prd", reason: "format", message: "problems" });
      for (const v of s.versions) if (v.meta.status === "approved") v.meta.status = "superseded";
      const latest = s.versions[0];
      let number: number;
      if (latest && same(latest.body, s.draft as Prd)) {
        latest.meta.status = "approved";
        latest.meta.approvedAt = now();
        number = latest.meta.number ?? 0;
      } else {
        number = freeze(s, "approved", "Disetujui.", s.draftMeta?.madeBy ?? "owner");
      }
      deps.done(String(projectId));
      return number;
    },
    export_prd: ({ projectId }: Record<string, unknown>) => `C:\\Users\\kamu\\Documents\\PRD ${deps.projectName(String(projectId))}.md`,
  };

  const streams = {
    generate_prd: async ({ projectId }: Record<string, unknown>, onEvent: (event: PrdEvent) => void) => {
      const id = String(projectId);
      const name = deps.projectName(id);
      for (let part = 1; part <= 3; part += 1) {
        onEvent({ kind: "step", part, total: 3, repair: false });
        await new Promise((resolve) => setTimeout(resolve, PIECE_MS * 2));
        if (deps.stopping.has(id)) {
          deps.stopping.delete(id);
          onEvent({ kind: "finished", state: "stopped", errorKind: null, problems: 0 });
          return undefined;
        }
        if (name.includes("gagal")) {
          onEvent({ kind: "finished", state: "failed", errorKind: "offline", problems: 0 });
          return undefined;
        }
      }
      const s = store(id);
      const lines = deps.stackLines(id);
      s.draft = lines ? withStack(samplePrd(name), lines) : samplePrd(name);
      const at = now();
      s.draftMeta = { id: deps.newId(), number: null, status: "draft", changeNote: "", madeBy: "gemini", createdAt: at, updatedAt: at, approvedAt: null };
      freeze(s, "saved", "Disusun Lentera dari percakapan.", "gemini");
      deps.done(id);
      onEvent({ kind: "finished", state: "complete", errorKind: null, problems: 0 });
      return undefined;
    },
  };

  return { handlers, streams };
}
