// Ringkasan: what the conversation settled. It opens with "Yang sudah jelas", the five parts of an
// idea and how clear each one is (R2), then the items grouped by those parts. The owner corrects it
// here, not by rewriting old messages: edit an item, confirm an assumption, mark a question
// answered, or say an item no longer holds. Gemini reads the list every turn and does not ask again.

import { useQueryClient } from "@tanstack/react-query";
import { Check, CircleDashed, CircleDot } from "lucide-react";
import { useState } from "react";

import { useApi } from "../../data/api";
import type { HubError } from "../../data/backend";
import type { LedgerArea } from "../../data/generated/LedgerArea";
import type { LedgerItem } from "../../data/generated/LedgerItem";
import type { LedgerKind } from "../../data/generated/LedgerKind";
import { keys, useStudioAct, useStudioLedger } from "../../data/queries";
import { id } from "../../strings/id";
import { AREAS, areaOf, summarize } from "./areas";
import type { AreaState } from "./areas";
import styles from "./project.module.css";

const t = id.studio.ledger;
const KINDS: LedgerKind[] = ["decision", "assumption", "question", "constraint"];

/** The id of a part's section, for "Ubah" in the check-answers sheet and the links above it. */
export const areaAnchor = (area: LedgerArea | "other") => `ledger-area-${area}`;

/** A state as a glyph and a word, never a colour alone. */
export function StateMark({ state }: { state: AreaState }) {
  const Icon = state === "clear" ? Check : state === "partial" ? CircleDot : CircleDashed;
  return (
    <span className={styles.stateMark} data-state={state}>
      <Icon size={15} strokeWidth={2} aria-hidden="true" />
      <span>{t.states[state]}</span>
    </span>
  );
}

function problem(error: HubError | null): string | null {
  if (!error) return null;
  return t.errors[error.view.reason ?? ""] ?? id.settings.errors.fallback as string;
}

export function Ledger({
  projectId,
  onShowMessage,
  onReview,
}: {
  projectId: string;
  onShowMessage: (messageId: string) => void;
  /** Stage 1 only: the check-answers sheet before moving on. */
  onReview?: () => void;
}) {
  const ledger = useStudioLedger(projectId);
  const items = ledger.data ?? [];
  const live = items.filter((item) => item.status !== "retired");
  const retired = items.filter((item) => item.status === "retired");
  const parts = summarize(items);
  const unplaced = live.filter((item) => areaOf(item) === null);
  const jump = (area: LedgerArea) => document.getElementById(areaAnchor(area))?.scrollIntoView({ block: "start", behavior: "smooth" });

  return (
    <section className={styles.ledger} aria-labelledby="ledger-title">
      <h2 className={styles.paneTitle} id="ledger-title">
        {t.title}
      </h2>
      <p className={styles.paneHint}>{t.hint}</p>

      <section className={styles.clear} aria-labelledby="ledger-clear">
        <h3 className={styles.ledgerHead} id="ledger-clear">
          {t.clearTitle}
        </h3>
        <ul className={styles.clearList}>
          {parts.map((part) => (
            <li key={part.area} className={styles.clearRow} data-state={part.state}>
              <span className={styles.clearText}>
                {part.items.length > 0 ? (
                  <button type="button" className={styles.clearName} onClick={() => jump(part.area)} aria-label={t.jump(t.areas[part.area] ?? part.area)}>
                    {t.areas[part.area]}
                  </button>
                ) : (
                  <span className={styles.clearName}>{t.areas[part.area]}</span>
                )}
                <span className={styles.clearCount}>{t.counts(part.decided, part.assumed, part.open)}</span>
              </span>
              <StateMark state={part.state} />
            </li>
          ))}
        </ul>
        <p className={styles.paneHint}>{t.clearHint}</p>
        {onReview && (
          <div>
            <button type="button" className="btn btn-secondary" onClick={onReview}>
              {t.review}
            </button>
          </div>
        )}
      </section>

      {ledger.isSuccess && items.length === 0 && <p className={styles.chatEmpty}>{t.empty}</p>}
      {[...AREAS, "other" as const].map((area) => {
        const group = area === "other" ? unplaced : (parts.find((p) => p.area === area)?.items ?? []);
        if (group.length === 0) return null;
        return (
          <section key={area} className={styles.ledgerGroup} aria-labelledby={`${areaAnchor(area)}-title`} id={areaAnchor(area)} tabIndex={-1}>
            <h3 className={styles.ledgerHead} id={`${areaAnchor(area)}-title`}>
              {t.areas[area]}
            </h3>
            <ul className={styles.ledgerList}>
              {group.map((item) => (
                <Item key={item.id} item={item} onShowMessage={onShowMessage} />
              ))}
            </ul>
          </section>
        );
      })}
      {retired.length > 0 && (
        <details className={styles.retired}>
          <summary>{t.retired(retired.length)}</summary>
          <ul className={styles.ledgerList}>
            {retired.map((item) => (
              <Item key={item.id} item={item} onShowMessage={onShowMessage} />
            ))}
          </ul>
        </details>
      )}
      <AddItem projectId={projectId} />
    </section>
  );
}

