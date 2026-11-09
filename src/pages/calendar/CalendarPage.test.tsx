import { act, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";

import { a11yProblems } from "../../test/a11y";
import { renderApp } from "../../test/renderApp";

afterEach(() => {
  vi.useRealTimers();
});

const lessons = (name: RegExp) => screen.queryAllByRole("button", { name });

describe("Kalender", () => {
  it("shows the week from Monday with routines and events, and switches views by key", async () => {
    const user = userEvent.setup();
    renderApp({ path: "/kalender" });
    await waitFor(() => expect(lessons(/^Produktif TJKT, 13.00 sampai 14.30/)).toHaveLength(4));
    expect(screen.getByRole("button", { name: /^Rapat OSIS, 15.00 sampai 16.30, Aula/ })).toBeInTheDocument();

    await user.keyboard("d");
    await waitFor(() => expect(lessons(/^Produktif TJKT/)).toHaveLength(1));
    await user.keyboard("m");
    expect(await screen.findByRole("button", { name: /^22, \d+ jadwal/ })).toBeInTheDocument();
  });

  it("makes an event with N and keeps it after saving", async () => {
    const user = userEvent.setup();
    const { backend } = renderApp({ path: "/kalender" });
    await waitFor(() => expect(lessons(/^Produktif TJKT/).length).toBeGreaterThan(0));
    await user.keyboard("n");
    const dialog = await screen.findByRole("dialog", { name: "Jadwal baru" });
    await user.type(within(dialog).getByLabelText("Judul"), "Les gitar");
    await user.click(within(dialog).getByRole("button", { name: "Simpan" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(await screen.findByRole("button", { name: /^Les gitar, 12.45 sampai 13.45/ })).toBeInTheDocument();
    expect(backend.calls.some((call) => call.startsWith("create_event"))).toBe(true);
  });

  it("edits an event and closes the sheet once saved", async () => {
    const user = userEvent.setup();
    const { backend } = renderApp({ path: "/kalender" });
    await user.click(await screen.findByRole("button", { name: /^Rapat OSIS/ }));
    const dialog = await screen.findByRole("dialog", { name: "Ubah jadwal" });
    const title = within(dialog).getByLabelText("Judul");
    expect(title).toHaveValue("Rapat OSIS");
    await user.clear(title);
    await user.type(title, "Rapat OSIS (pleno)");
    await user.click(within(dialog).getByRole("button", { name: "Simpan" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(await screen.findByRole("button", { name: /^Rapat OSIS \(pleno\)/ })).toBeInTheDocument();
    expect(backend.calls.some((call) => call.startsWith("update_event"))).toBe(true);
  });

  it("skips one lesson without touching the others", async () => {
    const user = userEvent.setup();
    const { backend } = renderApp({ path: "/kalender" });
    await waitFor(() => expect(lessons(/^Matematika/)).toHaveLength(2));
    await user.click(lessons(/^Matematika/)[1]!);
    const dialog = await screen.findByRole("dialog", { name: "Matematika" });
    await user.click(within(dialog).getByRole("button", { name: "Lewati kali ini" }));
    await waitFor(() => expect(lessons(/^Matematika/)).toHaveLength(1));
    expect(backend.calls.some((call) => call.startsWith("skip_occurrence"))).toBe(true);
  });

  it("schedules a task from the tray as a time block", async () => {
    const user = userEvent.setup();
    const { backend } = renderApp({ path: "/kalender" });
    const tray = await screen.findByRole("complementary", { name: "Tugas tanpa jam" });
    await user.click(await within(tray).findByRole("button", { name: "Jadwalkan: THM: Nmap Live Host Discovery" }));
    const dialog = await screen.findByRole("dialog", { name: "Jadwal baru" });
    expect(dialog).toHaveTextContent("Untuk tugas: THM: Nmap Live Host Discovery");
    await user.click(within(dialog).getByRole("button", { name: "Simpan" }));
    await waitFor(() => expect(backend.calls.some((call) => call.startsWith("create_event") && call.includes('"taskId"'))).toBe(true));
    await waitFor(() => expect(within(tray).queryByText("THM: Nmap Live Host Discovery")).not.toBeInTheDocument());
  });

  it("holds Pengingat as a fourth view: what went off, what is to come, and what was handled", async () => {
    const user = userEvent.setup();
    const { backend, router } = renderApp({ path: "/kalender", at: new Date(2026, 9, 22, 12, 0) });
    await waitFor(() => expect(lessons(/^Produktif TJKT/)).toHaveLength(4));
    await user.keyboard("p");
    expect(await screen.findByRole("heading", { level: 1, name: "Pengingat" })).toBeInTheDocument();
    expect(router.state.location.search).toMatchObject({ view: "pengingat" });
    // A list, not a span of time: no stepping through days here.
    expect(screen.queryByRole("button", { name: "Sebelumnya" })).not.toBeInTheDocument();
    const upcoming = await screen.findByRole("region", { name: "Akan datang" });
    expect(within(upcoming).getByText("Kumpulkan laporan di Classroom")).toBeInTheDocument();
    expect(a11yProblems()).toEqual([]);

    vi.setSystemTime(new Date(2026, 9, 22, 21, 1));
    act(() => backend.fireDue());
    const waiting = await screen.findByRole("region", { name: "Menunggu kamu" });
    await user.click(within(waiting).getByRole("button", { name: "Selesai" }));
    expect(await screen.findByRole("region", { name: "Minggu ini" })).toHaveTextContent("Kumpulkan laporan di Classroom");
    expect(backend.calls.some((call) => call.startsWith("done_reminder"))).toBe(true);

    // Back to the week, where the owner was.
    await user.click(screen.getByRole("radio", { name: "Minggu" }));
    await waitFor(() => expect(lessons(/^Produktif TJKT/)).toHaveLength(4));
  });
});
