// U4b: the results of a Studio project as cards in its conversation (docs/studio.md, "Result
// cards"): the tech stack, the PRD, the checkpoint plan, and the build, each placed where it was
// first made and showing where it stands now. Read from what Hub already stores; nothing new is
// saved for them.

import type { PrdView } from "../../data/generated/PrdView";
import type { RoadmapView } from "../../data/generated/RoadmapView";
import type { StudioMessage } from "../../data/generated/StudioMessage";
import type { StudioProject } from "../../data/generated/StudioProject";
import type { StudioStack } from "../../data/generated/StudioStack";
import { id } from "../../strings/id";

const t = id.studio.results;

export type ResultKind = "stack" | "prd" | "roadmap" | "build";

/** What a document gives the page so a card's Setujui starts the document's own approval. */
export interface Approver {
  approve: () => void;
}

export interface Result {
  kind: ResultKind;
  /** When it was first made: where it sits among the messages. */
  at: string;
  title: string;
  /** Where it stands, in a few words. */
  status: string;
  detail: string;
  /** It waits for the owner: an approval or a review. */
  waiting: boolean;
  /** Setujui on the card starts the same approval as the document's own button. */
  canApprove: boolean;
}

export function resultsOf(project: StudioProject, stack: StudioStack | null | undefined, prd: PrdView | undefined, roadmap: RoadmapView | undefined): Result[] {
  const results: Result[] = [];

  if (stack) {
    const chosen = stack.body.layers.map((layer) => layer.choice.trim()).filter((choice) => choice && choice !== "Tidak perlu");
    results.push({
      kind: "stack",
      at: stack.createdAt,
      title: t.stack,
      status: stack.status === "approved" ? t.approved : t.draft,
      detail: chosen.slice(0, 4).join(" · ") + (chosen.length > 4 ? ` ${t.more(chosen.length - 4)}` : ""),
      waiting: stack.status === "draft",
      // The same rule as the document's button: every layer chosen.
      canApprove: stack.status === "draft" && stack.body.layers.every((layer) => layer.choice.trim()),
    });
  }

  const versions = prd?.versions ?? [];
  const firstPrd = [prd?.draftMeta, ...versions].filter((v) => v !== null && v !== undefined).map((v) => v.createdAt).sort()[0];
  if (prd && firstPrd) {
    const approvedNow = prd.approved !== null && !prd.unsaved;
    const features = prd.draft?.features.length ?? 0;
    const questions = prd.draft?.delivery.questions.length ?? 0;
    results.push({
      kind: "prd",
      at: firstPrd,
      title: approvedNow && prd.approved?.number ? t.prdVersion(prd.approved.number) : t.prd,
      status: approvedNow ? t.approved : t.draft,
      detail: [t.features(features), questions > 0 ? t.questions(questions) : null, prd.problems.length > 0 ? t.problems(prd.problems.length) : null].filter(Boolean).join(" · "),
      waiting: !approvedNow && prd.draft !== null,
      canApprove: !approvedNow && prd.draft !== null && prd.problems.length === 0,
    });
  }

  const plan = roadmap?.draft ?? roadmap?.approved ?? null;
  if (roadmap && plan) {
    const isDraft = roadmap.draft !== null;
    results.push({
      kind: "roadmap",
      // Where the first plan was made, even when it was replaced since.
      at: [plan.meta.createdAt, ...roadmap.superseded.map((meta) => meta.createdAt)].sort()[0] ?? plan.meta.createdAt,
      title: t.roadmap,
      status: isDraft ? t.draft : t.approved,
      detail: [t.checkpoints(plan.checkpoints.length), isDraft && roadmap.problems.length > 0 ? t.problems(roadmap.problems.length) : null].filter(Boolean).join(" · "),
      waiting: isDraft,
      canApprove: isDraft && roadmap.problems.length === 0,
    });
  }

  const approvedPlan = roadmap?.approved ?? null;
  if (project.stage === "build" && approvedPlan) {
    const done = approvedPlan.checkpoints.filter((c) => c.status === "completed").length;
    const active = approvedPlan.checkpoints.find((c) => c.status !== "completed" && c.status !== "planned");
    results.push({
      kind: "build",
      at: approvedPlan.meta.approvedAt ?? approvedPlan.meta.updatedAt,
      title: t.build,
      status: project.waiting > 0 ? t.waitingFor(project.waiting) : active ? t.active(active.code) : t.quiet,
      detail: t.done(done, approvedPlan.checkpoints.length),
      waiting: project.waiting > 0,
      canApprove: false,
    });
  }

  return results;
}

/** The messages and the results in one order: a result after the last message made before it. */
export function interleave<M extends Pick<StudioMessage, "createdAt">>(messages: readonly M[], results: readonly Result[]): Array<{ message: M } | { result: Result }> {
  const out: Array<{ message: M } | { result: Result }> = [];
  const pending = [...results].sort((a, b) => a.at.localeCompare(b.at));
  let next = 0;
  for (const message of messages) {
    for (; next < pending.length && (pending[next]?.at ?? "") < message.createdAt; next++) out.push({ result: pending[next] as Result });
    out.push({ message });
  }
  for (const result of pending.slice(next)) out.push({ result });
  return out;
}
