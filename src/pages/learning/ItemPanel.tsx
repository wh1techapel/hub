// One learning item: what it is, its status and progress, the write-up, the sessions spent on it
// (timed or logged afterwards), and its files. Saves as it is edited, like the task panel.

import { ExternalLink, Play, Trash2, X } from "lucide-react";
import { useEffect, useState } from "react";

import { Field, Segmented, timeValue } from "../../components/Fields";
import { FileRows } from "../../components/FileRows";
import { useAct } from "../../components/Toast";
import { useApi } from "../../data/api";
import type { LearningDetail } from "../../data/api";
import type { Difficulty } from "../../data/generated/Difficulty";
import type { Domain } from "../../data/generated/Domain";
import type { LearningCard } from "../../data/generated/LearningCard";
import type { LearningInput } from "../../data/generated/LearningInput";
import type { LearningKind } from "../../data/generated/LearningKind";
import type { LearningStatus } from "../../data/generated/LearningStatus";
import { useCtfEvents, useLearningItem, useTracks } from "../../data/queries";
import { id } from "../../strings/id";
import { useNow } from "../../time/clock";
import { isoDate, minutesOf, parseClock, span, whenLabel } from "../../time/format";
import styles from "./learning.module.css";

const STATUSES: LearningStatus[] = ["todo", "doing", "done", "dropped"];
const KINDS: LearningKind[] = ["room", "path", "machine", "challenge", "practical"];
const DOMAINS: Domain[] = ["network", "web", "linux", "windows", "crypto", "forensics", "osint", "reversing", "pwn", "other"];
const DIFFICULTIES: Difficulty[] = ["easy", "medium", "hard", "insane"];

function inputOf(card: LearningCard): LearningInput {
  return {
    trackId: card.trackId,
    title: card.title,
    kind: card.kind,
    domain: card.domain,
    difficulty: card.difficulty,
    status: card.status,
    url: card.url,
    points: card.points,
    progress: card.progress,
    notes: card.notes,
    ctfEventId: card.ctfEventId,
  };
}

export function ItemPanel({ itemId, onClose }: { itemId: string; onClose: () => void }) {
  const item = useLearningItem(itemId);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !(event.target as HTMLElement | null)?.closest("[role=dialog]")) onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  return (
    <aside className={styles.panel} aria-label={id.learning.detail}>
      <button type="button" className={`icon-btn ${styles.closePanel}`} aria-label={id.tasks.close} onClick={onClose}>
        <X size={18} aria-hidden="true" />
      </button>
      {item.data && <ItemEditor key={item.data.card.id} detail={item.data} onClose={onClose} />}
    </aside>
  );
}

