import { act, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";

import { building } from "../../test/studioFlow";

const now = () => screen.getByRole("region", { name: "Sekarang" });
const waiting = () => screen.getByRole("region", { name: "Menunggu kamu" });

// Whole journeys through Pantau: over 5 s when every test file runs at once.
describe("Studio: Pantau", { timeout: 15_000 }, () => {
  it("starts the first checkpoint and fills the implementation prompt", async () => {
    const user = userEvent.setup();
    await building(user);
    expect(screen.getByRole("heading", { name: /Pantau/ })).toHaveTextContent("0 dari 2 checkpoint selesai");
    expect(within(now()).getByText("CP-01")).toBeInTheDocument();
    expect(within(now()).getByText("Direncanakan")).toBeInTheDocument();
    // R5: nothing heard from Claude Code yet, said as such, with how to connect.
    const claude = within(now()).getByRole("group", { name: "Claude Code" });
    const signals = within(claude).getByRole("group", { name: "Status Claude Code" });
    expect(within(signals).getAllByText("Status belum diketahui")).toHaveLength(2);
    expect(within(signals).getByText("Hub belum pernah mendengar kabar dari Claude Code di folder repo proyek ini.")).toBeInTheDocument();
    expect(within(claude).getByText("Akses Claude untuk proyek ini: Baca dan lapor.")).toBeInTheDocument();
    expect(within(claude).getByText(/Untuk menyambungkan: pasang Hub ke Claude Code/)).toBeInTheDocument();

    await user.click(within(now()).getByRole("button", { name: "Mulai CP-01" }));
    expect(await within(now()).findByText("Siap")).toBeInTheDocument();

    await user.click(within(now()).getByRole("button", { name: "Buat prompt implementasi" }));
    const prompt = await within(now()).findByLabelText<HTMLTextAreaElement>("Prompt untuk Claude Code");
    expect(prompt.value).toMatch(/^Kerjakan proyek Hub Studio "Absensi lab pakai kartu" \(id [\w-]+\) di folder C:\\Users\\kamu\\Projects\\absensi\./);
    expect(prompt.value).toContain("1. Jalankan /hub-connect");
    expect(prompt.value).toContain("Kerjakan hanya CP-01.");
    await user.click(within(now()).getByRole("button", { name: "Salin" }));
    expect(await within(now()).findByText("Tersalin.")).toBeInTheDocument();
    expect(await navigator.clipboard.readText()).toBe(prompt.value);

    // R4: "Mandiri", the whole of it in one text for working without the bridge.
    const kinds = within(now()).getByRole("radiogroup", { name: "Versi prompt" });
    expect(within(kinds).getByRole("radio", { name: "Dengan Hub" })).toBeChecked();
    await user.click(within(kinds).getByRole("radio", { name: "Mandiri" }));
    await waitFor(() => expect(within(now()).getByLabelText<HTMLTextAreaElement>("Prompt untuk Claude Code").value).toContain("tanpa Hub dan tanpa MCP"));
    const standalone = within(now()).getByLabelText<HTMLTextAreaElement>("Prompt untuk Claude Code").value;
    expect(standalone).toContain("## Checkpoint CP-01");
    expect(standalone).not.toContain("/hub-connect");
    expect(within(now()).getByText(/laporannya kamu catat sendiri di sini/)).toBeInTheDocument();
  });

  it("says what Claude Code is doing from what the bridge reported, and follows it", async () => {
    const user = userEvent.setup();
    const { backend } = await building(user);
    const claude = () => within(now()).getByRole("group", { name: "Claude Code" });
    const at = (minutesAgo: number) => new Date(Date.now() - minutesAgo * 60_000).toISOString();
    const session = { folder: "C:\\Users\\kamu\\Projects\\absensi", client: "claude-code", clientVersion: "2.1.317", startedAt: at(20), seenAt: at(0), endedAt: null, calls: 4 };
    act(() => backend.setClaudeSignals({ session: { ...session, state: "open" }, open: 1, activity: { tool: "hub_get_checkpoint", at: at(1), ok: true, reason: null } }));
    expect(await within(claude()).findByText("Terbuka sekarang")).toBeInTheDocument();
    expect(within(claude()).getByText(/^Sejak .+ di C:\\Users\\kamu\\Projects\\absensi \(Claude Code 2\.1\.317\)\.$/)).toBeInTheDocument();
    expect(within(claude()).getByText(/^Membaca checkpoint, /)).toBeInTheDocument();
    expect(within(claude()).getByText("hub_get_checkpoint")).toBeInTheDocument();
    expect(within(claude()).queryByText(/Untuk menyambungkan/)).not.toBeInTheDocument();

    // A refused connection says why; a session gone without a goodbye says so.
    act(() =>
      backend.setClaudeSignals({ session: { ...session, state: "quiet", seenAt: at(5) }, open: 0, activity: { tool: "hub_connect", at: at(5), ok: false, reason: "folder" } }),
    );
    expect(await within(claude()).findByText(/^Tidak ada kabar sejak /)).toBeInTheDocument();
    expect(within(claude()).getByText(/^Menyambung ke proyek gagal, /)).toBeInTheDocument();
    expect(within(claude()).getByText("Claude Code tidak dibuka di folder repo proyek ini.")).toBeInTheDocument();

    act(() => backend.setClaudeSignals({ session: { ...session, state: "closed", endedAt: at(2) }, open: 0, activity: { tool: "hub_submit_report", at: at(3), ok: true, reason: null } }));
    expect(await within(claude()).findByText(/^Ditutup /)).toBeInTheDocument();
    expect(within(claude()).getByText(/^Mengirim laporan, /)).toBeInTheDocument();
    // Only the signals were asked again: a session's notice does not refresh the rest of Studio.
    expect(backend.calls.filter((c) => c.startsWith("claude_signals")).length).toBeGreaterThan(1);
  });

  it("the owner builds a checkpoint alone: tasks, a checked report, changes, approval", async () => {
    const user = userEvent.setup();
    await building(user);
    await user.click(within(now()).getByRole("button", { name: "Mulai CP-01" }));
    await user.click(await within(now()).findByRole("button", { name: "Kerjakan sendiri" }));
    expect(await within(now()).findByText("Dikerjakan")).toBeInTheDocument();

    await user.click(within(now()).getByRole("checkbox", { name: /Tandai selesai: Bangun absen dengan kartu/ }));
    expect(await within(now()).findByText("Tugas: 1 dari 2 ditandai selesai")).toBeInTheDocument();

    // The report is checked before it is sent: a summary, and a reason for a test not run.
    await user.click(within(now()).getByRole("button", { name: "Tulis laporan" }));
    const form = screen.getByRole("form", { name: "Laporan checkpoint" });
    await user.click(within(form).getByRole("radio", { name: "Tidak dijalankan" }));
    await user.click(within(form).getByRole("button", { name: "Kirim laporan" }));
    const problems = await within(form).findByRole("alert");
    expect(problems).toHaveTextContent("wajib diisi");
    expect(problems).toHaveTextContent("jelaskan kenapa Uji FEAT-001 tidak lulus");
    await user.type(within(form).getByLabelText("Apa yang selesai"), "Absen dengan kartu jalan di lab.");
    await user.type(within(form).getByLabelText(/Yang belum terverifikasi/), "Pembaca kartu belum datang.");
    await user.click(within(form).getByRole("button", { name: "Kirim laporan" }));
    expect(await within(now()).findByText("Ditinjau")).toBeInTheDocument();

    // It waits for the owner, and the report shows the test not run first.
    expect(within(waiting()).getByText("Laporan CP-01 menunggu tinjauanmu.")).toBeInTheDocument();
    const reports = screen.getByRole("region", { name: /Laporan CP-01/ });
    expect(within(reports).getByText("Absen dengan kartu jalan di lab.")).toBeInTheDocument();
    expect(within(reports).getByText("Tidak dijalankan")).toBeInTheDocument();
    expect(within(reports).getByText("Pembaca kartu belum datang.")).toBeInTheDocument();

    await user.click(within(now()).getByRole("button", { name: "Minta perubahan" }));
    await user.type(within(now()).getByLabelText(/Apa yang perlu diubah/), "Jalankan ujinya dengan pembaca pinjaman.");
    await user.click(within(now()).getByRole("button", { name: "Kirim permintaan" }));
    expect(await within(now()).findByText("Dikerjakan")).toBeInTheDocument();
    expect(within(waiting()).getByText("Tidak ada yang menunggu kamu.")).toBeInTheDocument();

    await user.click(within(now()).getByRole("button", { name: "Tulis laporan" }));
    const again = screen.getByRole("form", { name: "Laporan checkpoint" });
    await user.type(within(again).getByLabelText("Apa yang selesai"), "Diuji dengan pembaca pinjaman.");
    await user.click(within(again).getByRole("button", { name: "Kirim laporan" }));
    await user.click(await within(now()).findByRole("button", { name: "Setujui checkpoint" }));

    expect(await screen.findByText("CP-01 disetujui.")).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: /Pantau/ })).toHaveTextContent("1 dari 2 checkpoint selesai");
    expect(within(now()).getByText("CP-02")).toBeInTheDocument();
    expect(within(now()).getByRole("button", { name: "Mulai CP-02" })).toBeEnabled();
    await user.click(screen.getByRole("button", { name: /CP-01.*Absen dengan kartu/ }));
    expect(within(screen.getByRole("region", { name: /Laporan CP-01/ })).getAllByRole("article")).toHaveLength(2);
  });

  it("a blocked checkpoint goes on once the blocker is answered", async () => {
    const user = userEvent.setup();
    await building(user);
    await user.click(within(now()).getByRole("button", { name: "Mulai CP-01" }));
    await user.click(await within(now()).findByRole("button", { name: "Kerjakan sendiri" }));
    await user.click(await within(now()).findByRole("button", { name: "Tandai terhambat" }));
    await user.type(within(now()).getByLabelText("Apa yang menghambat?"), "Belum ada pembaca kartu.");
    await user.click(within(now()).getByRole("button", { name: "Tandai terhambat" }));
    expect(await within(now()).findByText("Terhambat")).toBeInTheDocument();
    expect(within(now()).getByText("Belum ada pembaca kartu.")).toBeInTheDocument();
    expect(within(waiting()).getByText("Penghambat")).toBeInTheDocument();
    await user.type(within(now()).getByLabelText("Jawaban atau keputusan"), "Pinjam dari lab sebelah.");
    await user.click(within(now()).getByRole("button", { name: "Lanjutkan" }));
    expect(await within(now()).findByText("Dikerjakan")).toBeInTheDocument();
  });

  it("the linked Proyek says where its build stands", async () => {
    const user = userEvent.setup();
    await building(user);
    await user.click(within(now()).getByRole("button", { name: "Mulai CP-01" }));
    await within(now()).findByText("Siap");
    // U2: in Studio the sidebar holds Studio's history; Kembali brings Hub's pages back.
    await user.click(within(screen.getByRole("complementary", { name: "Navigasi" })).getByRole("button", { name: "Kembali" }));
    await user.click(await screen.findByRole("link", { name: "Proyek & Tugas" }));
    await user.click(await screen.findByRole("link", { name: /Absensi lab pakai kartu/ }));
    const line = await screen.findByRole("link", { name: "Dibangun di Studio: CP-01 siap dimulai" });
    await user.click(line);
    expect(await screen.findByRole("region", { name: "Sekarang" })).toBeInTheDocument();
  });

  // Found by the S6 native smoke: approving the last checkpoint said nothing.
  it("approving the last checkpoint says so, and Pantau says everything is done", async () => {
    const user = userEvent.setup();
    await building(user);
    for (const code of ["CP-01", "CP-02"]) {
      await user.click(await within(now()).findByRole("button", { name: `Mulai ${code}` }));
      await user.click(await within(now()).findByRole("button", { name: "Kerjakan sendiri" }));
      await user.click(await within(now()).findByRole("button", { name: "Tulis laporan" }));
      const form = screen.getByRole("form", { name: "Laporan checkpoint" });
      await user.type(within(form).getByLabelText("Apa yang selesai"), `${code} selesai.`);
      await user.click(within(form).getByRole("button", { name: "Kirim laporan" }));
      await user.click(await within(now()).findByRole("button", { name: "Setujui checkpoint" }));
    }
    expect(await within(now()).findByRole("status")).toHaveTextContent("CP-02 disetujui.");
    expect(within(now()).getByText("Semua checkpoint selesai.")).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: /Pantau/ })).toHaveTextContent("2 dari 2 checkpoint selesai");
  });
});
