import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";

import { renderApp } from "../../test/renderApp";

afterEach(() => {
  vi.useRealTimers();
});

describe("Belajar", () => {
  it("shows the week against the goal in active days, and the owner's tracks", async () => {
    renderApp({ path: "/belajar" });
    const week = await screen.findByRole("region", { name: "Minggu ini" });
    expect(within(week).getByText(/dari 4 hari aktif/)).toBeInTheDocument();
    const tabs = screen.getByRole("tablist", { name: "Jalur" });
    expect(
      within(tabs)
        .getAllByRole("tab")
        .map((tab) => tab.textContent.replace(/\d+\/\d+$/, "")),
    ).toEqual(["TryHackMe", "Hack The Box", "CTF", "Lab Sekolah", "Homelab"]);
    expect(within(tabs).getByRole("tab", { name: /TryHackMe/ })).toHaveAttribute("aria-selected", "true");
    expect(await screen.findByRole("region", { name: /Sedang/ })).toHaveTextContent("Nmap Live Host Discovery");
  });

  it("times a session: Lanjutkan starts the timer, Berhenti saves the minutes", async () => {
    const user = userEvent.setup();
    const { backend } = renderApp({ path: "/belajar" });
    await user.click(await screen.findByRole("button", { name: "Lanjutkan: OWASP Top 10 - 2021" }));
    expect(await screen.findByText(/Sedang belajar · TryHackMe/)).toBeInTheDocument();
    // The item moved from Belum to Sedang when the timer started.
    expect(await screen.findByRole("region", { name: /Sedang/ })).toHaveTextContent("OWASP Top 10 - 2021");

    await user.click(screen.getByRole("button", { name: "Berhenti" }));
    await waitFor(() => expect(screen.queryByText(/Sedang belajar · TryHackMe/)).not.toBeInTheDocument());
    expect(backend.calls.some((call) => call.startsWith("stop_timer"))).toBe(true);
    expect(await screen.findByText(/Sesi 1 m tersimpan\./)).toBeInTheDocument();
  });

  it("a Sesi Fokus on the item stops Belajar's own timer: one clock at a time", async () => {
    const user = userEvent.setup();
    const { backend } = renderApp({ path: "/belajar" });
    const current = await screen.findByRole("region", { name: "Lanjutkan belajar" });
    await user.click(within(current).getByRole("button", { name: "Fokus 25 m" }));
    await waitFor(() => expect(backend.calls.some((call) => call.startsWith("start_focus") && call.includes('"itemId"'))).toBe(true));
    expect((await backend.call<{ itemId: string | null } | null>("get_focus"))?.itemId).not.toBeNull();
    // Belajar's own timer started: the session ends.
    await user.click(await screen.findByRole("button", { name: "Lanjutkan: OWASP Top 10 - 2021" }));
    expect(await screen.findByText(/Sedang belajar · TryHackMe/)).toBeInTheDocument();
    expect(await backend.call("get_focus")).toBeNull();
  });

  it("adds an item to the open track and edits it beside the list", async () => {
    const user = userEvent.setup();
    const { backend } = renderApp({ path: "/belajar" });
    await user.click(await screen.findByRole("tab", { name: /Hack The Box/ }));
    await user.type(await screen.findByRole("textbox", { name: "Materi baru" }), "Blue{Enter}");
    expect(await screen.findByRole("button", { name: /^Blue/ })).toBeInTheDocument();
    expect(backend.calls.some((call) => call.startsWith("create_learning") && call.includes('"room"'))).toBe(true);

    await user.click(screen.getByRole("button", { name: /^Blue/ }));
    const panel = await screen.findByRole("complementary", { name: "Detail materi" });
    await user.selectOptions(within(panel).getByLabelText("Domain"), "Windows/AD");
    await user.click(within(panel).getByRole("radio", { name: "Selesai" }));
    await waitFor(() => expect(backend.calls.some((call) => call.startsWith("update_learning") && call.includes('"windows"'))).toBe(true));
    expect(await screen.findByRole("region", { name: /Selesai/ })).toHaveTextContent("Blue");

    await user.click(within(panel).getByRole("button", { name: "Catat sesi" }));
    expect(await within(panel).findByText("45 m")).toBeInTheDocument();
  });

  it("the heatmap says each day in words and walks with the arrow keys", async () => {
    renderApp({ path: "/belajar" });
    const grid = await screen.findByRole("grid", { name: "20 minggu terakhir" });
    const focused = grid.querySelector<HTMLElement>("[tabindex='0']");
    expect(focused?.getAttribute("aria-label")).toMatch(/^Kamis, 22 Oktober: /);
    expect(screen.getByText(/hari belajar dalam 20 minggu, total/)).toBeInTheDocument();
    focused?.focus();
    fireEvent.keyDown(grid, { key: "ArrowLeft" });
    expect(document.activeElement?.getAttribute("aria-label")).toMatch(/^Kamis, 15 Oktober: /);
    fireEvent.keyDown(grid, { key: "ArrowUp" });
    expect(document.activeElement?.getAttribute("aria-label")).toMatch(/^Rabu, 14 Oktober: /);
  });

  it("a CTF track lists its competitions", async () => {
    const user = userEvent.setup();
    renderApp({ path: "/belajar" });
    await user.click(await screen.findByRole("tab", { name: /CTF/ }));
    const events = await screen.findByRole("region", { name: "Kompetisi" });
    expect(within(events).getByText("LKS Cyber Security Kabupaten")).toBeInTheDocument();
    expect(within(events).getByText(/Peringkat 3 dari 24 · 2 dari 3 soal, 300 poin/)).toBeInTheDocument();
  });
});
