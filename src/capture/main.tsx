// The Catat Cepat window (Ctrl+Alt+H from anywhere): one line, Enter saves, Escape or clicking
// elsewhere hides it. It may only read and save a line (capabilities/capture.json).

import "@fontsource-variable/fraunces/full.css";
import "@fontsource-variable/instrument-sans";
import "../styles/global.css";

import { QueryClientProvider } from "@tanstack/react-query";
import { isTauri } from "@tauri-apps/api/core";
import { Check } from "lucide-react";
import { StrictMode, useEffect, useRef, useState } from "react";
import { createRoot } from "react-dom/client";

import { CaptureForm } from "../components/CaptureForm";
import { useApi } from "../data/api";
import type { Backend } from "../data/backend";
import { BackendContext, useBackend } from "../data/backend";
import type { Saved } from "../data/generated/Saved";
import { createQueryClient, useSettings } from "../data/queries";
import { createSoundEngine } from "../sound/engine";
import { SoundProvider } from "../sound/SoundProvider";
import { tauriBackend } from "../data/tauri";
import { useLight } from "../light/useLight";
import { id } from "../strings/id";
import { useNow } from "../time/clock";
import styles from "./capture-window.module.css";

/** Long enough to read "Tugas tersimpan", short enough to get back to work. */
const SAVED_MS = 900;

function CaptureWindow() {
  const backend = useBackend();
  const api = useApi();
  const settings = useSettings();
  const now = useNow();
  const light = useLight(settings.data, now, backend, false);
  const [round, setRound] = useState(0);
  const [saved, setSaved] = useState<Saved | null>(null);
  const shown = useRef(false);
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

  useEffect(() => {
    if (shown.current || (!light && !settings.isError)) return;
    shown.current = true;
    void document.fonts.ready.then(() => api.windowReady()).catch(() => undefined);
  }, [light, settings.isError, api]);

  // Shown again by the hotkey: a fresh, focused line.
  useEffect(
    () =>
      backend.on("hub://capture-shown", () => {
        clearTimeout(timer.current);
        setSaved(null);
        setRound((n) => n + 1);
      }),
    [backend],
  );

  useEffect(() => () => clearTimeout(timer.current), []);

  const hide = () => void api.hideCapture().catch(() => undefined);

  return (
    <main className={styles.window}>
      <h1 className={`section-label ${styles.head}`}>{id.capture.title}</h1>
      {saved ? (
        <p className={styles.saved} role="status">
          <Check size={18} aria-hidden="true" />
          <span>
            {id.capture.saved[saved.kind]}: <strong>{saved.title}</strong>
          </span>
        </p>
      ) : (
        <CaptureForm
          key={round}
          autoFocus
          onSaved={(next) => {
            setSaved(next);
            timer.current = setTimeout(() => {
              hide();
              setSaved(null);
              setRound((n) => n + 1);
            }, SAVED_MS);
          }}
          onCancel={hide}
        />
      )}
    </main>
  );
}

async function pickBackend(): Promise<Backend> {
  if (isTauri()) return tauriBackend;
  const { createMockBackend } = await import("../data/mock");
  return createMockBackend({ empty: new URLSearchParams(location.search).has("empty") });
}

const container = document.getElementById("root");
if (!container) throw new Error("#root missing from capture.html");

void pickBackend().then((backend) =>
  createRoot(container).render(
    <StrictMode>
      <BackendContext.Provider value={backend}>
        <QueryClientProvider client={createQueryClient()}>
          <SoundProvider engine={createSoundEngine()} place="capture">
            <CaptureWindow />
          </SoundProvider>
        </QueryClientProvider>
      </BackendContext.Provider>
    </StrictMode>,
  ),
);
