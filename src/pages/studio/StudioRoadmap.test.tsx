import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";

import { createMockBackend } from "../../data/mock";
import { renderApp } from "../../test/renderApp";
import { approveDocument, approveStackAndWritePrd, currentStudioStage, openStudioDoc } from "../../test/studioFlow";

const KEY = "AIzaTestKeyForHubStudio0123456789abcd";

type User = ReturnType<typeof userEvent.setup>;

/** A project with its PRD written and approved. U4b: the approval writes the plan at once, with
 * the Rencana checkpoint section open. */
async function approvingPrd(user: User, idea = "Absensi lab pakai kartu") {
  const backend = createMockBackend({ geminiKey: KEY });
  const app = renderApp({ path: "/studio", backend });
  await user.type(await screen.findByLabelText("Ceritakan idenya"), `${idea}{Enter}`);
  await waitFor(() => expect(screen.getByRole("button", { name: "Kirim" })).toBeInTheDocument(), { timeout: 3000 });
  await approveStackAndWritePrd(user);
  await screen.findByText("PRD tersusun.", undefined, { timeout: 3000 });
  await approveDocument(user);
  const ask = screen.getByRole("group", { name: /pertanyaan terbuka/ });
  await user.click(within(ask).getByRole("button", { name: "Setujui" }));
  return { ...app, backend };
}

async function withRoadmap(user: User, idea?: string) {
  const app = await approvingPrd(user, idea);
  await screen.findByText("Roadmap tersusun.", undefined, { timeout: 3000 });
  return app;
}

const list = () => screen.getByRole("list", { name: "Roadmap" });
const row = (code: string) => document.getElementById(`cp-${code}`) as HTMLElement;

