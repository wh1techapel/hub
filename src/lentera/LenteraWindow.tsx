// Lentera's window (Ctrl+Alt+L): the capsule and the bar, drawn in a fixed transparent window at
// the bottom right of the screen (src-tauri/src/lentera.rs). One surface changes shape with one
// spring: the bar grows out of the capsule and folds back into it. The window takes the mouse only
// over that shape; the page reports it whenever it changes.
//
// CP01: until Lentera can answer (CP03), the bar does something real: the line goes through Catat
// Cepat's reader (the same chips, offline) and Enter saves it, with a plain note that answers come
// later. After a save the bar folds into a capsule that says what was saved and leaves after a
// moment; a click elsewhere hides the bar, or folds it into a capsule that keeps the unsaved line.
//
// CP02: a Sesi Fokus shows in the capsule while it runs (unless tucked away): the time left, a thin
// ring, and one action. Hub shows the window for it without taking the keyboard; this page hides
// the window once it has nothing left to show. The bar carries the session's card.
//
// CP03: with a Gemini key the line goes to Lentera (Enter), which answers in a sentence and
// proposes cards; only a card's Simpan writes (talk.ts, Conversation.tsx). Dari Pesan takes
// a pasted group message, cleaned on this computer first. Without a key the line is Catat Cepat's,
// as in CP01; after a failure the owner may hand the line to Catat Cepat. A click elsewhere keeps a
// conversation with cards waiting in the capsule, as it keeps an unsaved line.

import { EyeOff, MessageSquareText, Pause, Play, SkipForward, Square, Timer, X } from "lucide-react";
import { useCallback, useEffect, useId, useLayoutEffect, useRef, useState } from "react";

import { CaptureForm } from "../components/CaptureForm";
import { LenteraMark } from "../components/LenteraMark";
import { useApi } from "../data/api";
import type { LenteraShape } from "../data/api";
import { toHubError, useBackend } from "../data/backend";
import type { DeskMode } from "../data/generated/DeskMode";
import type { FocusPhase } from "../data/generated/FocusPhase";
import type { FocusView } from "../data/generated/FocusView";
import type { Proposal } from "../data/generated/Proposal";
import type { Saved } from "../data/generated/Saved";
import { useChangeSync, useFocus, useGeminiStatus, useSettings } from "../data/queries";
import { useLight } from "../light/useLight";
import { lessMotion } from "../motion/motion";
import type { Cue } from "../sound/palette";
import { useSound } from "../sound/SoundProvider";
import { id } from "../strings/id";
import { useNow } from "../time/clock";
import { ConversationLog, failureText, LenteraLine, MessagePanel, PrivacyLine } from "./Conversation";
import { empty, history, kept, savedCards, step as converse, thinking, waiting } from "./talk";
import type { Card, Conversation, ConversationEvent, Exchange } from "./talk";
import { countdown, focusAction, focusLabel, focusNews, progress, secondsLeft } from "./focusTime";
import type { FocusActionKind } from "./focusTime";
import styles from "./lentera.module.css";
import { givesBack, initial, step } from "./state";
import type { LenteraEvent, LenteraState, View } from "./state";

/** --spring-gentle-duration (tokens.css) and a frame: the shape has settled. */
const SPRING_MS = 560;
/** The bar fading out before the window hides: an exit is quicker than an entry. */
const EXIT_MS = 120;
/** Long enough to read what was saved, short enough to be out of the way. */
export const SAVED_MS = 2400;
/** How long the window must stay without the keyboard before Lentera treats it as a click elsewhere. */
const BLUR_SETTLE_MS = 150;
/** lentera.module.css: the capsule's height (a pill: its radius is half) and the bar's corner. */
const CAPSULE_RADIUS = 20;
const BAR_RADIUS = 22;
/** The ring around Lentera's light (28 px, a 2.5 px stroke). */
const RING_R = 12.5;
const RING_C = 2 * Math.PI * RING_R;

function shapeOf(element: HTMLElement | null, radius: number): LenteraShape | null {
  if (!element) return null;
  const box = element.getBoundingClientRect();
  if (box.width === 0 || box.height === 0) return null;
  return { x: box.left, y: box.top, width: box.width, height: box.height, radius };
}

