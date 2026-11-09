// Pantau, the control center of a project being built (docs/studio.md, "Pantau"), reading local
// data only: Sekarang (the active checkpoint and what it needs next), Menunggu kamu (reports to
// review, questions, blockers), the checkpoints with their statuses, and the selected one's
// reports. Progress is counted, never an invented percentage. Claude Code works over MCP (S5) and
// its status shows here in three signals (R5); the owner can also build a checkpoint alone.

import { useGlide } from "../../motion/motion";
import { useSound } from "../../sound/SoundProvider";
import { useQueryClient } from "@tanstack/react-query";
import { useId, useState } from "react";

import { useApi } from "../../data/api";
import type { HubError } from "../../data/backend";
import type { Checkpoint } from "../../data/generated/Checkpoint";
import type { StudioProject } from "../../data/generated/StudioProject";
import type { StudioReport } from "../../data/generated/StudioReport";
import type { StudioRequest } from "../../data/generated/StudioRequest";
import { keys, useCheckpointReports, useClaudeSignals, useStudioAct, useStudioRequests, useStudioRoadmap } from "../../data/queries";
import { id } from "../../strings/id";
import { useNow } from "../../time/clock";
import { relativeDate } from "../../time/format";
import { ClaudeStatus } from "./ClaudeStatus";
import { ReportForm } from "./ReportForm";
import { StatusMark } from "./RoadmapPane";
import styles from "./prd.module.css";
import own from "./roadmap.module.css";

const t = id.studio.pantau;
const ACTIVE = new Set(["ready", "in_progress", "blocked", "in_review"]);

function failure(error: HubError | null): string {
  if (!error) return "";
  return t.errors[error.view.field ?? ""] ?? t.errors.fallback ?? "";
}

export function PantauPane({ project }: { project: StudioProject }) {
  const roadmap = useStudioRoadmap(project.id);
  const requests = useStudioRequests(project.id);
  const [selected, setSelected] = useState<string | null>(null);
  const approved = roadmap.data?.approved ?? null;

  if (!roadmap.data) return null;
  if (!approved) {
    return (
      <section className={styles.pane} aria-labelledby="pantau-title">
        <h2 className={styles.title} id="pantau-title">
          {t.title}
        </h2>
        <p className={styles.muted}>{t.empty}</p>
      </section>
    );
  }

  const list = approved.checkpoints;
  const done = list.filter((cp) => cp.status === "completed").length;
  const active = list.find((cp) => ACTIVE.has(cp.status)) ?? null;
  const next = active ? null : (list.find((cp) => cp.status === "planned") ?? null);
  const shownId = selected ?? active?.id ?? list.filter((cp) => cp.status === "completed").pop()?.id ?? null;
  const shown = list.find((cp) => cp.id === shownId) ?? null;
  const waiting = (requests.data ?? []).filter((r) => r.status === "open" && r.kind !== "changes");

  return (
    <section className={styles.pane} aria-labelledby="pantau-title">
      <header className={styles.head}>
        <h2 className={styles.title} id="pantau-title">
          {t.title}
          <span className={styles.status}> · {t.progress(done, list.length)}</span>
        </h2>
      </header>

      <Now project={project} checkpoints={list} active={active} next={next} requests={requests.data ?? []} />

      <section className={own.block} aria-labelledby="pantau-waiting">
        <h3 id="pantau-waiting">{t.waiting}</h3>
        {waiting.length === 0 ? (
          <p className={styles.muted}>{t.nothingWaiting}</p>
        ) : (
          <ul className={own.waiting}>
            {waiting.map((request) => (
              <Waiting
                key={request.id}
                request={request}
                checkpoints={list}
                onOpen={(cpId) => {
                  setSelected(cpId);
                  requestAnimationFrame(() => document.getElementById("pantau-reports-block")?.scrollIntoView({ block: "start" }));
                }}
              />
            ))}
          </ul>
        )}
      </section>

      <section className={own.block} aria-labelledby="pantau-list">
        <h3 id="pantau-list">{t.checkpoints}</h3>
        <ol className={own.compact}>
          {list.map((cp) => (
            <li key={cp.id}>
              <button
                type="button"
                className={own.pick}
                aria-pressed={cp.id === shownId}
                aria-label={`${cp.code} ${cp.title}, ${id.studio.roadmap.statuses[cp.status] ?? ""}`}
                onClick={() => setSelected(cp.id)}
              >
                <code className={own.mono}>{cp.code}</code>
                <span className={own.pickTitle}>{cp.title}</span>
                <StatusMark status={cp.status} />
              </button>
            </li>
          ))}
        </ol>
      </section>

      {shown && <Reports cp={shown} />}
    </section>
  );
}

