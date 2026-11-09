// The light of the moment (docs/design.md, "Light"): which of the four phases the page wears.

import type { ThemeMode } from "../data/generated/ThemeMode";

export type Phase = "pagi" | "siang" | "senja" | "malam";
export type Scheme = "light" | "dark";

/** Morning starts half an hour before waking and lasts until 10.00; day until 15.00; dusk until
 *  18.30; then night until the next morning. */
export function phaseAt(minutes: number, wake: number): Phase {
  if (minutes >= wake - 30 && minutes < 600) return "pagi";
  if (minutes >= 600 && minutes < 900) return "siang";
  if (minutes >= 900 && minutes < 1110) return "senja";
  return "malam";
}

export interface Light {
  phase: Phase;
  scheme: Scheme;
}

/** Otomatis follows the clock; Ikuti Windows takes light or dark from Windows (day light when
 *  light); Terang is always the day light; Gelap always the night. */
export function resolveLight(mode: ThemeMode, minutes: number, wake: number, systemDark: boolean): Light {
  let phase: Phase;
  switch (mode) {
    case "auto":
      phase = phaseAt(minutes, wake);
      break;
    case "system":
      phase = systemDark ? "malam" : "siang";
      break;
    case "light":
      phase = "siang";
      break;
    case "dark":
      phase = "malam";
      break;
  }
  return { phase, scheme: phase === "malam" ? "dark" : "light" };
}