/** The controls that take Tab inside a part of the window, in order. */
function focusables(root: HTMLElement | null): HTMLElement[] {
  return root ? [...root.querySelectorAll<HTMLElement>("button:not([disabled]), input:not([disabled]), textarea:not([disabled])")] : [];
}

/** Where the owner types (the line, Dari Pesan's text, or Catat Cepat's line). */
function lineOf(bar: HTMLElement | null): HTMLElement | null {
  return bar?.querySelector<HTMLElement>("[data-line] input, [data-line] textarea") ?? null;
}

/** What the bar's input is: Lentera's line, Dari Pesan, or Catat Cepat's (a line handed over). */
type LineMode = "lentera" | "message" | "capture";

/** A clock that moves once a second while `active`: the countdown, never faster. */
function useSecond(active: boolean): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (!active) return;
    let interval: ReturnType<typeof setInterval> | undefined;
    // On the second, so the countdown changes when the time does.
    const first = setTimeout(
      () => {
        setNow(Date.now());
        interval = setInterval(() => setNow(Date.now()), 1000);
      },
      1000 - (Date.now() % 1000),
    );
    // Right now too, so a countdown shown again is never a second stale.
    const at = setTimeout(() => setNow(Date.now()), 0);
    return () => {
      clearTimeout(first);
      clearTimeout(at);
      clearInterval(interval);
    };
  }, [active]);
  return now;
}

/** The session as one word for comparing two looks: its phase and whether it is paused. */
function sessionKey(view: FocusView | null): string {
  return view ? `${view.startedAt}|${view.phase}|${view.phaseStartedAt}|${view.pausedAt ?? ""}` : "none";
}

/** What a change sounds like (never the countdown). */
function cueFor(before: FocusView | null, after: FocusView | null): Cue | null {
  if (!before && after) return "sessionStart";
  if (before && !after) return "sessionStop";
  if (!before || !after || before.phase === after.phase) return null;
  if (after.phase === "short_break" || after.phase === "long_break") return "complete";
  if (after.phase === "waiting") return "open";
  return "sessionStart";
}

function Ring({ value, phase }: { value: number; phase: FocusPhase | null }) {
  return (
    <svg className={styles.ringSvg} viewBox="0 0 28 28" aria-hidden="true">
      {phase && (
        <>
          <circle className={styles.ringTrack} cx="14" cy="14" r={RING_R} />
          <circle
            className={styles.ringFill}
            data-phase={phase}
            cx="14"
            cy="14"
            r={RING_R}
            strokeDasharray={RING_C}
            strokeDashoffset={RING_C * (1 - value)}
            transform="rotate(-90 14 14)"
          />
        </>
      )}
    </svg>
  );
}

const ACTION_ICON: Record<FocusActionKind, typeof Pause> = { pause: Pause, resume: Play, skip: SkipForward, next: Play };

