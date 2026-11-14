// Lentera's three states (CP01): hidden (the default), the capsule, and the bar. One pure step
// function decides what each event does; the window (LenteraWindow.tsx) only carries it out. The
// capsule shows only when Lentera is called or has something to say (a line saved, a draft kept),
// never on its own; from CP02 it also carries the focus countdown.

import type { Saved } from "../data/generated/Saved";

export type View = "hidden" | "capsule" | "bar";

/** What the capsule says: a line (or, from CP03, `count` of Lentera's cards) just saved, or a
 * draft kept when the owner clicked elsewhere (since CP03 also a conversation with cards waiting). */
export type Note = { kind: "saved"; saved: Saved; count: number } | { kind: "draft" };

export interface LenteraState {
  view: View;
  note: Note | null;
}

export type LenteraEvent =
  /** Called: Ctrl+Alt+L, the tray, Hub's sidebar, Ctrl+L, the palette. */
  | { type: "open" }
  /** Esc, the close button, or the shortcut while the bar has the keyboard. */
  | { type: "close" }
  /** The line was saved, or the last of Lentera's cards was decided with `count` saved. */
  | { type: "saved"; saved: Saved; count?: number }
  /** The window lost the keyboard (a click elsewhere), with or without an unsaved line. */
  | { type: "blur"; draft: boolean }
  /** The capsule was clicked: back to the bar. */
  | { type: "expand" }
  /** The "saved" capsule has been read. */
  | { type: "expire" }
  /** The draft capsule's action: let the line go. */
  | { type: "discard" };

export const initial: LenteraState = { view: "hidden", note: null };

export function step(state: LenteraState, event: LenteraEvent): LenteraState {
  switch (event.type) {
    case "open":
    case "expand":
      return state.view === "bar" ? state : { view: "bar", note: null };
    case "close":
    case "discard":
      return state.view === "hidden" ? state : initial;
    case "saved":
      return { view: "capsule", note: { kind: "saved", saved: event.saved, count: event.count ?? 1 } };
    case "blur":
      // Only the bar folds away: a capsule is already out of the way.
      if (state.view !== "bar") return state;
      return event.draft ? { view: "capsule", note: { kind: "draft" } } : initial;
    case "expire":
      return state.view === "capsule" && state.note?.kind === "saved" ? initial : state;
  }
}

/** Whether hiding after this event gives the keyboard back to where it was before Lentera: yes
 * when the owner closed Lentera themselves, no after a click elsewhere (that click chose). */
export function givesBack(event: LenteraEvent): boolean {
  return event.type === "close" || event.type === "discard";
}