function Item({ item, onShowMessage }: { item: LedgerItem; onShowMessage: (messageId: string) => void }) {
  const api = useApi();
  const client = useQueryClient();
  const edit = useStudioAct((change: Parameters<typeof api.editStudioLedger>[1]) => api.editStudioLedger(item.id, change));
  const [editing, setEditing] = useState(false);
  const [statement, setStatement] = useState(item.statement);
  const [area, setArea] = useState<LedgerArea | "">(item.area ?? "");
  const apply = (change: Parameters<typeof api.editStudioLedger>[1]) =>
    edit.mutate(change, {
      onSuccess: () => {
        setEditing(false);
        void client.invalidateQueries({ queryKey: keys.studio });
      },
    });
  const settled = item.status !== "active";

  return (
    <li className={`${styles.ledgerItem} ${item.kind === "assumption" && !settled ? styles.assumption : ""}`}>
      <p className={styles.ledgerTopic}>
        <strong>
          {item.topic}
          {/* Grouped by part, an item still says when it is not settled yet. */}
          {(item.kind === "assumption" || item.kind === "question") && <span className={styles.kindTag}>{t.kinds[item.kind]}</span>}
        </strong>
        <span className={styles.ledgerMeta}>
          {t.by[item.statedBy]}
          {settled && ` · ${t.status[item.status]}`}
        </span>
      </p>
      {editing ? (
        <form
          className={styles.ledgerEdit}
          onSubmit={(event) => {
            event.preventDefault();
            apply(area ? { statement, area } : { statement });
          }}
        >
          <label className="sr-only" htmlFor={`ledger-edit-${item.id}`}>
            {t.statementLabel}
          </label>
          <textarea id={`ledger-edit-${item.id}`} className={styles.input} rows={2} value={statement} onChange={(event) => setStatement(event.target.value)} />
          <label className={styles.fieldLabel} htmlFor={`ledger-area-${item.id}`}>
            {t.areaLabel}
          </label>
          <AreaSelect id={`ledger-area-${item.id}`} value={area} onChange={setArea} />
          <p className={styles.problem}>{problem(edit.error) ?? ""}</p>
          <div className={styles.rowActions}>
            <button type="submit" className="btn btn-primary">
              {t.save}
            </button>
            <button type="button" className="btn btn-quiet" onClick={() => setEditing(false)}>
              {t.cancel}
            </button>
          </div>
        </form>
      ) : (
        <p className={styles.ledgerStatement}>{item.statement}</p>
      )}
      {!editing && (
        <div className={styles.rowActions}>
          {item.messageId && (
            <button type="button" className="btn btn-quiet" onClick={() => item.messageId && onShowMessage(item.messageId)}>
              {t.fromMessage}
            </button>
          )}
          {!settled && (
            <>
              <button type="button" className="btn btn-quiet" onClick={() => setEditing(true)}>
                {t.edit}
              </button>
              {item.kind === "assumption" && (
                <button type="button" className="btn btn-quiet" onClick={() => apply({ status: "confirmed" })}>
                  {t.confirm}
                </button>
              )}
              {item.kind === "question" && (
                <button type="button" className="btn btn-quiet" onClick={() => setEditing(true)}>
                  {t.answer}
                </button>
              )}
              <button type="button" className="btn btn-quiet" onClick={() => apply({ status: "retired" })}>
                {t.retire}
              </button>
            </>
          )}
        </div>
      )}
      {editing && item.kind === "question" && (
        <button type="button" className="btn btn-quiet" onClick={() => apply({ statement, status: "answered" })}>
          {t.answer}
        </button>
      )}
    </li>
  );
}

