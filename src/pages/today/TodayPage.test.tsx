import { act, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";

import { createMockBackend } from "../../data/mock";
import { renderApp } from "../../test/renderApp";

afterEach(() => {
  vi.useRealTimers();
});

describe("Hari Ini", () => {
  it("greets the owner and says what comes next", async () => {
    renderApp({ at: new Date(2026, 9, 22, 12, 40) });
    const heading = await screen.findByRole("heading", { level: 1 });
    expect(heading).toHaveTextContent("Selamat siang, Glenvio.");
    expect(heading.nextElementSibling).toHaveTextContent("Berikutnya Produktif TJKT pukul 13.00. Fokus 1 dari 3 selesai.");
    expect(screen.getByText(/Sisa hari 9 j 50 m/)).toHaveTextContent("Berikutnya Produktif TJKT 13.00, dalam 20 m");
    expect(screen.getByRole("img", { name: /Busur Hari: 6 jadwal hari ini, sekarang pukul 12.40/ })).toBeInTheDocument();
    expect(document.documentElement.dataset.phase).toBe("siang");
    expect(screen.queryByRole("button", { name: "Tutup Hari" })).not.toBeInTheDocument();
  });

  it("wears the night light and invites Tutup Hari in the evening", async () => {
    renderApp({ at: new Date(2026, 9, 22, 21, 5) });
    expect(await screen.findByRole("heading", { level: 1 })).toHaveTextContent("Selamat malam");
    expect(document.documentElement.dataset.phase).toBe("malam");
    expect(document.documentElement.dataset.scheme).toBe("dark");
    expect(screen.getByText("7 dari 9 hal selesai hari ini.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Tutup Hari" })).toBeInTheDocument();
  });

  it("checks a focus task, says so, and can undo it", async () => {
    const user = userEvent.setup();
    const { backend } = renderApp();
    const check = await screen.findByRole("button", { name: "Tandai selesai: THM: Nmap Live Host Discovery" });
    expect(check).toHaveAttribute("aria-pressed", "false");
    await user.click(check);
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Batalkan selesai: THM: Nmap Live Host Discovery" })).toHaveAttribute(
        "aria-pressed",
        "true",
      ),
    );
    expect(backend.calls.some((call) => call.startsWith("complete_task"))).toBe(true);
    expect(await screen.findByText("2 dari 3 selesai")).toBeInTheDocument();

    const status = screen.getByRole("status");
    expect(within(status).getByText("Selesai: THM: Nmap Live Host Discovery")).toBeInTheDocument();
    await user.click(within(status).getByRole("button", { name: "Urungkan" }));
    await waitFor(() => expect(backend.calls.some((call) => call.startsWith("reopen_task"))).toBe(true));
    expect(await screen.findByText("1 dari 3 selesai")).toBeInTheDocument();
  });

  it("marks what ULTRON added", async () => {
    renderApp();
    const row = (await screen.findByText("Bawa kabel UTP ke lab besok")).closest("li");
    expect(row).not.toBeNull();
    expect(within(row!).getByText("oleh ULTRON")).toBeInTheDocument();
    expect(screen.getByText("ULTRON menambahkan tugas “Bawa kabel UTP ke lab besok”.")).toBeInTheDocument();
  });

  it("refetches when the core announces a change from elsewhere", async () => {
    const { backend } = renderApp();
    await screen.findByText("Revisi slide presentasi KIK");
    backend.addTask("Baca materi subnetting", "ultron");
    act(() => backend.emit({ entity: "task", ids: [] }));
    expect(await screen.findByText("Baca materi subnetting")).toBeInTheDocument();
  });

  it("says plainly when the day is empty", async () => {
    renderApp({ empty: true });
    expect(await screen.findByText("Belum ada jadwal hari ini.")).toBeInTheDocument();
    expect(screen.getByText(/Tidak ada yang mendesak/)).toBeInTheDocument();
    expect(screen.getAllByText("Hari ini masih kosong.").length).toBeGreaterThan(0);
  });

  it("offers Hub's picks when no focus was chosen, and takes them in one press", async () => {
    vi.useFakeTimers({ toFake: ["Date"] });
    vi.setSystemTime(new Date(2026, 9, 22, 9, 0));
    const backend = createMockBackend({ empty: true });
    backend.addTask("Kerjakan LKS bab 3", "owner");
    backend.addTask("Cicil laporan lab", "owner", { doDate: null, dueDate: "2026-10-23", priority: 2 });
    const user = userEvent.setup();
    renderApp({ backend, at: new Date(2026, 9, 22, 9, 0) });

    const picks = await screen.findByRole("list", { name: "Saran Hub" });
    expect(
      within(picks)
        .getAllByRole("listitem")
        .map((li) => li.textContent),
    ).toEqual([expect.stringContaining("Cicil laporan lab"), expect.stringContaining("Kerjakan LKS bab 3")]);
    await user.click(screen.getByRole("button", { name: "Pakai saran ini" }));
    expect(await screen.findByRole("list", { name: "Fokus" })).toBeInTheDocument();
    expect(screen.getByText("0 dari 2 selesai")).toBeInTheDocument();
    expect(backend.calls.some((call) => call.startsWith("set_focus"))).toBe(true);
  });

  it("closes the day: moves what slipped, keeps a sentence, and fills the arc", async () => {
    const user = userEvent.setup();
    const { backend } = renderApp({ at: new Date(2026, 9, 22, 21, 5) });
    await user.click(await screen.findByRole("button", { name: "Tutup Hari" }));

    const sheet = await screen.findByRole("dialog", { name: "Tutup Hari" });
    expect(within(sheet).getByText("Hub: kunci arah desain")).toBeInTheDocument();
    const nmap = within(sheet).getByRole("radiogroup", { name: "THM: Nmap Live Host Discovery" });
    await user.click(within(nmap).getByLabelText("Lepaskan"));
    await user.type(within(sheet).getByLabelText("Satu kalimat untuk hari ini"), "Hari yang padat tapi jalan.");
    // What is let go is no longer offered for tomorrow; Hub's pick for tomorrow is ticked.
    expect(within(sheet).queryByRole("checkbox", { name: /THM: Nmap/ })).not.toBeInTheDocument();
    expect(within(sheet).getByRole("checkbox", { name: /Laporan Praktikum 5/ })).toBeChecked();
    await user.click(within(sheet).getByRole("button", { name: "Tutup hari" }));

    expect(await screen.findByRole("heading", { level: 1 })).toHaveTextContent("Hari ditutup.");
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(screen.getByText("Hari ditutup", { selector: "figcaption" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Tutup Hari" })).not.toBeInTheDocument();
    const close = backend.calls.find((call) => call.startsWith("close_day"));
    expect(close).toContain('"let_go"');
    expect(close).toContain("Hari yang padat tapi jalan.");
  });

  it("shows Belajar's week and the latest files, and picks learning up again", async () => {
    const user = userEvent.setup();
    const { backend } = renderApp();
    const learning = await screen.findByRole("region", { name: "Belajar" });
    expect(within(learning).getByText(/dari 4 hari aktif/)).toBeInTheDocument();
    expect(within(learning).getByRole("link", { name: "Nmap Live Host Discovery" })).toBeInTheDocument();
    await user.click(within(learning).getByRole("button", { name: "Lanjutkan" }));
    expect(await within(learning).findByText(/Sedang belajar/)).toBeInTheDocument();
    expect(backend.calls.some((call) => call.startsWith("start_timer"))).toBe(true);

    const files = screen.getByRole("region", { name: "File terakhir" });
    expect(within(files).getAllByRole("listitem")).toHaveLength(4);
    expect(within(files).getByText("Tidak ditemukan di tempatnya.")).toBeInTheDocument();
  });
});
