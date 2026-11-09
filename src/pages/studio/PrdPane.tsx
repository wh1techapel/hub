// The PRD tab: Gemini writes it from the conversation (Susun PRD), the owner reads it as a
// document, edits a section in place or asks Gemini to revise it (a proposal to accept or
// reject), saves versions, approves one, exports Markdown, and reads earlier versions.

import { useQueryClient } from "@tanstack/react-query";
import { useImperativeHandle, useState } from "react";
import type { Ref } from "react";

import { useApi } from "../../data/api";
import type { PrdEvent, PrdProposal } from "../../data/api";
import type { HubError } from "../../data/backend";
import type { Prd } from "../../data/generated/Prd";
import type { PrdSection } from "../../data/generated/PrdSection";
import type { PrdVersion } from "../../data/generated/PrdVersion";
import { keys, usePrdVersion, useStudioAct, useStudioPrd } from "../../data/queries";
import { id } from "../../strings/id";
import { relativeDate } from "../../time/format";
import { useNow } from "../../time/clock";
import { SectionBody } from "./PrdDocument";
import { SECTION_FIELDS, SECTIONS, sectionOf } from "./prdFields";
import type { Approver } from "./results";
import { PrdProgress } from "./PrdProgress";
import { SectionEditor } from "./SectionEditor";
import styles from "./prd.module.css";

const t = id.studio.prd;

export interface Writing {
  running: boolean;
  step: PrdEvent | null;
  notice: string | null;
  /** When the writing began (L3: the progress shows the time since). */
  startedAt?: string | null;
}

function failure(error: HubError | null): string | null {
  if (!error) return null;
  return t.errors[error.view.field ?? ""] ?? t.errors[error.view.reason ?? ""] ?? t.errors.fallback ?? null;
}

