import { act, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";

import { createMockBackend } from "../../data/mock";
import { renderApp } from "../../test/renderApp";

afterEach(() => {
  vi.useRealTimers();
});

describe("Pengaturan", () => {
  it("a new ink is saved and worn at once", async () => {
    const user = userEvent.setup();
    const { backend } = renderApp({ path: "/pengaturan/tampilan" });
    await user.click(await screen.findByRole("radio", { name: "Hijau Botol" }));
    await waitFor(() => expect(document.documentElement.dataset.ink).toBe("hijau"));
    expect(backend.calls).toContain('update_settings {"patch":{"ink":"hijau"}}');
  });

  it("the light mode can be fixed to dark", async () => {
    const user = userEvent.setup();
    renderApp({ path: "/pengaturan/tampilan", at: new Date(2026, 9, 22, 12, 0) });
    await user.click(await screen.findByRole("radio", { name: "Gelap" }));
    await waitFor(() => expect(document.documentElement.dataset.phase).toBe("malam"));
  });

  it("an empty name is refused next to the field", async () => {
    const user = userEvent.setup();
    renderApp({ path: "/pengaturan/hari" });
    const name = await screen.findByLabelText("Nama di sapaan");
    await user.clear(name);
    await user.type(name, "   {Enter}");
    expect(await screen.findByText("Nama harus 1 sampai 40 huruf.")).toBeInTheDocument();
    expect(name).toHaveAttribute("aria-invalid", "true");
  });

  it("the autostart switch says its state", async () => {
    const user = userEvent.setup();
    const { backend } = renderApp({ path: "/pengaturan/sistem" });
    const toggle = await screen.findByRole("switch", { name: "Jalan saat Windows menyala" });
    expect(toggle).toHaveAttribute("aria-checked", "true");
    await user.click(toggle);
    await waitFor(() => expect(toggle).toHaveAttribute("aria-checked", "false"));
    expect(backend.calls).toContain('update_settings {"patch":{"autostart":false}}');
  });

  it("the Tutup Hari nudge can be moved or turned off", async () => {
    const user = userEvent.setup();
    const { backend } = renderApp({ path: "/pengaturan/hari" });
    const time = await screen.findByLabelText("Jam pengingat Tutup Hari");
    expect(time).toHaveValue("21:00");
    await user.clear(time);
    await user.type(time, "20:30");
    await waitFor(() => expect(backend.calls).toContain('update_settings {"patch":{"closeNudgeTime":"20:30"}}'));
    await user.click(screen.getByRole("switch", { name: "Ingatkan Tutup Hari" }));
    await waitFor(() => expect(screen.queryByLabelText("Jam pengingat Tutup Hari")).not.toBeInTheDocument());
  });

  it("exports everything to a file the owner picks and says where", async () => {
    const user = userEvent.setup();
    const { backend } = renderApp({ path: "/pengaturan/sistem" });
    await user.click(await screen.findByRole("button", { name: "Ekspor semua data" }));
    expect(await screen.findByText(/^Tersimpan di C:\\Users\\kamu\\Documents\\Hub-ekspor-\d{4}-\d{2}-\d{2}\.json\.$/)).toBeInTheDocument();
    expect(backend.calls.some((call) => call.startsWith("export_data"))).toBe(true);
  });

  it("says whether the capture hotkey is active", async () => {
    renderApp({ path: "/pengaturan/sistem" });
    expect(await screen.findByText("Tekan Ctrl+Alt+H di aplikasi apa pun.")).toBeInTheDocument();
    expect(screen.getByText("Tekan Ctrl+Alt+L di aplikasi apa pun. Di dalam Hub juga Ctrl+L.")).toBeInTheDocument();
  });

  it("Sesi Fokus's lengths are the owner's, within what makes sense", async () => {
    const user = userEvent.setup();
    const { backend } = renderApp({ path: "/pengaturan/hari" });
    const focus = await screen.findByRole("region", { name: "Sesi Fokus" });
    const length = within(focus).getByLabelText("Lama fokus (menit)");
    expect(length).toHaveValue(25);
    await user.clear(length);
    await user.type(length, "50");
    await waitFor(() => expect(backend.calls).toContain('update_settings {"patch":{"focusMinutes":50}}'));
    // Out of range is not sent at all.
    const every = within(focus).getByLabelText("Istirahat panjang setiap (blok)");
    await user.clear(every);
    await user.type(every, "12");
    expect(backend.calls.some((call) => call.includes("longBreakEvery"))).toBe(false);
  });

  it("says when another application holds Lentera's Ctrl+Alt+L", async () => {
    const backend = createMockBackend();
    const call = backend.call;
    backend.call = <T,>(command: Parameters<typeof call>[0], args?: Record<string, unknown>) =>
      command === "get_shell"
        ? Promise.resolve({ hotkeyOk: true, hotkey: "Ctrl+Alt+H", lenteraHotkeyOk: false, lenteraHotkey: "Ctrl+Alt+L" } as T)
        : call<T>(command, args);
    renderApp({ path: "/pengaturan/sistem", backend });
    expect(await screen.findByText("Ctrl+Alt+L sedang dipakai aplikasi lain, jadi belum aktif.")).toBeInTheDocument();
    expect(screen.getByText("Tekan Ctrl+Alt+H di aplikasi apa pun.")).toBeInTheDocument();
  });

  it("ULTRON's access says where it runs, and can move, rotate, or stop", async () => {
    const user = userEvent.setup();
    const { backend } = renderApp({ path: "/pengaturan/ultron" });
    expect(await screen.findByText("Aktif di 127.0.0.1:47731.")).toBeInTheDocument();
    expect(screen.getByText("ULTRON sedang mendengarkan pengingat.")).toBeInTheDocument();

    const port = screen.getByLabelText("Port");
    await user.clear(port);
    await user.type(port, "80{Enter}");
    expect(await screen.findByText("Port harus angka 1024 sampai 65535.")).toBeInTheDocument();
    await user.clear(port);
    await user.type(port, "48000{Enter}");
    await waitFor(() => expect(backend.calls).toContain('update_settings {"patch":{"apiPort":48000}}'));

    await user.click(screen.getByRole("button", { name: "Buat token baru" }));
    expect(await screen.findByText("Token baru sudah dibuat.")).toBeInTheDocument();
    expect(backend.calls.some((call) => call.startsWith("rotate_api_token"))).toBe(true);

    const toggle = screen.getByRole("switch", { name: "Izinkan ULTRON membaca dan menulis" });
    await user.click(toggle);
    expect(await screen.findByText("Mati. ULTRON tidak bisa membaca atau mengubah apa pun di Hub.")).toBeInTheDocument();
    expect(screen.queryByLabelText("Port")).not.toBeInTheDocument();
  });

  it("the Belajar goal is a number of active days", async () => {
    const user = userEvent.setup();
    const { backend } = renderApp({ path: "/pengaturan/hari" });
    const goal = await screen.findByRole("radiogroup", { name: "Target hari aktif per minggu" });
    expect(within(goal).getByRole("radio", { name: "4" })).toBeChecked();
    await user.click(within(goal).getByRole("radio", { name: "5" }));
    await waitFor(() => expect(backend.calls).toContain('update_settings {"patch":{"learningGoalDays":5}}'));
  });
});

// P1: Pengaturan in five parts, and the sidebar that slides over to them.
describe("Pengaturan in parts", () => {
  const sidebar = () => screen.getByRole("complementary", { name: "Navigasi" });

  it("opening it slides the sidebar to its parts; Kembali returns to the page before", async () => {
    const user = userEvent.setup();
    const { router } = renderApp({ path: "/proyek?view=inbox" });
    await screen.findByRole("heading", { level: 2, name: "Kotak Masuk" });
    // Opening the app moves no focus into the sidebar.
    expect(document.body).toHaveFocus();
    await user.click(within(sidebar()).getByRole("link", { name: "Pengaturan" }));

    expect(await screen.findByRole("heading", { level: 1, name: "Tampilan" })).toBeInTheDocument();
    expect(router.state.location.pathname).toBe("/pengaturan/tampilan");
    // Only the parts are there for the owner now: the pages' side is hidden and out of reach.
    const parts = within(sidebar()).getByRole("navigation", { name: "Pengaturan" });
    expect(within(parts).getAllByRole("link").map((link) => link.textContent)).toEqual(["Tampilan", "Hari & kebiasaan", "Integrasi AI", "ULTRON", "Sistem & data"]);
    expect(within(parts).getByRole("link", { name: "Tampilan" })).toHaveAttribute("aria-current", "page");
    expect(within(sidebar()).queryByRole("link", { name: "Proyek & Tugas" })).not.toBeInTheDocument();
    expect(document.querySelector("[data-pane=main]")).toHaveAttribute("inert");
    // Focus came along to the side that slid in.
    await waitFor(() => expect(within(parts).getByRole("link", { name: "Tampilan" })).toHaveFocus());

    await user.click(within(parts).getByRole("link", { name: "Integrasi AI" }));
    expect(await screen.findByRole("heading", { level: 1, name: "Integrasi AI" })).toBeInTheDocument();
    expect(screen.getByRole("region", { name: "Google Gemini" })).toBeInTheDocument();
    expect(screen.getByRole("region", { name: "Profil developer" })).toBeInTheDocument();
    expect(screen.getByRole("region", { name: "Claude Code" })).toBeInTheDocument();
    // Each part holds only its own: no ULTRON here.
    expect(screen.queryByRole("switch", { name: "Izinkan ULTRON membaca dan menulis" })).not.toBeInTheDocument();

    await user.click(within(sidebar()).getByRole("button", { name: "Kembali" }));
    expect(await screen.findByRole("heading", { level: 2, name: "Kotak Masuk" })).toBeInTheDocument();
    await waitFor(() => expect(within(sidebar()).getByRole("link", { name: "Proyek & Tugas" })).toHaveFocus());
    expect(router.state.location.pathname).toBe("/proyek");
    expect(router.state.location.search).toEqual({ view: "inbox" });
    expect(within(sidebar()).getByRole("link", { name: "Proyek & Tugas" })).toBeInTheDocument();
    expect(within(sidebar()).queryByRole("navigation", { name: "Pengaturan" })).not.toBeInTheDocument();
  });

  it("opened straight from an address, Kembali goes to Hari Ini; old and unknown addresses open the first part", async () => {
    const user = userEvent.setup();
    const { router } = renderApp({ path: "/pengaturan" });
    expect(await screen.findByRole("heading", { level: 1, name: "Tampilan" })).toBeInTheDocument();
    expect(router.state.location.pathname).toBe("/pengaturan/tampilan");
    await act(() => router.navigate({ to: "/pengaturan/$bagian", params: { bagian: "lainnya" } }));
    await waitFor(() => expect(router.state.location.pathname).toBe("/pengaturan/tampilan"));
    await user.click(within(sidebar()).getByRole("button", { name: "Kembali" }));
    await waitFor(() => expect(router.state.location.pathname).toBe("/"));
  });

  it("each part is reachable from the sidebar with its own controls", async () => {
    const user = userEvent.setup();
    renderApp({ path: "/pengaturan/tampilan" });
    await screen.findByRole("heading", { level: 1, name: "Tampilan" });
    const parts = within(sidebar()).getByRole("navigation", { name: "Pengaturan" });
    const expected: [string, string][] = [
      ["Hari & kebiasaan", "Nama di sapaan"],
      ["ULTRON", "Izinkan ULTRON membaca dan menulis"],
      ["Sistem & data", "Jalan saat Windows menyala"],
      ["Tampilan", "Gelap"],
    ];
    for (const [part, control] of expected) {
      await user.click(within(parts).getByRole("link", { name: part }));
      expect(await screen.findByRole("heading", { level: 1, name: part })).toBeInTheDocument();
      expect(screen.getAllByLabelText(control).length + screen.queryAllByRole("switch", { name: control }).length + screen.queryAllByRole("radio", { name: control }).length).toBeGreaterThan(0);
    }
  });
});
