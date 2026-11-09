// The three stages the owner sees over the five stored ones (docs/studio.md, "Revision: three
// stages"). Stage 2, the tech stack, is its own record (R3): a project in stage 1 with a stack is in
// stage 2.

import type { StudioProject } from "../../data/generated/StudioProject";
import type { StudioStage } from "../../data/generated/StudioStage";
import { id } from "../../strings/id";

export type Step = 1 | 2 | 3;

export const STEP_OF: Record<StudioStage, Step> = { explore: 1, clarify: 1, prd: 3, roadmap: 3, build: 3 };

export const STEPS: { step: Step; key: "idea" | "stack" | "build" }[] = [
  { step: 1, key: "idea" },
  { step: 2, key: "stack" },
  { step: 3, key: "build" },
];

/** Where a project is: stage 3 once it reached the PRD, stage 2 once it has a stack (R3), else 1. */
export function progressOf(project: Pick<StudioProject, "stage" | "stackStatus">): Step {
  if (STEP_OF[project.stage] === 3) return 3;
  return project.stackStatus ? 2 : 1;
}

/** "Tahap 2: Tech stack" for a project. */
export function stepLabel(project: Pick<StudioProject, "stage" | "stackStatus">): string {
  const step = progressOf(project);
  const key = STEPS[step - 1]?.key ?? "idea";
  return id.studio.stepOf(step, id.studio.steps[key] ?? key);
}
