import { act, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";

import { renderApp } from "../test/renderApp";
import { matchPages } from "./nav";

afterEach(() => {
  vi.useRealTimers();
});

describe("navigation", () => {
  it("matches pages by name and by what they hold, without accents or case", () => {
    expect(matchPages("pengat").map((page) => page.path)).toEqual(["/pengaturan"]);
    expect(matchPages("tryhackme").map((page) => page.path)).toEqual(["/belajar"]);
    expect(matchPages("").length).toBe(8);
    // R6: tasks are in Proyek & Tugas, reminders in Kalender.
    expect(matchPages("tugas").map((page) => page.path)).toEqual(["/proyek"]);
    expect(matchPages("pengingat").map((page) => page.path)).toEqual(["/kalender"]);
    expect(matchPages("zzz")).toEqual([]);
  });

  it("Ctrl+K opens the palette, typing filters, Enter goes there", async () => {
    const user = userEvent.setup();
    renderApp();
    await screen.findByRole("heading", { level: 1 });
    await user.keyboard("{Control>}k{/Control}");
    const input = await screen.findByRole("combobox");
    expect(input).toHaveFocus();
    await user.type(input, "pengat");
    expect(screen.getAllByRole("option")).toHaveLength(1);
    await user.keyboard("{Enter}");
    // Pengaturan opens on its first part (P1).
    expect(await screen.findByRole("heading", { level: 1, name: "Tampilan" })).toBeInTheDocument();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("Escape closes the palette and arrows move the choice", async () => {
    const user = userEvent.setup();
    renderApp();
    await screen.findByRole("heading", { level: 1 });
    await user.click(screen.getByRole("button", { name: /Cari atau perintah/ }));
    await screen.findByRole("dialog");
    await user.keyboard("{ArrowDown}");
    expect(screen.getByRole("option", { name: /^Kalender/ })).toHaveAttribute("aria-selected", "true");
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
  });

  it("the palette can capture a line and make a task from it", async () => {
    const user = userEvent.setup();
    const { backend } = renderApp();
    await screen.findByRole("heading", { level: 1 });
    await user.click(screen.getByRole("button", { name: "Catat cepat" }));
    const dialog = await screen.findByRole("dialog", { name: "Catat Cepat" });
    const line = within(dialog).getByRole("textbox");
    await waitFor(() => expect(line).toHaveFocus());
    await user.type(line, "beli tinta printer{Enter}");
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(backend.calls.some((call) => call.startsWith("save_capture") && call.includes("beli tinta printer"))).toBe(true);
    expect(within(screen.getByRole("status")).getByText("Tugas tersimpan")).toBeInTheDocument();
  });

  it("a line that starts with “ingatkan” becomes a reminder", async () => {
    const user = userEvent.setup();
    renderApp();
    await screen.findByRole("heading", { level: 1 });
    await user.click(screen.getByRole("button", { name: "Catat cepat" }));
    const dialog = await screen.findByRole("dialog", { name: "Catat Cepat" });
    expect(within(dialog).getByRole("radio", { name: "Tugas" })).toHaveAttribute("aria-checked", "true");
    await user.type(within(dialog).getByRole("textbox"), "ingatkan aku minum obat");
    expect(within(dialog).getByRole("radio", { name: "Pengingat" })).toHaveAttribute("aria-checked", "true");
  });

  it("finds projects and commands by name", async () => {
    const user = userEvent.setup();
    renderApp();
    await screen.findByRole("heading", { level: 1 });
    await user.keyboard("{Control>}k{/Control}");
    await user.type(await screen.findByRole("combobox"), "kik");
    await user.keyboard("{Enter}");
    expect(await screen.findByRole("heading", { level: 1, name: "Presentasi KIK" })).toBeInTheDocument();
  });

  it("goes where a notification sends it", async () => {
    const { backend } = renderApp();
    await screen.findByRole("heading", { level: 1 });
    // An address from before R6: Pengingat is in Kalender now.
    act(() => backend.send("hub://navigate", "/tugas?view=reminders"));
    expect(await screen.findByRole("heading", { level: 1, name: "Pengingat" })).toBeInTheDocument();
    act(() => backend.send("hub://navigate", "https://example.com"));
    expect(screen.getByRole("heading", { level: 1, name: "Pengingat" })).toBeInTheDocument();
  });

  it("holds a reminder that went off above every page until it is handled", async () => {
    const user = userEvent.setup();
    const { backend } = renderApp({ at: new Date(2026, 9, 22, 21, 5) });
    await screen.findByRole("heading", { level: 1 });
    act(() => backend.fireDue());
    const alert = await screen.findByText("Kumpulkan laporan di Classroom", { selector: "strong" });
    await user.click(within(alert.closest("div")!).getByRole("button", { name: "Tunda 10 m" }));
    await waitFor(() => expect(screen.queryByText("Kumpulkan laporan di Classroom", { selector: "strong" })).not.toBeInTheDocument());
    expect(backend.calls.some((call) => call.startsWith("snooze_reminder"))).toBe(true);
  });

  it("the sidebar holds the main pages, then File and Jejak under a line, then Pengaturan", async () => {
    renderApp({ path: "/proyek/x" });
    const sidebar = await screen.findByRole("complementary", { name: "Navigasi" });
    const nav = within(sidebar).getByRole("navigation");
    const names = within(nav)
      .getAllByRole("link")
      .map((link) => link.textContent);
    expect(names).toEqual(["Hari Ini", "Kalender", "Proyek & Tugas", "Studio", "Belajar", "File", "Jejak"]);
    const line = within(nav).getByRole("separator");
    expect(line.previousElementSibling).toHaveTextContent("Belajar");
    expect(line.nextElementSibling).toHaveTextContent("File");
    expect(within(sidebar).getAllByRole("link").at(-1)).toHaveTextContent("Pengaturan");
    // A project's own page is inside Proyek & Tugas.
    expect(within(nav).getByRole("link", { name: "Proyek & Tugas" })).toHaveAttribute("aria-current", "page");
  });

  it("every page in the sidebar is built and marks itself", async () => {
    renderApp({ path: "/belajar" });
    expect(await screen.findByRole("heading", { level: 1, name: "Belajar" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Belajar" })).toHaveAttribute("aria-current", "page");
  });

  it("files dropped from Explorer go to the pool under the pointer, else to the Library", async () => {
    const user = userEvent.setup();
    const { backend } = renderApp({ path: "/proyek" });
    await user.click(await screen.findByRole("link", { name: /Laporan Lab Jaringan/ }));
    await screen.findByRole("heading", { level: 1, name: "Laporan Lab Jaringan" });

    // While files hover, the pools show: the Library and the open project.
    act(() => backend.send("hub://drag", { phase: "enter", x: 0, y: 0, dropId: null, count: 2, names: ["a.pdf", "b.pdf"] }));
    const overlay = await screen.findByRole("dialog", { name: "Lepaskan untuk menyimpan" });
    expect(within(overlay).getByText("2 file: a.pdf, b.pdf")).toBeInTheDocument();
    const pools = [...overlay.querySelectorAll<HTMLElement>("[data-pool]")].map((pool) => pool.dataset.pool);
    expect(pools).toEqual(["library", "project"]);

    // jsdom has no layout: the pool "under the pointer" is the one elementFromPoint names.
    const project = overlay.querySelector<HTMLElement>("[data-pool=project]")!;
    const at = vi.spyOn(document, "elementFromPoint").mockReturnValue(project);
    act(() => backend.dropFiles(["laporan-final.pdf"], 400, 300));
    at.mockRestore();
    expect(await within(screen.getByRole("status")).findByText("File dilampirkan ke Laporan Lab Jaringan")).toBeInTheDocument();
    expect(screen.queryByRole("dialog", { name: "Lepaskan untuk menyimpan" })).not.toBeInTheDocument();
    const attached = screen.getByRole("region", { name: "File" });
    expect(await within(attached).findByText("laporan-final.pdf")).toBeInTheDocument();
    expect(backend.calls.some((call) => call.startsWith("keep_drop") && call.includes('"project"'))).toBe(true);

    // Anywhere else: the Library, with Urungkan.
    act(() => backend.dropFiles(["catatan-rapat.txt"], 5, 5));
    const status = screen.getByRole("status");
    expect(await within(status).findByText("File tersimpan")).toBeInTheDocument();
    await user.click(within(status).getByRole("button", { name: "Urungkan" }));
    await waitFor(() => expect(backend.calls.some((call) => call.startsWith("delete_file"))).toBe(true));
  });

  it("the palette finds tasks, files, and learning items by any word", async () => {
    const user = userEvent.setup();
    renderApp();
    await screen.findByRole("heading", { level: 1 });
    await user.keyboard("{Control>}k{/Control}");
    await user.type(await screen.findByRole("combobox"), "nmap");
    const options = await screen.findAllByRole("option");
    await waitFor(() => expect(screen.getAllByRole("option").length).toBeGreaterThanOrEqual(3));
    expect(options.length).toBeGreaterThan(0);
    const file = screen.getByRole("option", { name: /nmap-cheatsheet\.pdf/ });
    await user.click(file);
    expect(await screen.findByRole("complementary", { name: "Detail file" })).toBeInTheDocument();
    expect(screen.getByLabelText("Nama di Hub")).toHaveValue("nmap-cheatsheet.pdf");
  });
});
