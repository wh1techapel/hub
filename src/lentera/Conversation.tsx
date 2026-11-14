// The parts of Lentera's bar that talk (CP03): the conversation with its cards, the line, Dari
// Pesan's panel, and the privacy notice. LenteraWindow.tsx holds the state and calls the core;
// these only draw it and say what the owner did.
//
// A card is Lentera's proposal, never a done thing: it says what would be saved (the same chips as
// Catat Cepat), and only its Simpan writes. Ubah opens its fields in place; Lewati folds it into one
// line that can be brought back. Text from Gemini is rendered as text, never as HTML.

import { Bell, CalendarDays, Check, ListTodo, MessageSquareText, NotebookPen, Square } from "lucide-react";
import { useEffect, useId, useRef, useState } from "react";
import type { ClipboardEvent, KeyboardEvent } from "react";

import { useApi } from "../data/api";
import type { Cleaned } from "../data/generated/Cleaned";
import type { Proposal } from "../data/generated/Proposal";
import { id } from "../strings/id";
import { proposalChips, proposalKind, proposalTitle } from "./talk";
import type { Card, Conversation, Exchange } from "./talk";
import styles from "./lentera.module.css";

const t = id.lentera;

const KIND_ICON = { task: ListTodo, reminder: Bell, event: CalendarDays, note: NotebookPen } as const;

/** Why an answer failed, in the owner's words. */
export function failureText(reason: string | null): string {
  if (!reason) return id.studio.chat.failed.fallback ?? "";
  return t.failed[reason] ?? id.studio.chat.failed[reason] ?? id.studio.chat.failed.fallback ?? "";
}

function lines(text: string): number {
  return text.split("\n").filter((l) => l.trim()).length;
}

// ---------- the conversation ----------

export function ConversationLog({
  conversation,
  today,
  onStop,
  onRetry,
  onCapture,
  onSave,
  onSkip,
  onRestore,
  onSaveAll,
}: {
  conversation: Conversation;
  today: Date;
  onStop: () => void;
  onRetry: (exchange: Exchange) => void;
  onCapture: (exchange: Exchange) => void;
  onSave: (card: Card, proposal: Proposal) => void;
  onSkip: (card: Card) => void;
  onRestore: (card: Card) => void;
  onSaveAll: (cards: Card[]) => void;
}) {
  const end = useRef<HTMLLIElement>(null);
  const count = conversation.exchanges.length;
  const latest = conversation.exchanges[count - 1];
  const cardCount = latest?.cards.length ?? 0;
  // A new question, and new cards, come into view (the list scrolls inside the bar).
  useEffect(() => {
    const node = end.current;
    // jsdom (the tests) has no scrolling.
    if (node && typeof (node as Partial<HTMLElement>).scrollIntoView === "function") node.scrollIntoView({ block: "nearest" });
  }, [count, cardCount, latest?.state]);
  if (!count) return null;
  return (
    <ol className={styles.log} aria-label={t.replies}>
      {conversation.exchanges.map((exchange, index) => (
        <li key={exchange.key} ref={index === count - 1 ? end : undefined} className={styles.exchange}>
          <p className={styles.mine}>
            <span className="sr-only">{t.you}: </span>
            {exchange.mode === "message" ? (
              <>
                <MessageSquareText size={14} strokeWidth={1.8} aria-hidden="true" />
                {t.message(lines(exchange.sent ?? exchange.text))}
              </>
            ) : (
              exchange.text
            )}
          </p>
          <ExchangeBody exchange={exchange} today={today} onStop={onStop} onRetry={onRetry} onCapture={onCapture} onSave={onSave} onSkip={onSkip} onRestore={onRestore} onSaveAll={onSaveAll} />
        </li>
      ))}
    </ol>
  );
}