export function PrdPane({
  projectId,
  writing,
  openQuestions,
  hasKey,
  onGenerate,
  onStop,
  approver,
  onApproved = () => undefined,
}: {
  projectId: string;
  writing: Writing;
  openQuestions: number;
  hasKey: boolean;
  onGenerate: () => void;
  onStop: () => void;
  /** U4b: Setujui on the PRD card in the conversation starts this pane's own approval. */
  approver?: Ref<Approver>;
  /** A version was approved (the plan may be written next, U4b). */
  onApproved?: () => void;
}) {
  const prd = useStudioPrd(projectId);
  const client = useQueryClient();
  const api = useApi();
  const now = useNow();
  const refresh = () => client.invalidateQueries({ queryKey: keys.studio });
  const [asking, setAsking] = useState<"generate" | "approve" | null>(null);
  const [history, setHistory] = useState(false);
  const [viewing, setViewing] = useState<PrdVersion | null>(null);
  const [note, setNote] = useState("");
  const [notice, setNotice] = useState<string | null>(null);
  // L3: what was just written fades in, once, when the writing ends (not on every visit). Kept
  // in step with `writing.running` while rendering, as React advises for state that follows a prop.
  const [wasRunning, setWasRunning] = useState(writing.running);
  const [arrived, setArrived] = useState(false);
  if (wasRunning !== writing.running) {
    setWasRunning(writing.running);
    setArrived(!writing.running);
  }
  const paneClass = arrived ? `${styles.pane} ${styles.arrived}` : styles.pane;
  const approve = useStudioAct(() => api.approvePrd(projectId));
  const save = useStudioAct((text: string) => api.savePrdVersion(projectId, text));
  const exportFile = useStudioAct((versionId: string | null) => api.exportPrd(projectId, versionId));
  const applyStack = useStudioAct(() => api.applyStackToPrd(projectId));

  const view = prd.data;
  const draft = view?.draft ?? null;
  const latest = view?.versions[0] ?? null;
  const approvedNow = (n: number) => {
    setNotice(t.approved(n));
    void refresh();
    onApproved();
  };
  // The same rule as the button: no problems, not already approved as it stands.
  const canApprove = draft !== null && (view?.problems.length ?? 0) === 0 && !approve.isPending && !(latest?.status === "approved" && !view?.unsaved);
  const startApprove = () => {
    if (!draft) return;
    if (draft.delivery.questions.length > 0) setAsking("approve");
    else approve.mutate(undefined, { onSuccess: approvedNow });
  };
  // U4b: from the card, the same as the button; with open questions the same confirmation first.
  useImperativeHandle(approver, () => ({
    approve: () => {
      if (canApprove && !writing.running) startApprove();
    },
  }));

  if (writing.running) return <PrdProgress step={writing.step} startedAt={writing.startedAt ?? null} onStop={onStop} />;

  const generate = () => {
    if (!draft && openQuestions > 0) setAsking("generate");
    else if (draft) setAsking("generate");
    else onGenerate();
  };

  if (!draft) {
    return (
      <section className={paneClass} aria-labelledby="prd-title">
        <h2 className={styles.title} id="prd-title">
          {t.title}
        </h2>
        {writing.notice && <p className={styles.notice}>{writing.notice}</p>}
        <p className={styles.muted}>{t.empty}</p>
        {asking === "generate" ? (
          <Confirm text={t.openAsk(openQuestions)} yes={t.openAskYes} onYes={() => { setAsking(null); onGenerate(); }} onNo={() => setAsking(null)} />
        ) : (
          <div>
            <button type="button" className="btn btn-primary" disabled={!hasKey} onClick={generate}>
              {t.generate}
            </button>
          </div>
        )}
      </section>
    );
  }

  const questions = draft.delivery.questions.length;
  const problems = view?.problems ?? [];
  // R4: the draft's technologies should be the approved stack's (after the stack changed, or for
  // a PRD written before R4).
  const stackLines = view?.stack ?? null;
  const stackDiffers =
    stackLines !== null && (!draft.technical.fromStack || JSON.stringify(draft.technical.technologies) !== JSON.stringify(stackLines));
  const heading = latest?.number ? t.version(latest.number) : t.draftOnly;

  return (
    <section className={paneClass} aria-labelledby="prd-title">
      <header className={styles.head}>
        <h2 className={styles.title} id="prd-title">
          {heading}
          {latest && <span className={styles.status}> · {t.statuses[latest.status]}</span>}
        </h2>
        <div className={styles.actions}>
          <button
            type="button"
            className="btn btn-primary"
            disabled={!canApprove}
            onClick={startApprove}
          >
            {t.approve}
          </button>
          <button type="button" className="btn btn-secondary" onClick={() => setHistory((open) => !open)} aria-expanded={history}>
            {history ? t.hideHistory : t.history}
          </button>
          <button
            type="button"
            className="btn btn-quiet"
            onClick={() => exportFile.mutate(viewing?.id ?? null, { onSuccess: (path) => path && setNotice(t.exported(path)) })}
          >
            {t.export}
          </button>
          <button type="button" className="btn btn-quiet" disabled={!hasKey} onClick={generate}>
            {t.regenerate}
          </button>
        </div>
      </header>

      {asking === "approve" && (
        <Confirm
          text={t.approveAsk(questions)}
          yes={t.approveYes}
          onYes={() => {
            setAsking(null);
            approve.mutate(undefined, { onSuccess: approvedNow });
          }}
          onNo={() => setAsking(null)}
        />
      )}
      {asking === "generate" && (
        <Confirm text={t.regenerateAsk} yes={t.regenerateYes} onYes={() => { setAsking(null); onGenerate(); }} onNo={() => setAsking(null)} />
      )}

      <div role="status" className={styles.notices}>
        {writing.notice && <p className={styles.notice}>{writing.notice}</p>}
        {notice && <p className={styles.notice}>{notice}</p>}
        {view?.approved?.number && view.approved.approvedAt && (
          <p className={styles.muted}>{t.approvedLine(view.approved.number, relativeDate(view.approved.approvedAt.slice(0, 10), now))}</p>
        )}
        {view?.approved && view.unsaved && <p className={styles.muted}>{t.draftDiffers}</p>}
        {view?.unsaved && <p className={styles.muted}>{t.unsaved}</p>}
      </div>

      {view?.unsaved && (
        <form
          className={styles.saveRow}
          onSubmit={(event) => {
            event.preventDefault();
            save.mutate(note, { onSuccess: (n) => { setNote(""); setNotice(t.saved(n)); void refresh(); } });
          }}
        >
          <label className="sr-only" htmlFor="prd-note">
            {t.notePlaceholder}
          </label>
          <input id="prd-note" className={styles.input} placeholder={t.notePlaceholder} value={note} maxLength={1000} onChange={(event) => setNote(event.target.value)} />
          <button type="submit" className="btn btn-secondary">
            {t.saveVersion}
          </button>
        </form>
      )}

      {stackDiffers && (
        <div className={styles.confirm} role="group" aria-label={t.stackDiffers}>
          <p>{t.stackDiffers}</p>
          <div className={styles.actions}>
            <button
              type="button"
              className="btn btn-secondary"
              onClick={() => applyStack.mutate(undefined, { onSuccess: () => { setNotice(t.stackApplied); void refresh(); } })}
            >
              {t.stackApply}
            </button>
          </div>
        </div>
      )}

      {problems.length > 0 && (
        <div className={styles.problems}>
          <p>
            <strong>{t.problems(problems.length)}</strong>
          </p>
          <ul>
            {problems.map((problem, index) => (
              <li key={index}>
                <a href={`#prd-sec-${sectionOf(problem.at)}`} onClick={(event) => { event.preventDefault(); document.getElementById(`prd-sec-${sectionOf(problem.at)}`)?.scrollIntoView({ block: "start" }); }}>
                  <code>{problem.at}</code>
                </a>{" "}
                {problem.message}
              </li>
            ))}
          </ul>
        </div>
      )}

      {history && (
        <ul className={styles.history} aria-label={t.history}>
          {view?.versions.map((version) => (
            <li key={version.id}>
              <span>
                <strong>v{version.number}</strong> · {t.statuses[version.status]} · {version.madeBy === "gemini" ? t.byAgent : t.byOwner} ·{" "}
                {relativeDate(version.createdAt.slice(0, 10), now)}
                {version.changeNote && <span className={styles.muted}> · {version.changeNote}</span>}
              </span>
              <button type="button" className="btn btn-quiet" onClick={() => setViewing(version)}>
                {t.open}
              </button>
            </li>
          ))}
        </ul>
      )}

      {viewing ? (
        <VersionView version={viewing} onBack={() => setViewing(null)} />
      ) : (
        <Document prd={draft} projectId={projectId} hasKey={hasKey} onChanged={() => void refresh()} />
      )}
    </section>
  );
}

