import { describe, expect, it } from "vitest";

import type { CalendarItem } from "../../data/generated/CalendarItem";
import { ARC, arcPoint, eventLabels, frac, momentAt } from "./arcGeometry";

const event = (title: string, start: number, end: number, allDay = false): CalendarItem => ({
  key: title,
  kind: "event",
  eventId: title,
  routineId: null,
  ruleDate: null,
  taskId: null,
  taskDone: false,
  title,
  location: "",
  allDay,
  startMinute: start,
  endMinute: end,
  continuesBefore: false,
  continuesAfter: false,
  moved: false,
  pigment: "toska",
  source: "owner",
});

describe("Day Arc geometry", () => {
  it("runs from the left horizon over the apex to the right one", () => {
    const [x0, y0] = arcPoint(0);
    const [xm, ym] = arcPoint(0.5);
    const [x1, y1] = arcPoint(1);
    expect(x0).toBeCloseTo(40);
    expect(y0).toBeCloseTo(160);
    expect(xm).toBeCloseTo(500);
    expect(ym).toBeCloseTo(30);
    expect(x1).toBeCloseTo(960);
    expect(y1).toBeCloseTo(160);
  });

  it("clamps time to the waking day", () => {
    expect(frac(200, 300, 1350)).toBe(0);
    expect(frac(1400, 300, 1350)).toBe(1);
    expect(frac(825, 300, 1350)).toBeCloseTo(0.5);
  });

  it("finds the event under way and the next, ignoring all-day ones", () => {
    const events = [event("Libur", 0, 1440, true), event("Matematika", 420, 510), event("Bahasa Inggris", 510, 600)];
    expect(momentAt(events, 430).current?.title).toBe("Matematika");
    expect(momentAt(events, 430).next?.title).toBe("Bahasa Inggris");
    expect(momentAt(events, 700)).toEqual({ current: undefined, next: undefined });
  });

  it("steps a label out when it would sit on the previous one or the sun", () => {
    const radius = (label: { x: number; y: number }) => Math.hypot(label.x - ARC.cx, label.y - ARC.cy) - ARC.r;
    const moment = momentAt([event("Matematika", 420, 510), event("Bahasa Inggris", 510, 600)], 430);

    // No sun nearby: the first sits close, the adjacent second steps out.
    const [first, second] = eventLabels(moment, 300, 1350, null);
    expect(first?.strong).toBe(true);
    expect(radius(first!)).toBeCloseTo(20);
    expect(radius(second!)).toBeCloseTo(40);

    // The sun on the first label pushes it out too.
    const [onSun] = eventLabels(moment, 300, 1350, frac(465, 300, 1350));
    expect(radius(onSun!)).toBeCloseTo(40);
  });
});
