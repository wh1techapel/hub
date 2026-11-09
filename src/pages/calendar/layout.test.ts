import { describe, expect, it } from "vitest";

import { addDays, addMonths, dayCode, layoutDay, mondayOf, monthStart, snap } from "./layout";

describe("calendar arithmetic", () => {
  it("weeks start on Monday", () => {
    expect(mondayOf("2026-10-23")).toBe("2026-10-19");
    expect(mondayOf("2026-10-25")).toBe("2026-10-19");
    expect(mondayOf("2026-10-26")).toBe("2026-10-26");
    expect(dayCode("2026-10-23")).toBe("fr");
  });

  it("months show from the Monday on or before the first", () => {
    expect(monthStart("2026-10-23")).toBe("2026-09-28");
    expect(addMonths("2026-10-23", 1)).toBe("2026-11-01");
    expect(addMonths("2026-01-31", -1)).toBe("2025-12-01");
    expect(addDays("2026-12-31", 1)).toBe("2027-01-01");
  });

  it("snaps to a quarter hour", () => {
    expect(snap(427)).toBe(420);
    expect(snap(428)).toBe(435);
  });

  it("puts overlapping items side by side and lone ones full width", () => {
    const placed = layoutDay([
      { key: "a", start: 420, end: 510 },
      { key: "b", start: 480, end: 540 },
      { key: "c", start: 515, end: 600 },
      { key: "d", start: 700, end: 760 },
    ]);
    expect(placed.get("a")).toEqual({ column: 0, columns: 2 });
    expect(placed.get("b")).toEqual({ column: 1, columns: 2 });
    expect(placed.get("c"), "reuses the first free column").toEqual({ column: 0, columns: 2 });
    expect(placed.get("d")).toEqual({ column: 0, columns: 1 });
  });
});
