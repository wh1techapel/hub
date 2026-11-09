import { screen, waitFor, within } from "@testing-library/react";
import type userEvent from "@testing-library/user-event";
import { expect } from "vitest";

import { createMockBackend } from "../data/mock";
import { renderApp } from "./renderApp";

export const STUDIO_KEY = "AIzaTestKeyForHubStudio0123456789abcd";

type User = ReturnType<typeof userEvent.setup>;

/** A Studio project with its PRD approved and a roadmap written, not yet approved; the Roadmap tab
 * open. U4b: approving the PRD writes the plan at once. */
export async function withDraftRoadmap(user: User) {
  const backend = createMockBackend({ geminiKey: STUDIO_KEY });
  const app = renderApp({ path: "/studio", backend });
  await user.type(await screen.findByLabelText("Ceritakan idenya"), "Absensi lab pakai kartu{Enter}");
  await waitFor(() => expect(screen.getByRole("button", { name: "Kirim" })).toBeInTheDocument(), { timeout: 3000 });
  await approveStackAndWritePrd(user);
  await screen.findByText("PRD tersusun.", undefined, { timeout: 3000 });
  await approveDocument(user);
  await user.click(within(screen.getByRole("group", { name: /pertanyaan terbuka/ })).getByRole("button", { name: "Setujui" }));
  await screen.findByText("Roadmap tersusun.", undefined, { timeout: 3000 });
  await openStudioDoc(user, "Bangun");
  expect(screen.getByText("Pantau mulai setelah roadmap disetujui di tab Roadmap.")).toBeInTheDocument();
  await openStudioDoc(user, "Rencana checkpoint");
  return { ...app, backend };
}

/** Setujui in the document open in the canvas (U4b: the result cards in the conversation have
 * their own Setujui). */
export async function approveDocument(user: User) {
  await user.click(within(screen.getByRole("tabpanel")).getByRole("button", { name: "Setujui" }));
}

/** A Studio project with its PRD and roadmap approved (new Hub Proyek, the demo folder), Pantau open. */
export async function building(user: User) {
  const app = await withDraftRoadmap(user);
  await user.click(screen.getByRole("button", { name: "Setujui roadmap" }));
  const dialog = screen.getByRole("dialog", { name: "Setujui roadmap" });
  await user.click(within(dialog).getByRole("button", { name: "Pilih folder…" }));
  await within(dialog).findByText("Repo git ditemukan.");
  await user.click(within(dialog).getByRole("radio", { name: /Baca dan lapor/ }));
  await user.click(within(dialog).getByRole("button", { name: "Setujui" }));
  await screen.findByText("Roadmap disetujui. Proyek pindah ke tahap Bangun.");
  await openStudioDoc(user, "Bangun");
  return app;
}

/** Opens a section of stage 3 (PRD, Rencana checkpoint, Bangun), first opening the stage itself. */
export async function openStudioDoc(user: User, name: "PRD" | "Rencana checkpoint" | "Bangun") {
  const stage = screen.getByRole("tab", { name: /PRD dan bangun/ });
  if (stage.getAttribute("aria-selected") !== "true") await user.click(stage);
  await user.click(screen.getByRole("tab", { name }));
}

/** The stage tab the project is at (aria-current="step"). */
export function currentStudioStage(): HTMLElement {
  const stages = screen.getByRole("tablist", { name: "Tahap" });
  const current = within(stages)
    .getAllByRole("tab")
    .find((tab) => tab.getAttribute("aria-current") === "step");
  if (!current) throw new Error("no current stage");
  return current;
}

/** Stage 2 the quick way (R3): Gemini's recommendation, approved, and the PRD written from it. */
export async function approveStackAndWritePrd(user: User) {
  await user.click(screen.getByRole("tab", { name: /Tech stack/ }));
  await user.click(screen.getByRole("button", { name: /Rekomendasikan dengan Lentera/ }));
  await screen.findByText(/Rekomendasi tersusun/, undefined, { timeout: 3000 });
  await user.click(screen.getByRole("button", { name: "Setujui tech stack" }));
  await user.click(within(screen.getByRole("dialog", { name: "Setujui tech stack" })).getByRole("button", { name: "Setujui dan susun PRD" }));
}
