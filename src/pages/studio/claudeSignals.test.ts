import { describe, expect, it } from "vitest";

import type { ClaudeCodeStatus } from "../../data/api";
import type { ClaudeSignals } from "../../data/generated/ClaudeSignals";
import { activitySignal, bridgeSignal, sessionSignal } from "./claudeSignals";

const now = new Date("2026-11-08T10:00:00+07:00");
const installed: ClaudeCodeStatus = {
  enabled: true,
  bridge: "C:\\Hub\\hub-mcp.exe",
  cli: "C:\\Users\\kamu\\.local\\bin\\claude.exe",
  registration: "ours",
  skill: "ours",
  skillPath: "",
  dataDir: "",
  commands: [],
};
const none: ClaudeSignals = { session: null, open: 0, activity: null };
const session = { folder: "D:\\repo", client: "claude-code", clientVersion: "2.1.317", startedAt: "2026-11-08T02:30:00Z", seenAt: "2026-11-08T02:59:30Z", endedAt: null, calls: 3 };

describe("Claude Code's three signals", () => {
  it("the bridge: missing parts first, then the switch, then what cannot be checked", () => {
    expect(bridgeSignal(undefined, none).line).toBe("Memeriksa...");
    expect(bridgeSignal(installed, none)).toMatchObject({ tone: "ok", line: "Siap" });
    expect(bridgeSignal({ ...installed, registration: "other", skill: "missing" }, none)).toMatchObject({
      tone: "problem",
      line: "Belum lengkap",
      detail: "Yang belum: pendaftaran di Claude Code, skill /hub-connect. Atur di Pengaturan > Integrasi AI > Claude Code.",
    });
    expect(bridgeSignal({ ...installed, enabled: false }, none)).toMatchObject({ tone: "problem", line: "Dimatikan di Hub" });
    // Without the claude command: unknown, unless Hub has heard from a session through the bridge.
    const noCli = { ...installed, cli: null, registration: "unknown" as const };
    expect(bridgeSignal(noCli, none)).toMatchObject({ tone: "unknown", line: "Status belum diketahui" });
    expect(bridgeSignal(noCli, { ...none, session: { ...session, state: "closed", endedAt: "2026-11-08T01:00:00Z" } })).toMatchObject({ tone: "ok", line: "Siap" });
  });

  it("the session: open, closed, or gone quiet, never a guess", () => {
    expect(sessionSignal(undefined, now, true).line).toBe("Memeriksa...");
    expect(sessionSignal(none, now, true)).toMatchObject({ tone: "unknown", line: "Status belum diketahui", detail: "Hub belum pernah mendengar kabar dari Claude Code di folder repo proyek ini." });
    expect(sessionSignal(none, now, false).detail).toBe("Hub belum pernah mendengar kabar dari Claude Code.");
    expect(sessionSignal({ ...none, open: 1, session: { ...session, state: "open" } }, now, true)).toEqual({
      tone: "ok",
      line: "Terbuka sekarang",
      detail: "Sejak 09.30 di D:\\repo (Claude Code 2.1.317).",
    });
    expect(sessionSignal({ ...none, open: 3, session: { ...session, state: "open", client: "", clientVersion: "" } }, now, false)).toMatchObject({
      line: "Terbuka sekarang, 3 sesi",
      detail: "Sejak 09.30 di D:\\repo.",
    });
    expect(sessionSignal({ ...none, session: { ...session, state: "closed", endedAt: "2026-11-08T02:45:00Z" } }, now, true)).toMatchObject({ tone: "past", line: "Ditutup 09.45" });
    expect(sessionSignal({ ...none, session: { ...session, state: "quiet" } }, now, true)).toMatchObject({ tone: "problem", line: "Tidak ada kabar sejak 09.59" });
  });

  it("the last call: what Claude did, and why it failed", () => {
    expect(activitySignal(none, now, true)).toMatchObject({ tone: "unknown", detail: "Belum ada panggilan dari Claude Code untuk proyek ini." });
    expect(activitySignal({ ...none, activity: { tool: "hub_get_prd", at: "2026-11-08T02:58:00Z", ok: true, reason: null } }, now, true)).toEqual({
      tone: "ok",
      line: "Membaca PRD, 09.58",
      detail: "",
      code: "hub_get_prd",
    });
    const failed = (reason: string | null) => activitySignal({ ...none, activity: { tool: "hub_connect", at: "2026-11-08T02:58:00Z", ok: false, reason } }, now, true);
    expect(failed("access")).toMatchObject({ tone: "problem", line: "Menyambung ke proyek gagal, 09.58", detail: "Akses Claude untuk proyek ini tidak cukup." });
    expect(failed("rate").detail).toBe("Alasan: rate.");
    expect(failed(null).detail).toBe("Tanpa alasan dari Hub.");
  });
});
