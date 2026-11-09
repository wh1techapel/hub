import { act, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";

import { createMockBackend } from "../../data/mock";
import { a11yProblems } from "../../test/a11y";
import { renderApp } from "../../test/renderApp";
import { building, openStudioDoc, STUDIO_KEY, withDraftRoadmap } from "../../test/studioFlow";

// S6: the accessibility pass on Studio's pages in each state they show (docs/studio.md, section 9).

const now = () => screen.getByRole("region", { name: "Sekarang" });

describe("Studio: accessibility", { timeout: 15_000 }, () => {
  it("the Studio list and a new project's conversation", async () => {
    const user = userEvent.setup();
    renderApp({ path: "/studio", backend: createMockBackend({ geminiKey: STUDIO_KEY }) });
    await screen.findByLabelText("Ceritakan idenya");
    expect(a11yProblems()).toEqual([]);
    // L2: the model menu, open with its list read.
    await user.click(screen.getByRole("button", { name: /^Model: / }));
    await waitFor(() => expect(within(screen.getByRole("menu", { name: "Model" })).getAllByRole("menuitemradio")).toHaveLength(5));
    expect(a11yProblems()).toEqual([]);
    await user.keyboard("{Escape}");
    await user.type(screen.getByLabelText("Ceritakan idenya"), "Absensi lab pakai kartu{Enter}");
    await screen.findByRole("button", { name: "Kirim" }, { timeout: 3000 });
    expect(a11yProblems()).toEqual([]);
  });

  it("a roadmap draft: its rows, details, the editor, and the approval sheet", async () => {
    const user = userEvent.setup();
    await withDraftRoadmap(user);
    expect(a11yProblems()).toEqual([]);
    await user.click(screen.getAllByRole("button", { name: "Rincian" })[0]!);
    expect(a11yProblems()).toEqual([]);
    await user.click(screen.getAllByRole("button", { name: "Ubah" })[0]!);
    expect(a11yProblems()).toEqual([]);
    await user.click(screen.getByRole("button", { name: "Setujui roadmap" }));
    screen.getByRole("dialog", { name: "Setujui roadmap" });
    expect(a11yProblems()).toEqual([]);
  });

  it("Roadmap approved, and Pantau through a report and its review", async () => {
    const user = userEvent.setup();
    await building(user);
    expect(a11yProblems()).toEqual([]);

    await openStudioDoc(user, "Rencana checkpoint");
    expect(a11yProblems()).toEqual([]);
    await openStudioDoc(user, "Bangun");

    await user.click(within(now()).getByRole("button", { name: "Mulai CP-01" }));
    await user.click(await within(now()).findByRole("button", { name: "Buat prompt implementasi" }));
    await within(now()).findByLabelText("Prompt untuk Claude Code");
    expect(a11yProblems()).toEqual([]);

    await user.click(within(now()).getByRole("button", { name: "Kerjakan sendiri" }));
    await user.click(await within(now()).findByRole("button", { name: "Tulis laporan" }));
    const form = screen.getByRole("form", { name: "Laporan checkpoint" });
    await user.click(within(form).getByRole("button", { name: "Kirim laporan" }));
    await within(form).findByRole("alert");
    expect(a11yProblems()).toEqual([]);

    await user.type(within(form).getByLabelText("Apa yang selesai"), "Absen dengan kartu jalan di lab.");
    await user.click(within(form).getByRole("button", { name: "Kirim laporan" }));
    await within(now()).findByRole("button", { name: "Setujui checkpoint" });
    expect(a11yProblems()).toEqual([]);

    await user.click(within(now()).getByRole("button", { name: "Minta perubahan" }));
    await within(now()).findByLabelText(/Apa yang perlu diubah/);
    expect(a11yProblems()).toEqual([]);
  });

  it("Pengaturan > Claude Code and the Pasang sheet", async () => {
    const user = userEvent.setup();
    renderApp({ path: "/pengaturan/ai" });
    const claude = await screen.findByRole("region", { name: "Claude Code" });
    await within(claude).findByText("Jembatan hub-mcp.exe");
    expect(a11yProblems()).toEqual([]);
    await user.click(within(claude).getByRole("button", { name: "Pasang ke Claude Code" }));
    screen.getByRole("dialog", { name: "Pasang ke Claude Code" });
    expect(a11yProblems()).toEqual([]);
  });
});

// R7: the same pass on what the revision (R1 to R6) added.
describe("Studio revision: accessibility", { timeout: 20_000 }, () => {
  it("stage 1 to 3: the question set, Yang sudah jelas, the check-answers sheet, the stack, and the PRD", async () => {
    const user = userEvent.setup();
    renderApp({ path: "/studio", backend: createMockBackend({ geminiKey: STUDIO_KEY }) });
    await user.type(await screen.findByLabelText("Ceritakan idenya"), "Absensi lab pakai kartu{Enter}");
    await screen.findByRole("region", { name: "Pertanyaan dari Lentera" }, { timeout: 3000 });
    screen.getByRole("region", { name: "Yang sudah jelas" });
    expect(a11yProblems()).toEqual([]);
    await user.click(within(screen.getByRole("region", { name: "Pertanyaan dari Lentera" })).getByRole("radio", { name: /Siswa/ }));
    await user.click(screen.getByRole("button", { name: "Kirim jawaban" }));
    await user.click(await screen.findByRole("button", { name: "Cek ringkasan dan lanjut" }, { timeout: 3000 }));
    screen.getByRole("dialog", { name: "Cek ringkasan sebelum lanjut" });
    expect(a11yProblems()).toEqual([]);

    await user.click(within(screen.getByRole("dialog", { name: "Cek ringkasan sebelum lanjut" })).getByRole("button", { name: "Lanjut ke tech stack" }));
    await screen.findByRole("heading", { name: /Tech stack/, level: 2 });
    expect(a11yProblems()).toEqual([]);
    await user.click(screen.getByRole("button", { name: /Rekomendasikan dengan Lentera/ }));
    await screen.findByText(/Rekomendasi tersusun/, undefined, { timeout: 3000 });
    await user.click(screen.getAllByText("Alasan dan biaya")[0]!);
    expect(a11yProblems()).toEqual([]);
    await user.click(screen.getByRole("button", { name: "Setujui tech stack" }));
    screen.getByRole("dialog", { name: "Setujui tech stack" });
    expect(a11yProblems()).toEqual([]);

    await user.click(within(screen.getByRole("dialog", { name: "Setujui tech stack" })).getByRole("button", { name: "Setujui dan susun PRD" }));
    await screen.findByText("PRD tersusun.", undefined, { timeout: 3000 });
    screen.getByText(/dari tech stack yang disetujui/);
    expect(a11yProblems()).toEqual([]);
  });

  it("Bangun: the Mandiri prompt and Claude Code's three signals", async () => {
    const user = userEvent.setup();
    const { backend } = await building(user);
    const at = new Date(Date.now() - 60_000).toISOString();
    act(() =>
      backend.setClaudeSignals({
        session: { state: "open", folder: "C:\\Users\\kamu\\Projects\\absensi", client: "claude-code", clientVersion: "2.1.317", startedAt: at, seenAt: at, endedAt: null, calls: 2 },
        open: 1,
        activity: { tool: "hub_connect", at, ok: false, reason: "folder" },
      }),
    );
    await within(now()).findByText("Terbuka sekarang");
    expect(a11yProblems()).toEqual([]);
    await user.click(within(now()).getByRole("button", { name: "Mulai CP-01" }));
    await user.click(await within(now()).findByRole("button", { name: "Buat prompt implementasi" }));
    await user.click(within(within(now()).getByRole("radiogroup", { name: "Versi prompt" })).getByRole("radio", { name: "Mandiri" }));
    await waitFor(() => expect(within(now()).getByLabelText<HTMLTextAreaElement>("Prompt untuk Claude Code").value).toContain("tanpa Hub dan tanpa MCP"));
    expect(a11yProblems()).toEqual([]);
  });

  it("Pengaturan: Claude Code's signals and the developer profile", async () => {
    renderApp({ path: "/pengaturan/ai" });
    await screen.findByRole("group", { name: "Status Claude Code" });
    await screen.findByText("Profil developer");
    expect(a11yProblems()).toEqual([]);
  });
});

// R7: every page of the sidebar (R6), and the views that moved, pass on their own.
describe("Every page: accessibility", () => {
  it.each(["/", "/kalender", "/kalender?view=pengingat", "/proyek", "/proyek?view=inbox", "/proyek?view=upcoming", "/studio", "/belajar", "/file", "/jejak", "/pengaturan/tampilan", "/pengaturan/hari", "/pengaturan/ai", "/pengaturan/ultron", "/pengaturan/sistem"])(
    "%s",
    async (path) => {
      renderApp({ path });
      await screen.findByRole("heading", { level: 1 });
      // Let the page's own queries land before reading it.
      await waitFor(() => expect(document.querySelectorAll("main [aria-busy=true]").length).toBe(0));
      expect(a11yProblems()).toEqual([]);
    },
  );
});
