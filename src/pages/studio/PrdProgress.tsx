// L3: the PRD being written, shown so the wait reads as progress. Lentera writes it in three parts,
// each with deep thinking, so a part can take minutes: the parts as steps (done, being written,
// waiting; a repair after them names its part), the time since it began, a word on why it takes
// a while, and a quiet outline of the document to come. Only the current step's words are a live
// region; the clock is not read out every second.

import { Check } from "lucide-react";

import type { PrdEvent } from "../../data/api";
import { id } from "../../strings/id";
import { clockOf, useElapsed } from "../../time/elapsed";
import styles from "./prd.module.css";

const t = id.studio.prd;

type StepState = "done" | "running" | "waiting";

/** Each part's state from the last step heard; before the first one, part 1 is being written. */
export function partStates(step: Extract<PrdEvent, { kind: "step" }> | null, total: number): StepState[] {
  return Array.from({ length: total }, (_, index) => {
    const part = index + 1;
    if (!step) return part === 1 ? "running" : "waiting";
    // A repair comes after every part was written once.
    if (step.repair) return part === step.part ? "running" : "done";
    return part < step.part ? "done" : part === step.part ? "running" : "waiting";
  });
}

const sentence = (text: string) => text.charAt(0).toUpperCase() + text.slice(1);

export function PrdProgress({ step, startedAt, onStop }: { step: PrdEvent | null; startedAt: string | null; onStop: () => void }) {
  const current = step?.kind === "step" ? step : null;
  const total = current?.total ?? t.parts.length;
  const states = partStates(current, total);
  const elapsed = useElapsed(startedAt);
  const now = current ? (current.repair ? t.repairing(current.part) : t.writing(current.part, current.total)) : t.writing(1, total);

  return (
    <section className={styles.pane} aria-labelledby="prd-title" aria-busy="true">
      <h2 className={styles.title} id="prd-title">
        {t.title}
      </h2>
      <div className={styles.writing}>
        <div className={styles.writingHead}>
          <p className={styles.writingNow} role="status">
            {now}
          </p>
          {startedAt && (
            <p className={styles.elapsed} aria-hidden="true">
              {t.elapsed(clockOf(elapsed))}
            </p>
          )}
        </div>
        <ol className={styles.steps} aria-label={t.stepsLabel}>
          {states.map((state, index) => {
            const repairing = state === "running" && current?.repair === true;
            return (
              <li key={index} className={styles.step} data-state={state}>
                <span className={styles.stepMark} aria-hidden="true">
                  {state === "done" && <Check size={13} strokeWidth={2.4} />}
                </span>
                <span className={styles.stepText}>
                  <span className={styles.stepName}>{sentence(t.parts[index] ?? t.part(index + 1))}</span>
                  <span className={styles.stepState}>{repairing ? t.stepState.repairing : t.stepState[state]}</span>
                </span>
              </li>
            );
          })}
        </ol>
        <p className={styles.patience}>{t.patience}</p>
      </div>
      <div className={styles.outline} aria-hidden="true">
        <span className={styles.outlineTitle} />
        <span />
        <span />
        <span className={styles.outlineShort} />
        <span className={styles.outlineTitle} />
        <span />
        <span className={styles.outlineShort} />
      </div>
      <div>
        <button type="button" className="btn btn-secondary" onClick={onStop}>
          {t.stop}
        </button>
      </div>
    </section>
  );
}