function ExchangeBody({
  exchange,
  today,
  onStop,
  onRetry,
  onCapture,
  onSave,
  onSkip,
  onRestore,
  onSaveAll,
}: {
  exchange: Exchange;
  today: Date;
  onStop: () => void;
  onRetry: (exchange: Exchange) => void;
  onCapture: (exchange: Exchange) => void;
  onSave: (card: Card, proposal: Proposal) => void;
  onSkip: (card: Card) => void;
  onRestore: (card: Card) => void;
  onSaveAll: (cards: Card[]) => void;
}) {
  // Cards open in their fields are left out of Simpan semua: their changes are not done yet.
  const [editing, setEditing] = useState<ReadonlySet<string>>(new Set());
  const edit = (key: string, open: boolean) =>
    setEditing((keys) => {
      const next = new Set(keys);
      if (open) next.add(key);
      else next.delete(key);
      return next;
    });
  const pending = exchange.cards.filter((c) => c.status === "pending" && !editing.has(c.key));
  return (
    <div className={styles.answer}>
      {exchange.reply.trim() && <p className={styles.reply}>{exchange.reply.trim()}</p>}
      {exchange.state === "thinking" && (
        <div className={styles.thinking}>
          {!exchange.reply && !exchange.cards.length && <span className={styles.thinkingText}>{t.thinking}</span>}
          <button type="button" className="btn btn-quiet" onClick={onStop}>
            <Square size={13} strokeWidth={1.8} aria-hidden="true" />
            {t.stop}
          </button>
        </div>
      )}
      {exchange.state === "stopped" && !exchange.cards.length && <p className={styles.quiet}>{t.stopped}</p>}
      {exchange.state === "done" && !exchange.reply.trim() && !exchange.cards.length && <p className={styles.quiet}>{t.nothingProposed}</p>}
      {exchange.state === "failed" && (
        <div className={styles.failure} role="alert">
          <p>{failureText(exchange.error)}</p>
          <div className={styles.cardActions}>
            {exchange.error !== "no_key" && (
              <button type="button" className="btn btn-secondary" onClick={() => onRetry(exchange)}>
                {t.retry}
              </button>
            )}
            {exchange.mode === "ask" && (
              <button type="button" className="btn btn-quiet" onClick={() => onCapture(exchange)}>
                {t.useCapture}
              </button>
            )}
          </div>
        </div>
      )}
      {exchange.cards.length > 0 && (
        <section className={styles.proposals} aria-label={t.proposalsLabel(exchange.cards.length)}>
          {pending.length > 1 && (
            <div className={styles.saveAll}>
              <button type="button" className="btn btn-secondary" onClick={() => onSaveAll(pending)}>
                <Check size={15} strokeWidth={1.8} aria-hidden="true" />
                {t.saveAll(pending.length)}
              </button>
            </div>
          )}
          {exchange.cards.map((c) => (
            <ProposalCard
              key={c.key}
              card={c}
              today={today}
              editing={editing.has(c.key)}
              onEditing={(open) => edit(c.key, open)}
              onSave={(proposal) => {
                edit(c.key, false);
                onSave(c, proposal);
              }}
              onSkip={() => onSkip(c)}
              onRestore={() => onRestore(c)}
            />
          ))}
        </section>
      )}
    </div>
  );
}

// ---------- a card ----------