describe("Studio: the roadmap", () => {
  it("waits for an approved PRD", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend({ geminiKey: KEY });
    renderApp({ path: "/studio", backend });
    await user.type(await screen.findByLabelText("Ceritakan idenya"), "Absensi{Enter}");
    await waitFor(() => expect(screen.getByRole("button", { name: "Kirim" })).toBeInTheDocument(), { timeout: 3000 });
    // Stage 3 waits for the approved stack (R3), then the roadmap for the approved PRD.
    await user.click(screen.getByRole("tab", { name: /PRD dan bangun/ }));
    expect(screen.getByText(/Setujui tech stack dulu di Tahap 2/)).toBeInTheDocument();
    await approveStackAndWritePrd(user);
    await screen.findByText("PRD tersusun.", undefined, { timeout: 3000 });
    await openStudioDoc(user, "Rencana checkpoint");
    expect(screen.getByText(/Setujui PRD dulu di tab PRD/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Susun roadmap" })).toBeDisabled();
  });

  it("approving the PRD writes the checkpoints at once and moves the stage (U4b)", async () => {
    const user = userEvent.setup();
    const { backend } = await withRoadmap(user);
    expect(screen.getByRole("heading", { name: /Roadmap · draf/ })).toHaveTextContent("2 checkpoint");
    expect(screen.getByText("Dari PRD v1.")).toBeInTheDocument();
    const first = row("CP-01");
    expect(within(first).getByRole("heading", { name: "Absen dengan kartu" })).toBeInTheDocument();
    expect(within(first).getByText("Direncanakan")).toBeInTheDocument();
    expect(within(first).getByText("FEAT-001")).toBeInTheDocument();
    expect(currentStudioStage()).toHaveTextContent(/PRD dan bangun/);
    expect(backend.calls.some((call) => call.startsWith("generate_roadmap"))).toBe(true);

    // Details are a disclosure: tasks with their codes, required tests, and the fixed DoD lines.
    const toggle = within(first).getByRole("button", { name: "Rincian" });
    await user.click(toggle);
    expect(toggle).toHaveAttribute("aria-expanded", "true");
    expect(within(first).getByText("CP-01.T01")).toBeInTheDocument();
    expect(within(first).getByText("npm test")).toBeInTheDocument();
    expect(within(first).getByText("Laporan dikirim ke Hub dan disetujui pemilik.")).toBeInTheDocument();
    expect(backend.calls.some((call) => call.startsWith("generate_roadmap"))).toBe(true);
  });

  it("the draft is edited, added to, reordered by keyboard, and its problems block approval", async () => {
    const user = userEvent.setup();
    await withRoadmap(user);

    await user.click(within(row("CP-02")).getByRole("button", { name: "Ubah" }));
    const form = screen.getByRole("form", { name: "Ubah CP-02" });
    const title = within(form).getByLabelText("Judul");
    await user.clear(title);
    await user.type(title, "Rekap kehadiran");
    // The DoD lines Hub keeps are not in the editor.
    expect(within(form).getByLabelText<HTMLTextAreaElement>("Definition of Done").value).not.toContain("Laporan dikirim");
    await user.click(within(form).getByRole("button", { name: "Simpan" }));
    expect(await within(row("CP-02")).findByRole("heading", { name: "Rekap kehadiran" })).toBeInTheDocument();

    // A blank checkpoint after CP-01 becomes CP-02 and shows its problems.
    await user.click(within(row("CP-01")).getByRole("button", { name: "Tambah setelah ini: sesudah CP-01" }));
    expect(await within(await waitFor(() => row("CP-02"))).findByRole("heading", { name: "Checkpoint baru" })).toBeInTheDocument();
    expect(within(row("CP-03")).getByRole("heading", { name: "Rekap kehadiran" })).toBeInTheDocument();
    expect(screen.getByText(/hal perlu diperbaiki sebelum disetujui/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Setujui roadmap" })).toBeDisabled();

    // Reorder with the buttons (the drag grip has the same effect), announced.
    await user.click(screen.getByRole("button", { name: "Naikkan CP-03" }));
    expect(await within(await waitFor(() => row("CP-02"))).findByRole("heading", { name: "Rekap kehadiran" })).toBeInTheDocument();
    expect(screen.getByText("CP-03 dipindah ke urutan 2.")).toBeInTheDocument();

    // Remove the blank one: nothing left to fix.
    await user.click(within(row("CP-03")).getByRole("button", { name: "Hapus" }));
    await user.click(within(screen.getByRole("group", { name: /Hapus CP-03\?/ })).getByRole("button", { name: "Hapus" }));
    await waitFor(() => expect(within(list()).getAllByRole("heading")).toHaveLength(2));
    expect(screen.getByRole("button", { name: "Setujui roadmap" })).toBeEnabled();
  });

  it("approval links a new Hub Proyek, the folder, and Claude's access, and moves to Bangun", async () => {
    const user = userEvent.setup();
    await withRoadmap(user);
    await user.click(screen.getByRole("button", { name: "Setujui roadmap" }));
    const dialog = screen.getByRole("dialog", { name: "Setujui roadmap" });
    const confirm = within(dialog).getByRole("button", { name: "Setujui" });
    expect(confirm).toBeDisabled();
    expect(within(dialog).getByRole("radio", { name: /Proyek baru "Absensi lab pakai kartu"/ })).toBeChecked();
    expect(within(dialog).getByRole("radio", { name: /Tidak ada/ })).toBeChecked();

    await user.click(within(dialog).getByRole("button", { name: "Pilih folder…" }));
    expect(await within(dialog).findByText("C:\\Users\\kamu\\Projects\\absensi")).toBeInTheDocument();
    expect(within(dialog).getByText("Repo git ditemukan.")).toBeInTheDocument();
    await user.click(within(dialog).getByRole("radio", { name: /Baca dan lapor/ }));
    await user.click(confirm);

    expect(await screen.findByText("Roadmap disetujui. Proyek pindah ke tahap Bangun.")).toBeInTheDocument();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(screen.getByRole("heading", { name: /Roadmap · disetujui/ })).toHaveTextContent("0 dari 2 checkpoint selesai");
    const settings = screen.getByRole("region", { name: "Proyek ini" });
    expect(within(settings).getByText("Absensi lab pakai kartu")).toBeInTheDocument();
    expect(within(settings).getByRole("radio", { name: /Baca dan lapor/ })).toBeChecked();
    expect(currentStudioStage()).toHaveTextContent(/PRD dan bangun/);

    // Not started yet: it can still be changed; access can be changed any time.
    expect(screen.getByRole("button", { name: "Ubah roadmap" })).toBeInTheDocument();
    await user.click(within(settings).getByRole("radio", { name: /Baca saja/ }));
    expect(await screen.findByText("Tersimpan.")).toBeInTheDocument();

    // "Ubah roadmap" opens a draft copied from it.
    await user.click(screen.getByRole("button", { name: "Ubah roadmap" }));
    expect(await screen.findByRole("heading", { name: /Roadmap · draf/ })).toBeInTheDocument();
  });

  it("a typed folder is checked: missing refuses, without git warns", async () => {
    const user = userEvent.setup();
    await withRoadmap(user);
    await user.click(screen.getByRole("button", { name: "Setujui roadmap" }));
    const dialog = screen.getByRole("dialog", { name: "Setujui roadmap" });
    const typed = within(dialog).getByLabelText("atau ketik path-nya");
    await user.type(typed, "C:\\tidak-ada\\repo");
    await user.click(within(dialog).getByRole("button", { name: "Periksa" }));
    expect(await within(dialog).findByText("Folder ini tidak ada.")).toBeInTheDocument();
    expect(within(dialog).getByRole("button", { name: "Setujui" })).toBeDisabled();

    await user.clear(typed);
    await user.type(typed, "C:\\kode\\tanpa-git");
    await user.click(within(dialog).getByRole("button", { name: "Periksa" }));
    expect(await within(dialog).findByText(/belum berisi repo git/)).toBeInTheDocument();
    expect(within(dialog).getByRole("button", { name: "Setujui" })).toBeEnabled();

    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("a failed writing says why and leaves no draft", async () => {
    const user = userEvent.setup();
    // The writing that follows the PRD's approval fails; Susun roadmap stays for another try.
    await approvingPrd(user, "Proyek putus");
    expect(await screen.findByText("Roadmap tidak tersusun. Tidak ada koneksi ke Google.", undefined, { timeout: 3000 })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Susun roadmap" })).toBeInTheDocument();
  });
});
