import { act, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";

import { createMockBackend } from "../../data/mock";
import type { StudioProject } from "../../data/generated/StudioProject";
import { renderApp } from "../../test/renderApp";

const section = () => screen.getByRole("region", { name: "Claude Code" });

describe("Integrasi AI: Claude Code", () => {
  it("shows what is installed, and Pasang lists what changes before changing it", async () => {
    const user = userEvent.setup();
    const { backend } = renderApp({ path: "/pengaturan/ai" });
    const claude = await screen.findByRole("region", { name: "Claude Code" });
    expect(within(claude).getByRole("switch", { name: "Izinkan Claude Code memakai Hub" })).toHaveAttribute("aria-checked", "false");
    expect(within(claude).getByText("Mati. Claude Code tidak bisa membaca atau melaporkan apa pun.")).toBeInTheDocument();
    expect(await within(claude).findByText("Jembatan hub-mcp.exe")).toBeInTheDocument();
    expect(within(claude).getByRole("button", { name: "Uji jembatan" })).toBeDisabled();
    // R5: the three signals, before anything is installed.
    const signals = within(claude).getByRole("group", { name: "Status Claude Code" });
    expect(within(signals).getByText("Belum lengkap")).toBeInTheDocument();
    expect(within(signals).getByText(/Yang belum: pendaftaran di Claude Code, skill \/hub-connect\./)).toBeInTheDocument();
    expect(within(signals).getAllByText("Status belum diketahui")).toHaveLength(2);

    await user.click(within(claude).getByRole("button", { name: "Pasang ke Claude Code" }));
    const sheet = screen.getByRole("dialog", { name: "Pasang ke Claude Code" });
    expect(within(sheet).getByText(/^claude mcp add --scope user hub -- ".*hub-mcp\.exe" --data-dir ".*Hub"$/)).toBeInTheDocument();
    expect(within(sheet).getByText(/skills\\hub-connect ditulis/)).toBeInTheDocument();
    expect(within(sheet).getByText("Claude Code dinyalakan di Hub (saklar di atas).")).toBeInTheDocument();
    // P1: the sheet says it is the CLI and the desktop app's Code tab alike, and how to start each.
    expect(within(sheet).getByText(/dibaca oleh CLI dan oleh tab Code di aplikasi Claude desktop/)).toBeInTheDocument();
    expect(within(sheet).getByText("Terminal (CLI):")).toBeInTheDocument();
    expect(within(sheet).getByText("Aplikasi Claude desktop:")).toBeInTheDocument();
    expect(within(sheet).getByText(/Jendela chat biasa di aplikasi Claude tidak bisa memakai Hub/)).toBeInTheDocument();
    expect(backend.calls.some((c) => c.startsWith("install_claude_code"))).toBe(false);
    await user.click(within(sheet).getByRole("button", { name: "Pasang" }));

    expect(await within(section()).findByText("Terpasang. Jalankan ulang Claude Code di terminal, atau buka sesi baru di tab Code.")).toBeInTheDocument();
    expect(within(section()).getByRole("switch", { name: "Izinkan Claude Code memakai Hub" })).toHaveAttribute("aria-checked", "true");
    expect(await within(section()).findByText("Ya")).toBeInTheDocument();
    expect(within(section()).getByText("Terpasang")).toBeInTheDocument();
    expect(within(within(section()).getByRole("group", { name: "Status Claude Code" })).getByText("Siap")).toBeInTheDocument();

    // Uji jembatan, step by step, each with its word.
    await user.click(within(section()).getByRole("button", { name: "Uji jembatan" }));
    const steps = await within(section()).findByRole("list", { name: "Uji jembatan" });
    expect(steps).toHaveTextContent("Jembatan berjalan: berhasil");
    expect(steps).toHaveTextContent("Token Claude Code diterima: berhasil");

    // Lepas asks once.
    await user.click(within(section()).getByRole("button", { name: "Lepas" }));
    await user.click(within(screen.getByRole("group", { name: /Lepas Hub dari Claude Code\?/ })).getByRole("button", { name: "Lepas" }));
    expect(await within(section()).findByText("Hub dilepas dari Claude Code.")).toBeInTheDocument();
  });

  it("shows every Claude Code session and the last call", async () => {
    const backend = createMockBackend({ claudeCli: false, settings: { claudeEnabled: true } });
    renderApp({ path: "/pengaturan/ai", backend });
    const signals = await screen.findByRole("group", { name: "Status Claude Code" });
    // Without the claude command the registration cannot be checked, so only the skill is named.
    expect(await within(signals).findByText("Yang belum: skill /hub-connect. Atur di Pengaturan > Integrasi AI > Claude Code.")).toBeInTheDocument();
    const at = new Date(Date.now() - 60_000).toISOString();
    const open = { state: "open" as const, folder: "D:\\repo\\kasir", client: "claude-code", clientVersion: "2.1.317", startedAt: at, seenAt: at, endedAt: null, calls: 1 };
    act(() => backend.setClaudeSignals({ session: open, open: 2, activity: { tool: "hub_status", at, ok: true, reason: null } }));
    expect(await within(signals).findByText("Terbuka sekarang, 2 sesi")).toBeInTheDocument();
    expect(within(signals).getByText(/^Mengecek Hub, /)).toBeInTheDocument();
    expect(within(signals).getByText("hub_status")).toBeInTheDocument();
  });

  it("without the claude command Hub still writes the skill, and the sheet gives the command to run by hand", async () => {
    const user = userEvent.setup();
    const { backend } = renderApp({ path: "/pengaturan/ai", backend: createMockBackend({ claudeCli: false }) });
    const claude = await screen.findByRole("region", { name: "Claude Code" });
    expect(await within(claude).findByText("Tidak diketahui (perintah claude tidak ada)")).toBeInTheDocument();
    await user.click(within(claude).getByRole("button", { name: "Pasang ke Claude Code" }));
    const sheet = screen.getByRole("dialog", { name: "Pasang ke Claude Code" });
    expect(within(sheet).getByText(/Hub tidak menemukan perintah claude/)).toBeInTheDocument();
    expect(within(sheet).queryByRole("button", { name: "Pasang" })).not.toBeInTheDocument();
    await user.click(within(sheet).getByRole("button", { name: "Salin" }));
    expect(await navigator.clipboard.readText()).toMatch(/^claude mcp add --scope user hub -- /);

    // U1: the skill needs no claude command, so Hub writes it here too; the sheet stays on the command.
    await user.click(within(sheet).getByRole("button", { name: "Tulis skill" }));
    expect(await within(sheet).findByText(/^Skill ditulis dan Claude Code dinyalakan\./)).toBeInTheDocument();
    expect(backend.calls.some((c) => c.startsWith("install_claude_code"))).toBe(true);
    expect(within(sheet).getByText(/^claude mcp add --scope user hub -- /)).toBeInTheDocument();
    expect(await within(sheet).findByRole("button", { name: "Tulis skill" })).toBeDisabled();
    expect(within(claude).getByText("Terpasang")).toBeInTheDocument();
    expect(within(claude).getByRole("switch", { name: "Izinkan Claude Code memakai Hub" })).toHaveAttribute("aria-checked", "true");
  });

  it("a bridge registered by hand without the skill says /hub-connect is missing, and Pasang completes it", async () => {
    const user = userEvent.setup();
    renderApp({ path: "/pengaturan/ai", backend: createMockBackend({ claudeRegistered: true }) });
    const claude = await screen.findByRole("region", { name: "Claude Code" });
    expect(await within(claude).findByText(/skill-nya belum ada, jadi \/hub-connect tidak muncul/)).toBeInTheDocument();
    await user.click(within(claude).getByRole("button", { name: "Lengkapi pemasangan" }));
    await user.click(within(screen.getByRole("dialog", { name: "Pasang ke Claude Code" })).getByRole("button", { name: "Pasang" }));
    expect(await within(section()).findByText("Terpasang. Jalankan ulang Claude Code di terminal, atau buka sesi baru di tab Code.")).toBeInTheDocument();
    expect(within(section()).queryByText(/skill-nya belum ada/)).not.toBeInTheDocument();
    expect(within(section()).getByRole("button", { name: "Pasang ulang" })).toBeInTheDocument();
  });
});

describe("Studio waits for the owner", () => {
  it("the sidebar counts what Claude left waiting", async () => {
    const project: StudioProject = {
      id: "00000000-0000-7000-8000-0000000000aa",
      name: "Absensi",
      idea: "",
      stage: "build",
      projectId: null,
      projectName: null,
      repoPath: "C:\\repo",
      claudeAccess: "report",
      stackStatus: null,
      waiting: 2,
      createdAt: "2026-11-08T01:00:00Z",
      updatedAt: "2026-11-08T01:00:00Z",
    };
    renderApp({ path: "/", backend: createMockBackend({ studioProjects: [project] }) });
    // On Studio's entry among Hub's pages (U2: Studio's history, hidden here, counts it too).
    await screen.findByRole("complementary", { name: "Navigasi" });
    const pages = document.querySelector<HTMLElement>("[data-pane=main]")!;
    expect(await within(pages).findByLabelText("2 hal menunggu kamu")).toHaveTextContent("2");
  });

  it("Jejak has a Studio group", async () => {
    renderApp({ path: "/jejak" });
    expect(await screen.findByRole("button", { name: "Studio" })).toHaveAttribute("aria-pressed", "false");
  });
});