export function LenteraWindow() {
  const backend = useBackend();
  const api = useApi();
  const sound = useSound();
  // The window lives on while hidden: it follows the owner's changes (the light, the ink, a session).
  useChangeSync();
  const settings = useSettings();
  const now = useNow();
  const light = useLight(settings.data, now, backend, false);
  const focus = useFocus();
  const session = focus.data ?? null;
  const sessionShows = session !== null && !session.tucked;
  // CP03: Lentera answers only with a key; without one the line is Catat Cepat's (as in CP01).
  const gemini = useGeminiStatus();
  const brain = gemini.data?.hasKey ?? false;
  const [state, setState] = useState<LenteraState>(initial);
  // What the window shows: a session's capsule stands in when nothing else is open.
  const shown: View = state.view === "hidden" && sessionShows ? "capsule" : state.view;
  // The unsaved line, for the capsule and for whether a click elsewhere keeps it.
  const [draft, setDraft] = useState("");
  // A new round is a fresh line (and kind): Lentera opened again with nothing left unsaved.
  const [round, setRound] = useState(0);
  // The conversation with Lentera (CP03), in this window's memory only.
  const [conversation, setConversation] = useState<Conversation>(empty);
  const [lineMode, setLineMode] = useState<LineMode>("lentera");
  // Dari Pesan opened by a paste into the line: what was pasted, cleaned as the panel opens.
  const [pasted, setPasted] = useState("");
  // The line handed to Catat Cepat after Lentera could not answer it.
  const [handed, setHanded] = useState("");
  const [privacyOpen, setPrivacyOpen] = useState(false);
  const privacyId = useId();
  // What the conversation says once to a screen reader (an answer, never its stream).
  const [spoken, setSpoken] = useState("");
  const current = useRef(state);
  const shownNow = useRef(shown);
  const sessionShowsNow = useRef(sessionShows);
  const draftNow = useRef("");
  const conversationNow = useRef<Conversation>(empty);
  const giveBack = useRef(false);
  const surface = useRef<HTMLDivElement>(null);
  const bar = useRef<HTMLDivElement>(null);
  const capsule = useRef<HTMLDivElement>(null);
  const exitTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const savedTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  useEffect(() => {
    shownNow.current = shown;
    sessionShowsNow.current = sessionShows;
  }, [shown, sessionShows]);

  const dispatch = useCallback(
    function dispatch(event: LenteraEvent) {
      const before = current.current;
      const after = step(before, event);
      if (after === before) return;
      current.current = after;
      setState(after);
      clearTimeout(savedTimer.current);
      giveBack.current = givesBack(event);
      if (event.type === "saved") {
        // The owner gets the keyboard back at once; the capsule says what was saved meanwhile.
        void api.returnLenteraFocus().catch(() => undefined);
        savedTimer.current = setTimeout(() => dispatch({ type: "expire" }), SAVED_MS);
      }
      // Closed into a session's capsule: the window stays, the keyboard goes back.
      if (giveBack.current && after.view === "hidden" && sessionShowsNow.current) void api.returnLenteraFocus().catch(() => undefined);
      // Opened again with nothing kept: a fresh line and a fresh conversation.
      if (before.view === "hidden" && after.view === "bar" && !draftNow.current.trim() && !kept(conversationNow.current)) setRound((n) => n + 1);
      // A capsule shown quietly (a session) cannot take the keyboard until the bar opens.
      if (after.view === "bar" && before.view !== "bar" && !document.hasFocus()) void api.focusLentera().catch(() => undefined);
    },
    [api],
  );

  // Nothing left to show: the window fades out, then hides. Shown again meanwhile, it stays.
  const wasShown = useRef(false);
  useEffect(() => {
    if (shown !== "hidden") {
      wasShown.current = true;
      clearTimeout(exitTimer.current);
      return;
    }
    if (!wasShown.current) return;
    wasShown.current = false;
    const give = giveBack.current;
    giveBack.current = false;
    exitTimer.current = setTimeout(() => void api.hideLentera(give).catch(() => undefined), lessMotion() ? 0 : EXIT_MS);
  }, [shown, api]);

  const keepDraft = useCallback((text: string) => {
    draftNow.current = text;
    setDraft(text);
  }, []);

  const converseWith = useCallback((event: ConversationEvent): Conversation => {
    const next = converse(conversationNow.current, event);
    conversationNow.current = next;
    setConversation(next);
    return next;
  }, []);

  // A new round starts a fresh conversation in Lentera's own line.
  useEffect(() => {
    converseWith({ type: "reset" });
    setLineMode("lentera");
    setPasted("");
    setHanded("");
    setPrivacyOpen(false);
  }, [round, converseWith]);

  /** The owner's line (or a cleaned group message) to Lentera, and its answer as it streams. */
  const ask = (text: string, mode: DeskMode) => {
    if (thinking(conversationNow.current)) return;
    const turns = history(conversationNow.current);
    converseWith({ type: "ask", text, mode });
    setLineMode("lentera");
    setPasted("");
    keepDraft("");
    setSpoken(id.lentera.thinking);
    void api
      .askLentera(text, turns, mode, (event) => {
        switch (event.kind) {
          case "started":
            converseWith({ type: "started", sent: event.sent });
            break;
          case "delta":
            converseWith({ type: "delta", text: event.text });
            break;
          case "proposals":
            converseWith({ type: "proposals", proposals: event.proposals });
            break;
          case "finished": {
            const next = converseWith({ type: "finished", memory: event.memory, stopped: event.stopped });
            const cards = next.exchanges.at(-1)?.cards.length ?? 0;
            setSpoken(event.stopped ? id.lentera.stopped : id.lentera.answered(cards));
            if (cards) sound.play("open", "capture");
            break;
          }
        }
      })
      .catch((failure: unknown) => {
        const reason = toHubError(failure).view.reason ?? "fallback";
        converseWith({ type: "failed", reason });
        setSpoken(failureText(reason));
      });
  };

  /** After a card is decided: the next waiting card's Simpan takes the keyboard, or the line;
   * when nothing waits and something was saved, the bar folds into the capsule that says so. */
  const afterDecision = (next: Conversation) => {
    if (!thinking(next) && waiting(next).length === 0) {
      const saved = savedCards(next);
      const last = saved.at(-1)?.saved;
      if (last) {
        dispatch({ type: "saved", saved: last, count: saved.length });
        return;
      }
    }
    requestAnimationFrame(() => {
      const nextCard = bar.current?.querySelector<HTMLElement>('[data-card-status="pending"] [data-card-primary]');
      (nextCard ?? lineOf(bar.current))?.focus();
    });
  };

  const save = async (card: Card, proposal: Proposal): Promise<void> => {
    converseWith({ type: "saving", card: card.key, proposal });
    let saved: Saved;
    try {
      saved = await api.saveLenteraProposal(proposal);
    } catch {
      converseWith({ type: "saveFailed", card: card.key });
      return;
    }
    sound.play("saved", "capture");
    afterDecision(converseWith({ type: "saved", card: card.key, saved }));
  };

  const saveAll = async (cards: Card[]) => {
    // One after another, in Lentera's order; each one is its own write, as its own Simpan would be.
    for (const card of cards) await save(card, card.proposal);
  };

  const skip = (card: Card) => afterDecision(converseWith({ type: "skip", card: card.key }));

  const handToCapture = (exchange: Exchange) => {
    setHanded(exchange.text);
    setLineMode("capture");
    keepDraft(exchange.text);
  };

  // Shown once the page is drawn in the right light (window_ready). Built because the owner called
  // Lentera, it opens the bar while the owner watches; built for a session, it shows the capsule.
  const ready = useRef(false);
  useEffect(() => {
    if (ready.current || (!light && !settings.isError)) return;
    ready.current = true;
    void document.fonts.ready
      .then(() => api.windowReady())
      .then(() => api.takeLenteraOpen())
      .then(
        (open) => {
          if (open) dispatch({ type: "open" });
        },
        () => dispatch({ type: "open" }),
      );
  }, [light, settings.isError, api, dispatch]);

  // Called again (the shortcut, the tray, Hub), or the shortcut pressed while the bar has the keyboard.
  useEffect(() => backend.on("hub://lentera-open", () => dispatch({ type: "open" })), [backend, dispatch]);
  useEffect(() => backend.on("hub://lentera-close", () => dispatch({ type: "close" })), [backend, dispatch]);

  // A click elsewhere: the bar hides, or folds into a capsule that keeps the unsaved line. Only a
  // blur that lasts counts: showing a new window moves focus between it and its WebView and back.
  useEffect(() => {
    let settle: ReturnType<typeof setTimeout> | undefined;
    const onBlur = () => {
      clearTimeout(settle);
      settle = setTimeout(() => {
        if (!document.hasFocus()) dispatch({ type: "blur", draft: draftNow.current.trim() !== "" || kept(conversationNow.current) });
      }, BLUR_SETTLE_MS);
    };
    window.addEventListener("blur", onBlur);
    return () => {
      clearTimeout(settle);
      window.removeEventListener("blur", onBlur);
    };
  }, [dispatch]);

  // Esc closes from anywhere in the window (the line clears first: CaptureForm takes that Esc).
  // Tab stays inside the bar or the capsule: past either end it comes round again.
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      const view = shownNow.current;
      if (event.defaultPrevented || view === "hidden") return;
      if (event.key === "Escape") {
        event.preventDefault();
        dispatch({ type: "close" });
        return;
      }
      if (event.key !== "Tab") return;
      const order = focusables(view === "bar" ? bar.current : capsule.current);
      const first = order[0];
      const last = order[order.length - 1];
      if (!first || !last) return;
      const at = document.activeElement;
      if (event.shiftKey && (at === first || !order.includes(at as HTMLElement))) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && (at === last || !order.includes(at as HTMLElement))) {
        event.preventDefault();
        first.focus();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [dispatch]);

  useEffect(
    () => () => {
      clearTimeout(exitTimer.current);
      clearTimeout(savedTimer.current);
    },
    [],
  );

  // The bar opened (or its input changed): the keyboard goes to its line.
  useEffect(() => {
    if (state.view !== "bar") return;
    const frame = requestAnimationFrame(() => lineOf(bar.current)?.focus());
    return () => cancelAnimationFrame(frame);
  }, [state.view, round, lineMode, brain]);

  // What changed in the session, said once and heard once (the first look says nothing).
  const [seen, setSeen] = useState<{ key: string; view: FocusView | null } | null>(null);
  const [news, setNews] = useState<{ text: string; cue: Cue | null; n: number }>({ text: "", cue: null, n: 0 });
  if (focus.isSuccess && seen?.key !== sessionKey(session)) {
    setSeen({ key: sessionKey(session), view: session });
    if (seen) {
      const text = focusNews(seen.view, session);
      if (text) setNews({ text, cue: cueFor(seen.view, session), n: news.n + 1 });
    }
  }
  useEffect(() => {
    if (news.cue) sound.play(news.cue, "capture");
  }, [news, sound]);

  // The countdown moves once a second while it is on screen and running.
  const ticking = shown !== "hidden" && session !== null && session.phase !== "waiting" && !session.pausedAt;
  const second = useSecond(ticking);

  const note = state.note;
  const waitingCount = waiting(conversation).length;
  const keptConversation = kept(conversation);
  const label =
    note?.kind === "saved"
      ? note.count > 1
        ? id.lentera.savedMany(note.count)
        : `${id.capture.saved[note.saved.kind]}: ${note.saved.title}`
      : note?.kind === "draft"
        ? keptConversation
          ? thinking(conversation)
            ? id.lentera.thinkingShort
            : id.lentera.waiting(waitingCount)
          : id.lentera.draft(draft.trim().split("\n")[0] ?? "")
        : session
          ? focusLabel(session, second)
          : id.lentera.name;
  const ringed = !note && session !== null;
  const action = !note && session ? focusAction(session) : null;
  const ActionIcon = action ? ACTION_ICON[action.kind] : null;

  // The capsule is as wide as what it says; the surface's clip reads that width.
  const reportedWidth = useRef(0);
  useLayoutEffect(() => {
    const width = capsule.current?.offsetWidth ?? 0;
    if (width <= 0) return;
    surface.current?.style.setProperty("--capsule-w", `${width}px`);
    // A capsule already folded follows its new width at once (a longer label).
    if (shownNow.current === "capsule" && Math.abs(width - reportedWidth.current) > 0.5 && reportedWidth.current > 0) {
      reportedWidth.current = width;
      const shape = shapeOf(capsule.current, CAPSULE_RADIUS);
      if (shape) void api.setLenteraShape(shape).catch(() => undefined);
    }
  }, [label, action?.kind, api]);

  // Where the window takes the mouse. The bar takes it at once, and follows its height as cards
  // and chips come and go; the capsule takes it once the shape has folded into it.
  useEffect(() => {
    if (shown !== "bar") return;
    const report = () => {
      const shape = shapeOf(surface.current, BAR_RADIUS);
      if (shape) void api.setLenteraShape(shape).catch(() => undefined);
    };
    report();
    const node = surface.current;
    if (typeof ResizeObserver !== "function" || !node) return;
    const watch = new ResizeObserver(report);
    watch.observe(node);
    return () => watch.disconnect();
  }, [shown, api]);
  useEffect(() => {
    if (shown !== "capsule") return;
    const timer = setTimeout(
      () => {
        const shape = shapeOf(capsule.current, CAPSULE_RADIUS);
        reportedWidth.current = shape?.width ?? 0;
        if (shape) void api.setLenteraShape(shape).catch(() => undefined);
      },
      lessMotion() ? 0 : SPRING_MS,
    );
    return () => clearTimeout(timer);
  }, [shown, note, draft, api]);

  const run = (kind: FocusActionKind) => {
    const call = { pause: api.pauseFocus, resume: api.resumeFocus, skip: api.nextFocus, next: api.nextFocus }[kind];
    void call().catch(() => undefined);
  };
  const tuck = () => {
    void api.tuckFocus(true).catch(() => undefined);
    dispatch({ type: "close" });
  };
  const minutes = settings.data?.focusMinutes ?? 25;
  const rest = settings.data?.breakMinutes ?? 5;

  return (
    <main className={styles.stage} data-view={shown} aria-label={id.lentera.name}>
      <div className={styles.lamp}>
        <div className={styles.surface} ref={surface}>
          <div className={styles.bar} ref={bar} inert={shown !== "bar"}>
            <header className={styles.head}>
              <LenteraMark size="large" />
              <h1 className={styles.title}>{id.lentera.name}</h1>
              {brain && (
                <button
                  type="button"
                  className={styles.headButton}
                  aria-pressed={lineMode === "message"}
                  onClick={() => {
                    setPasted("");
                    setLineMode(lineMode === "message" ? "lentera" : "message");
                  }}
                >
                  <MessageSquareText size={15} strokeWidth={1.8} aria-hidden="true" />
                  {id.lentera.fromMessage}
                </button>
              )}
              <button type="button" className={styles.close} aria-label={id.lentera.close} title={`${id.lentera.close} (Esc)`} onClick={() => dispatch({ type: "close" })}>
                <X size={16} strokeWidth={1.8} aria-hidden="true" />
              </button>
            </header>
            <section className={styles.cards} aria-label={id.lentera.replies}>
              {session ? (
                <FocusCard view={session} now={second} onAction={run} onTuck={tuck} onStop={() => void api.stopFocus().catch(() => undefined)} />
              ) : (
                conversation.exchanges.length === 0 && (
                  <button type="button" className={styles.startFocus} onClick={() => void api.startFocus({}).catch(() => undefined)}>
                    <Timer size={16} strokeWidth={1.8} aria-hidden="true" />
                    <span>
                      <strong>{id.focus.start}</strong>
                      <small>{id.focus.startHint(minutes, rest)}</small>
                    </span>
                  </button>
                )
              )}
              {/* Without a key the line below is Catat Cepat's, and the bar says so. */}
              {!brain && !session && <p className={styles.note}>{id.lentera.notYet}</p>}
              {brain && lineMode === "capture" && (
                <div className={`${styles.note} ${styles.captureMode}`}>
                  <span>{id.lentera.captureMode}</span>
                  <button type="button" className="btn btn-quiet" onClick={() => setLineMode("lentera")}>
                    {id.lentera.backToLentera}
                  </button>
                </div>
              )}
              <ConversationLog
                conversation={conversation}
                today={now}
                onStop={() => void api.stopLentera().catch(() => undefined)}
                onRetry={(exchange) => {
                  converseWith({ type: "forget", exchange: exchange.key });
                  ask(exchange.sent ?? exchange.text, exchange.mode);
                }}
                onCapture={handToCapture}
                onSave={(card, proposal) => void save(card, proposal)}
                onSkip={skip}
                onRestore={(card) => converseWith({ type: "restore", card: card.key })}
                onSaveAll={(cards) => void saveAll(cards)}
              />
              {brain && (
                <p id={privacyId} className={styles.privacyBody} hidden={!privacyOpen}>
                  {id.lentera.whatIsSentBody}
                </p>
              )}
            </section>
            <div data-line>
              {!brain || lineMode === "capture" ? (
                <CaptureForm
                  key={`${round}|${handed}`}
                  autoFocus
                  label={id.lentera.input}
                  initialText={lineMode === "capture" ? handed : ""}
                  onTextChange={keepDraft}
                  onSaved={(saved) => dispatch({ type: "saved", saved })}
                  onCancel={() => dispatch({ type: "close" })}
                />
              ) : lineMode === "message" ? (
                <MessagePanel
                  key={`${round}|${pasted}`}
                  pasted={pasted}
                  busy={thinking(conversation)}
                  onTextChange={keepDraft}
                  onSend={(text) => ask(text, "message")}
                  onCancel={() => {
                    keepDraft("");
                    setPasted("");
                    setLineMode("lentera");
                  }}
                />
              ) : (
                <LenteraLine
                  key={round}
                  busy={thinking(conversation)}
                  initialText={draftNow.current}
                  onTextChange={keepDraft}
                  onSubmit={(text) => ask(text, "ask")}
                  onPasteMessage={(text) => {
                    setPasted(text);
                    setLineMode("message");
                  }}
                />
              )}
            </div>
            {brain && lineMode !== "capture" && <PrivacyLine open={privacyOpen} controls={privacyId} onToggle={() => setPrivacyOpen((open) => !open)} />}
          </div>
          <div className={styles.capsule} ref={capsule} inert={shown !== "capsule"} data-ringed={ringed || undefined}>
            <button type="button" className={styles.capsuleMain} onClick={() => dispatch({ type: "expand" })}>
              <span className={styles.ring}>
                <Ring value={ringed ? progress(session, second) : 0} phase={ringed ? session.phase : null} />
                <LenteraMark />
              </span>
              <span className={styles.label}>{label}</span>
              <span className="sr-only">. {id.lentera.open}</span>
            </button>
            {note?.kind === "draft" && (
              <button
                type="button"
                className={styles.action}
                aria-label={keptConversation ? id.lentera.discardAll : id.lentera.discard}
                title={keptConversation ? id.lentera.discardAll : id.lentera.discard}
                onClick={() => {
                  if (thinking(conversationNow.current)) void api.stopLentera().catch(() => undefined);
                  keepDraft("");
                  setRound((n) => n + 1);
                  dispatch({ type: "discard" });
                }}
              >
                <X size={15} strokeWidth={1.8} aria-hidden="true" />
              </button>
            )}
            {action && ActionIcon && (
              <button type="button" className={styles.action} aria-label={action.label} title={action.label} onClick={() => run(action.kind)}>
                <ActionIcon size={15} strokeWidth={1.8} aria-hidden="true" />
              </button>
            )}
          </div>
        </div>
      </div>
      {/* What changed, said once: a saved line, or a session's start, break, or end. Never the
          countdown itself, which stays readable in the capsule's name. */}
      <p className="sr-only" role="status">
        {note?.kind === "saved" ? label : news.text}
      </p>
      {/* Lentera's answers, said once when each ends (never while it streams). */}
      <p className="sr-only" aria-live="polite">
        {spoken}
      </p>
    </main>
  );
}

