import { describe, expect, it } from "vitest";

import { phaseAt, resolveLight } from "./phase";

const at = (h: number, m = 0) => h * 60 + m;
const WAKE = at(5);

describe("phaseAt", () => {
  it("follows the day from half an hour before waking", () => {
    expect(phaseAt(at(4, 29), WAKE)).toBe("malam");
    expect(phaseAt(at(4, 30), WAKE)).toBe("pagi");
    expect(phaseAt(at(9, 59), WAKE)).toBe("pagi");
    expect(phaseAt(at(10), WAKE)).toBe("siang");
    expect(phaseAt(at(14, 59), WAKE)).toBe("siang");
    expect(phaseAt(at(15), WAKE)).toBe("senja");
    expect(phaseAt(at(18, 29), WAKE)).toBe("senja");
    expect(phaseAt(at(18, 30), WAKE)).toBe("malam");
    expect(phaseAt(at(23, 59), WAKE)).toBe("malam");
  });

  it("starts the morning later for a later riser", () => {
    expect(phaseAt(at(5), at(6))).toBe("malam");
    expect(phaseAt(at(5, 30), at(6))).toBe("pagi");
  });
});

describe("resolveLight", () => {
  it("auto follows the clock, night is dark", () => {
    expect(resolveLight("auto", at(12), WAKE, false)).toEqual({ phase: "siang", scheme: "light" });
    expect(resolveLight("auto", at(21), WAKE, false)).toEqual({ phase: "malam", scheme: "dark" });
  });

  it("the fixed modes ignore the clock", () => {
    expect(resolveLight("light", at(21), WAKE, true).phase).toBe("siang");
    expect(resolveLight("dark", at(12), WAKE, false).phase).toBe("malam");
  });

  it("system follows Windows", () => {
    expect(resolveLight("system", at(12), WAKE, true)).toEqual({ phase: "malam", scheme: "dark" });
    expect(resolveLight("system", at(21), WAKE, false)).toEqual({ phase: "siang", scheme: "light" });
  });
});
