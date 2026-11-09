// Tutup Hari: what got done, where each unfinished task goes, one sentence for the day, and up to
// three tasks for tomorrow. Two minutes, then the arc fills with ink and the moon rises.

import { useSound } from "../../sound/SoundProvider";
import { Check, Sparkles } from "lucide-react";
import { useState } from "react";

import { Dialog } from "../../components/Dialog";
import { Field } from "../../components/Fields";
import { useAct } from "../../components/Toast";
import { useApi } from "../../data/api";
import type { DayReview } from "../../data/generated/DayReview";
import type { Destination } from "../../data/generated/Destination";
import type { TaskCard } from "../../data/generated/TaskCard";
import { useReview } from "../../data/queries";
import { id } from "../../strings/id";
import { dateLabel, relativeDate } from "../../time/format";
import styles from "./review.module.css";

const FOCUS_MAX = 3;

type Choice = "tomorrow" | "date" | "let_go" | "keep";

export function ReviewSheet({ date, now, onClose }: { date: string; now: Date; onClose: () => void }) {
  const review = useReview(date, true);
  if (!review.data) return null;
  return <Sheet key={review.data.date} review={review.data} now={now} onClose={onClose} />;
}

function Sheet({ review, now, onClose }: { review: DayReview; now: Date; onClose: () => void }) {
  const api = useApi();
  const act = useAct();
  const [choices, setChoices] = useState<Record<string, { choice: Choice; date: string }>>(() =>
    Object.fromEntries(review.slipped.map((task) => [task.id, { choice: "tomorrow", date: review.tomorrow }])),
  );
  const [sentence, setSentence] = useState("");
  const initialFocus = (review.tomorrowFocus.length > 0 ? review.tomorrowFocus : review.suggestions).slice(0, FOCUS_MAX).map((t) => t.id);
  const [focus, setFocus] = useState<string[]>(initialFocus);
  const [full, setFull] = useState(false);
  const [saving, setSaving] = useState(false);

  // Tomorrow's candidates: what Hub suggests, what was already chosen, and what is moved to
  // tomorrow now. A task let go or kept today is not one.
  const candidates: TaskCard[] = [];
  for (const task of [...review.tomorrowFocus, ...review.suggestions, ...review.slipped]) {
    if (candidates.some((c) => c.id === task.id)) continue;
    const slip = choices[task.id];
    if (slip && slip.choice !== "tomorrow" && !(slip.choice === "date" && slip.date === review.tomorrow)) continue;
    candidates.push(task);
  }
  const chosen = focus.filter((taskId) => candidates.some((c) => c.id === taskId));

  const toggleFocus = (taskId: string) => {
    if (chosen.includes(taskId)) {
      setFocus(chosen.filter((x) => x !== taskId));
      setFull(false);
    } else if (chosen.length >= FOCUS_MAX) {
      setFull(true);
    } else {
      setFocus([...chosen, taskId]);
    }
  };

  const destination = (taskId: string): Destination => {
    const slip = choices[taskId] ?? { choice: "tomorrow", date: review.tomorrow };
    switch (slip.choice) {
      case "date":
        return { to: "date", date: slip.date };
      case "let_go":
        return { to: "let_go" };
      case "keep":
        return { to: "keep" };
      default:
        return { to: "tomorrow" };
    }
  };

  const sound = useSound();
  const close = async () => {
    setSaving(true);
    const result = await act(
      () =>
        api.closeDay({
          date: review.date,
          slips: review.slipped.map((task) => ({ taskId: task.id, destination: destination(task.id) })),
          reflection: sentence.trim() || null,
          tomorrowFocus: chosen,
        }),
      { text: id.review.closedToast },
    );
    setSaving(false);
    if (result.ok) {
      sound.play("hero");
      onClose();
    }
  };

  const dayName = dateLabel(new Date(`${review.date}T00:00`));

  if (review.closed) {
    return (
      <Dialog
        title={id.review.title}
        description={dayName}
        onClose={onClose}
        footer={
          <button type="button" className="btn btn-primary" onClick={onClose}>
            {id.common.close}
          </button>
        }
      >
        <p className={styles.note}>{id.review.alreadyClosed}</p>
      </Dialog>
    );
  }

  return (
    <Dialog
      title={id.review.title}
      description={dayName}
      width="wide"
      onClose={onClose}
      footer={
        <>
          <button type="button" className="btn btn-quiet" onClick={onClose}>
            {id.review.later}
          </button>
          <button type="button" className="btn btn-primary" disabled={saving} onClick={() => void close()}>
            {id.review.close}
          </button>
        </>
      }
    >
      <section aria-labelledby="review-done" className={styles.section}>
        <h3 className="section-label" id="review-done">
          {id.review.done}
        </h3>
        {review.done.length === 0 ? (
          <p className={styles.note}>{id.review.doneEmpty}</p>
        ) : (
          <ul className={styles.done}>
            {review.done.map((item, index) => (
              <li key={`${item.kind}-${index}`}>
                <Check size={14} aria-hidden="true" />
                {item.title}
              </li>
            ))}
          </ul>
        )}
      </section>

      <section aria-labelledby="review-slipped" className={styles.section}>
        <h3 className="section-label" id="review-slipped">
          {id.review.slipped}
        </h3>
        {review.slipped.length === 0 ? (
          <p className={styles.note}>{id.review.slippedEmpty}</p>
        ) : (
          <ul className={styles.slips}>
            {review.slipped.map((task) => {
              const slip = choices[task.id] ?? { choice: "tomorrow", date: review.tomorrow };
              const set = (patch: Partial<typeof slip>) => setChoices((all) => ({ ...all, [task.id]: { ...slip, ...patch } }));
              return (
                <li key={task.id}>
                  <span className={styles.slipTitle}>{task.title}</span>
                  <div className={styles.slipChoice} role="radiogroup" aria-label={task.title}>
                    {(["tomorrow", "date", "let_go", "keep"] as const).map((choice) => (
                      <label key={choice} className={styles.option}>
                        <input type="radio" name={`slip-${task.id}`} checked={slip.choice === choice} onChange={() => set({ choice })} />
                        <span>{id.review.choices[choice]}</span>
                      </label>
                    ))}
                    {slip.choice === "date" && (
                      <input
                        type="date"
                        className="input"
                        aria-label={`${id.review.pickDate}: ${task.title}`}
                        min={review.tomorrow}
                        value={slip.date}
                        onChange={(e) => e.target.value && set({ date: e.target.value })}
                      />
                    )}
                  </div>
                </li>
              );
            })}
          </ul>
        )}
      </section>

      <Field label={id.review.sentence} htmlFor="review-sentence" hint={id.review.sentenceHint}>
        <textarea
          id="review-sentence"
          className="input"
          rows={2}
          maxLength={500}
          value={sentence}
          onChange={(e) => setSentence(e.target.value)}
        />
      </Field>

      <section aria-labelledby="review-focus" className={styles.section}>
        <div className={styles.focusHead}>
          <h3 className="section-label" id="review-focus">
            {id.review.tomorrowFocus}
          </h3>
          <span>{id.review.tomorrowCount(chosen.length)}</span>
        </div>
        {review.tomorrowFocus.length === 0 && review.suggestions.length > 0 && (
          <p className={styles.note}>
            <Sparkles size={14} aria-hidden="true" /> {id.review.suggested}
          </p>
        )}
        {candidates.length === 0 ? (
          <p className={styles.note}>{id.review.noCandidates}</p>
        ) : (
          <ul className={styles.candidates}>
            {candidates.map((task) => (
              <li key={task.id}>
                <label className={styles.candidate}>
                  <input type="checkbox" checked={chosen.includes(task.id)} onChange={() => toggleFocus(task.id)} />
                  <span>{task.title}</span>
                  {task.dueDate && <small>{id.task.dueOn(relativeDate(task.dueDate, now))}</small>}
                </label>
              </li>
            ))}
          </ul>
        )}
        {full && (
          <p className={styles.warn} role="status">
            {id.review.focusFull}
          </p>
        )}
      </section>
    </Dialog>
  );
}
