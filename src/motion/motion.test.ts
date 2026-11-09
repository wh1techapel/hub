import { afterEach, describe, expect, it, vi } from "vitest";

import { insetOf, leave, movesOf } from "./motion";

// jsdom has no Web Animations: the tests lend one and take it back.
const original = Object.getOwnPropertyDescriptor(Element.prototype, "animate");
afterEach(() => {
  vi.restoreAllMocks();
  if (original) Object.defineProperty(Element.prototype, "animate", original);
  else delete (Element.prototype as { animate?: unknown }).animate;
  document.body.innerHTML = "";
});

describe("micro animation helpers", () => {
  it("the gliding pill is the group clipped to the chosen item; a line keeps only its bottom", () => {
    const group = { width: 300, height: 40 };
    const item = { x: 104, y: 4, width: 92, height: 32 };
    expect(insetOf(group, item)).toEqual({ top: 4, right: 104, bottom: 4, left: 104 });
    expect(insetOf(group, { x: 0, y: 0, width: 80, height: 40 }, 2)).toEqual({ top: 38, right: 220, bottom: 0, left: 0 });
    // Never a negative clip, even for an item spilling past the group.
    expect(insetOf(group, { x: -2, y: -1, width: 310, height: 44 })).toEqual({ top: 0, right: 0, bottom: 0, left: 0 });
  });

  it("rows that moved slide by their distance; new rows arrive, but not on the first paint", () => {
    const first = movesOf(new Map(), new Map([["a", 0], ["b", 40]]));
    expect(first).toEqual({ moved: [], fresh: [] });
    // "a" left: "b" moves up 40 px from where it was; "c" is new.
    const next = movesOf(new Map([["a", 0], ["b", 40]]), new Map([["b", 0], ["c", 40]]));
    expect(next).toEqual({ moved: [["b", 40]], fresh: ["c"] });
  });

  it("a dialog or toast leaves as a still copy: unreachable, without ids, gone when played out", async () => {
    let finish: () => void = () => undefined;
    const animate = vi.fn(() => ({ finished: new Promise<void>((done) => (finish = done)) }) as unknown as Animation);
    Element.prototype.animate = animate;
    vi.spyOn(window, "matchMedia").mockReturnValue({ matches: false } as MediaQueryList);
    const note = document.createElement("div");
    note.innerHTML = '<p id="inside">Tersimpan</p><button type="button">Urungkan</button>';
    note.id = "note";
    document.body.appendChild(note);

    leave([note], [[{ opacity: 1 }, { opacity: 0 }]], { pin: true });
    note.remove();
    const ghost = document.body.lastElementChild as HTMLElement;
    expect(ghost.getAttribute("aria-hidden")).toBe("true");
    expect(ghost.inert).toBe(true);
    expect(ghost.querySelectorAll("[id]").length + (ghost.id ? 1 : 0)).toBe(0);
    expect(ghost.style.position).toBe("fixed");
    expect(animate).toHaveBeenCalledTimes(1);
    finish();
    await Promise.resolve();
    await Promise.resolve();
    expect(document.body.contains(ghost)).toBe(false);
  });

  it("asks for less motion: nothing is copied or played", () => {
    const animate = vi.fn();
    Element.prototype.animate = animate;
    vi.spyOn(window, "matchMedia").mockReturnValue({ matches: true } as MediaQueryList);
    const note = document.createElement("div");
    document.body.appendChild(note);
    leave([note], [[{ opacity: 1 }, { opacity: 0 }]]);
    expect(document.body.children.length).toBe(1);
    expect(animate).not.toHaveBeenCalled();
  });
});
