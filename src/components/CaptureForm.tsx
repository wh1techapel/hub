// Catat Cepat: one line, read by the core as you type, saved with Enter. The chips under the line
// show exactly what will be saved (hub-core's capture.rs reads the line; the UI never guesses).
// Used by the Ctrl+Alt+H window, the command palette, and the add rows of the task lists.

import { useEffect, useId, useRef, useState } from "react";
import type { KeyboardEvent } from "react";

import { useApi } from "../data/api";
import { useSound } from "../sound/SoundProvider";
import { toHubError } from "../data/backend";
import type { CaptureKind } from "../data/generated/CaptureKind";
import type { Reading } from "../data/generated/Reading";
import type { Saved } from "../data/generated/Saved";
import { id } from "../strings/id";
import { useNow } from "../time/clock";
import { clock, parseClock, relativeDate } from "../time/format";
import styles from "./capture.module.css";

const ALL: CaptureKind[] = ["task", "reminder", "event", "note"];
const READ_DELAY_MS = 90;
/** "ingatkan aku ..." typed as a task means a reminder (the core reads that prefix for reminders). */
const REMINDER_START = /^\s*(ingatkan|remind me)\b/i;

interface Props {
  kinds?: readonly CaptureKind[];
  initialKind?: CaptureKind;
  /** A task made here belongs to this project (the project page's add row). */
  projectId?: string;
  /** ...and to this milestone of it (a milestone's own add row). */
  milestoneId?: string;
  /** The input's name for screen readers, when the kind alone ("Tugas") is not enough. */
  label?: string;
  /** The add rows are one quiet line; the capture window and palette are larger. */
  compact?: boolean;
  autoFocus?: boolean;
  /** Called after a save, with what was made. */
  onSaved?: (saved: Saved) => void;
  onCancel?: () => void;
  placeholder?: string;
}

function timeText(value: string | null): string | null {
  return value ? clock(parseClock(value) ?? 0) : null;
}

/** The chips for a reading, in the order they read best. */
export function readingChips(reading: Reading, today: Date): { text: string; tone?: "warn" | "project"; pigment?: string }[] {
  const chips: { text: string; tone?: "warn" | "project"; pigment?: string }[] = [];
  const day = reading.date ? relativeDate(reading.date, today) : null;
  const time = timeText(reading.time);
  if (reading.kind === "task") {
    if (day && reading.due) chips.push({ text: `${id.capture.chips.due} ${day}${time ? ` ${time}` : ""}` });
    else if (day) chips.push({ text: `${id.capture.chips.doDate} ${day}` });
    if (reading.priority > 0) chips.push({ text: "!".repeat(reading.priority) });
    if (reading.estimateMin) chips.push({ text: `${id.capture.chips.estimate} ${reading.estimateMin} m` });
  } else if (reading.kind === "event") {
    if (day) chips.push({ text: day });
    if (time) chips.push({ text: reading.endTime ? `${time} sampai ${timeText(reading.endTime) ?? ""}` : time });
    else if (day) chips.push({ text: id.capture.chips.allDay });
    if (reading.location) chips.push({ text: `${id.capture.chips.location} ${reading.location}` });
  } else if (reading.kind === "reminder") {
    if (day) chips.push({ text: day });
    if (time) chips.push({ text: `${id.capture.chips.at} ${time}` });
  } else if (day) {
    chips.push({ text: day });
  }
  if (reading.project) chips.push({ text: reading.project.name, tone: "project", pigment: reading.project.pigment });
  if (reading.unknownTag) chips.push({ text: id.capture.chips.unknownTag(reading.unknownTag), tone: "warn" });
  if (reading.kind === "task" && !reading.date && !reading.project && reading.title) chips.push({ text: id.capture.chips.inbox });
  return chips;
}

