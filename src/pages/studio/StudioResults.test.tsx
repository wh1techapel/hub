import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";

import { createMockBackend } from "../../data/mock";
import type { StudioStack } from "../../data/generated/StudioStack";
import { renderApp } from "../../test/renderApp";
import { STUDIO_KEY } from "../../test/studioFlow";
import { interleave, resultsOf } from "./results";
import type { Result } from "./results";

const result = (kind: Result["kind"], at: string): Result => ({ kind, at, title: kind, status: "", detail: "", waiting: false, canApprove: false });
const card = (name: string) => screen.getByRole("article", { name: `Hasil: ${name}` });

// U4b: the results as cards in the conversation.
describe("Studio: result cards", () => {
  it("a result sits after the messages made before it, in the order they were made", () => {
    const messages = [{ id: "a", createdAt: "2026-11-09T01:00:00Z" }, { id: "b", createdAt: "2026-11-09T03:00:00Z" }];
    const order = interleave(messages, [result("prd", "2026-11-09T04:00:00Z"), result("stack", "2026-11-09T02:00:00Z")]).map((entry) =>
      "message" in entry ? entry.message.id : entry.result.kind,
    );
    expect(order).toEqual(["a", "stack", "b", "prd"]);
    expect(interleave([], [result("stack", "2026-11-09T02:00:00Z")]).length).toBe(1);
  });

  it("Buka opens the document; Setujui opens the document's own approval, never approving by itself", async () => {
    const user = userEvent.setup();
    const { backend } = renderApp({ path: "/studio", backend: createMockBackend({ geminiKey: STUDIO_KEY }) });
    await user.type(await screen.findByLabelText("Ceritakan idenya"), "Absensi lab pakai kartu{Enter}");
    await waitFor(() => expect(screen.getByRole("button", { name: "Kirim" })).toBeInTheDocument(), { timeout: 3000 });
    await user.click(screen.getByRole("tab", { name: /Tech stack/ }));
    await user.click(screen.getByRole("button", { name: /Rekomendasikan dengan Lentera/ }));
    await screen.findByText(/Rekomendasi tersusun/, undefined, { timeout: 3000 });

    // The stack's card is in the conversation, waiting for the owner.
    const stack = card("Tech stack");
    expect(within(screen.getByRole("log")).getByRole("article", { name: "Hasil: Tech stack" })).toBe(stack);
    expect(within(stack).getByText("Draf, menunggu persetujuanmu")).toBeInTheDocument();

    // Buka from Ringkasan brings the stack back into the canvas.
    await user.click(screen.getByRole("tab", { name: /Mau buat apa/ }));
    await user.click(within(card("Tech stack")).getByRole("button", { name: "Buka" }));
    expect(screen.getByRole("tab", { name: /Tech stack/ })).toHaveAttribute("aria-selected", "true");

    // Setujui from Ringkasan opens the stack's own sheet; nothing is approved yet.
    await user.click(screen.getByRole("tab", { name: /Mau buat apa/ }));
    await user.click(within(card("Tech stack")).getByRole("button", { name: "Setujui" }));
    const sheet = await screen.findByRole("dialog", { name: "Setujui tech stack" });
    expect(backend.calls.some((call) => call.startsWith("approve_studio_stack"))).toBe(false);
    await user.click(within(sheet).getByRole("button", { name: "Setujui dan susun PRD" }));
    await screen.findByText("PRD tersusun.", undefined, { timeout: 3000 });
    expect(within(card("Tech stack")).getByText("Disetujui")).toBeInTheDocument();

    // The PRD's card: Setujui asks about its open questions first, as the document's button does.
    const prd = card("PRD");
    expect(within(prd).getByText(/fitur/)).toBeInTheDocument();
    await user.click(within(prd).getByRole("button", { name: "Setujui" }));
    expect(await screen.findByRole("group", { name: /pertanyaan terbuka/ })).toBeInTheDocument();
    expect(backend.calls.some((call) => call.startsWith("approve_prd"))).toBe(false);
  }, 15_000);

  it("a stack with a layer not chosen yet has no Setujui on its card", () => {
    const stack: StudioStack = {
      mode: "own",
      status: "draft",
      body: { layers: [{ layer: "frontend", choice: "", why: "", fit: "", ease: "", cost: "", alternatives: [], by: "owner" }], summary: "" },
      approvedAt: null,
      createdAt: "2026-11-09T01:00:00Z",
      updatedAt: "2026-11-09T01:00:00Z",
    };
    const project = { id: "p", name: "P", idea: "", stage: "clarify", projectId: null, projectName: null, repoPath: null, claudeAccess: "none", stackStatus: "draft", waiting: 0, createdAt: "", updatedAt: "" } as const;
    const [only] = resultsOf(project, stack, undefined, undefined);
    expect(only?.waiting).toBe(true);
    expect(only?.canApprove).toBe(false);
  });
});
