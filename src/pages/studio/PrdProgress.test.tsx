import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { partStates, PrdProgress } from "./PrdProgress";

const step = (part: number, repair = false) => ({ kind: "step" as const, part, total: 3, repair });

function states() {
  return within(screen.getByRole("list", { name: "Bagian PRD" }))
    .getAllByRole("listitem")
    .map((item) => item.getAttribute("data-state"));
}

describe("the PRD's writing as steps (L3)", () => {
  it("reads each part's state from the last step heard", () => {
    expect(partStates(null, 3)).toEqual(["running", "waiting", "waiting"]);
    expect(partStates(step(2), 3)).toEqual(["done", "running", "waiting"]);
    expect(partStates(step(3), 3)).toEqual(["done", "done", "running"]);
    // A repair comes after all three were written once.
    expect(partStates(step(1, true), 3)).toEqual(["running", "done", "done"]);
  });

  it("shows the parts, the time since it began, and stops on request", async () => {
    const user = userEvent.setup();
    const onStop = vi.fn();
    const startedAt = new Date(Date.now() - 65_000).toISOString();
    render(<PrdProgress step={step(2)} startedAt={startedAt} onStop={onStop} />);
    // One live line for the step; the clock is not read out.
    expect(screen.getByRole("status")).toHaveTextContent("Menyusun bagian 2 dari 3…");
    expect(screen.getByText(/^Berjalan 01:0[5-6]$/)).toHaveAttribute("aria-hidden", "true");
    expect(states()).toEqual(["done", "running", "waiting"]);
    const items = within(screen.getByRole("list", { name: "Bagian PRD" })).getAllByRole("listitem");
    expect(items[0]).toHaveTextContent("Identitas, latar belakang, tujuan, pengalaman penggunaSelesai");
    expect(items[1]).toHaveTextContent("Kebutuhan fungsionalSedang disusun");
    expect(items[2]).toHaveTextContent("Menunggu");
    expect(screen.getByText(/berpikir mendalam untuk setiap bagian/)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Hentikan" }));
    expect(onStop).toHaveBeenCalledOnce();
  });

  it("names the part being repaired", () => {
    render(<PrdProgress step={step(2, true)} startedAt={null} onStop={() => undefined} />);
    expect(screen.getByRole("status")).toHaveTextContent("Memperbaiki bagian 2…");
    expect(states()).toEqual(["done", "running", "done"]);
    expect(screen.getByText("Sedang diperbaiki")).toBeInTheDocument();
    // Without a start, no clock.
    expect(screen.queryByText(/^Berjalan/)).not.toBeInTheDocument();
  });
});
