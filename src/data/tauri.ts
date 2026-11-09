// The app's backend: Hub's own Tauri commands (src-tauri/src/commands.rs) and its events.

import { Channel, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import { toHubError } from "./backend";
import type { Backend } from "./backend";
import type { Change } from "./generated/Change";

function subscribe(event: string, listener: (payload: unknown) => void): () => void {
  let stop: (() => void) | null = null;
  let stopped = false;
  void listen<unknown>(event, (message) => {
    listener(message.payload);
  }).then((unlisten) => {
    if (stopped) unlisten();
    else stop = unlisten;
  });
  return () => {
    stopped = true;
    stop?.();
  };
}

export const tauriBackend: Backend = {
  call: async <T>(command: string, args?: Record<string, unknown>): Promise<T> => {
    try {
      return await invoke<T>(command, args);
    } catch (error) {
      throw toHubError(error);
    }
  },
  stream: async <T>(command: string, args: Record<string, unknown>, onEvent: (event: unknown) => void): Promise<T> => {
    const channel = new Channel<unknown>();
    channel.onmessage = onEvent;
    try {
      return await invoke<T>(command, { ...args, onEvent: channel });
    } catch (error) {
      throw toHubError(error);
    }
  },
  // The core sends a Change with every "hub://changed" (src-tauri/src/commands.rs).
  onChanged: (listener) => subscribe("hub://changed", (payload) => listener(payload as Change)),
  on: (event, listener) => subscribe(event, listener),
};
