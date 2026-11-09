// The owner's own report for a checkpoint they built (owner's decision, CONTEXT.md, S4): what is
// done, a result for every required test, what is not verified, open issues, and the commit. Hub
// checks it with the same rules as Claude's before it is sent.

import { useId, useState } from "react";

import { useApi } from "../../data/api";
import type { Checkpoint } from "../../data/generated/Checkpoint";
import type { Problem } from "../../data/generated/Problem";
import type { ReportInput } from "../../data/generated/ReportInput";
import type { TestResult } from "../../data/generated/TestResult";
import { useStudioAct } from "../../data/queries";
import { id } from "../../strings/id";
import styles from "./prd.module.css";
import own from "./roadmap.module.css";

const t = id.studio.pantau;
const f = t.form;
const RESULTS: TestResult[] = ["passed", "failed", "not_run"];
const lines = (text: string) => text.split("\n").map((l) => l.trim()).filter(Boolean);

export function ReportForm({ cp, onSent, onCancel }: { cp: Checkpoint; onSent: () => void; onCancel: () => void }) {
  const api = useApi();
  const base = useId();
  const [summary, setSummary] = useState("");
  const [tests, setTests] = useState<{ name: string; command: string; result: TestResult; outputTail: string }[]>(
    cp.requiredTests.map((test) => ({ name: test.name, command: test.how, result: "passed", outputTail: "" })),
  );
  const [notVerified, setNotVerified] = useState("");
  const [openIssues, setOpenIssues] = useState("");
  const [commit, setCommit] = useState("");
  const [problems, setProblems] = useState<Problem[]>([]);
  const report = (): ReportInput => ({
    summary: summary.trim(),
    changes: [],
    commit: commit.trim(),
    tests: tests.map((test) => ({ ...test })),
    evidence: [],
    notVerified: lines(notVerified),
    openIssues: lines(openIssues),
    next: "",
  });
  const send = useStudioAct(async () => {
    const found = await api.checkCheckpointReport(cp.id, report());
    setProblems(found);
    if (found.length > 0) return false;
    await api.submitCheckpointReport(cp.id, report());
    return true;
  });

  return (
    <form
      className={styles.editing}
      aria-label={f.title}
      onSubmit={(event) => {
        event.preventDefault();
        send.mutate(undefined, { onSuccess: (sent) => sent && onSent() });
      }}
    >
      <p className={styles.hint}>{f.intro}</p>
      <div className={styles.field}>
        <label htmlFor={`${base}-summary`}>{f.summary}</label>
        <textarea id={`${base}-summary`} className={styles.input} rows={3} maxLength={4000} value={summary} onChange={(event) => setSummary(event.target.value)} />
      </div>
      {tests.map((test, i) => {
        const update = (patch: Partial<typeof test>) => setTests((all) => all.map((x, j) => (j === i ? { ...x, ...patch } : x)));
        return (
          <fieldset key={test.name} className={styles.group}>
            <legend>{test.name}</legend>
            <div className={own.choices} role="radiogroup" aria-label={test.name}>
              {RESULTS.map((result) => (
                <label key={result} className={own.choice}>
                  <input type="radio" name={`${base}-r-${i}`} checked={test.result === result} onChange={() => update({ result })} />
                  <span>{t.results[result]}</span>
                </label>
              ))}
            </div>
            <div className={styles.field}>
              <label htmlFor={`${base}-cmd-${i}`}>{f.command}</label>
              <input id={`${base}-cmd-${i}`} className={`${styles.input} ${own.mono}`} maxLength={500} value={test.command} onChange={(event) => update({ command: event.target.value })} />
            </div>
            <div className={styles.field}>
              <label htmlFor={`${base}-out-${i}`}>{f.output}</label>
              <textarea id={`${base}-out-${i}`} className={`${styles.input} ${own.mono}`} rows={2} maxLength={4000} value={test.outputTail} onChange={(event) => update({ outputTail: event.target.value })} />
            </div>
          </fieldset>
        );
      })}
      <div className={styles.field}>
        <label htmlFor={`${base}-nv`}>{f.notVerified}</label>
        <textarea id={`${base}-nv`} className={styles.input} rows={2} value={notVerified} onChange={(event) => setNotVerified(event.target.value)} />
      </div>
      <div className={styles.field}>
        <label htmlFor={`${base}-issues`}>{f.openIssues}</label>
        <textarea id={`${base}-issues`} className={styles.input} rows={2} value={openIssues} onChange={(event) => setOpenIssues(event.target.value)} />
      </div>
      <div className={styles.field}>
        <label htmlFor={`${base}-commit`}>{f.commit}</label>
        <input id={`${base}-commit`} className={`${styles.input} ${own.mono}`} maxLength={64} value={commit} onChange={(event) => setCommit(event.target.value)} />
      </div>
      {problems.length > 0 && (
        <div className={styles.problems} role="alert">
          <p>
            <strong>{f.problems}</strong>
          </p>
          <ul>
            {problems.map((p, i) => (
              <li key={i}>{p.message}</li>
            ))}
          </ul>
        </div>
      )}
      <p className={styles.error}>{send.error ? (t.errors[send.error.view.field ?? ""] ?? t.errors.fallback) : ""}</p>
      <div className={styles.actions}>
        <button type="submit" className="btn btn-primary" disabled={send.isPending}>
          {f.send}
        </button>
        <button type="button" className="btn btn-quiet" onClick={onCancel}>
          {t.cancel}
        </button>
      </div>
    </form>
  );
}