function ItemEditor({ detail, onClose }: { detail: LearningDetail; onClose: () => void }) {
  const api = useApi();
  const act = useAct();
  const now = useNow();
  const tracks = useTracks();
  const ctf = useCtfEvents();
  const { card } = detail;
  const [value, setValue] = useState<LearningInput>(() => inputOf(card));
  const [typing, setTyping] = useState(false);
  const [seen, setSeen] = useState(card);
  const [log, setLog] = useState({ date: isoDate(now), start: timeValue(Math.max(0, minutesOf(now) - 60)), minutes: "45" });
  if (card !== seen) {
    setSeen(card);
    if (!typing) setValue(inputOf(card));
  }

  const track = tracks.data?.find((t) => t.id === value.trackId);
  const save = (patch: Partial<LearningInput>) => {
    const next = { ...value, ...patch };
    setValue(next);
    setTyping(false);
    if (!next.title.trim()) return;
    void act(() => api.updateLearning(card.id, next));
  };

  const addSession = async () => {
    const minutes = Number(log.minutes);
    const start = parseClock(log.start);
    if (!Number.isFinite(minutes) || minutes <= 0 || start === null) return;
    await act(() => api.addSession({ itemId: card.id, date: log.date, startMinute: start, minutes, note: "" }), { text: id.common.saved });
  };

  const remove = async () => {
    const result = await act(() => api.deleteLearning(card.id), {
      text: id.common.deleted(card.title),
      action: { label: id.common.undo, run: () => void act(() => api.restoreLearning(card.id)) },
    });
    if (result.ok) onClose();
  };

  return (
    <div className={styles.editor}>
      <label className="sr-only" htmlFor="item-title">
        {id.learning.itemTitle}
      </label>
      <textarea
        id="item-title"
        className={styles.titleInput}
        rows={2}
        value={value.title}
        maxLength={200}
        onFocus={() => setTyping(true)}
        onChange={(e) => setValue({ ...value, title: e.target.value.replace(/\n/g, " ") })}
        onBlur={() => save({})}
        onKeyDown={(e) => {
          if (e.key === "Enter") {
            e.preventDefault();
            e.currentTarget.blur();
          }
        }}
      />
      <div className={styles.buttons}>
        {value.status !== "done" && (
          <button type="button" className="btn btn-primary" data-sound="sessionStart" onClick={() => void act(() => api.startTimer(card.id))}>
            <Play size={15} aria-hidden="true" />
            {id.learning.resume}
          </button>
        )}
        {card.url && (
          <button type="button" className="btn btn-secondary" onClick={() => void act(() => api.openLink("learning_item", card.id))}>
            <ExternalLink size={15} aria-hidden="true" />
            {id.learning.url}
          </button>
        )}
      </div>

      <Field label={id.learning.status}>
        <Segmented
          label={id.learning.status}
          options={STATUSES.map((status) => [status, id.learning.statuses[status]] as const)}
          value={value.status ?? "todo"}
          onChange={(status) => save({ status })}
        />
      </Field>

      <div className="row">
        <Field label={id.learning.kind} htmlFor="item-kind">
          <select
            id="item-kind"
            className="input"
            value={value.kind ?? "room"}
            onChange={(e) => save({ kind: e.target.value as LearningKind })}
          >
            {KINDS.map((kind) => (
              <option key={kind} value={kind}>
                {id.learning.kinds[kind]}
              </option>
            ))}
          </select>
        </Field>
        <Field label={id.learning.domain} htmlFor="item-domain">
          <select
            id="item-domain"
            className="input"
            value={value.domain ?? "other"}
            onChange={(e) => save({ domain: e.target.value as Domain })}
          >
            {DOMAINS.map((domain) => (
              <option key={domain} value={domain}>
                {id.learning.domains[domain]}
              </option>
            ))}
          </select>
        </Field>
      </div>
      <div className="row">
        <Field label={id.learning.difficulty} htmlFor="item-difficulty">
          <select
            id="item-difficulty"
            className="input"
            value={value.difficulty ?? ""}
            onChange={(e) => save({ difficulty: (e.target.value || null) as Difficulty | null })}
          >
            <option value="">{id.learning.noDifficulty}</option>
            {DIFFICULTIES.map((difficulty) => (
              <option key={difficulty} value={difficulty}>
                {id.learning.difficulties[difficulty]}
              </option>
            ))}
          </select>
        </Field>
        <Field label={`${id.learning.progress} (%)`} htmlFor="item-progress">
          <input
            id="item-progress"
            className="input"
            type="number"
            min={0}
            max={100}
            step={5}
            value={value.progress ?? ""}
            onFocus={() => setTyping(true)}
            onChange={(e) =>
              setValue({ ...value, progress: e.target.value === "" ? null : Math.min(100, Math.max(0, Number(e.target.value))) })
            }
            onBlur={() => save({})}
          />
        </Field>
      </div>

      {track?.kind === "ctf" && (
        <div className="row">
          <Field label={id.learning.ctfEvent} htmlFor="item-ctf">
            <select
              id="item-ctf"
              className="input"
              value={value.ctfEventId ?? ""}
              onChange={(e) => save({ ctfEventId: e.target.value || null })}
            >
              <option value="">{id.learning.noCtf}</option>
              {(ctf.data ?? []).map((event) => (
                <option key={event.id} value={event.id}>
                  {event.name}
                </option>
              ))}
            </select>
          </Field>
          <Field label={id.learning.points} htmlFor="item-points">
            <input
              id="item-points"
              className="input"
              type="number"
              min={0}
              value={value.points ?? ""}
              onFocus={() => setTyping(true)}
              onChange={(e) => setValue({ ...value, points: e.target.value === "" ? null : Math.max(0, Number(e.target.value)) })}
              onBlur={() => save({})}
            />
          </Field>
        </div>
      )}

      <Field label={id.learning.url} htmlFor="item-url" hint={id.learning.urlHint}>
        <input
          id="item-url"
          className="input"
          type="url"
          value={value.url ?? ""}
          maxLength={2000}
          onFocus={() => setTyping(true)}
          onChange={(e) => setValue({ ...value, url: e.target.value || null })}
          onBlur={() => save({})}
        />
      </Field>

      <Field label={id.learning.notes} htmlFor="item-notes">
        <textarea
          id="item-notes"
          className="input"
          rows={5}
          value={value.notes}
          maxLength={20000}
          onFocus={() => setTyping(true)}
          onChange={(e) => setValue({ ...value, notes: e.target.value })}
          onBlur={() => save({})}
        />
      </Field>

      <section aria-labelledby="sessions-title" className={styles.block}>
        <h3 className="section-label" id="sessions-title">
          {id.learning.sessions} {card.minutes > 0 && <em>· {span(card.minutes)}</em>}
        </h3>
        {detail.sessions.length === 0 ? (
          <p className={styles.quiet}>{id.learning.noSessions}</p>
        ) : (
          <ul className={styles.sessions}>
            {detail.sessions.slice(0, 12).map((session) => (
              <li key={session.id}>
                <span>{whenLabel(session.startedAt, now)}</span>
                <strong>{span(session.durationMin)}</strong>
                <button
                  type="button"
                  className="icon-btn"
                  aria-label={`${id.learning.deleteSession} ${whenLabel(session.startedAt, now)}`}
                  onClick={() => void act(() => api.deleteSession(session.id))}
                >
                  <X size={14} aria-hidden="true" />
                </button>
              </li>
            ))}
          </ul>
        )}
        <fieldset className={styles.logForm}>
          <legend className="sr-only">{id.learning.logSession}</legend>
          <input
            className="input"
            type="date"
            aria-label={id.learning.sessionDate}
            value={log.date}
            onChange={(e) => setLog({ ...log, date: e.target.value })}
          />
          <input
            className="input"
            type="time"
            aria-label={id.learning.sessionStart}
            value={log.start}
            onChange={(e) => setLog({ ...log, start: e.target.value })}
          />
          <input
            className="input"
            type="number"
            min={1}
            max={960}
            aria-label={id.learning.sessionMinutes}
            value={log.minutes}
            onChange={(e) => setLog({ ...log, minutes: e.target.value })}
          />
          <button type="button" className="btn btn-secondary" onClick={() => void addSession()}>
            {id.learning.logSession}
          </button>
        </fieldset>
      </section>

      <section aria-labelledby="item-files" className={styles.block}>
        <h3 className="section-label" id="item-files">
          {id.files.section}
        </h3>
        <FileRows files={detail.files} target={{ kind: "learning_item", id: card.id }} label={card.title} />
      </section>

      <button type="button" className="btn btn-danger" onClick={() => void remove()}>
        <Trash2 size={16} aria-hidden="true" />
        {id.learning.deleteItem}
      </button>
    </div>
  );
}
