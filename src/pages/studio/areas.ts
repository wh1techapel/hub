// The five parts of an idea that stage 1 makes clear (docs/studio.md, "Revision: three stages",
// R2), and how clear each one is, read from the ledger alone. The same rule is in Rust
// (`hub_ai::prompt::area_state`), where Gemini reads it: change both together.

import type { LedgerArea } from "../../data/generated/LedgerArea";
import type { LedgerItem } from "../../data/generated/LedgerItem";

export const AREAS: LedgerArea[] = ["problem", "users", "features", "constraints", "success"];

export type AreaState = "clear" | "partial" | "unknown";

/** Words in an item's topic that place items from before R2, which have no part of their own. */
const GUESS: [LedgerArea, RegExp][] = [
  ["problem", /masalah|problem|keluhan|kendala utama/i],
  ["users", /pengguna|user|target|persona|siapa/i],
  ["success", /ukur|metrik|berhasil|sukses|indikator|kpi/i],
  ["features", /fitur|feature|mvp|lingkup|scope|alur/i],
  ["constraints", /batas|waktu|biaya|anggaran|perangkat|platform|offline|privasi|data|teknis/i],
];

/** The part an item settles: its own, or for an older item a guess from its kind and topic. */
export function areaOf(item: Pick<LedgerItem, "area" | "kind" | "topic">): LedgerArea | null {
  if (item.area) return item.area;
  if (item.kind === "constraint") return "constraints";
  return GUESS.find(([, words]) => words.test(item.topic))?.[0] ?? null;
}

export interface AreaSummary {
  area: LedgerArea;
  state: AreaState;
  items: LedgerItem[];
  decided: number;
  assumed: number;
  open: number;
}

/** Clear when something settles the part (an active decision or constraint, a confirmed
 * assumption, an answered question) and no question about it is open; partly clear when it has
 * items but not that; unknown when it has none. Retired items count for nothing. */
export function summarize(ledger: LedgerItem[]): AreaSummary[] {
  return AREAS.map((area) => {
    const items = ledger.filter((item) => item.status !== "retired" && areaOf(item) === area);
    const settles = (item: LedgerItem) =>
      ((item.kind === "decision" || item.kind === "constraint") && item.status === "active") ||
      (item.kind === "assumption" && item.status === "confirmed") ||
      (item.kind === "question" && item.status === "answered");
    const decided = items.filter(settles).length;
    const assumed = items.filter((item) => item.kind === "assumption" && item.status === "active").length;
    const open = items.filter((item) => item.kind === "question" && item.status === "active").length;
    const state: AreaState = items.length === 0 ? "unknown" : decided > 0 && open === 0 ? "clear" : "partial";
    return { area, state, items, decided, assumed, open };
  });
}
