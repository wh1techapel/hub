import { describe, expect, it } from "vitest";

import { gainOf, phaseOf, placeOf, recipe, sounds } from "./palette";
import type { Cue, Phase, Place } from "./palette";

const PLACES: Place[] = ["today", "calendar", "work", "studio", "learning", "capture", "settings", "quiet"];
const CUES: Cue[] = ["on", "off", "complete", "reopen", "select", "saved", "open", "close", "drop", "copy", "sessionStart", "sessionStop", "hero"];
const PHASES: Phase[] = ["pagi", "siang", "senja", "malam"];

describe("Hub's sound palette", () => {
  it("knows the place from the address", () => {
    expect(placeOf("/")).toBe("today");
    expect(placeOf("/kalender")).toBe("calendar");
    expect(placeOf("/proyek/x")).toBe("work");
    expect(placeOf("/studio/x")).toBe("studio");
    expect(placeOf("/belajar")).toBe("learning");
    expect(placeOf("/pengaturan/ai")).toBe("settings");
    expect(placeOf("/jejak")).toBe("quiet");
  });

  it("gives each place its own instrument", () => {
    const voice = (place: Place) => recipe("complete", place, "siang")?.voice;
    expect([voice("today"), voice("work"), voice("studio"), voice("capture")]).toEqual(["marimba", "kalimba", "glass", "paper"]);
    expect(recipe("sessionStart", "learning", "siang")?.voice).toBe("muted");
  });

  it("keeps Belajar quiet but for the session and the goal", () => {
    for (const cue of CUES) expect(sounds(cue, "learning")).toBe(cue === "sessionStart" || cue === "sessionStop" || cue === "hero");
    expect(recipe("on", "learning", "siang")).toBeNull();
  });

  it("follows the light: brighter in the morning, lower and darker at night", () => {
    const morning = recipe("complete", "today", "pagi")!;
    const night = recipe("complete", "today", "malam")!;
    expect(morning.notes[0]!.hz).toBeGreaterThan(night.notes[0]!.hz);
    expect(morning.brightness).toBeGreaterThan(night.brightness);
    expect(morning.notes[0]!.gain).toBeGreaterThan(night.notes[0]!.gain);
  });

  it("stays in a narrow loudness band, and only the hero sound rings long", () => {
    const gains: number[] = [];
    for (const place of PLACES) {
      for (const phase of PHASES) {
        for (const cue of CUES) {
          const r = recipe(cue, place, phase, 0.5);
          if (!r) continue;
          gains.push(...r.notes.map((n) => n.gain));
          const length = Math.max(...r.notes.map((n) => n.at)) + r.decay;
          if (cue === "hero") expect(length).toBeLessThan(1);
          else if (cue === "sessionStart" || cue === "sessionStop") expect(length).toBeLessThan(0.6);
          else expect(length).toBeLessThan(0.3);
        }
      }
    }
    // About 10 dB from the softest to the loudest, in every place and light.
    expect(Math.max(...gains) / Math.min(...gains)).toBeLessThan(3.2);
  });

  it("moves a repeated sound by a hair, never a step", () => {
    const low = recipe("select", "work", "siang", 0)!.notes[0]!.hz;
    const high = recipe("select", "work", "siang", 1)!.notes[0]!.hz;
    const cents = 1200 * Math.log2(high / low);
    expect(cents).toBeGreaterThan(15);
    expect(cents).toBeLessThan(25);
  });

  it("turns Hub's volume into a gain on a curve, silent at zero", () => {
    expect(gainOf(0)).toBe(0);
    expect(gainOf(30)).toBeLessThan(gainOf(60));
    expect(gainOf(100)).toBeCloseTo(0.35);
    expect(gainOf(250)).toBeCloseTo(0.35);
    expect(phaseOf("malam")).toBe("malam");
    expect(phaseOf(undefined)).toBe("siang");
  });
});
