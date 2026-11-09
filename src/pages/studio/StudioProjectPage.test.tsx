import { act, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";

import { HubError } from "../../data/backend";
import type { Command } from "../../data/backend";
import { createMockBackend } from "../../data/mock";
import { renderApp } from "../../test/renderApp";
import { currentStudioStage } from "../../test/studioFlow";

const KEY = "AIzaTestKeyForHubStudio0123456789abcd";

async function startProject(user: ReturnType<typeof userEvent.setup>, idea = "Absensi lab pakai kartu") {
  const backend = createMockBackend({ geminiKey: KEY });
  const app = renderApp({ path: "/studio", backend });
  await user.type(await screen.findByLabelText("Ceritakan idenya"), `${idea}{Enter}`);
  // The project opens with the idea already sent and Lentera's reply in: its questions (L1).
  await screen.findByRole("region", { name: "Pertanyaan dari Lentera" }, { timeout: 3000 });
  return { ...app, backend };
}

describe("Studio: a project's conversation", () => {
  it("an idea starts a project, is sent, and the reply streams in with a question and a note", async () => {
    const user = userEvent.setup();
    const { backend } = await startProject(user);
    expect(screen.getByLabelText("Nama proyek")).toHaveValue("Absensi lab pakai kartu");
    expect(backend.calls.some((call) => call.startsWith("send_studio_message") && call.includes("Absensi lab pakai kartu"))).toBe(true);
    // L1: a reply that asks is only its questions, under Lentera's name; the words the model
    // wrote around them (the mock writes some) are not shown.
    const log = screen.getByRole("log");
    expect(within(log).getByRole("region", { name: "Pertanyaan dari Lentera" })).toBeInTheDocument();
    expect(within(log).queryByText(/sebelum lanjut, aku perlu tahu/i)).not.toBeInTheDocument();
    expect(within(log).queryByText("Gemini")).not.toBeInTheDocument();
    // The question, with its recommendation, and "Belum tahu" always there.
    const group = await screen.findByRole("group", { name: "Siapa yang paling sering memakainya?" });
    expect(within(group).getByRole("radio", { name: /Siswa/ })).toBeInTheDocument();
    expect(within(group).getByText(/disarankan/)).toBeInTheDocument();
    expect(within(group).getByRole("radio", { name: "Belum tahu" })).toBeInTheDocument();
    // What Gemini recorded is in Ringkasan, marked as Gemini's.
    const ledger = screen.getByRole("region", { name: "Ringkasan" });
    expect(await within(ledger).findByText("Penggunanya siswa di lab sekolah.")).toBeInTheDocument();
    expect(within(ledger).getByText("Asumsi")).toBeInTheDocument();
    expect(screen.getByText("Lentera selesai menjawab.")).toBeInTheDocument();
  });

  // R2: one set of questions, grouped by part of the idea, answered in part and sent as one
  // message; the composer's own draft is left alone.
  it("the question set is grouped by part, answered in part, and sent as one message", async () => {
    const user = userEvent.setup();
    const { backend } = await startProject(user);
    const set = screen.getByRole("region", { name: "Pertanyaan dari Lentera" });
    expect(within(set).getAllByRole("heading", { level: 3 }).map((h) => h.textContent)).toEqual(["Pengguna", "Fitur inti", "Batasan"]);
    expect(within(set).getByText("0 dari 3 dijawab")).toBeInTheDocument();
    // Nothing chosen: said plainly, nothing sent.
    await user.click(within(set).getByRole("button", { name: "Kirim jawaban" }));
    expect(within(set).getByRole("alert")).toHaveTextContent("Pilih atau tulis paling sedikit satu jawaban.");
    await user.type(screen.getByLabelText("Pesan untuk Lentera"), "Draf sendiri");
    await user.click(within(set).getByRole("radio", { name: /Siswa/ }));
    const features = within(set).getByRole("group", { name: "Fitur mana yang wajib ada di versi pertama?" });
    expect(within(features).getByText(/boleh pilih lebih dari satu/)).toBeInTheDocument();
    await user.click(within(features).getByRole("checkbox", { name: /Absen dengan kartu/ }));
    await user.click(within(features).getByRole("checkbox", { name: "Rekap mingguan" }));
    await user.click(within(features).getByRole("button", { name: "Jawaban lain" }));
    await user.type(within(features).getByLabelText(/Jawaban lain untuk/), "Ekspor ke Excel");
    expect(within(set).getByText("2 dari 3 dijawab")).toBeInTheDocument();
    await user.click(within(set).getByRole("button", { name: "Kirim jawaban" }));
    await waitFor(() =>
      expect(backend.calls.filter((call) => call.startsWith("send_studio_message")).pop()).toContain(
        JSON.stringify(
          "Jawabanku:\n- Siapa yang paling sering memakainya? Siswa\n- Fitur mana yang wajib ada di versi pertama? Absen dengan kartu, Rekap mingguan, Ekspor ke Excel\n\nBelum kujawab:\n- Perangkat apa yang ada di lab?",
        ),
      ),
    );
    expect(screen.getByLabelText("Pesan untuk Lentera")).toHaveValue("Draf sendiri");
    expect(await screen.findByText(/Lentera menilai kebutuhannya sudah cukup untuk menyusun PRD/, undefined, { timeout: 3000 })).toBeInTheDocument();
    // The answered set leaves one line behind; the answers themselves are the owner's message.
    expect(screen.getByText("Mengajukan 3 pertanyaan.")).toBeInTheDocument();
    expect(screen.queryByRole("region", { name: "Pertanyaan dari Lentera" })).not.toBeInTheDocument();
    expect(currentStudioStage()).toHaveTextContent(/Mau buat apa\?/);
    // Many steps of typing: allowed more than the default 5 s, as the other journeys are.
  }, 15_000);

  // R2: check answers before leaving stage 1 (GOV.UK's pattern): each part, Ubah, then on.
  it("moving on shows the check-answers sheet; Ubah opens that part, and on goes to the tech stack", async () => {
    const user = userEvent.setup();
    const { backend } = await startProject(user);
    // Yang sudah jelas, from the ledger: the problem is settled, the users only assumed.
    const clear = screen.getByRole("region", { name: "Yang sudah jelas" });
    expect(within(clear).getByRole("button", { name: "Lihat catatan Masalah" }).closest("li")).toHaveTextContent(/1 keputusan.*jelas/);
    expect(within(clear).getByRole("button", { name: "Lihat catatan Pengguna" }).closest("li")).toHaveTextContent(/1 asumsi.*sebagian/);
    expect(within(clear).getByText("Ukuran berhasil").closest("li")).toHaveTextContent(/belum ada catatan.*belum/);

    await user.click(within(screen.getByRole("region", { name: "Pertanyaan dari Lentera" })).getByRole("radio", { name: /Siswa/ }));
    await user.click(screen.getByRole("button", { name: "Kirim jawaban" }));
    await user.click(await screen.findByRole("button", { name: "Cek ringkasan dan lanjut" }, { timeout: 3000 }));
    const sheet = screen.getByRole("dialog", { name: "Cek ringkasan sebelum lanjut" });
    expect(within(sheet).getByText("Absen manual memakan waktu di awal praktikum.")).toBeInTheDocument();
    expect(within(sheet).getByText(/Belum jelas: Pengguna, Fitur inti, Batasan, Ukuran berhasil\./)).toBeInTheDocument();
    expect(backend.calls.some((call) => call.startsWith("generate_prd"))).toBe(false);

    // Ubah: the sheet closes and Ringkasan opens at that part.
    await user.click(within(sheet).getByRole("button", { name: "Ubah Pengguna" }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    await waitFor(() => expect(screen.getByRole("region", { name: "Pengguna" })).toHaveFocus());

    // From Ringkasan again, and on.
    await user.click(within(screen.getByRole("region", { name: "Yang sudah jelas" })).getByRole("button", { name: "Cek ringkasan dan lanjut" }));
    await user.click(within(screen.getByRole("dialog", { name: "Cek ringkasan sebelum lanjut" })).getByRole("button", { name: "Lanjut ke tech stack" }));
    expect(screen.getByRole("tab", { name: /Tech stack/ })).toHaveAttribute("aria-selected", "true");
    expect(screen.getByRole("heading", { name: /Tech stack/, level: 2 })).toBeInTheDocument();
    expect(backend.calls.some((call) => call.startsWith("generate_prd"))).toBe(false);
  }, 15_000);

  // U4: the workspace. The conversation stays in its column in every stage; the stepper chooses
  // which stage's document the canvas beside it shows, so a draft survives every move.
  it("the conversation stays while the canvas shows each stage's document", async () => {
    const user = userEvent.setup();
    await startProject(user);
    await user.type(screen.getByLabelText("Pesan untuk Lentera"), "Belum dikirim");
    const canvas = () => screen.getByRole("tabpanel");
    // Stage 1's document is Ringkasan.
    expect(canvas()).toHaveAttribute("aria-labelledby", "studio-step-1");
    expect(within(canvas()).getByRole("region", { name: "Ringkasan" })).toBeInTheDocument();
    // Stage 2's is the tech stack; the conversation is still there, draft and all.
    await user.click(screen.getByRole("tab", { name: /Tech stack/ }));
    expect(within(canvas()).getByRole("heading", { name: /Tech stack/, level: 2 })).toBeInTheDocument();
    expect(screen.getByRole("log")).toBeInTheDocument();
    expect(screen.getByLabelText("Pesan untuk Lentera")).toHaveValue("Belum dikirim");
    // No side tabs any more: Ringkasan is stage 1's document.
    expect(screen.queryByRole("tablist", { name: "Isi panel" })).not.toBeInTheDocument();
    await user.click(screen.getByRole("tab", { name: /Mau buat apa/ }));
    expect(within(canvas()).getByRole("region", { name: "Ringkasan" })).toBeInTheDocument();
    expect(screen.getByLabelText("Pesan untuk Lentera")).toHaveValue("Belum dikirim");
  });

  it("the canvas hides and comes back, a stage opens it, and stage 3 waits for the approved stack", async () => {
    const user = userEvent.setup();
    await startProject(user);
    const toggle = screen.getByRole("button", { name: "Sembunyikan dokumen" });
    expect(toggle).toHaveAttribute("aria-expanded", "true");
    expect(toggle).toHaveAttribute("aria-controls", "studio-canvas");
    expect(screen.getByRole("region", { name: "Ringkasan" })).toBeInTheDocument();
    await user.click(toggle);
    expect(screen.getByRole("button", { name: "Tampilkan dokumen" })).toHaveAttribute("aria-expanded", "false");
    // A stage in the stepper opens its document.
    await user.click(screen.getByRole("tab", { name: /Tech stack/ }));
    expect(screen.getByRole("button", { name: "Sembunyikan dokumen" })).toHaveAttribute("aria-expanded", "true");
    await user.click(screen.getByRole("tab", { name: /Mau buat apa/ }));
    expect(screen.getByRole("tab", { name: /Tech stack/ })).not.toHaveAttribute("aria-disabled");
    const build = screen.getByRole("tab", { name: /PRD dan bangun/ });
    expect(build).toHaveAttribute("aria-disabled", "true");
    await user.click(build);
    expect(screen.getByText(/Setujui tech stack dulu di Tahap 2/)).toHaveAttribute("role", "status");
    expect(build).toHaveAttribute("aria-selected", "false");
  });

  it("a failed reply says why and can be tried again", async () => {
    const user = userEvent.setup();
    await startProject(user);
    const input = screen.getByLabelText("Pesan untuk Lentera");
    await user.type(input, "coba sampai gagal{Enter}");
    expect(await screen.findByText("Tidak ada koneksi ke Google.", undefined, { timeout: 3000 })).toBeInTheDocument();
    expect(screen.getByText("Sebentar, aku sedang")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Coba lagi" }));
    // The mock fails again for the same words: the retry is a new reply with its own outcome.
    await waitFor(() => expect(screen.getAllByText("Tidak ada koneksi ke Google.")).toHaveLength(2), { timeout: 3000 });
  });

  // L2: Google counts the free quota per model, so a reply that ran out offers another model.
  it("a reply out of quota offers Ganti model, and the retry uses the new one", async () => {
    const user = userEvent.setup();
    const { backend } = await startProject(user);
    await user.type(screen.getByLabelText("Pesan untuk Lentera"), "kuota habis{Enter}");
    expect(await screen.findByText("Kuota model ini habis. Ganti model atau coba lagi nanti.", undefined, { timeout: 3000 })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Ganti model" }));
    await user.click(await within(screen.getByRole("menu", { name: "Model" })).findByRole("menuitemradio", { name: /3\.5 Flash-Lite/ }));
    await waitFor(() => expect(backend.calls).toContain('update_settings {"patch":{"geminiModel":"gemini-3.5-flash-lite"}}'));
    // The input's chip follows the same setting.
    expect(await screen.findByRole("button", { name: "Model: 3.5 Flash-Lite. Ganti model" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Coba lagi" }));
    await waitFor(() => expect(screen.queryByRole("button", { name: "Ganti model" })).not.toBeInTheDocument(), { timeout: 3000 });
    expect(screen.getAllByText("Kuota model ini habis. Ganti model atau coba lagi nanti.")).toHaveLength(1);
  });

  // Real run, 2026-11-09: Google answers "high demand" per model, so a busy model offers another.
  it("a reply refused because the model is busy offers Ganti model", async () => {
    const user = userEvent.setup();
    await startProject(user);
    await user.type(screen.getByLabelText("Pesan untuk Lentera"), "model sibuk{Enter}");
    expect(await screen.findByText("Model ini sedang sibuk di Google. Ganti model atau coba lagi nanti.", undefined, { timeout: 3000 })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Ganti model" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Coba lagi" })).toBeInTheDocument();
  });

  it("an offline failure offers only Coba lagi", async () => {
    const user = userEvent.setup();
    await startProject(user);
    await user.type(screen.getByLabelText("Pesan untuk Lentera"), "coba sampai gagal{Enter}");
    await screen.findByText("Tidak ada koneksi ke Google.", undefined, { timeout: 3000 });
    expect(screen.getByRole("button", { name: "Coba lagi" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Ganti model" })).not.toBeInTheDocument();
  });

  it("Hentikan stops a reply and keeps what arrived", async () => {
    const user = userEvent.setup();
    await startProject(user);
    await user.type(screen.getByLabelText("Pesan untuk Lentera"), "jawab panjang{Enter}");
    await screen.findByText(/Bagian 2./, undefined, { timeout: 3000 });
    await user.click(await screen.findByRole("button", { name: "Hentikan" }));
    expect(await screen.findByText("Dihentikan.", undefined, { timeout: 3000 })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Coba lagi" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Kirim" })).toBeInTheDocument();
  });

  it("Ringkasan corrects what was settled: confirm, add, and let go", async () => {
    const user = userEvent.setup();
    const { backend } = await startProject(user);
    const ledger = screen.getByRole("region", { name: "Ringkasan" });
    await user.click(await within(ledger).findByRole("button", { name: "Konfirmasi" }));
    expect(await within(ledger).findByText(/dikonfirmasi/)).toBeInTheDocument();
    await user.click(within(ledger).getByRole("button", { name: "Tambah catatan" }));
    await user.selectOptions(within(ledger).getByLabelText("Jenis"), "constraint");
    await user.selectOptions(within(ledger).getByLabelText("Bagian"), "constraints");
    await user.type(within(ledger).getByLabelText("Topik"), "Waktu");
    await user.type(within(ledger).getByLabelText("Isi"), "Selesai dalam dua minggu");
    await user.click(within(ledger).getByRole("button", { name: "Simpan" }));
    // Grouped by part: the constraint is under Batasan, and that part is now clear.
    const part = await within(ledger).findByRole("region", { name: "Batasan" });
    expect(within(part).getByText("Selesai dalam dua minggu")).toBeInTheDocument();
    expect(backend.calls.some((call) => call.startsWith("add_studio_ledger") && call.includes('"area":"constraints"'))).toBe(true);
    expect(within(screen.getByRole("region", { name: "Yang sudah jelas" })).getByRole("button", { name: "Lihat catatan Batasan" }).closest("li")).toHaveTextContent(/jelas/);
    const item = within(ledger).getByText("Selesai dalam dua minggu").closest("li")!;
    await user.click(within(item).getByRole("button", { name: "Tidak berlaku" }));
    expect(await within(ledger).findByText("Tidak berlaku lagi (1)")).toBeInTheDocument();
    expect(backend.calls.some((call) => call.startsWith("edit_studio_ledger") && call.includes('"status":"retired"'))).toBe(true);
  });

  it("without a key the conversation can be read but not continued", async () => {
    const backend = createMockBackend({ geminiKey: KEY });
    renderApp({ path: "/studio", backend });
    const user = userEvent.setup();
    await user.type(await screen.findByLabelText("Ceritakan idenya"), "Bank soal{Enter}");
    await screen.findByRole("region", { name: "Pertanyaan dari Lentera" }, { timeout: 3000 });
    await backend.call("gemini_delete_key");
    await user.click(within(screen.getByRole("complementary", { name: "Navigasi" })).getByRole("link", { name: "Ide baru" }));
    // U3: the projects are Studio's history in the sidebar.
    await user.click(await within(screen.getByRole("navigation", { name: "Riwayat Studio" })).findByRole("link", { name: "Bank soal" }));
    expect(await screen.findByLabelText("Pesan untuk Lentera")).toBeDisabled();
    expect(screen.getByText(/Simpan API key Google di Pengaturan/)).toBeInTheDocument();
    // The questions stay readable; answering them waits for a key.
    const set = screen.getByRole("region", { name: "Pertanyaan dari Lentera" });
    expect(within(set).getByRole("button", { name: "Kirim jawaban" })).toBeDisabled();
  });

  it("the project can be renamed and removed", async () => {
    const user = userEvent.setup();
    await startProject(user);
    const name = screen.getByLabelText("Nama proyek");
    await user.clear(name);
    await user.type(name, "Absensi SMK{Enter}");
    // U3: the projects are Studio's history in the sidebar; the new name shows there.
    const history = screen.getByRole("navigation", { name: "Riwayat Studio" });
    await user.click(within(screen.getByRole("complementary", { name: "Navigasi" })).getByRole("link", { name: "Ide baru" }));
    await user.click(await within(history).findByRole("link", { name: "Absensi SMK" }));
    await user.click(await screen.findByRole("button", { name: "Hapus proyek" }));
    await user.click(screen.getByRole("button", { name: "Hapus" }));
    expect(await within(history).findByText("Belum ada proyek. Ide pertamamu akan muncul di sini.")).toBeInTheDocument();
  });

  // S6: the native smoke once found the chat input gone. A refresh of the project that fails
  // (after a change from Claude, say) must not take the page away while its data is still there.
  it("a failed refresh in the background keeps the project on screen", async () => {
    const user = userEvent.setup();
    const { backend } = await startProject(user);
    const call = backend.call;
    let refused = 0;
    backend.call = <T,>(command: Command, args?: Record<string, unknown>): Promise<T> => {
      if (command !== "get_studio_project") return call<T>(command, args);
      refused += 1;
      return Promise.reject(new HubError({ kind: "internal", field: null, reason: null, message: "busy" }));
    };
    act(() => backend.emit({ entity: "studio_project", ids: [] }));
    // The first try and the one retry both fail.
    await waitFor(() => expect(refused).toBe(2), { timeout: 3000 });
    await act(() => new Promise((done) => setTimeout(done, 50)));
    expect(screen.getByLabelText("Pesan untuk Lentera")).toBeInTheDocument();
    expect(screen.getByLabelText("Nama proyek")).toHaveValue("Absensi lab pakai kartu");
  });
});
