import { act, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it } from "vitest";

import { createMockBackend } from "../data/mock";
import type { StudioProject } from "../data/generated/StudioProject";
import { renderApp } from "../test/renderApp";
import { historyOf } from "./StudioSide";

function project(name: string, updatedAt: string, waiting = 0): StudioProject {
  return {
    id: `00000000-0000-7000-8000-0000000000${name.length.toString().padStart(2, "0")}`,
    name,
    idea: `${name} untuk sekolah`,
    stage: "explore",
    projectId: null,
    projectName: null,
    repoPath: null,
    claudeAccess: "report",
    stackStatus: null,
    waiting,
    createdAt: updatedAt,
    updatedAt,
  };
}

const absensi = project("Absensi Lab", "2026-10-20T01:00:00Z");
const piket = project("Bot Piket Kelas", "2026-10-21T01:00:00Z", 2);
const osis = project("Web OSIS", "2026-10-22T01:00:00Z");

const sidebar = () => screen.getByRole("complementary", { name: "Navigasi" });
const history = () => within(sidebar()).getByRole("navigation", { name: "Riwayat Studio" });

afterEach(() => {
  localStorage.clear();
});

// U2: Studio's history in the sidebar.
describe("Studio's history", () => {
  it("puts what waits for the owner first, then the newest, and finds by name or idea", () => {
    const all = [absensi, piket, osis];
    expect(historyOf(all, "")).toEqual({ waiting: [piket], recent: [osis, absensi] });
    expect(historyOf(all, "osis").recent).toEqual([osis]);
    // The idea counts too, without case or accents.
    expect(historyOf(all, "SEKOLAH lab").recent).toEqual([absensi]);
    expect(historyOf(all, "zzz")).toEqual({ waiting: [], recent: [] });
  });

  it("opening Studio slides the sidebar to its history; Kembali returns to the page before", async () => {
    const user = userEvent.setup();
    const { router } = renderApp({ path: "/kalender", backend: createMockBackend({ studioProjects: [absensi, piket, osis] }) });
    await screen.findByRole("heading", { level: 1 });
    await user.click(within(sidebar()).getByRole("link", { name: /^Studio/ }));

    await waitFor(() => expect(router.state.location.pathname).toBe("/studio"));
    expect(document.querySelector("[data-pane=main]")).toHaveAttribute("inert");
    // Waiting first (with its count), then the newest.
    const groups = within(history()).getAllByRole("region");
    expect(groups.map((group) => group.getAttribute("aria-labelledby"))).toEqual(["studio-history-waiting", "studio-history-recent"]);
    expect(within(groups[0]!).getByRole("link", { name: /Bot Piket Kelas/ })).toHaveTextContent("2");
    expect(within(groups[1]!).getAllByRole("link").map((link) => link.textContent)).toEqual(["Web OSIS", "Absensi Lab"]);
    // Studio's home is Ide baru, so focus lands there.
    await waitFor(() => expect(within(sidebar()).getByRole("link", { name: "Ide baru" })).toHaveFocus());

    await user.click(within(history()).getByRole("link", { name: "Web OSIS" }));
    await waitFor(() => expect(router.state.location.pathname).toBe(`/studio/${osis.id}`));
    expect(within(history()).getByRole("link", { name: "Web OSIS" })).toHaveAttribute("aria-current", "page");

    await user.click(within(sidebar()).getByRole("button", { name: "Kembali" }));
    await waitFor(() => expect(router.state.location.pathname).toBe("/kalender"));
    expect(within(sidebar()).queryByRole("navigation", { name: "Riwayat Studio" })).not.toBeInTheDocument();
    expect(within(sidebar()).getByRole("link", { name: "Kalender" })).toBeInTheDocument();
  });

  it("searches the projects, and says when nothing matches", async () => {
    const user = userEvent.setup();
    renderApp({ path: "/studio", backend: createMockBackend({ studioProjects: [absensi, piket, osis] }) });
    await screen.findByRole("complementary", { name: "Navigasi" });
    const search = await within(sidebar()).findByRole("searchbox", { name: "Cari proyek Studio" });
    await user.type(search, "piket");
    expect(within(history()).getAllByRole("link").map((link) => link.textContent)).toEqual(["Bot Piket Kelas2"]);
    await user.clear(search);
    await user.type(search, "zzz");
    expect(within(history()).getByText("Tidak ada proyek yang cocok.")).toBeInTheDocument();
    // Esc empties the search.
    await user.keyboard("{Escape}");
    expect(search).toHaveValue("");
    expect(within(history()).getAllByRole("link")).toHaveLength(3);
  });

  it("an empty Studio says where ideas will appear", async () => {
    renderApp({ path: "/studio", backend: createMockBackend({ studioProjects: [] }) });
    await screen.findByRole("complementary", { name: "Navigasi" });
    expect(await within(sidebar()).findByText("Belum ada proyek. Ide pertamamu akan muncul di sini.")).toBeInTheDocument();
  });

  it("folds into a rail and remembers it on this computer", async () => {
    const user = userEvent.setup();
    const first = renderApp({ path: "/studio", backend: createMockBackend({ studioProjects: [piket] }) });
    await screen.findByRole("complementary", { name: "Navigasi" });
    const fold = await within(sidebar()).findByRole("button", { name: "Ciutkan riwayat" });
    expect(fold).toHaveAttribute("aria-expanded", "true");
    expect(fold).toHaveAttribute("aria-controls", "studio-history");
    await user.click(fold);
    const unfold = within(sidebar()).getByRole("button", { name: "Buka riwayat" });
    expect(unfold).toHaveAttribute("aria-expanded", "false");
    // Folded, the rail still says what waits.
    expect(within(unfold).getByLabelText("2 hal menunggu kamu")).toBeInTheDocument();
    expect(sidebar().parentElement).toHaveAttribute("data-folded");
    expect(localStorage.getItem("hub:studio-folded")).toBe("1");
    first.unmount();

    // Opened again, it is still folded; outside Studio the pages are never folded.
    const again = renderApp({ path: "/studio", backend: createMockBackend({ studioProjects: [piket] }) });
    await screen.findByRole("complementary", { name: "Navigasi" });
    expect(await within(sidebar()).findByRole("button", { name: "Buka riwayat" })).toBeInTheDocument();
    await user.click(within(sidebar()).getByRole("button", { name: "Kembali" }));
    await waitFor(() => expect(again.router.state.location.pathname).toBe("/"));
    expect(sidebar().parentElement).not.toHaveAttribute("data-folded");

    // The rail's search opens the history with the cursor in the field.
    await act(() => again.router.navigate({ to: "/studio" }));
    await user.click(await within(sidebar()).findByRole("button", { name: "Cari proyek Studio" }));
    expect(within(sidebar()).getByRole("button", { name: "Ciutkan riwayat" })).toHaveAttribute("aria-expanded", "true");
    await waitFor(() => expect(within(sidebar()).getByRole("searchbox", { name: "Cari proyek Studio" })).toHaveFocus());
    expect(localStorage.getItem("hub:studio-folded")).toBeNull();
  });

  // U4: a wide document in a project's canvas folds the history, as Gemini's Canvas does; the
  // owner can open it again, and none of it is kept.
  it("folds beside a wide document in the canvas, and opens again on request", async () => {
    const user = userEvent.setup();
    renderApp({ path: `/studio/${absensi.id}`, backend: createMockBackend({ geminiKey: "AIzaTestKeyForHubStudio0123456789abcd", studioProjects: [absensi] }) });
    await screen.findByRole("tab", { name: /Tech stack/ });
    // Ringkasan is narrow: the history stays open.
    expect(sidebar().parentElement).not.toHaveAttribute("data-folded");
    await user.click(screen.getByRole("tab", { name: /Tech stack/ }));
    await waitFor(() => expect(sidebar().parentElement).toHaveAttribute("data-folded"));
    await user.click(within(sidebar()).getByRole("button", { name: "Buka riwayat" }));
    expect(sidebar().parentElement).not.toHaveAttribute("data-folded");
    expect(within(sidebar()).getByRole("button", { name: "Ciutkan riwayat" })).toHaveAttribute("aria-expanded", "true");
    // Closing the document, or going back to Ringkasan, leaves the history as the owner keeps it.
    await user.click(screen.getByRole("tab", { name: /Mau buat apa/ }));
    expect(sidebar().parentElement).not.toHaveAttribute("data-folded");
    await user.click(screen.getByRole("tab", { name: /Tech stack/ }));
    await waitFor(() => expect(sidebar().parentElement).toHaveAttribute("data-folded"));
    await user.click(screen.getByRole("button", { name: "Sembunyikan dokumen" }));
    await waitFor(() => expect(sidebar().parentElement).not.toHaveAttribute("data-folded"));
    expect(localStorage.getItem("hub:studio-folded")).toBeNull();
  });
});
