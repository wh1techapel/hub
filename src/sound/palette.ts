// What Hub sounds like (P2), as plain data: which instrument a place plays, in which notes, how
// bright and how loud the light of the day lets it be. The engine (engine.ts) only plays what this
// returns, so every rule is here and testable without audio.
//
// Rules from the research behind P2 (docs/design.md, "Sound"):
// - Silence is the default. Typing, keyboard movement, hover, and scrolling make no sound.
// - Micro sounds (switches, checks, choices, a saved line) are short and quiet; hero sounds (the
//   day closed, a checkpoint approved, the week's Belajar goal, every focus task done) are rare and
//   a little longer, never much louder: loudness is what annoys, so every sound stays in a narrow
//   band under one volume.
// - A sound always comes with a visible change; it never carries meaning on its own.
// - The character follows where the owner is and what they do: wood for the day, a kalimba for
//   tasks, glass for Studio, near silence for Belajar's focus, paper for Catat Cepat. The light
//   follows the day: brighter in the morning, lower and softer at night.

export type Place = "today" | "calendar" | "work" | "studio" | "learning" | "capture" | "settings" | "quiet";
export type Phase = "pagi" | "siang" | "senja" | "malam";
export type Cue = "on" | "off" | "complete" | "reopen" | "select" | "saved" | "open" | "close" | "drop" | "copy" | "sessionStart" | "sessionStop" | "hero";
export type Voice = "marimba" | "kalimba" | "glass" | "muted" | "paper";

/** One note: when it starts (s after the cue), its frequency, its gain (before the volume). */
export interface Note {
  at: number;
  hz: number;
  gain: number;
}

export interface Recipe {
  voice: Voice;
  notes: Note[];
  /** How long a note rings (s): micro sounds die fast, hero sounds a little slower. */
  decay: number;
  /** The low-pass cutoff (Hz): the light's brightness. */
  brightness: number;
}

/** The place an address belongs to. */
export function placeOf(pathname: string): Place {
  if (pathname === "/") return "today";
  if (pathname.startsWith("/kalender")) return "calendar";
  if (pathname.startsWith("/proyek")) return "work";
  if (pathname.startsWith("/studio")) return "studio";
  if (pathname.startsWith("/belajar")) return "learning";
  if (pathname.startsWith("/pengaturan")) return "settings";
  return "quiet";
}

interface Character {
  voice: Voice;
  /** The root of the place's pentatonic scale (Hz). */
  root: number;
  /** Its loudness in the narrow band (0.8 to 1). */
  level: number;
}

const CHARACTER: Record<Place, Character> = {
  today: { voice: "marimba", root: 392, level: 1 },
  calendar: { voice: "marimba", root: 349.23, level: 1 },
  work: { voice: "kalimba", root: 587.33, level: 0.95 },
  studio: { voice: "glass", root: 659.25, level: 0.85 },
  learning: { voice: "muted", root: 220, level: 0.9 },
  capture: { voice: "paper", root: 523.25, level: 0.9 },
  settings: { voice: "kalimba", root: 523.25, level: 0.85 },
  quiet: { voice: "marimba", root: 440, level: 0.85 },
};

/** The light of the day: semitones to move, the brightness, and a little less at night. */
const LIGHT: Record<Phase, { shift: number; brightness: number; level: number }> = {
  pagi: { shift: 2, brightness: 7200, level: 1 },
  siang: { shift: 0, brightness: 6000, level: 1 },
  senja: { shift: -2, brightness: 4200, level: 0.92 },
  malam: { shift: -4, brightness: 2800, level: 0.82 },
};

/** Major pentatonic steps, in semitones from the root, two octaves. */
const SCALE = [0, 2, 4, 7, 9, 12, 14, 16, 19, 21];

/** Each cue's shape: scale degrees, spacing (s), relative gain, ring. Hero is the only long one. */
const SHAPE: Record<Cue, { degrees: number[]; step: number; gain: number; decay: number }> = {
  on: { degrees: [2, 4], step: 0.045, gain: 0.55, decay: 0.12 },
  off: { degrees: [4, 2], step: 0.045, gain: 0.5, decay: 0.12 },
  complete: { degrees: [0, 3], step: 0.06, gain: 0.6, decay: 0.22 },
  reopen: { degrees: [3], step: 0, gain: 0.4, decay: 0.12 },
  select: { degrees: [4], step: 0, gain: 0.38, decay: 0.08 },
  saved: { degrees: [3], step: 0, gain: 0.5, decay: 0.16 },
  open: { degrees: [1], step: 0, gain: 0.32, decay: 0.1 },
  close: { degrees: [0], step: 0, gain: 0.3, decay: 0.08 },
  drop: { degrees: [0], step: 0, gain: 0.55, decay: 0.14 },
  copy: { degrees: [5, 7], step: 0.04, gain: 0.4, decay: 0.08 },
  sessionStart: { degrees: [0, 2, 4], step: 0.09, gain: 0.55, decay: 0.35 },
  sessionStop: { degrees: [4, 2, 0], step: 0.09, gain: 0.55, decay: 0.35 },
  hero: { degrees: [0, 2, 4, 7], step: 0.11, gain: 0.62, decay: 0.6 },
};

/** Micro sounds Belajar keeps quiet while the owner studies: only the session and the goal sound. */
const FOCUS_ONLY: ReadonlySet<Cue> = new Set(["sessionStart", "sessionStop", "hero"]);

/** Whether a cue sounds in a place at all. */
export function sounds(cue: Cue, place: Place): boolean {
  return place !== "learning" || FOCUS_ONLY.has(cue);
}

const hz = (root: number, semitones: number) => root * 2 ** (semitones / 12);

/**
 * What to play. `variation` (0 to 1) moves a repeated cue a hair in pitch and loudness, so the
 * same sound heard many times stays easy on the ear.
 */
export function recipe(cue: Cue, place: Place, phase: Phase, variation = 0.5): Recipe | null {
  if (!sounds(cue, place)) return null;
  const character = CHARACTER[place];
  const light = LIGHT[phase];
  const shape = SHAPE[cue];
  const cents = (variation - 0.5) * 20;
  const level = character.level * light.level * (1 + (variation - 0.5) * 0.1);
  // A drop lands an octave lower, as a thing set down.
  const octave = cue === "drop" ? -12 : 0;
  return {
    voice: character.voice,
    decay: shape.decay,
    brightness: light.brightness,
    notes: shape.degrees.map((degree, index) => ({
      at: index * shape.step,
      hz: hz(character.root, (SCALE[degree] ?? 0) + light.shift + octave + cents / 100),
      gain: shape.gain * level,
    })),
  };
}

/** The phase as the light module writes it on the page; midday when it is not known yet. */
export function phaseOf(value: string | undefined): Phase {
  return value === "pagi" || value === "siang" || value === "senja" || value === "malam" ? value : "siang";
}

/** Hub's 0 to 100 volume as a gain: a curve, so the low end stays usable. */
export function gainOf(volume: number): number {
  const v = Math.min(100, Math.max(0, volume)) / 100;
  return 0.35 * v * v;
}
