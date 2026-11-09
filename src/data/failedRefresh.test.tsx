import { act, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";

import { renderApp } from "../test/renderApp";
import { HubError } from "./backend";
import type { Command } from "./backend";
import type { ErrorView } from "./generated/ErrorView";
import type { MockBackend } from "./mock";

// A page keeps what it shows when a refresh fails in the background, and gives way to its error
// view only when it has nothing to show or what it shows is gone (failedForGood).

/** From now on `command` fails with `kind`; returns how many times it was asked. */
function failing(backend: MockBackend, command: Command, kind: ErrorView["kind"]) {
  const call = backend.call;
  const asked = { count: 0 };
  backend.call = <T,>(name: Command, args?: Record<string, unknown>): Promise<T> => {
    if (name !== command) return call<T>(name, args);
    asked.count += 1;
    return Promise.reject(new HubError({ kind, field: null, reason: null, message: "failed" }));
  };
  return asked;
}

describe("a refresh that fails", () => {
  it("keeps Hari Ini on screen", async () => {
    const { backend } = renderApp();
    await screen.findByText("Revisi slide presentasi KIK");
    const asked = failing(backend, "get_today", "internal");
    act(() => backend.emit({ entity: "task", ids: [] }));
    await waitFor(() => expect(asked.count).toBe(2), { timeout: 3000 });
    expect(screen.getByText("Revisi slide presentasi KIK")).toBeInTheDocument();
    expect(screen.queryByText("Hari ini tidak bisa dimuat.")).not.toBeInTheDocument();
  });

  it("keeps a project on screen, and says so when the project is gone", async () => {
    const user = userEvent.setup();
    const { backend } = renderApp({ path: "/proyek" });
    await user.click(await screen.findByRole("link", { name: /Laporan Lab Jaringan/ }));
    await screen.findByRole("heading", { level: 1, name: "Laporan Lab Jaringan" });

    const busy = failing(backend, "get_project", "internal");
    act(() => backend.emit({ entity: "project", ids: [] }));
    await waitFor(() => expect(busy.count).toBe(2), { timeout: 3000 });
    expect(screen.getByRole("heading", { level: 1, name: "Laporan Lab Jaringan" })).toBeInTheDocument();

    failing(backend, "get_project", "not_found");
    act(() => backend.emit({ entity: "project", ids: [] }));
    expect(await screen.findByText("Proyek ini tidak ditemukan.", undefined, { timeout: 3000 })).toBeInTheDocument();
  });
});
