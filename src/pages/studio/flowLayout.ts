// U5b: where each step and arrow of a PRD's user flow diagram goes, drawn top to bottom. A small
// layered layout of Hub's own (no library): arrows that go back up (a "coba lagi") are found
// first and drawn along the right side; the rest decide each step's row by the longest way from a
// start; steps in a row are ordered once by where their predecessors sit, to cross fewer arrows.

import type { FlowChart } from "../../data/generated/FlowChart";
import type { FlowNodeKind } from "../../data/generated/FlowNodeKind";

export const NODE_WIDTH = 196;
const ROW_GAP = 64;
const COLUMN_GAP = 44;
const LINE_HEIGHT = 18;
const PADDING_Y = 14;
const MARGIN = 16;
/** About this many characters fit a line of a step's box. */
const LINE_CHARS = 24;
const MAX_LINES = 3;
const BACK_LANE = 22;

export interface PlacedNode {
  id: string;
  kind: FlowNodeKind;
  feature: string;
  lines: string[];
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface PlacedEdge {
  from: string;
  to: string;
  label: string;
  /** The path's corners, from the source's edge to the target's. */
  points: [number, number][];
  labelAt: [number, number] | null;
  /** Goes back up to an earlier row. */
  back: boolean;
}

export interface FlowLayout {
  width: number;
  height: number;
  nodes: PlacedNode[];
  edges: PlacedEdge[];
  /** Some arrows named steps that do not exist and were left out. */
  incomplete: boolean;
}

/** A label in at most three lines of about 24 characters, the last cut with an ellipsis. */
export function wrap(text: string): string[] {
  const words = text.trim().split(/\s+/).filter(Boolean);
  const lines: string[] = [];
  let line = "";
  for (const word of words) {
    const next = line ? `${line} ${word}` : word;
    if (next.length <= LINE_CHARS || !line) line = next;
    else {
      lines.push(line);
      line = word;
    }
  }
  if (line) lines.push(line);
  if (lines.length <= MAX_LINES) return lines.map((l) => (l.length > LINE_CHARS + 6 ? `${l.slice(0, LINE_CHARS + 5)}…` : l));
  const kept = lines.slice(0, MAX_LINES);
  kept[MAX_LINES - 1] = `${(kept[MAX_LINES - 1] ?? "").slice(0, LINE_CHARS - 1)}…`;
  return kept;
}

export function layoutFlow(flow: FlowChart): FlowLayout {
  const byId = new Map(flow.nodes.map((node) => [node.id, node]));
  const edges = flow.edges.filter((edge) => byId.has(edge.from) && byId.has(edge.to) && edge.from !== edge.to);
  const incomplete = edges.length !== flow.edges.length;
  if (flow.nodes.length === 0) return { width: 0, height: 0, nodes: [], edges: [], incomplete };

  const outgoing = new Map<string, typeof edges>();
  for (const edge of edges) outgoing.set(edge.from, [...(outgoing.get(edge.from) ?? []), edge]);

  // Arrows back to a step still on the way (a cycle) are drawn as returns, not as rows.
  const back = new Set<(typeof edges)[number]>();
  const state = new Map<string, "open" | "done">();
  const visit = (id: string) => {
    state.set(id, "open");
    for (const edge of outgoing.get(id) ?? []) {
      const seen = state.get(edge.to);
      if (seen === "open") back.add(edge);
      else if (!seen) visit(edge.to);
    }
    state.set(id, "done");
  };
  const starts = flow.nodes.filter((node) => node.kind === "start").map((node) => node.id);
  for (const id of [...starts, ...flow.nodes.map((node) => node.id)]) if (!state.has(id)) visit(id);

  // Rows: the longest way from a step with nothing before it.
  const forward = edges.filter((edge) => !back.has(edge));
  const incoming = new Map<string, number>(flow.nodes.map((node) => [node.id, 0]));
  for (const edge of forward) incoming.set(edge.to, (incoming.get(edge.to) ?? 0) + 1);
  const row = new Map<string, number>(flow.nodes.map((node) => [node.id, 0]));
  const queue = flow.nodes.filter((node) => incoming.get(node.id) === 0).map((node) => node.id);
  for (let i = 0; i < queue.length; i++) {
    const id = queue[i] as string;
    for (const edge of forward.filter((e) => e.from === id)) {
      row.set(edge.to, Math.max(row.get(edge.to) ?? 0, (row.get(id) ?? 0) + 1));
      const left = (incoming.get(edge.to) ?? 0) - 1;
      incoming.set(edge.to, left);
      if (left === 0) queue.push(edge.to);
    }
  }

  // Order in a row: as found, then once by where the predecessors sit.
  const rows: string[][] = [];
  for (const id of queue.length === flow.nodes.length ? queue : flow.nodes.map((node) => node.id)) {
    const r = row.get(id) ?? 0;
    (rows[r] ??= []).push(id);
  }
  const column = new Map<string, number>();
  rows.forEach((ids) => ids.forEach((id, i) => column.set(id, i)));
  for (let r = 1; r < rows.length; r++) {
    const ids = rows[r] ?? [];
    const weight = (id: string) => {
      const before = forward.filter((e) => e.to === id).map((e) => column.get(e.from) ?? 0);
      return before.length ? before.reduce((a, b) => a + b, 0) / before.length : (column.get(id) ?? 0);
    };
    ids.sort((a, b) => weight(a) - weight(b));
    ids.forEach((id, i) => column.set(id, i));
  }

  // Sizes and places.
  const lines = new Map(flow.nodes.map((node) => [node.id, wrap(node.label)]));
  const heightOf = (id: string) => {
    const node = byId.get(id);
    const extra = node?.feature.trim() ? 1 : 0;
    return PADDING_Y * 2 + LINE_HEIGHT * ((lines.get(id)?.length ?? 1) + extra);
  };
  const widest = Math.max(...rows.map((ids) => ids.length));
  const innerWidth = widest * NODE_WIDTH + (widest - 1) * COLUMN_GAP;
  const placed = new Map<string, PlacedNode>();
  let y = MARGIN;
  for (const ids of rows) {
    const rowHeight = Math.max(...ids.map(heightOf));
    const rowWidth = ids.length * NODE_WIDTH + (ids.length - 1) * COLUMN_GAP;
    let x = MARGIN + (innerWidth - rowWidth) / 2;
    for (const id of ids) {
      const node = byId.get(id);
      if (!node) continue;
      const height = heightOf(id);
      placed.set(id, { id, kind: node.kind, feature: node.feature.trim(), lines: lines.get(id) ?? [], x, y: y + (rowHeight - height) / 2, width: NODE_WIDTH, height });
      x += NODE_WIDTH + COLUMN_GAP;
    }
    y += rowHeight + ROW_GAP;
  }
  const right = MARGIN + innerWidth;

  // Arrows: down from the source's bottom (spread when there are several) to the target's top;
  // returns go out to the right, up, and back in.
  let lane = 0;
  const outEdges: PlacedEdge[] = [];
  for (const node of flow.nodes) {
    const from = placed.get(node.id);
    if (!from) continue;
    const mine = (outgoing.get(node.id) ?? []).filter((edge) => !back.has(edge));
    mine.sort((a, b) => (placed.get(a.to)?.x ?? 0) - (placed.get(b.to)?.x ?? 0));
    mine.forEach((edge, i) => {
      const to = placed.get(edge.to);
      if (!to) return;
      const sx = from.x + (from.width * (i + 1)) / (mine.length + 1);
      const sy = from.y + from.height;
      const tx = to.x + to.width / 2;
      const ty = to.y;
      const midY = sy + Math.min(ROW_GAP / 2, (ty - sy) / 2);
      const points: [number, number][] = Math.abs(sx - tx) < 1 ? [[sx, sy], [tx, ty]] : [[sx, sy], [sx, midY], [tx, midY], [tx, ty]];
      outEdges.push({ from: edge.from, to: edge.to, label: edge.label.trim(), points, labelAt: edge.label.trim() ? [sx, sy + 14] : null, back: false });
    });
  }
  for (const edge of edges.filter((e) => back.has(e))) {
    const from = placed.get(edge.from);
    const to = placed.get(edge.to);
    if (!from || !to) continue;
    lane += 1;
    const laneX = right + BACK_LANE * lane;
    const sy = from.y + from.height / 2;
    const ty = to.y + to.height / 2;
    const points: [number, number][] = [
      [from.x + from.width, sy],
      [laneX, sy],
      [laneX, ty],
      [to.x + to.width, ty],
    ];
    outEdges.push({ from: edge.from, to: edge.to, label: edge.label.trim(), points, labelAt: edge.label.trim() ? [laneX, (sy + ty) / 2] : null, back: true });
  }

  return {
    // Room right of the last lane for a return's label.
    width: right + MARGIN + (lane > 0 ? BACK_LANE * lane + 96 : 0),
    height: y - ROW_GAP + MARGIN,
    nodes: [...placed.values()],
    edges: outEdges,
    incomplete,
  };
}
