// Percakapan: the owner and Lentera (Studio's agent, Gemini underneath), on paper rather than in
// bubbles. A reply streams in as it is written (Hentikan or Esc stops it, what arrived stays); a
// stopped or failed one offers Coba lagi. Lentera's questions come as one set grouped by part of
// the idea (R2), answered in part and sent as one message, and a reply that asks shows only them
// (L1); its proposal to move on opens the check-answers sheet. The messages scroll inside their own area and follow new text only while
// the owner is at the end; the input stays at the bottom and grows with what is typed.

import { useQueryClient } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { ArrowDown, ArrowUp, Square } from "lucide-react";
import { useEffect, useId, useLayoutEffect, useRef, useState } from "react";

import { useApi } from "../../data/api";
import type { ReplyEvent } from "../../data/api";
import { toHubError } from "../../data/backend";
import type { Question } from "../../data/generated/Question";
import type { StageProposal } from "../../data/generated/StageProposal";
import type { StudioMessage } from "../../data/generated/StudioMessage";
import type { StudioProject } from "../../data/generated/StudioProject";
import type { StudioStage } from "../../data/generated/StudioStage";
import { keys, useStudioMessages } from "../../data/queries";
import { id } from "../../strings/id";
import { AREAS } from "./areas";
import { takePending } from "./pending";
import { STEP_OF } from "./steps";
import { Prose } from "./Prose";
import { ResultCard } from "./ResultCard";
import { ModelPicker } from "./ModelPicker";
import { interleave } from "./results";
import type { Result, ResultKind } from "./results";
import styles from "./project.module.css";

const t = id.studio.chat;
// Failures another model may not have. Google answers "high demand" (service) and refuses a request
// (bad_request) per model, and counts the free quota per model (real run, 2026-11-09).
const MODEL_FAILURES = new Set(["quota", "model", "payment", "service", "bad_request"]);

interface Live {
  id: string | null;
  text: string;
  parts: unknown;
}

function questionsOf(parts: unknown): Question[] {
  const list = (parts as { questions?: unknown } | null)?.questions;
  // Questions stored before R2 have no part and no "multiple": read them as ungrouped, one answer.
  return Array.isArray(list)
    ? (list as Partial<Question>[])
        .filter((q): q is Partial<Question> & { text: string } => typeof q.text === "string")
        .map((q) => ({ text: q.text, why: q.why ?? "", options: q.options ?? [], area: q.area ?? "", multiple: q.multiple === true }))
    : [];
}

function stageOf(parts: unknown): StageProposal | null {
  const stage = (parts as { stage?: StageProposal } | null)?.stage;
  return stage && typeof stage.next === "string" ? stage : null;
}

