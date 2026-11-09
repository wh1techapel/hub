// The root: backend, query cache, the light, and the router. The window stays hidden until the
// first frame is drawn in the right light with the right fonts (then `windowReady`).

import { QueryClientProvider } from "@tanstack/react-query";
import type { QueryClient } from "@tanstack/react-query";
import { RouterProvider } from "@tanstack/react-router";
import { useEffect, useRef } from "react";

import { ToastProvider } from "../components/Toast";
import { api } from "../data/api";
import { BackendContext, useBackend } from "../data/backend";
import type { Backend } from "../data/backend";
import { useChangeSync, useSettings } from "../data/queries";
import { createSoundEngine } from "../sound/engine";
import type { SoundEngine } from "../sound/engine";
import { SoundProvider } from "../sound/SoundProvider";
import { useLight } from "../light/useLight";
import { id } from "../strings/id";
import { useNow } from "../time/clock";
import type { AppRouter } from "./router";

/** The main window's sounds (P2): one engine for as long as the window lives. */
const mainSound = createSoundEngine();

export function App({ backend, client, router, sound = mainSound }: { backend: Backend; client: QueryClient; router: AppRouter; sound?: SoundEngine }) {
  return (
    <BackendContext.Provider value={backend}>
      <QueryClientProvider client={client}>
        <SoundProvider engine={sound}>
          <ToastProvider>
            <Root router={router} />
          </ToastProvider>
        </SoundProvider>
      </QueryClientProvider>
    </BackendContext.Provider>
  );
}

function Root({ router }: { router: AppRouter }) {
  useChangeSync();
  const backend = useBackend();
  const settings = useSettings();
  const now = useNow();
  const light = useLight(settings.data, now, backend);
  const shown = useRef(false);

  const failed = settings.isError;
  useEffect(() => {
    if (shown.current || (!light && !failed)) return;
    shown.current = true;
    void document.fonts.ready.then(() => api(backend).windowReady()).catch(() => undefined);
  }, [light, failed, backend]);

  if (failed) {
    return (
      <div role="alert" style={{ padding: "var(--space-10)" }}>
        <h1 className="display">{id.app.name}</h1>
        <p>{id.today.loadError}</p>
      </div>
    );
  }
  if (!settings.data) return null;
  return <RouterProvider router={router} />;
}
