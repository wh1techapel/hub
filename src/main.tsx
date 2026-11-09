import "@fontsource-variable/fraunces/full.css";
import "@fontsource-variable/fraunces/full-italic.css";
import "@fontsource-variable/instrument-sans";
import "@fontsource-variable/jetbrains-mono";
import "./styles/global.css";

import { isTauri } from "@tauri-apps/api/core";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { App } from "./app/App";
import { createAppRouter } from "./app/router";
import type { Backend } from "./data/backend";
import { createQueryClient } from "./data/queries";
import { tauriBackend } from "./data/tauri";

/** The app's core, or (in the browser: npm run dev) the demo day, loaded only there, so the
 *  release window never carries the mock. `?empty` starts from an empty Hub. */
async function pickBackend(): Promise<Backend> {
  if (isTauri()) return tauriBackend;
  const { createMockBackend } = await import("./data/mock");
  const mock = createMockBackend({ empty: new URLSearchParams(location.search).has("empty") });
  if (import.meta.env.DEV) Object.assign(window, { __hubMock: mock });
  return mock;
}

// The window opens on index.html without a hash; give the router a real "/" so the sidebar marks
// Hari Ini from the first frame.
if (!location.hash) history.replaceState(null, "", `${location.pathname}${location.search}#/`);

const container = document.getElementById("root");
if (!container) throw new Error("#root missing from index.html");

void pickBackend().then((backend) =>
  createRoot(container).render(
    <StrictMode>
      <App backend={backend} client={createQueryClient()} router={createAppRouter()} />
    </StrictMode>,
  ),
);
