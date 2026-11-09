// The light engine: puts the phase, scheme, and ink on the root element (design/tokens.css does the
// rest), and paints the Windows title bar to match once the light has settled.

import { useLayoutEffect, useRef, useSyncExternalStore } from "react";

import { api } from "../data/api";
import type { Rgb } from "../data/api";
import type { Backend } from "../data/backend";
import type { Settings } from "../data/generated/Settings";
import { minutesOf, parseClock } from "../time/format";
import { resolveLight } from "./phase";
import type { Light } from "./phase";

/** design/tokens.css --dur-ambient, plus a little. */
const SHIFT_MS = 2500;

const darkQuery = typeof matchMedia === "function" ? matchMedia("(prefers-color-scheme: dark)") : null;

function subscribeDark(listener: () => void): () => void {
  darkQuery?.addEventListener("change", listener);
  return () => darkQuery?.removeEventListener("change", listener);
}

function useSystemDark(): boolean {
  return useSyncExternalStore(subscribeDark, () => darkQuery?.matches ?? false);
}

/** A CSS colour (any syntax the engine knows, oklch included) as sRGB bytes. */
export function cssColorToRgb(color: string): Rgb | null {
  const canvas = document.createElement("canvas");
  canvas.width = 1;
  canvas.height = 1;
  const context = canvas.getContext("2d", { willReadFrequently: true });
  if (!context || !color) return null;
  context.fillStyle = color;
  context.fillRect(0, 0, 1, 1);
  const [r = 0, g = 0, b = 0] = context.getImageData(0, 0, 1, 1).data;
  return { r, g, b };
}

function paintCaption(backend: Backend): void {
  const style = getComputedStyle(document.documentElement);
  const paper = cssColorToRgb(style.getPropertyValue("--paper-2").trim());
  const ink = cssColorToRgb(style.getPropertyValue("--ink").trim());
  if (paper && ink) void api(backend).setCaption(paper, ink).catch(() => undefined);
}

/** `paintTitleBar` is off for the frameless Catat Cepat window, which has no title bar. */
export function useLight(settings: Settings | undefined, now: Date, backend: Backend, paintTitleBar = true): Light | null {
  const systemDark = useSystemDark();
  const applied = useRef<string | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

  const light = settings
    ? resolveLight(settings.themeMode, minutesOf(now), parseClock(settings.wakeTime) ?? 300, systemDark)
    : null;
  const phase = light?.phase;
  const scheme = light?.scheme;
  const ink = settings?.ink;

  useLayoutEffect(() => {
    if (!phase || !scheme || !ink) return;
    const root = document.documentElement;
    const key = `${phase}/${ink}`;
    if (applied.current === key) return;
    const first = applied.current === null;
    const phaseChanged = root.dataset.phase !== phase;
    if (first) root.style.transition = "none";
    root.dataset.phase = phase;
    root.dataset.scheme = scheme;
    root.dataset.ink = ink;
    applied.current = key;
    clearTimeout(timer.current);
    if (first || !phaseChanged) {
      // The first light is set before the window is shown and must not fade in from the
      // placeholder in index.html: flush it with transitions off, then turn them back on.
      if (first) {
        getComputedStyle(root).getPropertyValue("--paper");
        root.style.removeProperty("transition");
      }
      delete root.dataset.shifting;
      if (paintTitleBar) paintCaption(backend);
      return;
    }
    // While the light cross-fades, element colour transitions would restart every frame.
    root.dataset.shifting = "";
    timer.current = setTimeout(() => {
      delete root.dataset.shifting;
      if (paintTitleBar) paintCaption(backend);
    }, SHIFT_MS);
  }, [phase, scheme, ink, backend, paintTitleBar]);

  useLayoutEffect(() => () => clearTimeout(timer.current), []);

  return light;
}