/** The session in the bar: where it is, the time left, and every control. */
function FocusCard({
  view,
  now,
  onAction,
  onTuck,
  onStop,
}: {
  view: FocusView;
  now: number;
  onAction: (kind: FocusActionKind) => void;
  onTuck: () => void;
  onStop: () => void;
}) {
  const action = focusAction(view);
  const Icon = ACTION_ICON[action.kind];
  const phase = view.pausedAt ? id.focus.paused : (id.focus.phase[view.phase] ?? "");
  return (
    <article className={styles.focusCard} aria-label={id.focus.name}>
      <div className={styles.focusHead}>
        <span className={styles.ring}>
          <Ring value={progress(view, now)} phase={view.phase} />
          <LenteraMark />
        </span>
        <div className={styles.focusWhat}>
          <p className={styles.focusPhase}>
            {phase}
            {view.phase === "focus" && ` · ${id.focus.block(view.blocksDone, view.longBreakEvery)}`}
          </p>
          <p className={styles.focusTitle}>{view.title || id.focus.name}</p>
        </div>
        {view.phase !== "waiting" && <p className={styles.focusTime}>{countdown(secondsLeft(view, now))}</p>}
      </div>
      {view.phase === "waiting" && <p className={styles.focusHint}>{id.focus.waitingHint}</p>}
      <div className={styles.focusButtons}>
        <button type="button" className="btn btn-primary" onClick={() => onAction(action.kind)}>
          <Icon size={15} strokeWidth={1.8} aria-hidden="true" />
          {action.label}
        </button>
        <button type="button" className="btn btn-quiet" title={id.focus.tuckHint} onClick={onTuck}>
          <EyeOff size={15} strokeWidth={1.8} aria-hidden="true" />
          {id.focus.tuck}
        </button>
        <button type="button" className="btn btn-quiet" onClick={onStop}>
          <Square size={14} strokeWidth={1.8} aria-hidden="true" />
          {id.focus.stop}
        </button>
      </div>
    </article>
  );
}
