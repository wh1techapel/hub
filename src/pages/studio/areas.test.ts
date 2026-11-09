import { describe, expect, it } from "vitest";

import type { LedgerItem } from "../../data/generated/LedgerItem";
import { areaOf, summarize } from "./areas";

let n = 0;
function item(kind: LedgerItem["kind"], status: LedgerItem["status"], area: LedgerItem["area"], topic = "t"): LedgerItem {
  n += 1;
  return {
    id: `i${n}`,
    kind,
    topic,
    statement: "s",
    rationale: "",
    status,
    area,
    statedBy: "gemini",
    messageId: null,
    replacesId: null,
    createdAt: "",
    updatedAt: "",
  };
}

describe("the parts of an idea", () => {
  // The same cases as hub_ai::prompt's test, so the two rules stay one rule.
  it("a part is clear when something settles it and no question is open", () => {
    const states = summarize([
      item("decision", "active", "problem"),
      item("assumption", "active", "users"),
      item("assumption", "confirmed", "features"),
      item("question", "active", "features"),
      item("question", "answered", "constraints"),
      item("decision", "retired", "success"),
    ]).map((s) => [s.area, s.state]);
    expect(states).toEqual([
      ["problem", "clear"],
      ["users", "partial"],
      ["features", "partial"],
      ["constraints", "clear"],
      ["success", "unknown"],
    ]);
  });

  it("counts what settles a part, what is assumed, and what is open", () => {
    const features = summarize([item("decision", "active", "features"), item("assumption", "active", "features"), item("question", "active", "features")])[2]!;
    expect([features.decided, features.assumed, features.open]).toEqual([1, 1, 1]);
  });

  it("places an older item by its kind or topic, and leaves the rest unplaced", () => {
    expect(areaOf({ area: null, kind: "constraint", topic: "Apa saja" })).toBe("constraints");
    expect(areaOf({ area: null, kind: "assumption", topic: "Pengguna" })).toBe("users");
    expect(areaOf({ area: null, kind: "decision", topic: "Fitur utama" })).toBe("features");
    expect(areaOf({ area: null, kind: "decision", topic: "Warna" })).toBeNull();
    expect(areaOf({ area: "success", kind: "decision", topic: "Pengguna" })).toBe("success");
  });
});
