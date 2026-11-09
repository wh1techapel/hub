import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";

import { createMockBackend } from "../../data/mock";
import { renderApp } from "../../test/renderApp";
import { approveStackAndWritePrd, currentStudioStage } from "../../test/studioFlow";

const KEY = "AIzaTestKeyForHubStudio0123456789abcd";

async function withPrd(user: ReturnType<typeof userEvent.setup>, idea = "Absensi lab pakai kartu") {
  const backend = createMockBackend({ geminiKey: KEY });
  const app = renderApp({ path: "/studio", backend });
  await user.type(await screen.findByLabelText("Ceritakan idenya"), `${idea}{Enter}`);
  // The conversation settles first (a reply, or a failure for "gagal").
  await waitFor(() => expect(screen.getByRole("button", { name: "Kirim" })).toBeInTheDocument(), { timeout: 3000 });
  await approveStackAndWritePrd(user);
  return { ...app, backend };
}

function section(name: string) {
  return screen.getByRole("region", { name });
}

describe("Studio: the PRD", () => {
  it("Susun PRD writes it in three parts, moves the stage, and keeps version 1", async () => {
    const user = userEvent.setup();
    const { backend } = await withPrd(user);
    expect(await screen.findByText("PRD tersusun.", undefined, { timeout: 3000 })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: /PRD v1/ })).toHaveTextContent("PRD v1 · Tersimpan");
    expect(within(section("Kebutuhan fungsional")).getByText("Absen dengan kartu")).toBeInTheDocument();
    expect(within(section("Kebutuhan fungsional")).getByText("AC-001-1").closest("li")).toHaveTextContent("Then kehadiran tercatat dalam 2 detik");
    expect(within(section("Latar belakang")).getByText("ASM-01")).toBeInTheDocument();
    // The project is now in stage 3, with stage 2 (the approved tech stack, R3) done.
    expect(currentStudioStage()).toHaveTextContent(/PRD dan bangun/);
    expect(screen.getByRole("tab", { name: /Tech stack/ })).toHaveTextContent(/selesai/);
    expect(backend.calls.some((call) => call.startsWith("generate_prd"))).toBe(true);
    // R4: the technologies are the approved stack's, and security is a section of its own.
    const technical = section("Perencanaan teknis");
    expect(within(technical).getByText(/dari tech stack yang disetujui/)).toBeInTheDocument();
    expect(within(technical).getByText("Backend: Supabase")).toBeInTheDocument();
    expect(within(technical).getByText(/Usulan di atas tech stack itu/)).toBeInTheDocument();
    expect(within(section("Persyaratan keamanan")).getByText("SEC-01").closest("li")).toHaveTextContent("Data: Rekap kehadiran hanya bisa dilihat guru.");
    expect(screen.queryByText(/belum mengikuti tech stack/)).not.toBeInTheDocument();
  });

  it("when the stack changes after the PRD, the PRD says so and follows it on request", async () => {
    const user = userEvent.setup();
    const { backend } = await withPrd(user);
    await screen.findByText("PRD tersusun.", undefined, { timeout: 3000 });
    // Back to stage 2: another backend, approved again.
    await user.click(screen.getByRole("tab", { name: /Tech stack/ }));
    const backendLayer = screen.getByRole("combobox", { name: "Pilihan untuk Backend" }).closest("li")!;
    await user.click(within(backendLayer).getByRole("button", { name: "Pakai Laravel" }));
    await waitFor(() => expect(screen.getByRole("combobox", { name: "Pilihan untuk Backend" })).toHaveValue("Laravel"));
    await user.click(screen.getByRole("button", { name: "Setujui tech stack" }));
    await user.click(within(screen.getByRole("dialog", { name: "Setujui tech stack" })).getByRole("button", { name: "Setujui" }));
    await user.click(screen.getByRole("tab", { name: /PRD dan bangun/ }));
    await user.click(screen.getByRole("tab", { name: "PRD" }));
    expect(await screen.findByText("Teknologi di PRD ini belum mengikuti tech stack yang disetujui.")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Perbarui dari tech stack" }));
    expect(await screen.findByText("Teknologi PRD mengikuti tech stack yang disetujui.")).toBeInTheDocument();
    expect(within(section("Perencanaan teknis")).getByText("Backend: Laravel")).toBeInTheDocument();
    expect(backend.calls.some((call) => call.startsWith("apply_stack_to_prd"))).toBe(true);
  }, 15_000);

  it("a section is edited in place, and the draft is saved as a new version", async () => {
    const user = userEvent.setup();
    await withPrd(user);
    await screen.findByText("PRD tersusun.", undefined, { timeout: 3000 });
    const goals = section("Tujuan");
    await user.click(within(goals).getByRole("button", { name: "Ubah" }));
    const primary = within(goals).getByLabelText("Tujuan utama");
    await user.clear(primary);
    await user.type(primary, "Absen satu kelas selesai dalam 90 detik.");
    await user.click(within(goals).getByRole("button", { name: "Simpan" }));
    expect(await within(section("Tujuan")).findByText("Absen satu kelas selesai dalam 90 detik.")).toBeInTheDocument();
    expect(screen.getByText("Draf punya perubahan yang belum disimpan sebagai versi.")).toBeInTheDocument();
    await user.type(screen.getByLabelText("Catatan perubahan (boleh kosong)"), "Target lebih ketat");
    await user.click(screen.getByRole("button", { name: "Simpan versi" }));
    expect(await screen.findByText("Tersimpan sebagai versi 2.")).toBeInTheDocument();
  });

  it("a broken rule blocks approval and says where; open questions are confirmed once", async () => {
    const user = userEvent.setup();
    await withPrd(user);
    await screen.findByText("PRD tersusun.", undefined, { timeout: 3000 });
    const goals = section("Tujuan");
    await user.click(within(goals).getByRole("button", { name: "Ubah" }));
    const metrics = within(goals).getByRole("group", { name: "Indikator keberhasilan" });
    await user.click(within(metrics).getByRole("button", { name: "Hapus" }));
    await user.click(within(goals).getByRole("button", { name: "Simpan" }));
    expect(await screen.findByText("1 hal perlu diperbaiki sebelum disetujui")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Setujui" })).toBeDisabled();

    // Put an indicator back: the rule is met again.
    await user.click(within(section("Tujuan")).getByRole("button", { name: "Ubah" }));
    const again = within(section("Tujuan")).getByRole("group", { name: "Indikator keberhasilan" });
    await user.click(within(again).getByRole("button", { name: /Tambah indikator/ }));
    await user.type(within(again).getByLabelText("Indikator"), "Waktu absen");
    await user.type(within(again).getByLabelText("Target"), "di bawah 2 menit");
    await user.click(within(section("Tujuan")).getByRole("button", { name: "Simpan" }));
    // U4b: the PRD's card in the conversation has its own Setujui; this is the document's.
    const canvas = () => within(screen.getByRole("tabpanel"));
    await waitFor(() => expect(canvas().getByRole("button", { name: "Setujui" })).toBeEnabled());

    await user.click(canvas().getByRole("button", { name: "Setujui" }));
    const ask = screen.getByRole("group", { name: /masih punya 1 pertanyaan terbuka/ });
    await user.click(within(ask).getByRole("button", { name: "Setujui" }));
    // U4b: the plan is written at once, and the card says the PRD is approved.
    expect(await screen.findByText("Roadmap tersusun.", undefined, { timeout: 3000 })).toBeInTheDocument();
    const card = screen.getByRole("article", { name: "Hasil: PRD versi 2" });
    expect(within(card).getByText("Disetujui")).toBeInTheDocument();
    expect(within(card).queryByRole("button", { name: "Setujui" })).not.toBeInTheDocument();
  });

  it("Gemini's revision is a proposal: shown, then accepted", async () => {
    const user = userEvent.setup();
    const { backend } = await withPrd(user);
    await screen.findByText("PRD tersusun.", undefined, { timeout: 3000 });
    const goals = section("Tujuan");
    await user.click(within(goals).getByRole("button", { name: "Minta revisi" }));
    await user.type(within(goals).getByLabelText("Apa yang perlu diubah di bagian ini?"), "lebih terukur");
    await user.click(within(goals).getByRole("button", { name: "Minta" }));
    expect(await within(goals).findByText("Usulan revisi dari Lentera")).toBeInTheDocument();
    expect(within(goals).getByText(/Tujuan utama \(berubah\)/)).toBeInTheDocument();
    expect(backend.calls.some((call) => call.startsWith("accept_prd_revision"))).toBe(false);
    await user.click(within(goals).getByRole("button", { name: "Terima" }));
    expect(await within(section("Tujuan")).findByText(/\(lebih terukur\)/)).toBeInTheDocument();
    expect(backend.calls.some((call) => call.startsWith("accept_prd_revision"))).toBe(true);
  });

  it("history opens an earlier version read-only, and Markdown is exported", async () => {
    const user = userEvent.setup();
    await withPrd(user);
    await screen.findByText("PRD tersusun.", undefined, { timeout: 3000 });
    await user.click(screen.getByRole("button", { name: "Riwayat" }));
    const history = screen.getByRole("list", { name: "Riwayat" });
    expect(within(history).getByText("v1")).toBeInTheDocument();
    await user.click(within(history).getByRole("button", { name: "Buka" }));
    expect(await screen.findByText("Membaca versi 1 (hanya baca).")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Ubah" })).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Kembali ke draf" }));
    await user.click(screen.getByRole("button", { name: "Ekspor Markdown" }));
    expect(await screen.findByText(/Tersimpan di C:\\Users\\kamu\\Documents\\PRD Absensi lab pakai kartu\.md\./)).toBeInTheDocument();
  });

  it("a failed writing says why and leaves no draft", async () => {
    const user = userEvent.setup();
    await withPrd(user, "Proyek gagal");
    expect(await screen.findByText("PRD tidak tersusun. Tidak ada koneksi ke Google.", undefined, { timeout: 3000 })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Susun PRD" })).toBeInTheDocument();
  });
});
