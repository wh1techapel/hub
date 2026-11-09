// The sound engine (P2): plays palette.ts's recipes with the Web Audio API, synthesised in the
// app (no sound files, nothing to license, nothing to download). One AudioContext, made on the
// first sound (always the answer to something the owner did), one low-pass for the light of the
// day, one gain for Hub's own volume. Nothing plays while the window is hidden, and two sounds
// never start within 80 ms of each other. When the last sound has rung out the context is
// suspended, so at rest Hub keeps no audio thread busy (P2c); the next sound wakes it.

import { gainOf, recipe } from "./palette";
import type { Cue, Phase, Place, Voice } from "./palette";

export interface SoundEngine {
  play(cue: Cue, place: Place, phase: Phase): void;
  setVolume(volume: number): void;
}

const GAP_MS = 80;
/** How long after a sound has rung out the context rests. */
const REST_MS = 1500;

type Wave = OscillatorType;

function partial(ctx: AudioContext, out: AudioNode, wave: Wave, hz: number, start: number, gain: number, decay: number, attack = 0.003) {
  const osc = ctx.createOscillator();
  osc.type = wave;
  osc.frequency.value = hz;
  const env = ctx.createGain();
  env.gain.setValueAtTime(0, start);
  env.gain.linearRampToValueAtTime(gain, start + attack);
  env.gain.exponentialRampToValueAtTime(0.0001, start + attack + decay);
  osc.connect(env).connect(out);
  osc.start(start);
  osc.stop(start + attack + decay + 0.05);
}

function rustle(ctx: AudioContext, out: AudioNode, start: number, gain: number, decay: number) {
  const length = Math.ceil(ctx.sampleRate * (decay + 0.02));
  const buffer = ctx.createBuffer(1, length, ctx.sampleRate);
  const data = buffer.getChannelData(0);
  for (let i = 0; i < length; i += 1) data[i] = Math.random() * 2 - 1;
  const source = ctx.createBufferSource();
  source.buffer = buffer;
  const band = ctx.createBiquadFilter();
  band.type = "bandpass";
  band.frequency.setValueAtTime(1800, start);
  band.frequency.linearRampToValueAtTime(3200, start + decay);
  band.Q.value = 1.2;
  const env = ctx.createGain();
  env.gain.setValueAtTime(0, start);
  env.gain.linearRampToValueAtTime(gain, start + 0.008);
  env.gain.exponentialRampToValueAtTime(0.0001, start + decay);
  source.connect(band).connect(env).connect(out);
  source.start(start);
  source.stop(start + decay + 0.02);
}

/** One note in a voice: a few sine partials shaped like the instrument. */
function note(ctx: AudioContext, out: AudioNode, voice: Voice, hz: number, start: number, gain: number, decay: number) {
  switch (voice) {
    case "marimba":
      partial(ctx, out, "sine", hz, start, gain, decay);
      partial(ctx, out, "sine", hz * 4, start, gain * 0.16, decay * 0.3);
      partial(ctx, out, "sine", hz * 9.9, start, gain * 0.03, decay * 0.12);
      break;
    case "kalimba":
      partial(ctx, out, "sine", hz, start, gain, decay);
      partial(ctx, out, "sine", hz * 5.93, start, gain * 0.09, decay * 0.25);
      partial(ctx, out, "triangle", hz * 2, start, gain * 0.07, decay * 0.5);
      break;
    case "glass":
      partial(ctx, out, "sine", hz, start, gain * 0.8, decay * 1.4, 0.006);
      partial(ctx, out, "sine", hz * 2.76, start, gain * 0.2, decay);
      partial(ctx, out, "sine", hz * 5.4, start, gain * 0.07, decay * 0.6);
      break;
    case "muted": {
      const soft = ctx.createBiquadFilter();
      soft.type = "lowpass";
      soft.frequency.value = 1100;
      soft.connect(out);
      partial(ctx, soft, "triangle", hz, start, gain, decay, 0.012);
      break;
    }
    case "paper":
      rustle(ctx, out, start, gain * 0.6, 0.07);
      partial(ctx, out, "sine", hz, start + 0.02, gain * 0.35, decay);
      break;
  }
}

/** The engine for the window it runs in. Where there is no Web Audio (tests), it stays silent. */
export function createSoundEngine(): SoundEngine {
  let ctx: AudioContext | null = null;
  let light: BiquadFilterNode | null = null;
  let master: GainNode | null = null;
  let volume = 30;
  let last = Number.NEGATIVE_INFINITY;
  let rest: ReturnType<typeof setTimeout> | undefined;

  const ready = (): AudioContext | null => {
    if (!ctx) {
      if (typeof window === "undefined" || typeof window.AudioContext !== "function") return null;
      ctx = new window.AudioContext();
      light = ctx.createBiquadFilter();
      light.type = "lowpass";
      master = ctx.createGain();
      master.gain.value = gainOf(volume);
      light.connect(master).connect(ctx.destination);
    }
    if (ctx.state === "suspended") void ctx.resume();
    return ctx;
  };

  return {
    setVolume(next) {
      volume = next;
      if (ctx && master) master.gain.setTargetAtTime(gainOf(next), ctx.currentTime, 0.02);
    },
    play(cue, place, phase) {
      if (typeof document !== "undefined" && document.hidden) return;
      const now = performance.now();
      if (now - last < GAP_MS) return;
      const sound = recipe(cue, place, phase, Math.random());
      if (!sound) return;
      const audio = ready();
      if (!audio || !light) return;
      last = now;
      light.frequency.setValueAtTime(sound.brightness, audio.currentTime);
      const start = audio.currentTime + 0.005;
      for (const n of sound.notes) note(audio, light, sound.voice, n.hz, start + n.at, n.gain, sound.decay);
      const rings = Math.max(...sound.notes.map((n) => n.at)) + sound.decay;
      clearTimeout(rest);
      rest = setTimeout(() => {
        if (ctx?.state === "running") void ctx.suspend();
      }, rings * 1000 + REST_MS);
    },
  };
}
