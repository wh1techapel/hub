import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";

import { a11yProblems } from "../../test/a11y";
import { renderApp } from "../../test/renderApp";

afterEach(() => {
  vi.useRealTimers();
});

const places = () => screen.getByRole("navigation", { name: "Proyek & Tugas" });

describe("Proyek & Tugas", () => {
  it("opens on the projects by Ruang, counts each task list, and moves between them", async () => {
    const user = userEvent.setup();
    renderApp({ path: "/proyek" });
    expect(await screen.findByRole("heading", { level: 1, name: "Proyek & Tugas" })).toBeInTheDocument();
    expect(await screen.findByRole("heading", { level: 2, name: "Proyek" })).toBeInTheDocument();
    expect(await screen.findByRole("region", { name: "Sekolah" })).toHaveTextContent("Laporan Lab Jaringan");
    expect(within(places()).getByRole("button", { name: "Proyek" })).toHaveAttribute("aria-current", "page");
    const lists = within(places()).getByRole("group", { name: "Tugas" });
    expect(await within(lists).findByRole("button", { name: "Kotak Masuk 1" })).toBeInTheDocument();
    // Hari Ini's list is the dashboard's now, and Pengingat is Kalender's.
    expect(within(places()).queryByRole("button", { name: /Hari Ini/ })).not.toBeInTheDocument();
    expect(within(places()).queryByRole("button", { name: /Pengingat/ })).not.toBeInTheDocument();

    await user.click(within(lists).getByRole("button", { name: /Kotak Masuk/ }));
    expect(await screen.findByText("Ide: dashboard homelab")).toBeInTheDocument();
    expect(screen.getByRole("heading", { level: 2, name: "Kotak Masuk" })).toBeInTheDocument();
    await user.click(within(lists).getByRole("button", { name: /Mendatang/ }));
    expect(await screen.findByRole("heading", { level: 3, name: /Sabtu/ })).toBeInTheDocument();
    expect(a11yProblems()).toEqual([]);
    await user.click(within(places()).getByRole("button", { name: "Proyek" }));
    expect(await screen.findByRole("region", { name: "Belajar Cyber" })).toBeInTheDocument();
    expect(a11yProblems()).toEqual([]);
  });

  it("adds a task from the line at the top", async () => {
    const user = userEvent.setup();
    const { backend } = renderApp({ path: "/proyek?view=inbox" });
    const line = await screen.findByRole("textbox", { name: /Tugas/ });
    await user.type(line, "cari referensi VLAN{Enter}");
    expect(await within(screen.getByRole("status")).findByText("Tersimpan: cari referensi VLAN")).toBeInTheDocument();
    expect(await screen.findByRole("button", { name: "cari referensi VLAN" })).toBeInTheDocument();
    expect(backend.calls.some((call) => call.startsWith("save_capture"))).toBe(true);
  });

  it("opens a task beside the list and saves each change as it is made", async () => {
    const user = userEvent.setup();
    const { backend } = renderApp({ path: "/proyek?view=anytime" });
    await user.click(await screen.findByRole("button", { name: "Revisi slide presentasi KIK" }));
    const panel = await screen.findByRole("complementary", { name: "Detail tugas" });
    expect(within(panel).getByLabelText("Judul")).toHaveValue("Revisi slide presentasi KIK");

    await user.click(within(panel).getByRole("radio", { name: "!!" }));
    await waitFor(() => expect(backend.calls.some((call) => call.startsWith("update_task") && call.includes('"priority":2'))).toBe(true));

    await user.type(within(panel).getByRole("textbox", { name: "Tambah langkah" }), "Ganti foto sampul{Enter}");
    expect(await within(panel).findByRole("button", { name: "Ganti foto sampul" })).toBeInTheDocument();

    // Focus already holds three tasks.
    expect(within(panel).getByRole("button", { name: "Jadikan fokus hari ini" })).toBeDisabled();
    expect(within(panel).getByText("Fokus hari ini sudah tiga.")).toBeInTheDocument();

    // CP02: a Sesi Fokus for this task, in Lentera's capsule.
    await user.click(within(panel).getByRole("button", { name: "Fokus 25 m" }));
    await waitFor(() => expect(backend.calls.some((call) => call.startsWith("start_focus") && call.includes('"taskId"'))).toBe(true));
    const session = await backend.call<{ title: string } | null>("get_focus");
    expect(session?.title).toBe("Revisi slide presentasi KIK");

    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("complementary", { name: "Detail tugas" })).not.toBeInTheDocument());
  });

  it("deletes with the keyboard and can bring it back", async () => {
    const user = userEvent.setup();
    const { backend } = renderApp({ path: "/proyek?view=anytime" });
    const row = await screen.findByRole("button", { name: "Ide: dashboard homelab" });
    row.focus();
    await user.keyboard("{Delete}");
    await waitFor(() => expect(screen.queryByRole("button", { name: "Ide: dashboard homelab" })).not.toBeInTheDocument());
    await user.click(within(screen.getByRole("status")).getByRole("button", { name: "Urungkan" }));
    expect(await screen.findByRole("button", { name: "Ide: dashboard homelab" })).toBeInTheDocument();
    expect(backend.calls.some((call) => call.startsWith("restore_task"))).toBe(true);
  });

  it("a task on Hari Ini opens here, beside every open task", async () => {
    const user = userEvent.setup();
    const { router } = renderApp({ path: "/" });
    await user.click(await screen.findByRole("button", { name: "Revisi slide presentasi KIK" }));
    expect(await screen.findByRole("complementary", { name: "Detail tugas" })).toBeInTheDocument();
    expect(router.state.location.pathname).toBe("/proyek");
    expect(screen.getByRole("heading", { level: 2, name: "Semua" })).toBeInTheDocument();
  });
});

describe("Old addresses (before R6)", () => {
  it("each Tugas list leads to its place now", async () => {
    const { router } = renderApp({ path: "/tugas?view=inbox" });
    expect(await screen.findByRole("heading", { level: 2, name: "Kotak Masuk" })).toBeInTheDocument();
    expect(router.state.location.pathname).toBe("/proyek");
    expect(router.state.location.search).toEqual({ view: "inbox" });
  });

  it("Tugas alone opens every open task, and a task named in it opens beside them", async () => {
    const { router } = renderApp({ path: "/tugas" });
    expect(await screen.findByRole("heading", { level: 2, name: "Semua" })).toBeInTheDocument();
    expect(router.state.location.search).toEqual({ view: "anytime" });
  });

  it("Hari Ini's list leads to Hari Ini, unless it names a task", async () => {
    const { router } = renderApp({ path: "/tugas?view=today" });
    await waitFor(() => expect(router.state.location.pathname).toBe("/"));
  });

  it("Pengingat leads to Kalender", async () => {
    const { router } = renderApp({ path: "/tugas?view=reminders" });
    expect(await screen.findByRole("heading", { level: 1, name: "Pengingat" })).toBeInTheDocument();
    expect(router.state.location.pathname).toBe("/kalender");
    expect(router.state.location.search).toEqual({ view: "pengingat" });
  });
});