function Confirm({ text, yes, onYes, onNo }: { text: string; yes: string; onYes: () => void; onNo: () => void }) {
  return (
    <div className={styles.confirm} role="group" aria-label={text}>
      <p>{text}</p>
      <div className={styles.actions}>
        <button type="button" className="btn btn-primary" onClick={onYes}>
          {yes}
        </button>
        <button type="button" className="btn btn-quiet" onClick={onNo}>
          {t.cancel}
        </button>
      </div>
    </div>
  );
}

function VersionView({ version, onBack }: { version: PrdVersion; onBack: () => void }) {
  const body = usePrdVersion(version.id);
  return (
    <div>
      <div className={styles.viewing}>
        <p>{t.viewing(version.number ?? 0)}</p>
        <button type="button" className="btn btn-secondary" onClick={onBack}>
          {t.backToDraft}
        </button>
      </div>
      {body.data && <Document prd={body.data} readOnly />}
    </div>
  );
}

function Document({ prd, projectId, hasKey = false, readOnly = false, onChanged }: { prd: Prd; projectId?: string; hasKey?: boolean; readOnly?: boolean; onChanged?: () => void }) {
  return (
    <div className={styles.document}>
      <h3 className={`display ${styles.docName}`}>{prd.identity.name}</h3>
      <nav className={styles.toc} aria-label={t.contents}>
        <ol>
          {SECTIONS.map((section) => (
            <li key={section}>
              <a href={`#prd-sec-${section}`} onClick={(event) => { event.preventDefault(); document.getElementById(`prd-sec-${section}`)?.scrollIntoView({ block: "start" }); }}>
                {t.sections[section]}
              </a>
            </li>
          ))}
        </ol>
      </nav>
      {SECTIONS.map((section) => (
        <SectionBlock key={section} prd={prd} section={section} projectId={projectId} hasKey={hasKey} readOnly={readOnly} onChanged={onChanged} />
      ))}
    </div>
  );
}