export function Conversation({
  project,
  hasKey,
  onAdvance,
  results = [],
  onOpenResult = () => undefined,
  onApproveResult = () => undefined,
}: {
  project: StudioProject;
  hasKey: boolean;
  onAdvance: (to: StudioStage) => void;
  /** U4b: the project's results, shown as cards among the messages. */
  results?: Result[];
  onOpenResult?: (kind: ResultKind) => void;
  onApproveResult?: (kind: ResultKind) => void;
}) {
  const api = useApi();
  const client = useQueryClient();
  const messages = useStudioMessages(project.id);
  const [draft, setDraft] = useState("");
  const [live, setLive] = useState<Live | null>(null);
  const [busy, setBusy] = useState(false);
  const [said, setSaid] = useState("");
  const [problem, setProblem] = useState<string | null>(null);
  const [problemKind, setProblemKind] = useState<string | null>(null);
  const [atEnd, setAtEnd] = useState(true);
  const log = useRef<HTMLDivElement>(null);
  const following = useRef(true);
  const input = useRef<HTMLTextAreaElement>(null);

  const list = messages.data ?? [];
  const refresh = () => client.invalidateQueries({ queryKey: keys.studio });

  const onEvent = (event: ReplyEvent) => {
    switch (event.kind) {
      case "started":
        setLive({ id: event.messageId, text: "", parts: null });
        break;
      case "delta":
        setLive((now) => (now ? { ...now, text: now.text + event.text } : now));
        break;
      case "parts":
        setLive((now) => (now ? { ...now, parts: event.parts } : now));
        break;
      case "finished":
        setSaid(event.state === "complete" ? t.answered : "");
        break;
    }
  };

  const run = async (act: () => Promise<undefined>) => {
    setBusy(true);
    setProblem(null);
    setProblemKind(null);
    setSaid("");
    try {
      await act();
    } catch (error) {
      const view = toHubError(error).view;
      setProblem(view.reason === "no_key" ? t.noKey : (t.failed[view.reason ?? ""] ?? t.failed.fallback ?? ""));
      setProblemKind(view.reason ?? null);
    } finally {
      await refresh();
      setLive(null);
      setBusy(false);
    }
  };

  /** Sends a message; the input's draft is cleared only when it was the input that sent it. */
  const send = (text: string, fromInput = true) => {
    const body = text.trim();
    if (!body || busy || !hasKey) return;
    if (fromInput) setDraft("");
    // What the owner sends is followed to the end, wherever they were reading.
    following.current = true;
    setAtEnd(true);
    void run(() => api.sendStudioMessage(project.id, body, onEvent));
  };
  const retry = () => void run(() => api.retryStudioReply(project.id, onEvent));
  const stop = () => void api.stopStudioReply(project.id);

  // The first idea, typed on the Studio page, is sent once the conversation is on screen (taken
  // inside the timer, so a remount in development sends it once).
  useEffect(() => {
    const timer = setTimeout(() => {
      const first = takePending(project.id);
      if (first) send(first);
    }, 0);
    return () => clearTimeout(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [project.id]);

  const toEnd = () => {
    const node = log.current;
    if (node) node.scrollTop = node.scrollHeight;
  };
  const onScroll = () => {
    const node = log.current;
    if (!node) return;
    const near = node.scrollHeight - node.scrollTop - node.clientHeight < 48;
    following.current = near;
    setAtEnd(near);
  };

  const last = list[list.length - 1];
  const lastReply = [...list].reverse().find((m) => m.role === "model");
  const answeredAfter = lastReply ? list.indexOf(lastReply) < list.length - 1 : true;
  const canRetry =
    !busy && hasKey && last !== undefined && (last.role === "owner" || last.state === "stopped" || last.state === "failed");
  const questions = !busy && lastReply?.state === "complete" && !answeredAfter ? questionsOf(lastReply.parts) : [];
  const proposal = !busy && lastReply?.state === "complete" ? stageOf(lastReply.parts) : null;
  const showLiveAlone = live?.id !== null && live !== null && !list.some((m) => m.id === live.id);
  // L2: a reply that failed for its model (quota, unavailable, billing) offers another model
  // beside Coba lagi; Google counts the free quota per model, so another one can answer now.
  const modelFailed =
    canRetry &&
    (MODEL_FAILURES.has(problemKind ?? "") || (last.role === "model" && last.state === "failed" && MODEL_FAILURES.has(last.errorKind ?? "")));

  // A change of size (the panel opening, the conversation moving into it) keeps the end in view.
  useEffect(() => {
    const node = log.current;
    if (!node || typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(() => {
      if (following.current) node.scrollTop = node.scrollHeight;
    });
    observer.observe(node);
    return () => observer.disconnect();
  }, []);

  // New words keep the end in view, but only while the owner is there: reading an earlier
  // message is not interrupted.
  const hasProposal = proposal !== null;
  useEffect(() => {
    if (following.current) toEnd();
  }, [list.length, live?.text, questions.length, hasProposal, canRetry, results.length]);

  // The input grows with its text, up to its CSS limit, then scrolls.
  useLayoutEffect(() => {
    const node = input.current;
    if (!node) return;
    node.style.height = "auto";
    node.style.height = `${node.scrollHeight}px`;
  }, [draft]);

  return (
    <section className={styles.chat} aria-labelledby="chat-title">
      <h2 className="sr-only" id="chat-title">
        {t.title}
      </h2>
      <div className={styles.logWrap}>
        <div className={styles.log} role="log" aria-label={t.title} ref={log} onScroll={onScroll} tabIndex={0}>
          <div className={styles.logInner}>
            {messages.isSuccess && list.length === 0 && !live && <p className={styles.chatEmpty}>{t.empty}</p>}
            {interleave(list, results).map((entry) =>
              "message" in entry ? (
                <MessageView
                  key={entry.message.id}
                  message={entry.message}
                  live={live?.id === entry.message.id ? live : null}
                  asking={questions.length > 0 && entry.message.id === lastReply?.id}
                />
              ) : (
                <ResultCard
                  key={`result-${entry.result.kind}`}
                  result={entry.result}
                  onOpen={() => onOpenResult(entry.result.kind)}
                  onApprove={() => onApproveResult(entry.result.kind)}
                />
              ),
            )}
            {showLiveAlone && (
              <article className={styles.reply}>
                <p className={styles.who}>{t.agent}</p>
                <ReplyBody text={live.text} parts={live.parts} streaming asking={false} />
                <span className={styles.caret} aria-hidden="true" />
              </article>
            )}
            {canRetry && (
              <div className={styles.retry}>
                <button type="button" className="btn btn-secondary" onClick={retry}>
                  {t.retry}
                </button>
                {modelFailed && <ModelPicker placement="down" align="start" variant="switch" />}
              </div>
            )}
            {questions.length > 0 && lastReply && <QuestionSet key={lastReply.id} questions={questions} disabled={!hasKey} onSend={(text) => send(text, false)} />}
            {proposal && <Proposal proposal={proposal} stage={project.stage} onAdvance={onAdvance} />}
          </div>
        </div>
        {!atEnd && (
          <button
            type="button"
            className={`btn btn-secondary ${styles.latest}`}
            onClick={() => {
              following.current = true;
              setAtEnd(true);
              toEnd();
            }}
          >
            <ArrowDown size={15} strokeWidth={1.8} aria-hidden="true" />
            {t.latest}
          </button>
        )}
      </div>

      <p className="sr-only" role="status">
        {said}
      </p>
      {problem && (
        <p className={styles.problem} role="alert">
          {problem}
        </p>
      )}

      <form
        className={styles.composer}
        onSubmit={(event) => {
          event.preventDefault();
          send(draft);
        }}
      >
        <label className="sr-only" htmlFor="studio-input">
          {t.inputLabel}
        </label>
        {/* U4: the same pill as Studio's home; a click on its padding puts the cursor in it. */}
        <div
          className={styles.pill}
          data-disabled={!hasKey || undefined}
          onClick={(event) => {
            if (event.target === event.currentTarget) input.current?.focus();
          }}
        >
          <textarea
            id="studio-input"
            ref={input}
            className={styles.input}
            rows={1}
            value={draft}
            maxLength={20000}
            disabled={!hasKey}
            aria-describedby="studio-input-note"
            onChange={(event) => setDraft(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Escape" && busy) {
                event.preventDefault();
                stop();
              } else if (event.key === "Enter" && !event.shiftKey && !event.nativeEvent.isComposing) {
                event.preventDefault();
                send(draft);
              }
            }}
          />
          {/* L2: the model, beside the send button as in Gemini's input. */}
          <div className={styles.pillModel}>
            <ModelPicker placement="up" align="end" />
          </div>
          {busy ? (
            <button type="button" className={styles.round} data-kind="stop" aria-label={t.stop} title={t.stop} onClick={stop}>
              <Square size={13} strokeWidth={0} fill="currentColor" aria-hidden="true" />
            </button>
          ) : (
            <button type="submit" className={styles.round} aria-label={t.send} title={t.send} disabled={!hasKey || !draft.trim()}>
              <ArrowUp size={18} strokeWidth={2} aria-hidden="true" />
            </button>
          )}
        </div>
        <div className={styles.composerRow}>
          <div className={styles.note} id="studio-input-note">
            {hasKey ? (
              <>
                <span>{t.sentToGoogle} </span>
                <details className={styles.disclosure}>
                  <summary>{t.whatIsSent}</summary>
                  <p>{t.whatIsSentBody}</p>
                </details>
              </>
            ) : (
              <span>
                {t.noKey} <Link to="/pengaturan/$bagian" params={{ bagian: "ai" }}>{id.studio.openSettings}</Link>
              </span>
            )}
          </div>
        </div>
      </form>
    </section>
  );
}

/**
 * A reply's body. L1: a reply that asks shows no text at all, only its questions: while it is
 * written, a quiet "preparing" line; once done, the question set under the conversation (so
 * nothing here), or, after it was answered, one line saying how much was asked.
 */
function ReplyBody({ text, parts, streaming, asking }: { text: string; parts: unknown; streaming: boolean; asking: boolean }) {
  const asked = questionsOf(parts).length;
  if (asked === 0) return text ? <Prose text={text} className={styles.prose} /> : null;
  if (streaming) return <p className={styles.preparing}>{t.preparing}</p>;
  return asking ? null : <p className={styles.asked}>{t.asked(asked)}</p>;
}

function MessageView({ message, live, asking }: { message: StudioMessage; live: Live | null; asking: boolean }) {
  if (message.role === "owner") {
    return (
      <article className={styles.mine} id={`message-${message.id}`}>
        <p className={styles.who}>{t.you}</p>
        <p className={styles.mineText}>{message.text}</p>
      </article>
    );
  }
  const streaming = live !== null || message.state === "streaming";
  const text = live ? live.text : message.text;
  const parts = live ? live.parts : message.parts;
  // The set the owner is answering now stands on its own, titled "Pertanyaan dari Lentera".
  if (asking && !streaming && message.state === "complete") return null;
  return (
    <article className={styles.reply} id={`message-${message.id}`} aria-busy={streaming}>
      <p className={styles.who}>{t.agent}</p>
      <ReplyBody text={text} parts={parts} streaming={streaming} asking={asking} />
      {streaming && <span className={styles.caret} aria-hidden="true" />}
      {message.state === "stopped" && <p className={styles.ended}>{t.stopped}</p>}
      {message.state === "failed" && <p className={styles.ended}>{t.failed[message.errorKind ?? ""] ?? t.failed.fallback}</p>}
    </article>
  );
}

interface Answer {
  picked: string[];
  own: string;
  unknown: boolean;
}

const EMPTY: Answer = { picked: [], own: "", unknown: false };
const PART_ORDER = [...AREAS, ""];

/** What the owner chose for a question, as words; empty when nothing was chosen. */
function said(answer: Answer): string {
  if (answer.unknown) return t.unknownAnswer;
  return [...answer.picked, answer.own.trim()].filter(Boolean).join(", ");
}

/** One message for the whole set: what was answered, then what was left for later. */
export function answersMessage(questions: Question[], answers: Record<number, Answer>): string {
  const lines = questions.map((q, i) => ({ q, a: said(answers[i] ?? EMPTY) }));
  const answered = lines.filter((l) => l.a);
  const skipped = lines.filter((l) => !l.a);
  const text = [t.answersHead, ...answered.map((l) => `- ${l.q.text} ${l.a}`)];
  if (skipped.length > 0) text.push("", t.skippedHead, ...skipped.map((l) => `- ${l.q.text}`));
  return text.join("\n");
}

/** Gemini's questions as one set, grouped by part of the idea, answerable in part. */
function QuestionSet({ questions, disabled, onSend }: { questions: Question[]; disabled: boolean; onSend: (text: string) => void }) {
  const [answers, setAnswers] = useState<Record<number, Answer>>({});
  const [writing, setWriting] = useState<Record<number, boolean>>({});
  const [error, setError] = useState(false);
  const base = useId();
  const answered = questions.filter((_, i) => said(answers[i] ?? EMPTY) !== "").length;
  const grouped = questions.some((q) => q.area);
  const groups = PART_ORDER.map((area) => ({ area, items: questions.map((q, i) => ({ q, i })).filter(({ q }) => q.area === area) })).filter((g) => g.items.length > 0);

  const change = (index: number, next: Partial<Answer>) => {
    setError(false);
    setAnswers((now) => ({ ...now, [index]: { ...(now[index] ?? EMPTY), ...next } }));
  };
  const toggle = (index: number, question: Question, label: string) => {
    const now = answers[index] ?? EMPTY;
    if (!question.multiple) {
      change(index, { picked: [label], unknown: false });
      return;
    }
    const picked = now.picked.includes(label) ? now.picked.filter((l) => l !== label) : [...now.picked, label];
    change(index, { picked, unknown: false });
  };

  const one = ({ q, i }: { q: Question; i: number }) => {
    const answer = answers[i] ?? EMPTY;
    const name = `${base}-q${i}`;
    const type = q.multiple ? "checkbox" : "radio";
    return (
      <fieldset key={i} className={styles.question}>
        <legend>{q.text}</legend>
        {(q.why || q.multiple) && <p className={styles.why}>{[q.why, q.multiple ? t.several : ""].filter(Boolean).join(" · ")}</p>}
        <div className={styles.choices}>
          {q.options.map((option) => (
            <label key={option.label} className={`${styles.choice} ${option.recommended ? styles.recommended : ""}`} title={option.detail || undefined}>
              <input type={type} name={name} value={option.label} checked={!answer.unknown && answer.picked.includes(option.label)} onChange={() => toggle(i, q, option.label)} />
              <span>
                {option.label}
                {option.recommended && <small> · {t.recommended}</small>}
              </span>
            </label>
          ))}
          <label className={styles.choice}>
            <input type={type} name={name} value={t.unknownAnswer} checked={answer.unknown} onChange={() => change(i, { unknown: !answer.unknown || !q.multiple, picked: [], own: "" })} />
            <span>{t.unknown}</span>
          </label>
          <button
            type="button"
            className={`${styles.choice} ${styles.ownToggle}`}
            aria-expanded={Boolean(writing[i])}
            aria-controls={writing[i] ? `${name}-own` : undefined}
            onClick={() => setWriting((now) => ({ ...now, [i]: !now[i] }))}
          >
            <span>{t.own}</span>
          </button>
        </div>
        {writing[i] && (
          <input
            id={`${name}-own`}
            className={styles.ownInput}
            aria-label={t.ownLabel(q.text)}
            value={answer.own}
            maxLength={500}
            onChange={(event) => change(i, { own: event.target.value, unknown: false, picked: q.multiple ? answer.picked : [] })}
          />
        )}
      </fieldset>
    );
  };

  return (
    <section className={styles.questions} aria-labelledby={`${base}-title`}>
      <div className={styles.setHead}>
        <p className={styles.who} id={`${base}-title`}>
          {t.questionsTitle}
        </p>
        <p className={styles.setProgress} aria-live="polite">
          {t.setProgress(answered, questions.length)}
        </p>
      </div>
      {groups.map((group) =>
        grouped ? (
          <div key={group.area || "other"} className={styles.setGroup}>
            <h3 className={styles.setPart}>{id.studio.ledger.areas[group.area || "other"]}</h3>
            {group.items.map(one)}
          </div>
        ) : (
          group.items.map(one)
        ),
      )}
      <div className={styles.setFoot}>
        <p className={styles.why}>{t.setHint}</p>
        <button
          type="button"
          className="btn btn-primary"
          disabled={disabled}
          onClick={() => {
            if (answered === 0) {
              setError(true);
              return;
            }
            onSend(answersMessage(questions, answers));
          }}
        >
          {t.sendAnswers}
        </button>
      </div>
      {error && (
        <p className={styles.problem} role="alert">
          {t.noAnswer}
        </p>
      )}
    </section>
  );
}

function Proposal({ proposal, stage, onAdvance }: { proposal: StageProposal; stage: StudioStage; onAdvance: (to: StudioStage) => void }) {
  const next = proposal.next as StudioStage;
  // Only out of stage 1 (R2: exploring and clarifying are one conversation, so the PRD follows
  // either), or an older proposal from exploring to clarifying, and only words Hub has for it.
  const forward = (next === "prd" && STEP_OF[stage] === 1) || (next === "clarify" && stage === "explore");
  if (!forward || !t.ready[next] || !t.moveOn[next]) return null;
  return (
    <div className={styles.proposal}>
      <p>{t.ready[next]}</p>
      <p className={styles.why}>{proposal.readyBecause}</p>
      {proposal.stillOpen.length > 0 && (
        <>
          <p className={styles.why}>{t.stillOpen}</p>
          <ul className={styles.open}>
            {proposal.stillOpen.map((item) => (
              <li key={item}>{item}</li>
            ))}
          </ul>
        </>
      )}
      <div>
        <button type="button" className="btn btn-primary" onClick={() => onAdvance(next)}>
          {t.moveOn[next]}
        </button>
      </div>
    </div>
  );
}