export function CaptureForm({
  kinds = ALL,
  initialKind,
  projectId,
  milestoneId,
  label,
  compact = false,
  autoFocus = false,
  onSaved,
  onCancel,
  placeholder,
}: Props) {
  const sound = useSound();
  const api = useApi();
  const now = useNow();
  const inputId = useId();
  const previewId = useId();
  const input = useRef<HTMLInputElement>(null);
  const [kind, setKind] = useState<CaptureKind>(initialKind ?? kinds[0] ?? "task");
  const [text, setText] = useState("");
  const [reading, setReading] = useState<Reading | null>(null);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (autoFocus) input.current?.focus();
  }, [autoFocus]);

  // Read the line in the core, a moment after typing stops.
  useEffect(() => {
    if (!text.trim()) return;
    let stale = false;
    const timer = setTimeout(() => {
      void api
        .readCapture(text, kind)
        .then((next) => {
          if (!stale) setReading(next);
        })
        .catch(() => undefined);
    }, READ_DELAY_MS);
    return () => {
      stale = true;
      clearTimeout(timer);
    };
  }, [api, text, kind]);

  const save = async () => {
    if (!text.trim() || saving) return;
    setSaving(true);
    setError(null);
    try {
      let saved: Saved;
      if (projectId && kind === "task") {
        // The project page: the core reads the line, the task joins this project.
        const r = await api.readCapture(text, "task");
        if (r.missing) throw new Error(r.missing);
        const taskId = await api.createTask({
          title: r.title,
          notes: "",
          priority: r.priority,
          doDate: r.due ? null : r.date,
          dueDate: r.due ? r.date : null,
          dueTime: r.due ? r.time : null,
          estimateMin: r.estimateMin,
          projectId,
          areaId: null,
          milestoneId: milestoneId ?? null,
          parentId: null,
        });
        saved = { kind: "task", id: taskId, title: r.title, date: r.date };
      } else {
        saved = await api.saveCapture(text, kind);
      }
      setText("");
      setReading(null);
      sound.play("saved", "capture");
      onSaved?.(saved);
    } catch (failure) {
      const view = toHubError(failure).view;
      const missing = (text.trim() ? reading?.missing : null) ?? (view.field === "time" ? "time" : view.field === "date" ? "date" : null);
      setError(missing ? id.capture.missing[missing] : id.common.failed);
    } finally {
      setSaving(false);
      input.current?.focus();
    }
  };

  const onKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "Enter") {
      event.preventDefault();
      void save();
    } else if (event.key === "Escape") {
      event.preventDefault();
      if (text) setText("");
      else onCancel?.();
    } else if (event.key === "Tab" && kinds.length > 1 && !event.shiftKey && !compact) {
      event.preventDefault();
      const next = kinds[(kinds.indexOf(kind) + 1) % kinds.length];
      if (next) setKind(next);
    }
  };

  // An emptied line shows nothing, whatever the last reading was.
  const shown = text.trim() ? reading : null;
  const missing = shown?.missing ? id.capture.missing[shown.missing] : null;
  const chips = shown ? readingChips(shown, now) : [];

  return (
    <div className={`${styles.capture} ${compact ? styles.compact : ""}`}>
      {kinds.length > 1 && (
        <div className={styles.kinds} role="radiogroup" aria-label={id.capture.title}>
          {kinds.map((k) => (
            <button
              key={k}
              type="button"
              role="radio"
              aria-checked={k === kind}
              className={styles.kind}
              onClick={() => {
                setKind(k);
                input.current?.focus();
              }}
            >
              {id.capture.kinds[k]}
            </button>
          ))}
        </div>
      )}
      <label className="sr-only" htmlFor={inputId}>
        {label ?? id.capture.kinds[kind]}
      </label>
      <input
        ref={input}
        id={inputId}
        className={styles.input}
        value={text}
        placeholder={placeholder ?? id.capture.placeholder[kind]}
        maxLength={500}
        autoComplete="off"
        spellCheck={false}
        aria-describedby={previewId}
        aria-invalid={error !== null}
        onChange={(event) => {
          const next = event.target.value;
          setText(next);
          setError(null);
          if (kind === "task" && kinds.includes("reminder") && REMINDER_START.test(next)) setKind("reminder");
        }}
        onKeyDown={onKeyDown}
      />
      <div id={previewId} className={styles.preview} aria-live="polite">
        {shown?.title && <span className={styles.titleChip}>{shown.title}</span>}
        {chips.map((chip) => (
          <span
            key={chip.text}
            className={chip.tone === "project" ? "chip" : `${styles.chip} ${chip.tone === "warn" ? styles.warn : ""}`}
            data-pigment={chip.pigment}
          >
            {chip.text}
          </span>
        ))}
        {(error ?? (text.trim() ? missing : null)) && <span className={styles.missing}>{error ?? missing}</span>}
        {!compact && !text && <span className={styles.hint}>{id.capture.hint}</span>}
      </div>
    </div>
  );
}
