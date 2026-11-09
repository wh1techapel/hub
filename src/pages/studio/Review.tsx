// Check answers before leaving stage 1 (R2; GOV.UK's "check answers" pattern): each part of the
// idea with what settles it and how clear it is, an "Ubah" per part that opens Ringkasan there, and
// one button that names what happens next. Parts not clear yet are said plainly; the owner may
// still go on.

import { Dialog } from "../../components/Dialog";
import type { LedgerArea } from "../../data/generated/LedgerArea";
import { useStudioLedger } from "../../data/queries";
import { id } from "../../strings/id";
import { summarize } from "./areas";
import { StateMark } from "./Ledger";
import styles from "./project.module.css";

const t = id.studio.review;
const l = id.studio.ledger;

export function Review({
  projectId,
  onClose,
  onChange,
  onGo,
}: {
  projectId: string;
  onClose: () => void;
  onChange: (area: LedgerArea) => void;
  onGo: () => void;
}) {
  const ledger = useStudioLedger(projectId);
  const parts = summarize(ledger.data ?? []);
  const unclear = parts.filter((part) => part.state !== "clear").map((part) => l.areas[part.area] ?? part.area);

  return (
    <Dialog
      title={t.title}
      description={t.intro}
      width="wide"
      onClose={onClose}
      footer={
        <>
          <button type="button" className="btn btn-quiet" onClick={onClose}>
            {t.back}
          </button>
          <button type="button" className="btn btn-primary" onClick={onGo} data-autofocus>
            {t.go}
          </button>
        </>
      }
    >
      <dl className={styles.review}>
        {parts.map((part) => {
          const name = l.areas[part.area] ?? part.area;
          return (
            <div key={part.area} className={styles.reviewRow}>
              <dt>
                <span className={styles.reviewName}>{name}</span>
                <StateMark state={part.state} />
              </dt>
              <dd>
                {part.items.length === 0 ? (
                  <p className={styles.why}>{t.nothing}</p>
                ) : (
                  <ul className={styles.reviewItems}>
                    {part.items.map((item) => (
                      <li key={item.id}>
                        {item.statement}
                        {(item.kind === "assumption" || item.kind === "question") && item.status === "active" && (
                          <span className={styles.kindTag}>{l.kinds[item.kind]}</span>
                        )}
                      </li>
                    ))}
                  </ul>
                )}
              </dd>
              <dd className={styles.reviewAction}>
                <button type="button" className="btn btn-quiet" onClick={() => onChange(part.area)} aria-label={t.changeLabel(name)}>
                  {t.change}
                </button>
              </dd>
            </div>
          );
        })}
      </dl>
      <p className={styles.reviewNote}>{unclear.length > 0 ? t.notClear(unclear.join(", ")) : t.allClear}</p>
    </Dialog>
  );
}
