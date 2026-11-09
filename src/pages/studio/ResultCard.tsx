// U4b: one result in the conversation (results.ts): what it is, where it stands, and Buka (the
// document in the canvas) with Setujui when it waits for the owner's approval. Setujui starts the
// document's own approval, with its own confirmation; the card never approves by itself.

import { FileText, Hammer, Layers, ListChecks } from "lucide-react";
import type { LucideIcon } from "lucide-react";

import { id } from "../../strings/id";
import type { Result, ResultKind } from "./results";
import styles from "./project.module.css";

const t = id.studio.results;

const ICON: Record<ResultKind, LucideIcon> = { stack: Layers, prd: FileText, roadmap: ListChecks, build: Hammer };

export function ResultCard({ result, onOpen, onApprove }: { result: Result; onOpen: () => void; onApprove: () => void }) {
  const Icon = ICON[result.kind];
  return (
    <article className={styles.result} data-waiting={result.waiting || undefined} aria-label={t.label(result.title)}>
      <span className={styles.resultIcon} aria-hidden="true">
        <Icon size={18} strokeWidth={1.6} />
      </span>
      <div className={styles.resultText}>
        <p className={styles.resultTitle}>{result.title}</p>
        <p className={styles.resultStatus}>{result.status}</p>
        {result.detail && <p className={styles.resultDetail}>{result.detail}</p>}
      </div>
      <div className={styles.resultActions}>
        <button type="button" className="btn btn-secondary" onClick={onOpen}>
          {t.open}
        </button>
        {result.canApprove && (
          <button type="button" className="btn btn-primary" onClick={onApprove}>
            {t.approve}
          </button>
        )}
      </div>
    </article>
  );
}
