// The bar's conversation with Lentera (CP03): what the owner sent, what Lentera answered, and the
// cards it proposed, each waiting for Simpan, Ubah, or Lewati. One pure step function, like the
// window's states (state.ts); LenteraWindow.tsx carries it out and Conversation.tsx draws it. The
// conversation lives in the window's memory only: Hub's database never holds it, and the next turn
// sends its last few turns.

import type { HistoryTurn } from "../data/api";
import type { DeskMode } from "../data/generated/DeskMode";
import type { Proposal } from "../data/generated/Proposal";
import type { Saved } from "../data/generated/Saved";
import { id } from "../strings/id";
import { clock, parseClock, relativeDate } from "../time/format";

export type CardStatus = "pending" | "saving" | "saved" | "skipped";

export interface Card {
  key: string;
  proposal: Proposal;
  status: CardStatus;
  /** The last Simpan was refused (shown on the card). */
  failed: boolean;
  saved: Saved | null;
}

export type ExchangeState = "thinking" | "done" | "stopped" | "failed";

export interface Exchange {
  key: number;
  /** What the owner typed or pasted. */
  text: string;
  mode: DeskMode;
  /** What was sent in the owner's turn (a message as cleaned), once the request left. */
  sent: string | null;
  reply: string;
  cards: Card[];
  state: ExchangeState;
  /** Why it failed: Gemini's failure kind or Hub's refusal (`no_key`, `offline`, ...). */
  error: string | null;
  /** What the next turn sends of Lentera's answer (its text and what it proposed). */
  memory: string | null;
}

export interface Conversation {
  exchanges: Exchange[];
  next: number;
}

export type ConversationEvent =
  | { type: "ask"; text: string; mode: DeskMode }
  | { type: "started"; sent: string }
  | { type: "delta"; text: string }
  | { type: "proposals"; proposals: Proposal[] }
  | { type: "finished"; memory: string; stopped: boolean }
  | { type: "failed"; reason: string }
  | { type: "saving"; card: string; proposal: Proposal }
  | { type: "saved"; card: string; saved: Saved }
  | { type: "saveFailed"; card: string }
  | { type: "skip"; card: string }
  | { type: "restore"; card: string }
  /** A failed answer asked again: the failed one goes. */
  | { type: "forget"; exchange: number }
  | { type: "reset" };

export const empty: Conversation = { exchanges: [], next: 1 };

function last(conversation: Conversation): Exchange | undefined {
  return conversation.exchanges[conversation.exchanges.length - 1];
}

/** The running answer, changed by `change`; nothing when none is running. */
function running(conversation: Conversation, change: (exchange: Exchange) => Exchange): Conversation {
  const current = last(conversation);
  if (current?.state !== "thinking") return conversation;
  return { ...conversation, exchanges: [...conversation.exchanges.slice(0, -1), change(current)] };
}

/** The card `key`, changed by `change`; nothing changes when `change` says no (null). */
function card(conversation: Conversation, key: string, change: (card: Card) => Card | null): Conversation {
  const exchanges = conversation.exchanges.map((exchange) => {
    const index = exchange.cards.findIndex((c) => c.key === key);
    const found = exchange.cards[index];
    const next = found ? change(found) : null;
    return next ? { ...exchange, cards: exchange.cards.map((c, i) => (i === index ? next : c)) } : exchange;
  });
  return exchanges.some((exchange, i) => exchange !== conversation.exchanges[i]) ? { ...conversation, exchanges } : conversation;
}

export function step(conversation: Conversation, event: ConversationEvent): Conversation {
  switch (event.type) {
    case "ask": {
      // One answer at a time (the native side stops an older one too).
      if (last(conversation)?.state === "thinking") return conversation;
      const exchange: Exchange = {
        key: conversation.next,
        text: event.text,
        mode: event.mode,
        sent: null,
        reply: "",
        cards: [],
        state: "thinking",
        error: null,
        memory: null,
      };
      return { exchanges: [...conversation.exchanges, exchange], next: conversation.next + 1 };
    }
    case "started":
      return running(conversation, (e) => ({ ...e, sent: event.sent }));
    case "delta":
      return running(conversation, (e) => ({ ...e, reply: e.reply + event.text }));
    case "proposals":
      return running(conversation, (e) => ({
        ...e,
        cards: [
          ...e.cards,
          ...event.proposals.map((proposal, index) => ({ key: `${e.key}-${e.cards.length + index}`, proposal, status: "pending" as const, failed: false, saved: null })),
        ],
      }));
    case "finished":
      return running(conversation, (e) => ({ ...e, state: event.stopped ? "stopped" : "done", memory: event.memory }));
    case "failed":
      return running(conversation, (e) => ({ ...e, state: "failed", error: event.reason }));
    case "saving":
      return card(conversation, event.card, (c) => (c.status === "pending" ? { ...c, proposal: event.proposal, status: "saving", failed: false } : null));
    case "saved":
      return card(conversation, event.card, (c) => (c.status === "saving" ? { ...c, status: "saved", saved: event.saved } : null));
    case "saveFailed":
      return card(conversation, event.card, (c) => (c.status === "saving" ? { ...c, status: "pending", failed: true } : null));
    case "skip":
      return card(conversation, event.card, (c) => (c.status === "pending" ? { ...c, status: "skipped", failed: false } : null));
    case "restore":
      return card(conversation, event.card, (c) => (c.status === "skipped" ? { ...c, status: "pending" } : null));
    case "forget":
      return conversation.exchanges.some((e) => e.key === event.exchange && e.state === "failed")
        ? { ...conversation, exchanges: conversation.exchanges.filter((e) => e.key !== event.exchange) }
        : conversation;
    case "reset":
      return conversation.exchanges.length ? { exchanges: [], next: conversation.next } : conversation;
  }
}

