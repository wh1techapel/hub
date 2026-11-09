// P2b's micro animation, in three small helpers (docs/design.md, "Motion"): a selection that glides
// to the new choice, list rows that slide to their new place when another leaves or moves, and a
// dialog or toast that leaves instead of vanishing. Each moves only transform, opacity, or
// clip-path, takes 160 to 240 ms, rests when done, and stays still when Windows asks for less
// motion. Where there is no Web Animations API (tests), they do nothing.

import { useLayoutEffect, useRef } from "react";
import type { RefObject } from "react";

export function lessMotion(): boolean {
  return typeof window === "undefined" || typeof window.matchMedia !== "function" || window.matchMedia("(prefers-reduced-motion: reduce)").matches;
}

function canAnimate(): boolean {
  return typeof Element !== "undefined" && typeof Element.prototype.animate === "function" && !lessMotion();
}

// ---------- the gliding selection ----------

export interface Inset {
  top: number;
  right: number;
  bottom: number;
  left: number;
}

/** How far the chosen item sits from each edge of its group: the clip that draws only it. */
export function insetOf(group: { width: number; height: number }, item: { x: number; y: number; width: number; height: number }, line = 0): Inset {
  const bottom = Math.max(0, group.height - (item.y + item.height));
  return {
    top: line > 0 ? Math.max(0, item.y + item.height - line) : Math.max(0, item.y),
    right: Math.max(0, group.width - (item.x + item.width)),
    bottom,
    left: Math.max(0, item.x),
  };
}

/**
 * The group's pill (or, with `line`, its underline of that many px) follows the chosen item:
 * `[data-glide]::before` in global.css draws it, clipped to the item; a new choice moves the clip.
 * `chosen` finds the item: an `input:checked` is taken by its label.
 */
export function useGlide<T extends HTMLElement>(chosen: string, line = 0): RefObject<T | null> {
  const ref = useRef<T>(null);
  useLayoutEffect(() => {
    const group = ref.current;
    if (!group) return;
    const place = () => {
      const found = group.querySelector<HTMLElement>(chosen);
      const item = found instanceof HTMLInputElement ? (found.closest("label") ?? found) : found;
      if (!item) {
        group.style.setProperty("--glide-r", "100%");
        return;
      }
      const g = group.getBoundingClientRect();
      const r = item.getBoundingClientRect();
      const inset = insetOf({ width: g.width, height: g.height }, { x: r.left - g.left, y: r.top - g.top, width: r.width, height: r.height }, line);
      group.style.setProperty("--glide-t", `${inset.top}px`);
      group.style.setProperty("--glide-r", `${inset.right}px`);
      group.style.setProperty("--glide-b", `${inset.bottom}px`);
      group.style.setProperty("--glide-l", `${inset.left}px`);
    };
    place();
    if (line > 0) group.dataset.glideLine = "";
    // The first place is drawn still; only later choices glide.
    if (!group.dataset.glide) {
      group.dataset.glide = "still";
      requestAnimationFrame(() => {
        if (group.isConnected) group.dataset.glide = "moving";
      });
    }
    if (typeof ResizeObserver !== "function") return;
    const watch = new ResizeObserver(place);
    watch.observe(group);
    return () => watch.disconnect();
  });
  return ref;
}

// ---------- rows that slide into place ----------

/** Rows whose top moved, by how much (px), and rows that are new. */
export function movesOf(before: ReadonlyMap<string, number>, after: ReadonlyMap<string, number>): { moved: [string, number][]; fresh: string[] } {
  const moved: [string, number][] = [];
  const fresh: string[] = [];
  for (const [key, top] of after) {
    const was = before.get(key);
    if (was === undefined) fresh.push(key);
    else if (Math.abs(was - top) > 0.5) moved.push([key, was - top]);
  }
  return { moved, fresh: before.size > 0 ? fresh : [] };
}

/** The list's rows (`[data-flip]`, keyed by its value) slide from where they were to where they are. */
export function useFlip(list: RefObject<HTMLElement | null>): void {
  const tops = useRef<Map<string, number>>(new Map());
  useLayoutEffect(() => {
    const root = list.current;
    if (!root) return;
    const rows = [...root.querySelectorAll<HTMLElement>("[data-flip]")];
    const now = new Map(rows.map((row) => [row.dataset.flip ?? "", row.getBoundingClientRect().top]));
    const { moved, fresh } = movesOf(tops.current, now);
    tops.current = now;
    if (!canAnimate()) return;
    const byKey = new Map(rows.map((row) => [row.dataset.flip ?? "", row]));
    for (const [key, dy] of moved) {
      byKey.get(key)?.animate([{ transform: `translateY(${dy}px)` }, { transform: "none" }], { duration: 240, easing: "cubic-bezier(0.2, 0, 0, 1)" });
    }
    for (const key of fresh) {
      byKey.get(key)?.animate([{ opacity: 0, transform: "translateY(-4px)" }, { opacity: 1, transform: "none" }], { duration: 200, easing: "cubic-bezier(0.2, 0, 0, 1)" });
    }
  });
}

// ---------- leaving, not vanishing ----------

/**
 * Something about to be removed (a dialog, a toast) leaves as a copy: still, unreachable, without
 * ids, played out, then gone. The real one is removed at once, so nothing waits for the motion.
 */
export function leave(nodes: (HTMLElement | null)[], frames: Keyframe[][], options: { duration?: number; pin?: boolean } = {}): void {
  const duration = options.duration ?? 160;
  if (!canAnimate()) return;
  for (const [index, node] of nodes.entries()) {
    if (!node?.isConnected) continue;
    const box = node.getBoundingClientRect();
    const ghost = node.cloneNode(true) as HTMLElement;
    // Something placed by its parent (a toast in its region) keeps its place on screen.
    if (options.pin) {
      Object.assign(ghost.style, { position: "fixed", left: `${box.left}px`, top: `${box.top}px`, width: `${box.width}px`, height: `${box.height}px`, margin: "0", zIndex: "60" });
    }
    ghost.removeAttribute("id");
    for (const named of ghost.querySelectorAll("[id]")) named.removeAttribute("id");
    ghost.setAttribute("aria-hidden", "true");
    ghost.inert = true;
    ghost.style.pointerEvents = "none";
    ghost.style.animation = "none";
    document.body.appendChild(ghost);
    const motion = ghost.animate(frames[index] ?? frames[0] ?? [], { duration, easing: "cubic-bezier(0.4, 0, 1, 1)", fill: "forwards" });
    const gone = () => ghost.remove();
    motion.finished.then(gone, gone);
    window.setTimeout(gone, duration + 200);
  }
}
