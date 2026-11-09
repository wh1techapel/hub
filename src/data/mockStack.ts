// Stage 2 in the demo backend (R3): one stack per project, the seven layers in order, a draft on
// every save, approval only with every layer chosen, and a pretend Gemini that recommends a stack
// for a lab attendance app. An ask containing "gagal" fails as offline. The same rules as hub-core
// `stack`, enough for the browser preview and the page tests.

import { HubError } from "./backend";
import type { StackEvent } from "./api";
import type { StackBody } from "./generated/StackBody";
import type { StackLayer } from "./generated/StackLayer";
import type { StackLayerKind } from "./generated/StackLayerKind";
import type { StackMode } from "./generated/StackMode";
import type { StudioStack } from "./generated/StudioStack";

export const LAYERS: StackLayerKind[] = ["platform", "frontend", "backend", "database", "auth", "hosting", "extras"];

const PIECE_MS = 15;

const LABELS: Record<StackLayerKind, string> = {
  platform: "Platform",
  frontend: "Frontend",
  backend: "Backend",
  database: "Database",
  auth: "Autentikasi",
  hosting: "Hosting",
  extras: "Layanan tambahan",
};

function fail(field: string, reason: string): never {
  throw new HubError({ kind: "invalid", field, reason, message: `invalid ${field}` });
}

/** A body as the core keeps it: seven layers once each, in order, trimmed. */
function checked(body: StackBody): StackBody {
  const layers = LAYERS.map((kind) => {
    const found = body.layers.filter((l) => l.layer === kind);
    const layer = found[0];
    if (!layer) fail("layers", "unknown");
    if (found.length > 1) fail("layers", "too_many");
    if (layer.choice.trim().length > 120) fail("choice", "length");
    if (layer.alternatives.length > 3) fail("alternatives", "too_many");
    return { ...layer, choice: layer.choice.trim(), why: layer.why.trim() };
  });
  if (body.layers.length !== LAYERS.length) fail("layers", "unknown");
  return { layers, summary: body.summary.trim() };
}

const RECOMMENDED: Record<StackLayerKind, [string, string, string]> = {
  platform: ["Web (PWA)", "Dibuka dari PC lab dan HP tanpa instalasi.", "Aplikasi Android"],
  frontend: ["React + Vite", "Sudah kamu kuasai dan cepat dibangun.", "Vue"],
  backend: ["Supabase", "Database, auth, dan API sekaligus tanpa server sendiri.", "Laravel"],
  database: ["PostgreSQL (Supabase)", "Termasuk di Supabase, tier gratis cukup untuk satu sekolah.", "MySQL"],
  auth: ["Supabase Auth", "Login guru dengan email; siswa cukup kartu.", "Tidak perlu"],
  hosting: ["Vercel", "Gratis untuk proyek kecil dan deploy dari Git.", "VPS sekolah"],
  extras: ["Tidak perlu", "Fitur inti tidak butuh layanan tambahan.", "Notifikasi push"],
};

function recommendation(ask: string): StackBody {
  const cheaper = ask.includes("murah") || ask.includes("tanpa");
  return {
    layers: LAYERS.map((kind): StackLayer => {
      const [choice, why, alternative] = RECOMMENDED[kind];
      const local = cheaper && kind === "hosting";
      return {
        layer: kind,
        choice: local ? "VPS sekolah" : choice,
        why: local ? "Sesuai permintaanmu: tanpa biaya di luar sekolah." : why,
        fit: "Cocok untuk absen dengan kartu dan rekap mingguan.",
        ease: "Mudah dipelihara oleh satu orang.",
        cost: "Gratis di tier awal; kompleksitas rendah.",
        alternatives: [{ name: local ? "Vercel" : alternative, when: "Bila kebutuhannya berubah." }],
        by: "gemini",
      };
    }),
    summary: "Stack web sederhana yang bisa dibangun satu orang dan berjalan gratis di awal.",
  };
}

interface StackDeps {
  hasKey: () => boolean;
  /** The project's stored stage and a way to say its stack changed. */
  stage: (projectId: string) => string;
  setStatus: (projectId: string, status: StudioStack["status"]) => void;
  stopping: Set<string>;
}

export function stackMock(deps: StackDeps) {
  const stacks = new Map<string, StudioStack>();
  const now = () => new Date().toISOString();

  const save = (projectId: string, body: StackBody, mode: StackMode) => {
    const at = now();
    const before = stacks.get(projectId);
    stacks.set(projectId, { mode, status: "draft", body: checked(body), approvedAt: null, createdAt: before?.createdAt ?? at, updatedAt: at });
    deps.setStatus(projectId, "draft");
  };

  const handlers = {
    studio_stack: ({ projectId }: Record<string, unknown>) => {
      deps.stage(String(projectId));
      return stacks.get(String(projectId)) ?? null;
    },
    save_studio_stack: ({ projectId, body, mode }: Record<string, unknown>) => {
      deps.stage(String(projectId));
      save(String(projectId), body as StackBody, mode as StackMode);
      return undefined;
    },
    approve_studio_stack: ({ projectId }: Record<string, unknown>) => {
      const stack = stacks.get(String(projectId));
      if (!stack) throw new HubError({ kind: "not_found", field: "stack", reason: null, message: "not found" });
      if (stack.body.layers.some((l) => !l.choice)) fail("choice", "length");
      const at = now();
      stacks.set(String(projectId), { ...stack, status: "approved", approvedAt: at, updatedAt: at });
      deps.setStatus(String(projectId), "approved");
      return undefined;
    },
  };

  const streams = {
    recommend_studio_stack: async ({ projectId, ask }: Record<string, unknown>, onEvent: (event: StackEvent) => void) => {
      if (!deps.hasKey()) fail("gemini", "no_key");
      const id = String(projectId);
      deps.stage(id);
      onEvent({ kind: "step", repair: false });
      await new Promise((resolve) => setTimeout(resolve, PIECE_MS * 4));
      if (deps.stopping.has(id)) {
        deps.stopping.delete(id);
        onEvent({ kind: "finished", state: "stopped", errorKind: null });
        return undefined;
      }
      const words = typeof ask === "string" ? ask : "";
      if (words.includes("gagal")) {
        onEvent({ kind: "finished", state: "failed", errorKind: "offline" });
        return undefined;
      }
      save(id, recommendation(words), "gemini");
      onEvent({ kind: "finished", state: "complete", errorKind: null });
      return undefined;
    },
  };

  /** The PRD follows the approved stack (hub-core `stack::require_for_prd`). */
  const requireForPrd = (projectId: string) => {
    const stage = deps.stage(projectId);
    if ((stage === "explore" || stage === "clarify") && stacks.get(projectId)?.status !== "approved") fail("stack", "order");
  };

  /** The approved stack as the PRD's technologies (hub-core `prd::technologies`). */
  const technologies = (projectId: string): string[] | null => {
    const stack = stacks.get(projectId);
    if (stack?.status !== "approved") return null;
    return stack.body.layers.map((l) => `${LABELS[l.layer]}: ${l.choice}`);
  };

  return { handlers, streams, requireForPrd, technologies };
}