/** The active checkpoint (or the next one to start) and the one thing it needs now. */
function Now({ project, checkpoints, active, next, requests }: { project: StudioProject; checkpoints: Checkpoint[]; active: Checkpoint | null; next: Checkpoint | null; requests: StudioRequest[] }) {
  const api = useApi();
  const client = useQueryClient();
  const refresh = () => client.invalidateQueries({ queryKey: keys.studio });
  const base = useId();
  const [mode, setMode] = useState<"report" | "block" | "changes" | null>(null);
  const [text, setText] = useState("");
  const [prompt, setPrompt] = useState<string | null>(null);
  // R4: "Dengan Hub" (Claude reads over MCP) or "Mandiri" (everything in one text).
  const [promptKind, setPromptKind] = useState<"hub" | "standalone">("hub");
  const kinds = useGlide<HTMLDivElement>("input:checked");
  const [notice, setNotice] = useState("");
  const ready = useStudioAct((cpId: string) => api.makeCheckpointReady(cpId));
  const start = useStudioAct((cpId: string) => api.startCheckpoint(cpId));
  const block = useStudioAct(({ cpId, reason }: { cpId: string; reason: string }) => api.blockCheckpoint(cpId, reason));
  const resume = useStudioAct(({ cpId, answer }: { cpId: string; answer: string }) => api.resumeCheckpoint(cpId, answer));
  const approve = useStudioAct((cpId: string) => api.approveCheckpoint(cpId));
  const sound = useSound();
  const changes = useStudioAct(({ cpId, note }: { cpId: string; note: string }) => api.requestCheckpointChanges(cpId, note));
  const task = useStudioAct(({ taskId, doneNow }: { taskId: string; doneNow: boolean }) => api.setStudioTaskStatus(taskId, doneNow ? "done" : "todo", null));
  const makePrompt = useStudioAct(({ cpId, kind }: { cpId: string; kind: "hub" | "standalone" }) =>
    kind === "hub" ? api.implementationPrompt(cpId) : api.standalonePrompt(cpId),
  );
  const showPrompt = (cpId: string, kind: "hub" | "standalone") => {
    setPromptKind(kind);
    makePrompt.mutate({ cpId, kind }, { onSuccess: (text) => setPrompt(text) });
  };
  const error = failure(ready.error ?? start.error ?? block.error ?? resume.error ?? approve.error ?? changes.error ?? task.error ?? makePrompt.error);
  const after = (message = "") => {
    setMode(null);
    setText("");
    setNotice(message);
    void refresh();
  };
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(prompt ?? "");
      setNotice(t.copied);
    } catch {
      setNotice(t.copyFailed);
    }
  };

  if (!active && !next) {
    return (
      <section className={own.block} aria-labelledby="pantau-now">
        <h3 id="pantau-now">{t.now}</h3>
        <p>{t.allDone}</p>
        {/* The approval of the last checkpoint is still said, and announced. */}
        <div role="status" className={styles.notices}>
          {notice && <p className={styles.notice}>{notice}</p>}
        </div>
      </section>
    );
  }

  const cp = (active ?? next) as Checkpoint;
  const waitingFor = cp.status === "planned" ? cp.dependsOn.filter((code) => checkpoints.find((c) => c.code === code)?.status !== "completed") : [];
  const top = cp.tasks.filter((x) => x.parentId === null);
  const blockers = requests.filter((r) => r.checkpointId === cp.id && r.kind === "blocker" && r.status === "open");
  const canPrompt = ["ready", "in_progress", "blocked", "in_review"].includes(cp.status);

  return (
    <section className={own.block} aria-labelledby="pantau-now">
      <h3 id="pantau-now">{t.now}</h3>
      <div className={own.nowHead}>
        <code className={own.code}>{cp.code}</code>
        <div className={own.rowMain}>
          <p className={own.cpTitle}>{cp.title}</p>
          {cp.goal && <p className={own.goal}>{cp.goal}</p>}
          <p className={own.meta}>
            <StatusMark status={cp.status} />
            {top.length > 0 && <span>{t.tasks(top.filter((x) => x.status === "done").length, top.length)}</span>}
          </p>
        </div>
      </div>
      <ClaudeLine project={project} />

      <div role="status" className={styles.notices}>
        {notice && <p className={styles.notice}>{notice}</p>}
      </div>
      <p className={styles.error}>{error}</p>

      {cp.status === "planned" && (
        <>
          {waitingFor.length > 0 ? (
            <p className={styles.muted}>{t.waitingFor(waitingFor.join(", "))}</p>
          ) : (
            <p className={styles.hint}>{t.startHint}</p>
          )}
          <div className={styles.actions}>
            <button type="button" className="btn btn-primary" disabled={waitingFor.length > 0} onClick={() => ready.mutate(cp.id, { onSuccess: () => after() })}>
              {t.start(cp.code)}
            </button>
          </div>
        </>
      )}

      {(cp.status === "in_progress" || cp.status === "blocked") && top.length > 0 && (
        <ul className={own.taskChecks}>
          {top.map((x) => (
            <li key={x.id}>
              <label className={styles.check}>
                <input
                  type="checkbox"
                  data-sound="check"
                  checked={x.status === "done"}
                  aria-label={t.markDone(x.title)}
                  onChange={(event) => task.mutate({ taskId: x.id, doneNow: event.target.checked }, { onSuccess: () => void refresh() })}
                />
                <span>
                  <code className={own.mono}>{x.code}</code> {x.title}
                  {x.note && <span className={styles.muted}> · {x.note}</span>}
                </span>
              </label>
            </li>
          ))}
        </ul>
      )}

      {cp.status === "blocked" && (
        <form
          className={styles.revise}
          onSubmit={(event) => {
            event.preventDefault();
            resume.mutate({ cpId: cp.id, answer: text }, { onSuccess: () => after() });
          }}
        >
          <p className={styles.muted}>{t.blockers}:</p>
          <ul>
            {blockers.map((r) => (
              <li key={r.id}>
                {r.text} <span className={styles.muted}>({t.by[r.madeBy]})</span>
              </li>
            ))}
          </ul>
          <label htmlFor={`${base}-answer`}>{t.resumeLabel}</label>
          <input id={`${base}-answer`} className={styles.input} value={text} maxLength={4000} onChange={(event) => setText(event.target.value)} />
          <div className={styles.actions}>
            <button type="submit" className="btn btn-primary" disabled={!text.trim()}>
              {t.resume}
            </button>
          </div>
        </form>
      )}

      {cp.status === "in_review" && mode !== "changes" && (
        <div className={styles.actions}>
          <button type="button" className="btn btn-primary" onClick={() =>
              approve.mutate(cp.id, {
                onSuccess: () => {
                  sound.play("hero");
                  after(t.approved(cp.code));
                },
              })
            }
          >
            {t.approve}
          </button>
          <button type="button" className="btn btn-secondary" onClick={() => setMode("changes")}>
            {t.changes}
          </button>
        </div>
      )}

      {(cp.status === "ready" || cp.status === "in_progress") && mode === null && (
        <div className={styles.actions}>
          {cp.status === "ready" && (
            <button type="button" className="btn btn-secondary" onClick={() => start.mutate(cp.id, { onSuccess: () => after() })}>
              {t.workAlone}
            </button>
          )}
          {cp.status === "in_progress" && (
            <>
              <button type="button" className="btn btn-secondary" onClick={() => setMode("report")}>
                {t.writeReport}
              </button>
              <button type="button" className="btn btn-quiet" onClick={() => setMode("block")}>
                {t.block}
              </button>
            </>
          )}
        </div>
      )}

      {canPrompt && (
        <div className={own.prompt}>
          {prompt === null ? (
            <div className={styles.actions}>
              <button
                type="button"
                className={`btn ${cp.status === "ready" ? "btn-primary" : "btn-secondary"}`}
                onClick={() => showPrompt(cp.id, promptKind)}
              >
                {t.prompt}
              </button>
            </div>
          ) : (
            <>
              <label htmlFor={`${base}-prompt`} className={own.promptLabel}>
                {t.promptTitle}
              </label>
              <div ref={kinds} className={own.kinds} role="radiogroup" aria-label={t.promptKinds}>
                {(["hub", "standalone"] as const).map((kind) => (
                  <label key={kind} className={own.kind}>
                    <input
                      type="radio"
                      name={`${base}-kind`}
                      value={kind}
                      checked={promptKind === kind}
                      onChange={() => showPrompt(cp.id, kind)}
                    />
                    <span>{t.promptKind[kind]}</span>
                  </label>
                ))}
              </div>
              <p className={styles.hint}>{t.promptKindHint[promptKind]}</p>
              <p className={styles.hint}>{t.promptHint}</p>
              {promptKind === "hub" && project.claudeAccess === "none" && <p className={own.warn}>{t.accessNone}</p>}
              <textarea id={`${base}-prompt`} className={`${styles.input} ${own.mono}`} rows={10} readOnly value={prompt} />
              <div className={styles.actions}>
                <button type="button" className="btn btn-primary" data-sound="copy" onClick={() => void copy()}>
                  {t.copy}
                </button>
                <button type="button" className="btn btn-quiet" onClick={() => setPrompt(null)}>
                  {t.hidePrompt}
                </button>
              </div>
            </>
          )}
        </div>
      )}

      {mode === "report" && <ReportForm cp={cp} onSent={() => after()} onCancel={() => setMode(null)} />}

      {(mode === "block" || mode === "changes") && (
        <form
          className={styles.revise}
          onSubmit={(event) => {
            event.preventDefault();
            if (mode === "block") block.mutate({ cpId: cp.id, reason: text }, { onSuccess: () => after() });
            else changes.mutate({ cpId: cp.id, note: text }, { onSuccess: () => after() });
          }}
        >
          <label htmlFor={`${base}-text`}>{mode === "block" ? t.blockLabel : t.changesLabel}</label>
          <textarea id={`${base}-text`} className={styles.input} rows={3} maxLength={4000} value={text} onChange={(event) => setText(event.target.value)} />
          <div className={styles.actions}>
            <button type="submit" className="btn btn-primary" disabled={!text.trim()}>
              {mode === "block" ? t.blockSend : t.changesSend}
            </button>
            <button type="button" className="btn btn-quiet" onClick={() => { setMode(null); setText(""); }}>
              {t.cancel}
            </button>
          </div>
        </form>
      )}
    </section>
  );
}

