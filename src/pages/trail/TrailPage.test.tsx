import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";

import { renderApp } from "../../test/renderApp";

afterEach(() => {
  vi.useRealTimers();
});

describe("Jejak", () => {
  it("holds this week against the four weeks before, in words", async () => {
    renderApp({ path: "/jejak" });
    const week = await screen.findByRole("list", { name: "Minggu ini" });
    const measures = within(week).getAllByRole("listitem");
    expect(measures.map((m) => m.querySelector("span")?.textContent)).toEqual([
      "Tugas selesai",
      "Waktu belajar",
      "Hari aktif belajar",
      "File baru",
      "Proyek bergerak",
    ]);
    expect(measures[0]).toHaveTextContent(/rata-rata/);
  });

  it("lists what changed by day and filters by who and what", async () => {
    const user = userEvent.setup();
    const { backend } = renderApp({ path: "/jejak" });
    expect(await screen.findByText("ULTRON menambahkan tugas “Bawa kabel UTP ke lab besok”.")).toBeInTheDocument();
    expect(screen.getByText("Kamu menyelesaikan “Hub: kunci arah desain”.")).toBeInTheDocument();

    const filters = screen.getByRole("group", { name: "Saring jejak" });
    await user.click(within(filters).getByRole("button", { name: "ULTRON" }));
    await waitFor(() => expect(screen.queryByText("Kamu menyelesaikan “Hub: kunci arah desain”.")).not.toBeInTheDocument());
    expect(screen.getByText("ULTRON menambahkan tugas “Bawa kabel UTP ke lab besok”.")).toBeInTheDocument();

    await user.click(within(filters).getByRole("button", { name: "File" }));
    expect(await screen.findByText("Belum ada jejak untuk saringan ini.")).toBeInTheDocument();
    expect(backend.calls.some((call) => call.startsWith("get_trail") && call.includes('"files"') && call.includes('"ultron"'))).toBe(true);
  });
});