function SectionBlock({
  prd,
  section,
  projectId,
  hasKey,
  readOnly,
  onChanged,
}: {
  prd: Prd;
  section: PrdSection;
  projectId?: string;
  hasKey: boolean;
  readOnly: boolean;
  onChanged?: () => void;
}) {
  const api = useApi();
  const [editing, setEditing] = useState<unknown>(null);
  const [revising, setRevising] = useState(false);
  const [ask, setAsk] = useState("");
  const [proposal, setProposal] = useState<PrdProposal | null>(null);
  const update = useStudioAct((value: unknown) => api.updatePrdSection(projectId ?? "", section, value));
  const revise = useStudioAct((text: string) => api.revisePrdSection(projectId ?? "", section, text));
  const accept = useStudioAct((value: unknown) => api.acceptPrdRevision(projectId ?? "", section, value));
  const current = prd[section] as unknown;
  const changed = (key: string) =>
    proposal !== null && JSON.stringify((current as Record<string, unknown> | null)?.[key]) !== JSON.stringify((proposal.value as Record<string, unknown> | null)?.[key]);

  return (
    <section className={styles.section} id={`prd-sec-${section}`} aria-labelledby={`prd-h-${section}`}>
      <div className={styles.sectionHead}>
        <h3 id={`prd-h-${section}`}>{t.sections[section]}</h3>
        {!readOnly && editing === null && !proposal && (
          <div className={styles.actions}>
            <button type="button" className="btn btn-quiet" onClick={() => setEditing(structuredClone(current))}>
              {t.edit}
            </button>
            <button type="button" className="btn btn-quiet" disabled={!hasKey} onClick={() => setRevising((open) => !open)} aria-expanded={revising}>
              {t.revise}
            </button>
          </div>
        )}
      </div>

      {revising && !proposal && (
        <form
          className={styles.revise}
          onSubmit={(event) => {
            event.preventDefault();
            revise.mutate(ask, { onSuccess: (next) => { setProposal(next); setRevising(false); setAsk(""); } });
          }}
        >
          <label htmlFor={`prd-ask-${section}`}>{t.reviseLabel}</label>
          <input id={`prd-ask-${section}`} className={styles.input} value={ask} maxLength={2000} placeholder={t.revisePlaceholder} onChange={(event) => setAsk(event.target.value)} />
          <p className={styles.error}>{failure(revise.error) ?? ""}</p>
          <div className={styles.actions}>
            <button type="submit" className="btn btn-primary" disabled={!ask.trim() || revise.isPending}>
              {revise.isPending ? t.revising : t.reviseSend}
            </button>
            <button type="button" className="btn btn-quiet" onClick={() => setRevising(false)}>
              {t.cancel}
            </button>
          </div>
        </form>
      )}

      {proposal ? (
        <div className={styles.proposal}>
          <p className={styles.proposalHead}>{t.proposal}</p>
          {proposal.changeNote && <p>{proposal.changeNote}</p>}
          {proposal.impacts.length > 0 && (
            <>
              <p className={styles.muted}>{t.impacts}</p>
              <ul>
                {proposal.impacts.map((impact, i) => (
                  <li key={i}>{impact}</li>
                ))}
              </ul>
            </>
          )}
          {!Array.isArray(proposal.value) && (
            <p className={styles.muted}>
              {Object.keys((proposal.value ?? {}) as Record<string, unknown>)
                .filter(changed)
                .map((key) => `${t.fields[key] ?? key} (${t.changed})`)
                .join(", ")}
            </p>
          )}
          <SectionBody prd={{ ...prd, [section]: proposal.value }} section={section} />
          <p className={styles.muted}>{t.afterProblems(proposal.problems.length)}</p>
          <div className={styles.actions}>
            <button
              type="button"
              className="btn btn-primary"
              onClick={() => accept.mutate(proposal.value, { onSuccess: () => { setProposal(null); onChanged?.(); } })}
            >
              {t.accept}
            </button>
            <button type="button" className="btn btn-quiet" onClick={() => setProposal(null)}>
              {t.reject}
            </button>
          </div>
        </div>
      ) : editing !== null ? (
        <form
          className={styles.editing}
          onSubmit={(event) => {
            event.preventDefault();
            update.mutate(editing, { onSuccess: () => { setEditing(null); onChanged?.(); } });
          }}
        >
          <SectionEditor fields={SECTION_FIELDS[section]} value={editing} onChange={setEditing} section={section} />
          <p className={styles.error}>{failure(update.error) ?? ""}</p>
          <div className={styles.actions}>
            <button type="submit" className="btn btn-primary" disabled={update.isPending}>
              {t.save}
            </button>
            <button type="button" className="btn btn-quiet" onClick={() => setEditing(null)}>
              {t.cancel}
            </button>
          </div>
        </form>
      ) : (
        <div className={styles.body}>
          <SectionBody prd={prd} section={section} />
        </div>
      )}
    </section>
  );
}