function AddItem({ projectId }: { projectId: string }) {
  const api = useApi();
  const client = useQueryClient();
  const add = useStudioAct((input: Parameters<typeof api.addStudioLedger>[1]) => api.addStudioLedger(projectId, input));
  const [open, setOpen] = useState(false);
  const [kind, setKind] = useState<LedgerKind>("decision");
  const [area, setArea] = useState<LedgerArea | "">("");
  const [topic, setTopic] = useState("");
  const [statement, setStatement] = useState("");

  if (!open) {
    return (
      <div>
        <button type="button" className="btn btn-quiet" onClick={() => setOpen(true)}>
          {t.add}
        </button>
      </div>
    );
  }
  return (
    <form
      className={styles.ledgerAdd}
      onSubmit={(event) => {
        event.preventDefault();
        add.mutate(
          area ? { kind, topic, statement, rationale: "", area } : { kind, topic, statement, rationale: "" },
          {
            onSuccess: () => {
              setTopic("");
              setStatement("");
              setOpen(false);
              void client.invalidateQueries({ queryKey: keys.studio });
            },
          },
        );
      }}
    >
      <label className={styles.fieldLabel} htmlFor="ledger-kind">
        {t.kindLabel}
      </label>
      <select id="ledger-kind" className={styles.input} value={kind} onChange={(event) => setKind(event.target.value as LedgerKind)}>
        {KINDS.map((k) => (
          <option key={k} value={k}>
            {t.kinds[k]}
          </option>
        ))}
      </select>
      <label className={styles.fieldLabel} htmlFor="ledger-area">
        {t.areaLabel}
      </label>
      <AreaSelect id="ledger-area" value={area} onChange={setArea} />
      <label className={styles.fieldLabel} htmlFor="ledger-topic">
        {t.topicLabel}
      </label>
      <input id="ledger-topic" className={styles.input} value={topic} maxLength={120} onChange={(event) => setTopic(event.target.value)} />
      <label className={styles.fieldLabel} htmlFor="ledger-statement">
        {t.statementLabel}
      </label>
      <textarea id="ledger-statement" className={styles.input} rows={2} value={statement} maxLength={2000} onChange={(event) => setStatement(event.target.value)} />
      <p className={styles.problem}>{problem(add.error) ?? ""}</p>
      <div className={styles.rowActions}>
        <button type="submit" className="btn btn-primary">
          {t.save}
        </button>
        <button type="button" className="btn btn-quiet" onClick={() => setOpen(false)}>
          {t.cancel}
        </button>
      </div>
    </form>
  );
}

function AreaSelect({ id: inputId, value, onChange }: { id: string; value: LedgerArea | ""; onChange: (area: LedgerArea | "") => void }) {
  return (
    <select id={inputId} className={styles.input} value={value} onChange={(event) => onChange(event.target.value as LedgerArea | "")}>
      <option value="">{t.areas.other}</option>
      {AREAS.map((a) => (
        <option key={a} value={a}>
          {t.areas[a]}
        </option>
      ))}
    </select>
  );
}