/** Claude Code and this project (R5): the bridge, the session in the project's folder, and the
 * last call about it, each from what Hub really heard; the project's access; and how to connect
 * while Hub has heard nothing yet. */
function ClaudeLine({ project }: { project: StudioProject }) {
  const signals = useClaudeSignals(project.id);
  const access = t.access(id.studio.roadmap.approval.accessLevels[project.claudeAccess] ?? "");
  const nothing = signals.data !== undefined && !signals.data.session && !signals.data.activity;
  return (
    <div className={own.claudeLine} role="group" aria-labelledby="pantau-claude">
      <h4 id="pantau-claude" className={own.claudeTitle}>
        {t.claudeTitle}
      </h4>
      <ClaudeStatus projectId={project.id} />
      <p className={styles.muted}>{access}</p>
      {nothing && <p className={styles.hint}>{t.claudeSteps}</p>}
    </div>
  );
}

function Waiting({ request, checkpoints, onOpen }: { request: StudioRequest; checkpoints: Checkpoint[]; onOpen: (cpId: string) => void }) {
  const api = useApi();
  const client = useQueryClient();
  const base = useId();
  const [text, setText] = useState("");
  const answer = useStudioAct((value: string) => api.answerStudioRequest(request.id, value));
  const cp = checkpoints.find((c) => c.id === request.checkpointId);
  return (
    <li className={own.waitingItem}>
      <p className={own.meta}>
        <strong>{t.kinds[request.kind]}</strong>
        {cp && <code className={own.mono}>{cp.code}</code>}
        <span>{t.by[request.madeBy]}</span>
      </p>
      <p>{request.text}</p>
      {request.kind === "review" && cp ? (
        <div className={styles.actions}>
          <button type="button" className="btn btn-secondary" onClick={() => onOpen(cp.id)}>
            {t.openReport}
          </button>
        </div>
      ) : (
        request.kind !== "review" && (
          <form
            className={styles.saveRow}
            onSubmit={(event) => {
              event.preventDefault();
              answer.mutate(text, { onSuccess: () => { setText(""); void client.invalidateQueries({ queryKey: keys.studio }); } });
            }}
          >
            <label htmlFor={`${base}-a`} className="sr-only">
              {t.answerLabel}
            </label>
            <input id={`${base}-a`} className={styles.input} placeholder={t.answerLabel} value={text} maxLength={4000} onChange={(event) => setText(event.target.value)} />
            <button type="submit" className="btn btn-secondary" disabled={!text.trim()}>
              {t.answer}
            </button>
          </form>
        )
      )}
    </li>
  );
}