function ProposalCard({
  card,
  today,
  editing,
  onEditing,
  onSave,
  onSkip,
  onRestore,
}: {
  card: Card;
  today: Date;
  editing: boolean;
  onEditing: (open: boolean) => void;
  onSave: (proposal: Proposal) => void;
  onSkip: () => void;
  onRestore: () => void;
}) {
  const setEditing = onEditing;
  const proposal = card.proposal;
  const Icon = KIND_ICON[proposal.kind];
  const kind = proposalKind(proposal);
  const title = proposalTitle(proposal);
  if (card.status === "saved" || card.status === "skipped") {
    const saved = card.status === "saved";
    return (
      <article className={styles.cardLine} data-card-status={card.status} aria-label={t.card(kind, title)}>
        {saved ? <Check size={15} strokeWidth={2} aria-hidden="true" className={styles.savedMark} /> : <Icon size={15} strokeWidth={1.8} aria-hidden="true" />}
        <span className={styles.cardLineText}>
          <strong>{saved ? t.savedLine : t.skippedLine}</strong> {title}
        </span>
        {!saved && (
          <button type="button" className="btn btn-quiet" onClick={onRestore}>
            {t.restore}
          </button>
        )}
      </article>
    );
  }
  const saving = card.status === "saving";
  return (
    <article className={styles.card} data-card-status={card.status} aria-label={t.card(kind, title)} aria-busy={saving || undefined}>
      <p className={styles.cardKind}>
        <Icon size={14} strokeWidth={1.8} aria-hidden="true" />
        {kind}
      </p>
      {editing ? (
        <CardEditor
          proposal={proposal}
          saving={saving}
          onSave={(edited) => onSave(edited)}
          onCancel={() => setEditing(false)}
        />
      ) : (
        <>
          <p className={styles.cardTitle}>{title}</p>
          <div className={styles.chips}>
            {proposalChips(proposal, today).map((chip) => (
              <span key={chip.text} className={chip.tone === "project" ? `${styles.chip} ${styles.chipProject}` : styles.chip}>
                {chip.text}
              </span>
            ))}
          </div>
          {proposal.kind === "task" && proposal.notes && <p className={styles.cardNotes}>{proposal.notes}</p>}
          {card.failed && (
            <p className={styles.cardError} role="alert">
              {t.saveFailed}
            </p>
          )}
          <div className={styles.cardActions}>
            <button type="button" className="btn btn-primary" data-card-primary disabled={saving} onClick={() => onSave(proposal)}>
              {saving ? t.saving : t.save}
            </button>
            <button type="button" className="btn btn-quiet" disabled={saving} onClick={() => setEditing(true)}>
              {t.edit}
            </button>
            <button type="button" className="btn btn-quiet" disabled={saving} onClick={onSkip}>
              {t.skip}
            </button>
          </div>
        </>
      )}
    </article>
  );
}

/** One labelled input of a card's editor. */
function Input({
  label,
  value,
  onChange,
  type = "text",
  required = false,
  autoFocus = false,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  type?: "text" | "date" | "time";
  required?: boolean;
  autoFocus?: boolean;
}) {
  const inputId = useId();
  const ref = useRef<HTMLInputElement>(null);
  useEffect(() => {
    if (autoFocus) ref.current?.focus();
  }, [autoFocus]);
  return (
    <div className={styles.editField} data-wide={type === "text" || undefined}>
      <label htmlFor={inputId}>{label}</label>
      <input
        ref={ref}
        id={inputId}
        className="input"
        type={type}
        value={value}
        required={required}
        maxLength={type === "text" ? 200 : undefined}
        onChange={(event) => onChange(event.target.value)}
      />
    </div>
  );
}

const orNull = (value: string) => (value.trim() ? value : null);

