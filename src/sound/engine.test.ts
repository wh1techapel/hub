import { afterEach, describe, expect, it, vi } from "vitest";

import { createSoundEngine } from "./engine";

/** Just enough of Web Audio to count what the engine starts. */
class FakeParam {
  value = 0;
  setValueAtTime() {}
  linearRampToValueAtTime() {}
  exponentialRampToValueAtTime() {}
  setTargetAtTime(value: number) {
    this.value = value;
  }
}
class FakeNode {
  gain = new FakeParam();
  frequency = new FakeParam();
  Q = new FakeParam();
  type = "";
  buffer: unknown = null;
  connect(next: FakeNode) {
    return next;
  }
  start() {
    FakeContext.started += 1;
  }
  stop() {}
}
class FakeContext {
  static made = 0;
  static started = 0;
  static suspended = 0;
  state = "running";
  currentTime = 0;
  sampleRate = 8000;
  destination = new FakeNode();
  constructor() {
    FakeContext.made += 1;
  }
  resume() {
    this.state = "running";
    return Promise.resolve();
  }
  suspend() {
    FakeContext.suspended += 1;
    this.state = "suspended";
    return Promise.resolve();
  }
  createOscillator() {
    return new FakeNode();
  }
  createGain() {
    return new FakeNode();
  }
  createBiquadFilter() {
    return new FakeNode();
  }
  createBufferSource() {
    return new FakeNode();
  }
  createBuffer(_channels: number, length: number) {
    return { getChannelData: () => new Float32Array(length) };
  }
}

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
  FakeContext.made = 0;
  FakeContext.started = 0;
  FakeContext.suspended = 0;
  vi.useRealTimers();
});

describe("the sound engine", () => {
  it("stays silent, and does not fail, where there is no Web Audio", () => {
    vi.stubGlobal("AudioContext", undefined);
    const engine = createSoundEngine();
    expect(() => engine.play("complete", "work", "siang")).not.toThrow();
  });

  it("makes its context on the first sound, keeps sounds 80 ms apart, and is silent while hidden", () => {
    vi.stubGlobal("AudioContext", FakeContext);
    let now = 1000;
    vi.spyOn(performance, "now").mockImplementation(() => now);
    const engine = createSoundEngine();
    expect(FakeContext.made).toBe(0);

    engine.play("complete", "work", "siang");
    expect(FakeContext.made).toBe(1);
    const afterOne = FakeContext.started;
    expect(afterOne).toBeGreaterThan(0);

    now += 40;
    engine.play("complete", "work", "siang");
    expect(FakeContext.started).toBe(afterOne);
    now += 60;
    engine.play("select", "work", "siang");
    expect(FakeContext.started).toBeGreaterThan(afterOne);

    const before = FakeContext.started;
    vi.spyOn(document, "hidden", "get").mockReturnValue(true);
    now += 500;
    engine.play("hero", "work", "siang");
    expect(FakeContext.started).toBe(before);
    // A cue a place keeps quiet starts nothing either.
    vi.spyOn(document, "hidden", "get").mockReturnValue(false);
    engine.play("on", "learning", "siang");
    expect(FakeContext.started).toBe(before);
    expect(FakeContext.made).toBe(1);
  });

  it("rests when the last sound has rung out, and wakes for the next", () => {
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
    vi.stubGlobal("AudioContext", FakeContext);
    let now = 1000;
    vi.spyOn(performance, "now").mockImplementation(() => now);
    const engine = createSoundEngine();
    engine.play("hero", "today", "siang");
    vi.advanceTimersByTime(1000);
    expect(FakeContext.suspended).toBe(0);
    vi.advanceTimersByTime(2500);
    expect(FakeContext.suspended).toBe(1);
    now += 5000;
    engine.play("select", "today", "siang");
    expect(FakeContext.suspended).toBe(1);
    vi.advanceTimersByTime(2000);
    expect(FakeContext.suspended).toBe(2);
  });
});
