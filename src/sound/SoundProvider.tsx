// Hub's sounds in a window (P2): the engine, the owner's switch and volume, and where the owner is.
// Most micro sounds need no code where they happen: a switch (`role="switch"`), a tab, a radio or
// a checkbox sounds by itself, and any control can name its cue with `data-sound` (a Cue, or
// "check" for a checkbox that marks something done). The rest call `useSound().play` where the
// moment is known (a task checked, the day closed, a checkpoint approved, the week's goal).

import { createContext, useContext, useEffect, useMemo, useRef } from "react";
import type { ReactNode } from "react";

import { useSettings } from "../data/queries";
import type { SoundEngine } from "./engine";
import { phaseOf } from "./palette";
import type { Cue, Place } from "./palette";

export interface Sound {
  /** Play a cue where the owner is (or in `place`), if sounds are on. */
  play(cue: Cue, place?: Place): void;
  /** Pengaturan's Coba: a place's check and its hero sound, heard even with sounds off. */
  preview(place: Place): void;
  /** The Shell says where the owner is. */
  setPlace(place: Place): void;
}

const silent: Sound = { play: () => undefined, preview: () => undefined, setPlace: () => undefined };
const SoundContext = createContext<Sound>(silent);

export function useSound(): Sound {
  return useContext(SoundContext);
}

const CUES: ReadonlySet<string> = new Set<Cue>(["on", "off", "complete", "reopen", "select", "saved", "open", "close", "drop", "copy", "sessionStart", "sessionStop", "hero"]);

function isCue(value: string | undefined): value is Cue {
  return value !== undefined && CUES.has(value);
}

const phase = () => phaseOf(document.documentElement.dataset.phase);

export function SoundProvider({ engine, place: fixed, children }: { engine: SoundEngine; place?: Place; children: ReactNode }) {
  const settings = useSettings();
  const enabled = useRef(false);
  const where = useRef<Place>(fixed ?? "quiet");
  const on = settings.data?.soundEnabled ?? false;
  const volume = settings.data?.soundVolume;

  useEffect(() => {
    enabled.current = on;
  }, [on]);
  useEffect(() => {
    if (volume !== undefined) engine.setVolume(volume);
  }, [engine, volume]);

  const sound = useMemo<Sound>(
    () => ({
      play: (cue, place) => {
        if (enabled.current) engine.play(cue, place ?? where.current, phase());
      },
      preview: (place) => {
        engine.play("complete", place, phase());
        window.setTimeout(() => engine.play("hero", place, phase()), 420);
      },
      setPlace: (place) => {
        if (!fixed) where.current = place;
      },
    }),
    [engine, fixed],
  );

  // The micro sounds of plain controls, heard wherever they are.
  useEffect(() => {
    const onClick = (event: MouseEvent) => {
      const target = event.target instanceof Element ? event.target : null;
      if (!target) return;
      const named = target.closest<HTMLElement>("[data-sound]");
      if (named) {
        const cue = named.dataset.sound;
        if (cue === "check" && named instanceof HTMLInputElement) sound.play(named.checked ? "complete" : "reopen");
        else if (isCue(cue)) sound.play(cue);
        return;
      }
      const toggle = target.closest('[role="switch"]');
      if (toggle) {
        // Heard before the switch's own handler: the state it is leaving.
        sound.play(toggle.getAttribute("aria-checked") === "true" ? "off" : "on");
        return;
      }
      const tab = target.closest('[role="tab"]');
      if (tab && tab.getAttribute("aria-selected") !== "true") sound.play("select");
    };
    const onChange = (event: Event) => {
      const input = event.target;
      if (!(input instanceof HTMLInputElement) || input.dataset.sound !== undefined) return;
      if (input.type === "radio" || (input.type === "checkbox" && input.checked)) sound.play("select");
    };
    document.addEventListener("click", onClick, true);
    document.addEventListener("change", onChange, true);
    return () => {
      document.removeEventListener("click", onClick, true);
      document.removeEventListener("change", onChange, true);
    };
  }, [sound]);

  return <SoundContext.Provider value={sound}>{children}</SoundContext.Provider>;
}