function CardEditor({ proposal, saving, onSave, onCancel }: { proposal: Proposal; saving: boolean; onSave: (proposal: Proposal) => void; onCancel: () => void }) {
  const [draft, setDraft] = useState<Proposal>(proposal);
  const f = t.fields;
  const fields = (() => {
    switch (draft.kind) {
      case "task":
        return (
          <>
            <Input label={f.title} value={draft.title} required autoFocus onChange={(title) => setDraft({ ...draft, title })} />
            <Input label={f.dueDate} type="date" value={draft.dueDate ?? ""} onChange={(v) => setDraft({ ...draft, dueDate: orNull(v), dueTime: v ? draft.dueTime : null })} />
            <Input label={f.dueTime} type="time" value={draft.dueTime ?? ""} onChange={(v) => setDraft({ ...draft, dueTime: orNull(v) })} />
            <Input label={f.doDate} type="date" value={draft.doDate ?? ""} onChange={(v) => setDraft({ ...draft, doDate: orNull(v) })} />
          </>
        );
      case "reminder":
        return (
          <>
            <Input label={f.text} value={draft.text} required autoFocus onChange={(text) => setDraft({ ...draft, text })} />
            <Input label={f.date} type="date" value={draft.date} required onChange={(date) => setDraft({ ...draft, date })} />
            <Input label={f.time} type="time" value={draft.time} required onChange={(time) => setDraft({ ...draft, time })} />
          </>
        );
      case "event":
        return (
          <>
            <Input label={f.title} value={draft.title} required autoFocus onChange={(title) => setDraft({ ...draft, title })} />
            <Input label={f.date} type="date" value={draft.date} required onChange={(date) => setDraft({ ...draft, date })} />
            <Input label={f.start} type="time" value={draft.start ?? ""} onChange={(v) => setDraft({ ...draft, start: orNull(v), end: v ? draft.end : null })} />
            <Input label={f.end} type="time" value={draft.end ?? ""} onChange={(v) => setDraft({ ...draft, end: orNull(v) })} />
            <Input label={f.location} value={draft.location} onChange={(location) => setDraft({ ...draft, location })} />
          </>
        );
      case "note":
        return (
          <>
            <Input label={f.text} value={draft.text} required autoFocus onChange={(text) => setDraft({ ...draft, text })} />
            <Input label={f.date} type="date" value={draft.date} required onChange={(date) => setDraft({ ...draft, date })} />
          </>
        );
    }
  })();
  const title = proposalTitle(draft).trim();
  return (
    <form
      className={styles.editor}
      onSubmit={(event) => {
        event.preventDefault();
        if (title) onSave(draft);
      }}
      onKeyDown={(event) => {
        // Esc leaves the fields, not the bar.
        if (event.key === "Escape") {
          event.preventDefault();
          onCancel();
        }
      }}
    >
      <div className={styles.editFields}>{fields}</div>
      <div className={styles.cardActions}>
        <button type="submit" className="btn btn-primary" data-card-primary disabled={saving || !title}>
          {saving ? t.saving : t.save}
        </button>
        <button type="button" className="btn btn-quiet" disabled={saving} onClick={onCancel}>
          {t.cancel}
        </button>
      </div>
    </form>
  );
}

// ---------- the line ----------

export function LenteraLine({
  busy,
  initialText,
  onSubmit,
  onPasteMessage,
  onTextChange,
}: {
  busy: boolean;
  initialText: string;
  onSubmit: (text: string) => void;
  onPasteMessage: (text: string) => void;
  onTextChange: (text: string) => void;
}) {
  const inputId = useId();
  const [text, setLine] = useState(initialText);
  const setText = (next: string) => {
    setLine(next);
    onTextChange(next);
  };
  const onKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "Enter") {
      event.preventDefault();
      if (!text.trim() || busy) return;
      onSubmit(text.trim());
      setText("");
    } else if (event.key === "Escape" && text) {
      // The line clears first; the next Esc closes the bar (LenteraWindow).
      event.preventDefault();
      setText("");
    }
  };
  // A pasted group message (more than one line) goes to Dari Pesan, cleaned before it is shown.
  const onPaste = (event: ClipboardEvent<HTMLInputElement>) => {
    const pasted = event.clipboardData.getData("text");
    if (!pasted.includes("\n") || lines(pasted) < 2) return;
    event.preventDefault();
    onPasteMessage(pasted);
  };
  return (
    <div className={styles.line}>
      <label className="sr-only" htmlFor={inputId}>
        {t.input}
      </label>
      <input
        id={inputId}
        className={styles.lineInput}
        value={text}
        placeholder={t.placeholder}
        maxLength={1000}
        autoComplete="off"
        spellCheck={false}
        onChange={(event) => setText(event.target.value)}
        onKeyDown={onKeyDown}
        onPaste={onPaste}
      />
    </div>
  );
}

// ---------- Dari Pesan ----------

function cleanedParts(cleaned: Cleaned): string[] {
  const p = t.cleanedParts;
  const parts: string[] = [];
  if (cleaned.senders) parts.push(p.senders(cleaned.senders));
  if (cleaned.phones) parts.push(p.phones(cleaned.phones));
  if (cleaned.emails) parts.push(p.emails(cleaned.emails));
  if (cleaned.mentions) parts.push(p.mentions(cleaned.mentions));
  if (cleaned.times) parts.push(p.times(cleaned.times));
  if (cleaned.dropped) parts.push(p.dropped(cleaned.dropped));
  return parts;
}

