import { describe, expect, it } from "vitest";

import type { Saved } from "../data/generated/Saved";
import { givesBack, initial, step } from "./state";
import type { LenteraEvent, LenteraState } from "./state";

const saved: Saved = { kind: "task", id: "t1", title: "beli tinta printer", date: null };
const bar: LenteraState = { view: "bar", note: null };

function run(...events: LenteraEvent[]): LenteraState {
  return events.reduce(step, initial);
}

describe("Lentera's states", () => {
  it("starts hidden, opens to the bar, and closes to hidden", () => {
    expect(initial).toEqual({ view: "hidden", note: null });
    expect(run({ type: "open" })).toEqual(bar);
    expect(run({ type: "open" }, { type: "close" })).toEqual(initial);
  });

  it("folds into a capsule that says what was saved, then leaves", () => {
    const told = run({ type: "open" }, { type: "saved", saved });
    expect(told).toEqual({ view: "capsule", note: { kind: "saved", saved, count: 1 } });
    // CP03: the last of Lentera's cards decided, with several saved.
    expect(run({ type: "open" }, { type: "saved", saved, count: 3 }).note).toEqual({ kind: "saved", saved, count: 3 });
    expect(step(told, { type: "expire" })).toEqual(initial);
    // Clicked while it says so: back to the bar, and the late expiry changes nothing.
    const reopened = step(told, { type: "expand" });
    expect(reopened).toEqual(bar);
    expect(step(reopened, { type: "expire" })).toBe(reopened);
  });

  it("a click elsewhere hides an empty bar and keeps an unsaved line in the capsule", () => {
    expect(run({ type: "open" }, { type: "blur", draft: false })).toEqual(initial);
    const kept = run({ type: "open" }, { type: "blur", draft: true });
    expect(kept).toEqual({ view: "capsule", note: { kind: "draft" } });
    // The capsule is already out of the way: another blur leaves it, the expiry is not its.
    expect(step(kept, { type: "blur", draft: true })).toBe(kept);
    expect(step(kept, { type: "expire" })).toBe(kept);
    expect(step(kept, { type: "expand" })).toEqual(bar);
    expect(step(kept, { type: "discard" })).toEqual(initial);
  });

  it("the saved capsule ignores a blur: giving the keyboard back must not hide it", () => {
    const told = run({ type: "open" }, { type: "saved", saved });
    expect(step(told, { type: "blur", draft: false })).toBe(told);
  });

  it("changes nothing when there is nothing to change", () => {
    expect(step(bar, { type: "open" })).toBe(bar);
    expect(step(initial, { type: "close" })).toBe(initial);
    expect(step(initial, { type: "blur", draft: true })).toBe(initial);
  });

  it("gives the keyboard back only when the owner closed Lentera", () => {
    expect(givesBack({ type: "close" })).toBe(true);
    expect(givesBack({ type: "discard" })).toBe(true);
    expect(givesBack({ type: "blur", draft: false })).toBe(false);
    expect(givesBack({ type: "expire" })).toBe(false);
  });
});
