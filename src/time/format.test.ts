import { describe, expect, it } from "vitest";

import { clock, dateLabel, greeting, isoDate, parseClock, relativeDate, span, whenLabel } from "./format";

describe("format", () => {
  it("writes times the Indonesian way", () => {
    expect(clock(0)).toBe("00.00");
    expect(clock(7 * 60 + 5)).toBe("07.05");
    expect(clock(22 * 60 + 30)).toBe("22.30");
    expect(clock(1440)).toBe("00.00");
  });

  it("writes spans short", () => {
    expect(span(25)).toBe("25 m");
    expect(span(60)).toBe("1 j");
    expect(span(72)).toBe("1 j 12 m");
    expect(span(-5)).toBe("0 m");
  });

  it("reads stored clock times strictly", () => {
    expect(parseClock("05:00")).toBe(300);
    expect(parseClock("22:30")).toBe(1350);
    expect(parseClock("5:00")).toBeNull();
    expect(parseClock("24:00")).toBeNull();
  });

  it("dates in Indonesian", () => {
    const thursday = new Date(2026, 9, 22, 12, 0);
    expect(dateLabel(thursday)).toBe("Kamis, 22 Oktober");
    expect(isoDate(thursday)).toBe("2026-10-22");
    expect(relativeDate("2026-10-22", thursday)).toBe("hari ini");
    expect(relativeDate("2026-10-23", thursday)).toBe("besok");
    expect(relativeDate("2026-10-24", thursday)).toBe("Sabtu");
    expect(relativeDate("2026-11-05", thursday)).toMatch(/5 Nov/);
  });

  it("says when something happened", () => {
    const now = new Date(2026, 9, 22, 12, 0);
    expect(whenLabel(new Date(2026, 9, 22, 11, 20).toISOString(), now)).toBe("11.20");
    expect(whenLabel(new Date(2026, 9, 21, 9, 0).toISOString(), now)).toBe("Rabu");
  });

  it("greets by the hour", () => {
    expect(greeting(6 * 60, 300)).toBe("Selamat pagi");
    expect(greeting(12 * 60, 300)).toBe("Selamat siang");
    expect(greeting(16 * 60, 300)).toBe("Selamat sore");
    expect(greeting(21 * 60, 300)).toBe("Selamat malam");
    expect(greeting(2 * 60, 300)).toBe("Selamat malam");
  });
});
