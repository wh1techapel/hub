import { createMemoryHistory } from "@tanstack/react-router";
import { render } from "@testing-library/react";
import { vi } from "vitest";

import { App } from "../app/App";
import { createAppRouter } from "../app/router";
import { createMockBackend } from "../data/mock";
import type { MockBackend } from "../data/mock";
import { createQueryClient } from "../data/queries";
import type { SoundEngine } from "../sound/engine";
import { refreshClock } from "../time/clock";

/** The whole app on a memory history, a mock backend, and a pinned clock. */
export function renderApp(options: { path?: string; backend?: MockBackend; empty?: boolean; at?: Date } = {}) {
  vi.useFakeTimers({ toFake: ["Date"] });
  vi.setSystemTime(options.at ?? new Date(2026, 9, 22, 12, 40));
  refreshClock();
  // Made after the clock is pinned: the demo week is built around "today".
  const backend = options.backend ?? createMockBackend({ empty: options.empty });
  const router = createAppRouter(createMemoryHistory({ initialEntries: [options.path ?? "/"] }));
  // The sounds the app asked for, as "cue@place" (P2); jsdom has no audio.
  const played: string[] = [];
  const sound: SoundEngine = { play: (cue, place) => void played.push(`${cue}@${place}`), setVolume: () => undefined };
  const result = render(<App backend={backend} client={createQueryClient()} router={router} sound={sound} />);
  return { ...result, backend, router, played };
}