function Reports({ cp }: { cp: Checkpoint }) {
  const reports = useCheckpointReports(cp.id);
  return (
    <section className={own.block} aria-labelledby="pantau-reports" id="pantau-reports-block">
      <h3 id="pantau-reports">
        {t.reports} <code className={own.mono}>{cp.code}</code>
      </h3>
      {(reports.data ?? []).length === 0 ? <p className={styles.muted}>{t.noReports}</p> : reports.data?.map((report) => <ReportView key={report.id} report={report} />)}
    </section>
  );
}

function ReportView({ report }: { report: StudioReport }) {
  const now = useNow();
  const body = report.body;
  // Tests that did not pass come first: they are what the review is about.
  const tests = [...body.tests].sort((a, b) => Number(a.result === "passed") - Number(b.result === "passed"));
  return (
    <article className={own.report} aria-label={t.reportBy(t.by[report.madeBy] ?? "", relativeDate(report.createdAt.slice(0, 10), now))}>
      <p className={own.meta}>{t.reportBy(t.by[report.madeBy] ?? "", relativeDate(report.createdAt.slice(0, 10), now))}</p>
      <p>{body.summary}</p>
      {body.commit && (
        <p className={styles.muted}>
          {t.commit}: <code className={own.mono}>{body.commit}</code>
        </p>
      )}
      {body.changes.length > 0 && (
        <div className={styles.para}>
          <h4>{t.changesMade}</h4>
          <ul>
            {body.changes.map((c, i) => (
              <li key={i}>
                <code className={own.mono}>{c.path}</code> {c.what}
              </li>
            ))}
          </ul>
        </div>
      )}
      <div className={styles.para}>
        <h4>{t.tests}</h4>
        <ul className={own.testList}>
          {tests.map((test, i) => (
            <li key={i} data-result={test.result}>
              <strong>{t.results[test.result]}</strong> {test.name}
              {test.command && (
                <>
                  {" "}
                  <code className={own.mono}>{test.command}</code>
                </>
              )}
              {typeof test.exitCode === "number" && <span className={styles.muted}> · {t.exit(test.exitCode)}</span>}
              {typeof test.durationMs === "number" && <span className={styles.muted}> · {t.duration(test.durationMs)}</span>}
              {test.outputTail && (
                <details>
                  <summary>{t.output}</summary>
                  <pre className={own.output}>{test.outputTail}</pre>
                </details>
              )}
            </li>
          ))}
        </ul>
      </div>
      {body.notVerified.length > 0 && (
        <div className={styles.para}>
          <h4>{t.notVerified}</h4>
          <ul>
            {body.notVerified.map((x, i) => (
              <li key={i}>{x}</li>
            ))}
          </ul>
        </div>
      )}
      {body.openIssues.length > 0 && (
        <div className={styles.para}>
          <h4>{t.openIssues}</h4>
          <ul>
            {body.openIssues.map((x, i) => (
              <li key={i}>{x}</li>
            ))}
          </ul>
        </div>
      )}
      {body.next && (
        <p className={styles.muted}>
          {t.nextStep}: {body.next}
        </p>
      )}
    </article>
  );
}