export function MessagePanel({
  pasted,
  busy,
  onSend,
  onCancel,
  onTextChange,
}: {
  /** Text pasted into the line, cleaned as the panel opens. */
  pasted: string;
  busy: boolean;
  onSend: (text: string) => void;
  onCancel: () => void;
  onTextChange: (text: string) => void;
}) {
  const api = useApi();
  const areaId = useId();
  const statusId = useId();
  const area = useRef<HTMLTextAreaElement>(null);
  const [text, setArea] = useState("");
  const [cleaned, setCleaned] = useState<Cleaned | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const setText = (next: string) => {
    setArea(next);
    onTextChange(next);
  };
  const clean = async (raw: string) => {
    const result = await api.cleanLenteraMessage(raw);
    setText(result.text);
    setCleaned(result);
    return result;
  };
  // Cleaned once, when the panel opens with what was pasted into the line (the panel is keyed by
  // it, so a new paste is a new panel).
  useEffect(() => {
    area.current?.focus();
    if (!pasted) return;
    let stale = false;
    api.cleanLenteraMessage(pasted).then(
      (result) => {
        if (stale) return;
        setArea(result.text);
        onTextChange(result.text);
        setCleaned(result);
      },
      () => undefined,
    );
    return () => {
      stale = true;
    };
  }, [api, pasted, onTextChange]);

  const onPaste = (event: ClipboardEvent<HTMLTextAreaElement>) => {
    const piece = event.clipboardData.getData("text");
    if (!piece) return;
    event.preventDefault();
    const field = event.currentTarget;
    const next = text.slice(0, field.selectionStart) + piece + text.slice(field.selectionEnd);
    setNotice(null);
    void clean(next).catch(() => undefined);
  };

  const send = async () => {
    if (busy || !text.trim()) {
      if (!text.trim()) setNotice(t.emptyMessage);
      return;
    }
    // What is sent is what the owner sees: cleaned again, and shown again if that changed it.
    const result = await clean(text).catch(() => null);
    if (!result) return;
    if (result.tooLong) return;
    if (result.text !== text.trim()) {
      setNotice(t.cleanedAgain);
      return;
    }
    if (!result.text.trim()) {
      setNotice(t.emptyMessage);
      return;
    }
    onSend(result.text);
  };

  const onKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key === "Enter" && (event.ctrlKey || event.metaKey)) {
      event.preventDefault();
      void send();
    } else if (event.key === "Escape") {
      event.preventDefault();
      onCancel();
    }
  };

  const tooLong = cleaned?.tooLong ?? false;
  return (
    <div className={styles.messagePanel}>
      <label className={styles.messageLabel} htmlFor={areaId}>
        {t.messageLabel}
      </label>
      <p className={styles.quiet}>{t.fromMessageHint}</p>
      <textarea
        ref={area}
        id={areaId}
        className={`input ${styles.messageArea}`}
        value={text}
        rows={6}
        placeholder={t.messagePlaceholder}
        spellCheck={false}
        aria-describedby={statusId}
        onChange={(event) => {
          setText(event.target.value);
          setNotice(null);
        }}
        onPaste={onPaste}
        onKeyDown={onKeyDown}
      />
      <div id={statusId} className={styles.cleanedStatus} aria-live="polite">
        {cleaned && !tooLong && (
          <>
            <p>{t.cleaned(cleanedParts(cleaned))}</p>
            <p>{t.checkBeforeSend}</p>
          </>
        )}
        {tooLong && <p className={styles.warn}>{t.tooLong}</p>}
        {notice && <p className={styles.warn}>{notice}</p>}
      </div>
      <div className={styles.cardActions}>
        <button type="button" className="btn btn-primary" disabled={busy || tooLong || !text.trim()} onClick={() => void send()}>
          {t.send}
        </button>
        <button type="button" className="btn btn-quiet" onClick={onCancel}>
          {t.cancel}
        </button>
      </div>
    </div>
  );
}

// ---------- the privacy notice ----------

export function PrivacyLine({ open, onToggle, controls }: { open: boolean; onToggle: () => void; controls: string }) {
  return (
    <p className={styles.privacy}>
      {t.sentToGoogle}{" "}
      <button type="button" className={styles.linkButton} aria-expanded={open} aria-controls={controls} onClick={onToggle}>
        {t.whatIsSent}
      </button>
    </p>
  );
}
