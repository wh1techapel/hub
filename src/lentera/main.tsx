// Lentera's window (Ctrl+Alt+L from anywhere): the capsule and the bar (LenteraWindow.tsx). It may
// only read and save a line, say where it takes the mouse, give the keyboard back, and hide itself
// (capabilities/lentera.json).

import "@fontsource-variable/fraunces/full.css";
import "@fontsource-variable/instrument-sans";
import "../styles/global.css";

import { QueryClientProvider } from "@tanstack/react-query";
import { isTauri } from "@tauri-apps/api/core";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import type { Backend } from "../data/backend";
import { BackendContext } from "../data/backend";
import { createQueryClient } from "../data/queries";
import { tauriBackend } from "../data/tauri";
import { createSoundEngine } from "../sound/engine";
import { SoundProvider } from "../sound/SoundProvider";
import { LenteraWindow } from "./LenteraWindow";

async function pickBackend(): Promise<Backend> {
  if (isTauri()) return tauriBackend;
  // The browser preview: no desktop under the page, and Ctrl+Alt+L stands in for the global
  // shortcut, so the bar can be opened again after it hides.
  document.body.dataset.preview = "";
  const { createMockBackend } = await import("../data/mock");
  // `?key` gives the preview a Gemini key, so the bar answers through the pretend Gemini (CP03).
  const query = new URLSearchParams(location.search);
  const mock = createMockBackend({ empty: query.has("empty"), geminiKey: query.has("key") ? "AIzaPreviewKey" : null });
  window.addEventListener("keydown", (event) => {
    if (event.ctrlKey && event.altKey && event.key.toLowerCase() === "l") mock.send("hub://lentera-open", null);
  });
  return mock;
}

const container = document.getElementById("root");
if (!container) throw new Error("#root missing from lentera.html");

void pickBackend().then((backend) =>
  createRoot(container).render(
    <StrictMode>
      <BackendContext.Provider value={backend}>
        <QueryClientProvider client={createQueryClient()}>
          <SoundProvider engine={createSoundEngine()} place="capture">
            <LenteraWindow />
          </SoundProvider>
        </QueryClientProvider>
      </BackendContext.Provider>
    </StrictMode>,
  ),
);
