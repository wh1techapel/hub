import "@testing-library/jest-dom/vitest";

import { cleanup } from "@testing-library/react";
import { afterEach } from "vitest";

afterEach(() => {
  cleanup();
  document.documentElement.removeAttribute("data-shifting");
  document.documentElement.removeAttribute("style");
});

// jsdom has no matchMedia and no font loading.
if (typeof window.matchMedia !== "function") {
  Object.defineProperty(window, "matchMedia", {
    value: (query: string) => ({
      matches: false,
      media: query,
      addEventListener: () => undefined,
      removeEventListener: () => undefined,
    }),
  });
}

if (!("fonts" in document)) {
  Object.defineProperty(document, "fonts", { value: { ready: Promise.resolve() } });
}

// jsdom has no pointer capture; the calendar's drag handles ask for it.
if (!("setPointerCapture" in Element.prototype)) {
  Object.assign(Element.prototype, {
    setPointerCapture: () => undefined,
    releasePointerCapture: () => undefined,
    hasPointerCapture: () => false,
  });
}

// jsdom has no layout, so no hit testing; the drop pools ask for it (tests stub it per case).
if (typeof document.elementFromPoint !== "function") {
  Object.defineProperty(document, "elementFromPoint", { value: () => null, writable: true, configurable: true });
}

// jsdom has no layout, so no scrolling; Studio keeps the end of the conversation in view.
if (typeof Element.prototype.scrollIntoView !== "function") {
  Object.assign(Element.prototype, { scrollIntoView: () => undefined });
}