/** Lentera is answering. */
export function thinking(conversation: Conversation): boolean {
  return last(conversation)?.state === "thinking";
}

/** Cards that still wait for the owner (saving ones too). */
export function waiting(conversation: Conversation): Card[] {
  return conversation.exchanges.flatMap((e) => e.cards).filter((c) => c.status === "pending" || c.status === "saving");
}

export function savedCards(conversation: Conversation): Card[] {
  return conversation.exchanges.flatMap((e) => e.cards).filter((c) => c.status === "saved");
}

/** Something a click elsewhere must not throw away: an answer coming, or cards waiting. */
export function kept(conversation: Conversation): boolean {
  return thinking(conversation) || waiting(conversation).length > 0;
}

/** The turns sent with the next question: finished exchanges only, the owner's as sent. */
export function history(conversation: Conversation): HistoryTurn[] {
  return conversation.exchanges.flatMap((e): HistoryTurn[] => {
    if ((e.state !== "done" && e.state !== "stopped") || !e.sent) return [];
    const answer = (e.memory ?? e.reply).trim();
    if (!answer) return [];
    return [
      { from: "owner", text: e.sent, message: e.mode === "message" },
      { from: "lentera", text: answer, message: false },
    ];
  });
}

// ---------- how a proposal reads on its card ----------

export function proposalKind(proposal: Proposal): string {
  return id.lentera.kinds[proposal.kind] ?? proposal.kind;
}

export function proposalTitle(proposal: Proposal): string {
  return proposal.kind === "task" || proposal.kind === "event" ? proposal.title : proposal.text;
}

function timeText(value: string | null): string | null {
  const minutes = value ? parseClock(value) : null;
  return minutes === null ? null : clock(minutes);
}

/** The chips under a card's title, in the order they read best (as Catat Cepat's). */
export function proposalChips(proposal: Proposal, today: Date): { text: string; tone?: "project" }[] {
  const chips: { text: string; tone?: "project" }[] = [];
  const t = id.capture.chips;
  switch (proposal.kind) {
    case "task": {
      if (proposal.dueDate) {
        const at = timeText(proposal.dueTime);
        chips.push({ text: `${t.due} ${relativeDate(proposal.dueDate, today)}${at ? ` ${at}` : ""}` });
      }
      if (proposal.doDate) chips.push({ text: `${t.doDate} ${relativeDate(proposal.doDate, today)}` });
      if (proposal.priority > 0) chips.push({ text: "!".repeat(proposal.priority) });
      if (proposal.estimateMin) chips.push({ text: `${t.estimate} ${proposal.estimateMin} m` });
      if (proposal.project) chips.push({ text: proposal.project.name, tone: "project" });
      if (proposal.area) chips.push({ text: proposal.area.name, tone: "project" });
      if (!proposal.dueDate && !proposal.doDate && !proposal.project && !proposal.area) chips.push({ text: t.inbox });
      break;
    }
    case "reminder": {
      chips.push({ text: relativeDate(proposal.date, today) });
      chips.push({ text: `${t.at} ${timeText(proposal.time) ?? proposal.time}` });
      break;
    }
    case "event": {
      chips.push({ text: relativeDate(proposal.date, today) });
      const start = timeText(proposal.start);
      const end = timeText(proposal.end);
      chips.push({ text: start ? (end ? `${start} sampai ${end}` : start) : t.allDay });
      if (proposal.location) chips.push({ text: `${t.location} ${proposal.location}` });
      if (proposal.project) chips.push({ text: proposal.project.name, tone: "project" });
      break;
    }
    case "note":
      chips.push({ text: relativeDate(proposal.date, today) });
      break;
  }
  return chips;
}
