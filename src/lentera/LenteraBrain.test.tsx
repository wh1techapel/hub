// Lentera's brain in the bar (CP03), against the demo backend's pretend Gemini (mockLentera.ts):
// the line goes to Lentera, cards wait for the owner, and nothing is written before Simpan.

import { QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";

import { BackendContext } from "../data/backend";
import { createMockBackend } from "../data/mock";
import type { MockBackend } from "../data/mock";
import { createQueryClient } from "../data/queries";
import { SoundProvider } from "../sound/SoundProvider";
import { a11yProblems } from "../test/a11y";
import { LenteraWindow } from "./LenteraWindow";
import { empty, history, step } from "./talk";

afterEach(() => {
  vi.restoreAllMocks();
});

async function opened(key: string | null = "AIzaDemoKey") {
  const backend: MockBackend = createMockBackend({ geminiKey: key });
  render(
    <BackendContext.Provider value={backend}>
      <QueryClientProvider client={createQueryClient()}>
        <SoundProvider engine={{ play: () => undefined, setVolume: () => undefined }} place="capture">
          <LenteraWindow />
        </SoundProvider>
      </QueryClientProvider>
    </BackendContext.Provider>,
  );
  const stage = screen.getByRole("main", { name: "Lentera" });
  await waitFor(() => expect(stage).toHaveAttribute("data-view", "bar"));
  await waitFor(() => expect(line()).toHaveFocus());
  return { backend, stage };
}

const line = () => screen.getByRole("textbox", { name: "Tulis untuk Lentera" });
const wrote = (backend: MockBackend) => backend.calls.filter((c) => /^(create_task|create_reminder|create_event|save_capture|save_lentera_proposal)/.test(c));
const card = (name: RegExp) => screen.findByRole("article", { name });

describe("Lentera's brain in the bar", () => {
  it("Enter asks Lentera; its card waits, and only Simpan writes", async () => {
    const user = userEvent.setup();
    const { backend, stage } = await opened();
    await user.type(line(), "laporan lab jam 7{Enter}");
    const proposal = await card(/Usulan tugas: laporan lab/);
    expect(within(proposal).getByText(/Tenggat besok 19\.00/)).toBeInTheDocument();
    expect(screen.getByText("Ini usulanku.")).toBeInTheDocument();
    // Nothing is written yet, and Catat Cepat's reader was not used.
    expect(wrote(backend)).toEqual([]);
    expect(backend.calls.some((c) => c.startsWith("ask_lentera") && c.includes('"mode":"ask"'))).toBe(true);
    expect(a11yProblems()).toEqual([]);
    await user.click(within(proposal).getByRole("button", { name: "Simpan" }));
    await waitFor(() => expect(stage).toHaveAttribute("data-view", "capsule"));
    expect(wrote(backend).filter((c) => c.startsWith("save_lentera_proposal"))).toHaveLength(1);
    expect(backend.calls.some((c) => c.startsWith("save_lentera_proposal") && c.includes('"dueTime":"19:00"'))).toBe(true);
    expect(screen.getByRole("status")).toHaveTextContent("Tugas tersimpan: laporan lab");
  });

  it("several cards: Simpan semua saves each, and the capsule counts them", async () => {
    const user = userEvent.setup();
    const { backend, stage } = await opened();
    await user.type(line(), "poster K3 dan rangkuman bab 2{Enter}");
    await card(/poster K3/);
    await user.click(screen.getByRole("button", { name: "Simpan semua (2)" }));
    await waitFor(() => expect(stage).toHaveAttribute("data-view", "capsule"));
    expect(backend.calls.filter((c) => c.startsWith("save_lentera_proposal"))).toHaveLength(2);
    expect(screen.getByRole("button", { name: /2 usulan tersimpan\. Buka Lentera/ })).toBeInTheDocument();
  });

  it("Lewati folds a card into one line that can come back; skipping everything writes nothing", async () => {
    const user = userEvent.setup();
    const { backend, stage } = await opened();
    await user.type(line(), "beli kertas{Enter}");
    const proposal = await card(/beli kertas/);
    await user.click(within(proposal).getByRole("button", { name: "Lewati" }));
    const skipped = await card(/beli kertas/);
    expect(skipped).toHaveTextContent("Dilewati");
    expect(stage).toHaveAttribute("data-view", "bar");
    await user.click(within(skipped).getByRole("button", { name: "Kembalikan" }));
    expect(await card(/beli kertas/)).toHaveTextContent("Simpan");
    expect(wrote(backend)).toEqual([]);
  });

  it("Ubah changes the card in place, and Simpan sends what the owner changed", async () => {
    const user = userEvent.setup();
    const { backend } = await opened();
    await user.type(line(), "laporan{Enter}");
    const proposal = await card(/Usulan tugas: laporan/);
    await user.click(within(proposal).getByRole("button", { name: "Ubah" }));
    const title = within(proposal).getByLabelText("Judul");
    await waitFor(() => expect(title).toHaveFocus());
    await user.clear(title);
    await user.type(title, "Laporan praktikum jaringan");
    fireEvent.change(within(proposal).getByLabelText("Jam tenggat"), { target: { value: "06:30" } });
    // Esc leaves the fields, not the bar: here the owner saves instead.
    await user.click(within(proposal).getByRole("button", { name: "Simpan" }));
    await waitFor(() => expect(backend.calls.some((c) => c.startsWith("save_lentera_proposal") && c.includes("Laporan praktikum jaringan") && c.includes('"dueTime":"06:30"'))).toBe(true));
  });

  it("a failure says why, and the line can go to Catat Cepat instead", async () => {
    const user = userEvent.setup();
    const { backend, stage } = await opened();
    await user.type(line(), "rapat gagal besok jam 7{Enter}");
    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent("Tidak ada koneksi ke Google.");
    await user.click(within(alert).getByRole("button", { name: "Catat Cepat saja" }));
    expect(screen.getByText(/Tidak ada yang dikirim ke Google/)).toBeInTheDocument();
    await waitFor(() => expect(line()).toHaveValue("rapat gagal besok jam 7"));
    await waitFor(() => expect(line()).toHaveFocus());
    await user.keyboard("{Enter}");
    await waitFor(() => expect(stage).toHaveAttribute("data-view", "capsule"));
    expect(backend.calls.some((c) => c.startsWith("save_capture") && c.includes("rapat gagal"))).toBe(true);
  });

  it("Dari Pesan: a pasted group message is cleaned here and shown before it is sent", async () => {
    const user = userEvent.setup();
    const { backend } = await opened();
    await user.click(line());
    await user.paste("10/11/26 19.02 - Budi: tugas MTK hal 45 dikumpulkan besok\n10/11/26 19.03 - Sari: WA aku 0812 3456 7890");
    const area = await screen.findByRole("textbox", { name: "Pesan grup" });
    await waitFor(() => expect(area).toHaveValue("- tugas MTK hal 45 dikumpulkan besok\n- WA aku [nomor]"));
    expect(screen.getByText(/Dihapus di komputermu: 2 nama pengirim, 1 nomor, 2 jam kirim\./)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Dari Pesan", pressed: true })).toBeInTheDocument();
    // Nothing has left yet.
    expect(backend.calls.some((c) => c.startsWith("ask_lentera"))).toBe(false);
    await user.click(screen.getByRole("button", { name: "Kirim ke Lentera" }));
    await card(/tugas MTK hal 45/);
    const asked = backend.calls.find((c) => c.startsWith("ask_lentera")) ?? "";
    expect(asked).toContain('"mode":"message"');
    expect(asked).not.toContain("0812");
    expect(asked).not.toContain("Budi");
    expect(screen.getByText("Pesan grup, 2 baris")).toBeInTheDocument();
  });

  it("text typed into Dari Pesan is cleaned again before it leaves, and shown again first", async () => {
    const user = userEvent.setup();
    const { backend } = await opened();
    await user.click(screen.getByRole("button", { name: "Dari Pesan" }));
    const area = await screen.findByRole("textbox", { name: "Pesan grup" });
    await user.type(area, "tugas PR fisika, tanya 081234567890");
    await user.click(screen.getByRole("button", { name: "Kirim ke Lentera" }));
    await waitFor(() => expect(area).toHaveValue("tugas PR fisika, tanya [nomor]"));
    expect(screen.getByText(/Hub membersihkan teksnya lagi/)).toBeInTheDocument();
    expect(backend.calls.some((c) => c.startsWith("ask_lentera"))).toBe(false);
  });

  it("the privacy notice says what is sent", async () => {
    const user = userEvent.setup();
    await opened();
    expect(screen.getByText("Dikirim ke Google Gemini saat kamu menekan Enter.")).toBeInTheDocument();
    const more = screen.getByRole("button", { name: "Apa yang dikirim?" });
    await user.click(more);
    expect(more).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByText(/Tugas, catatan, jadwal, dan file tidak dikirim\./)).toBeInTheDocument();
  });

  it("Hentikan stops an answer where it is", async () => {
    const user = userEvent.setup();
    const { backend } = await opened();
    await user.type(line(), "pikir lama{Enter}");
    await user.click(await screen.findByRole("button", { name: "Hentikan" }));
    // Shown in the conversation, and said once.
    await waitFor(() => expect(screen.getAllByText("Dihentikan.")).toHaveLength(2));
    expect(screen.queryByRole("button", { name: "Hentikan" })).not.toBeInTheDocument();
    expect(backend.calls).toContain("stop_lentera");
  });

  it("a click elsewhere keeps the cards waiting in the capsule", async () => {
    const user = userEvent.setup();
    const { stage } = await opened();
    await user.type(line(), "beli tinta{Enter}");
    await card(/beli tinta/);
    const focus = vi.spyOn(document, "hasFocus").mockReturnValue(false);
    fireEvent.blur(window);
    await waitFor(() => expect(stage).toHaveAttribute("data-view", "capsule"));
    focus.mockReturnValue(true);
    await user.click(screen.getByRole("button", { name: /1 usulan menunggu\. Buka Lentera/ }));
    expect(await card(/beli tinta/)).toHaveTextContent("Simpan");
  });

  it("the next question carries the last turns as they were sent", async () => {
    const user = userEvent.setup();
    const { backend } = await opened();
    await user.type(line(), "beli tinta{Enter}");
    await card(/beli tinta/);
    await user.type(line(), "jadikan lusa{Enter}");
    await waitFor(() => expect(backend.calls.filter((c) => c.startsWith("ask_lentera"))).toHaveLength(2));
    const second = backend.calls.filter((c) => c.startsWith("ask_lentera"))[1] ?? "";
    expect(second).toContain('{"from":"owner","text":"beli tinta","message":false}');
    expect(second).toContain('"from":"lentera"');
  });
});

describe("the conversation's steps", () => {
  it("a card is decided once, and a failed answer can be forgotten", () => {
    const asked = step(step(empty, { type: "ask", text: "x", mode: "ask" }), { type: "started", sent: "x" });
    const proposed = step(asked, { type: "proposals", proposals: [{ kind: "note", text: "x", date: "2026-11-10" }] });
    const done = step(proposed, { type: "finished", memory: "Usulan: 1", stopped: false });
    const key = done.exchanges[0]!.cards[0]!.key;
    // Saved only from saving; a second save changes nothing.
    expect(step(done, { type: "saved", card: key, saved: { kind: "note", id: "n1", title: "x", date: null } })).toBe(done);
    const skipped = step(done, { type: "skip", card: key });
    expect(step(skipped, { type: "skip", card: key })).toBe(skipped);
    expect(history(done)).toEqual([
      { from: "owner", text: "x", message: false },
      { from: "lentera", text: "Usulan: 1", message: false },
    ]);
    const failed = step(step(done, { type: "ask", text: "y", mode: "ask" }), { type: "failed", reason: "offline" });
    expect(history(failed)).toHaveLength(2);
    expect(step(failed, { type: "forget", exchange: failed.exchanges[1]!.key }).exchanges).toHaveLength(1);
    // A second question waits for the first answer.
    const busy = step(empty, { type: "ask", text: "a", mode: "ask" });
    expect(step(busy, { type: "ask", text: "b", mode: "ask" })).toBe(busy);
  });
});
