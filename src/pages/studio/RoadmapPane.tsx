// The Roadmap tab (docs/studio.md, "Roadmap"): Gemini writes the checkpoints from the approved PRD
// (Susun roadmap), the owner reads them as a list with statuses, edits the draft (fields, add,
// remove, reorder with buttons or by dragging the grip), and approves it once with the Hub Proyek,
// the repository folder, and Claude's access. An approved roadmap can be changed again only until a
// checkpoint starts; after that it grows at its end.

import { ChevronDown, ChevronUp, Circle, CircleAlert, CircleCheck, CircleDot, CirclePlay, Eye, GripVertical } from "lucide-react";
import { useQueryClient } from "@tanstack/react-query";
import { useId, useImperativeHandle, useRef, useState } from "react";
import type { Ref } from "react";
import type { PointerEvent } from "react";

import { useApi } from "../../data/api";
import type { RoadmapEvent } from "../../data/api";
import type { HubError } from "../../data/backend";
import type { Checkpoint } from "../../data/generated/Checkpoint";
import type { CheckpointStatus } from "../../data/generated/CheckpointStatus";
import type { Feature } from "../../data/generated/Feature";
import type { PlanCheckpoint } from "../../data/generated/PlanCheckpoint";
import type { Problem } from "../../data/generated/Problem";
import type { StudioProject } from "../../data/generated/StudioProject";
import { keys, usePrdVersion, useStudioAct, useStudioRoadmap } from "../../data/queries";
import { id } from "../../strings/id";
import { AccessChoice, ApproveRoadmapDialog, FolderChooser } from "./ApproveRoadmap";
import { CheckpointEditor, ownDoD } from "./CheckpointEditor";
import type { Approver } from "./results";
import styles from "./prd.module.css";
import own from "./roadmap.module.css";

const t = id.studio.roadmap;

export interface RoadmapWriting {
  running: boolean;
  step: RoadmapEvent | null;
  notice: string | null;
}

const GLYPH: Record<CheckpointStatus, typeof Circle> = {
  planned: Circle,
  ready: CircleDot,
  in_progress: CirclePlay,
  blocked: CircleAlert,
  in_review: Eye,
  completed: CircleCheck,
};

/** The status as a word with a small glyph: never colour alone. */
export function StatusMark({ status }: { status: CheckpointStatus }) {
  const Glyph = GLYPH[status];
  return (
    <span className={own.status} data-status={status}>
      <Glyph size={14} aria-hidden="true" />
      {t.statuses[status]}
    </span>
  );
}

function failure(error: HubError | null): string | null {
  if (!error) return null;
  return t.errors[error.view.field ?? ""] ?? t.errors[error.view.reason ?? ""] ?? t.errors.fallback ?? null;
}

function planOf(cp: Checkpoint): PlanCheckpoint {
  return {
    code: cp.code,
    title: cp.title,
    goal: cp.goal,
    description: cp.description,
    features: [...cp.features],
    tasks: cp.tasks
      .filter((task) => task.parentId === null)
      .map((task) => ({
        code: task.code,
        title: task.title,
        detail: task.detail,
        feature: task.featureId ?? "",
        subtasks: cp.tasks.filter((s) => s.parentId === task.id).map((s) => s.title),
      })),
    dependsOn: [...cp.dependsOn],
    difficulty: cp.difficulty,
    risks: [...cp.risks],
    acceptance: [...cp.acceptance],
    requiredTests: cp.requiredTests.map((x) => ({ ...x })),
    deliverables: [...cp.deliverables],
    doneWhen: [...cp.doneWhen],
  };
}

function blankPlan(): PlanCheckpoint {
  return { code: "", title: "", goal: "", description: "", features: [], tasks: [], dependsOn: [], difficulty: "medium", risks: [], acceptance: [], requiredTests: [], deliverables: [], doneWhen: [] };
}

export function RoadmapPane({
  project,
  writing,
  hasKey,
  onGenerate,
  onStop,
  approver,
}: {
  project: StudioProject;
  writing: RoadmapWriting;
  hasKey: boolean;
  onGenerate: () => void;
  onStop: () => void;
  /** U4b: Setujui on the plan card in the conversation opens this pane's own approval. */
  approver?: Ref<Approver>;
}) {
  const roadmap = useStudioRoadmap(project.id);
  const view = roadmap.data;
  const source = usePrdVersion(view?.prd?.id ?? null);
  const client = useQueryClient();
  const api = useApi();
  const refresh = () => client.invalidateQueries({ queryKey: keys.studio });
  const [asking, setAsking] = useState<"generate" | "discard" | null>(null);
  const [approving, setApproving] = useState(false);
  const [appending, setAppending] = useState<PlanCheckpoint | null>(null);
  const [appendProblems, setAppendProblems] = useState<Problem[]>([]);
  const [notice, setNotice] = useState<string | null>(null);
  const add = useStudioAct((after: string | null) => api.addRoadmapCheckpoint(project.id, after));
  const edit = useStudioAct(() => api.editRoadmap(project.id));
  const discard = useStudioAct(() => api.discardRoadmapDraft(project.id));
  const append = useStudioAct((value: PlanCheckpoint) => api.appendRoadmapCheckpoint(project.id, value));

  const features: Feature[] = (source.data?.features ?? []).filter((f) => f.priority !== "wont");

  // U4b: from the card, the same sheet as the button (the Hub Proyek, the folder, the access).
  useImperativeHandle(approver, () => ({
    approve: () => {
      if (!writing.running && view?.draft && view.problems.length === 0) setApproving(true);
    },
  }));

  if (writing.running) {
    const repair = writing.step?.kind === "step" && writing.step.repair;
    return (
      <section className={styles.pane} aria-labelledby="roadmap-title">
        <h2 className={styles.title} id="roadmap-title">
          {t.title}
        </h2>
        <div className={styles.progress} role="status">
          <p>{repair ? t.repairing : t.writing}</p>
        </div>
        <div>
          <button type="button" className="btn btn-secondary" onClick={onStop}>
            {t.stop}
          </button>
        </div>
      </section>
    );
  }
  if (!view) return null;

  const draft = view.draft;
  const approved = view.approved;
  const shown = draft ?? approved;
  const canGenerate = hasKey && view.prd !== null && !view.locked;
  const generate = () => (shown ? setAsking("generate") : onGenerate());

  if (!shown) {
    return (
      <section className={styles.pane} aria-labelledby="roadmap-title">
        <h2 className={styles.title} id="roadmap-title">
          {t.title}
        </h2>
        {writing.notice && <p className={styles.notice}>{writing.notice}</p>}
        <p className={styles.muted}>{view.prd ? t.empty : t.needPrd}</p>
        <div>
          <button type="button" className="btn btn-primary" disabled={!canGenerate} onClick={generate}>
            {t.generate}
          </button>
        </div>
      </section>
    );
  }

  const isDraft = draft !== null;
  const done = shown.checkpoints.filter((cp) => cp.status === "completed").length;
  const problems = isDraft ? view.problems : [];

  return (
    <section className={styles.pane} aria-labelledby="roadmap-title">
      <header className={styles.head}>
        <h2 className={styles.title} id="roadmap-title">
          {isDraft ? t.draftTitle : t.approvedTitle}
          <span className={styles.status}> · {isDraft ? t.count(shown.checkpoints.length) : t.done(done, shown.checkpoints.length)}</span>
        </h2>
        <div className={styles.actions}>
          {isDraft ? (
            <>
              <button type="button" className="btn btn-primary" disabled={problems.length > 0} onClick={() => setApproving(true)}>
                {t.approve}
              </button>
              <button type="button" className="btn btn-secondary" onClick={() => add.mutate(null, { onSuccess: () => void refresh() })}>
                {t.add}
              </button>
              <button type="button" className="btn btn-quiet" disabled={!canGenerate} onClick={generate}>
                {t.regenerate}
              </button>
              <button type="button" className="btn btn-quiet" onClick={() => setAsking("discard")}>
                {t.discard}
              </button>
            </>
          ) : (
            <>
              {!view.locked && (
                <button type="button" className="btn btn-secondary" onClick={() => edit.mutate(undefined, { onSuccess: () => void refresh() })}>
                  {t.edit}
                </button>
              )}
              <button type="button" className="btn btn-secondary" onClick={() => { setAppending(blankPlan()); setAppendProblems([]); }}>
                {t.append}
              </button>
              {!view.locked && (
                <button type="button" className="btn btn-quiet" disabled={!canGenerate} onClick={generate}>
                  {t.regenerate}
                </button>
              )}
            </>
          )}
        </div>
      </header>

      {asking === "generate" && (
        <Confirm text={t.regenerateAsk} yes={t.regenerateYes} onYes={() => { setAsking(null); onGenerate(); }} onNo={() => setAsking(null)} />
      )}
      {asking === "discard" && (
        <Confirm
          text={t.discardAsk}
          yes={t.discardYes}
          onYes={() => { setAsking(null); discard.mutate(undefined, { onSuccess: () => void refresh() }); }}
          onNo={() => setAsking(null)}
        />
      )}

      <div role="status" className={styles.notices}>
        {writing.notice && <p className={styles.notice}>{writing.notice}</p>}
        {notice && <p className={styles.notice}>{notice}</p>}
        {shown.meta.prdNumber !== null && <p className={styles.muted}>{t.fromPrd(shown.meta.prdNumber)}</p>}
        {isDraft && view.draftPrdChanged && <p className={own.warn}>{t.prdChanged}</p>}
        {!isDraft && view.locked && <p className={styles.muted}>{t.locked}</p>}
        {view.superseded.length > 0 && <p className={styles.muted}>{t.superseded(view.superseded.length)}</p>}
      </div>

      {problems.length > 0 && (
        <div className={styles.problems}>
          <p>
            <strong>{t.problems(problems.length)}</strong>
          </p>
          <ul>
            {problems.map((problem, index) => {
              const code = problem.at.split(".")[0] ?? "";
              return (
                <li key={index}>
                  <a href={`#cp-${code}`} onClick={(event) => { event.preventDefault(); document.getElementById(`cp-${code}`)?.scrollIntoView({ block: "start" }); }}>
                    <code>{problem.at}</code>
                  </a>{" "}
                  {problem.message}
                </li>
              );
            })}
          </ul>
        </div>
      )}

      <CheckpointList projectId={project.id} checkpoints={shown.checkpoints} editable={isDraft} features={features} onChanged={() => void refresh()} />

      {appending && (
        <form
          className={styles.editing}
          aria-label={t.append}
          onSubmit={(event) => {
            event.preventDefault();
            append.mutate(appending, {
              onSuccess: (result) => {
                if (result.id) {
                  setAppending(null);
                  void refresh();
                } else setAppendProblems(result.problems);
              },
            });
          }}
        >
          <CheckpointEditor value={appending} onChange={setAppending} features={features} earlier={shown.checkpoints.map((cp) => cp.code)} />
          {appendProblems.length > 0 && (
            <ul className={styles.problems}>
              {appendProblems.map((p, i) => (
                <li key={i}>
                  <code>{p.at}</code> {p.message}
                </li>
              ))}
            </ul>
          )}
          <p className={styles.error}>{failure(append.error) ?? ""}</p>
          <div className={styles.actions}>
            <button type="submit" className="btn btn-primary" disabled={append.isPending}>
              {t.save}
            </button>
            <button type="button" className="btn btn-quiet" onClick={() => setAppending(null)}>
              {t.cancel}
            </button>
          </div>
        </form>
      )}

      {!isDraft && <ProjectSettings project={project} onSaved={() => setNotice(t.settings.saved)} />}

      {approving && (
        <ApproveRoadmapDialog
          project={project}
          proyekName={view.prd?.proyekName ?? project.name}
          onClose={() => setApproving(false)}
          onApproved={() => {
            setApproving(false);
            setNotice(t.approval.approved);
            void refresh();
          }}
        />
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

/** The approved project's link, folder, and Claude's access, changeable later. */
function ProjectSettings({ project, onSaved }: { project: StudioProject; onSaved: () => void }) {
  const api = useApi();
  const name = useId();
  const client = useQueryClient();
  const refresh = () => client.invalidateQueries({ queryKey: keys.studio });
  const [changing, setChanging] = useState(false);
  const setPath = useStudioAct((path: string) => api.setRepoPath(project.id, path));
  const setAccess = useStudioAct((access: StudioProject["claudeAccess"]) => api.setClaudeAccess(project.id, access));
  const s = t.settings;
  return (
    <section className={own.settings} aria-labelledby={`${name}-h`}>
      <h3 id={`${name}-h`}>{s.title}</h3>
      <dl>
        <dt>{s.hubProject}</dt>
        <dd>{project.projectName ?? t.none}</dd>
        <dt>{s.folder}</dt>
        <dd>
          <code className={own.path}>{project.repoPath ?? t.none}</code>{" "}
          {!changing && (
            <button type="button" className="btn btn-quiet" onClick={() => setChanging(true)}>
              {s.changeFolder}
            </button>
          )}
        </dd>
      </dl>
      {changing && (
        <FolderChooser
          folder={null}
          onFolder={(folder) => {
            if (folder?.exists) {
              setPath.mutate(folder.path, { onSuccess: () => { setChanging(false); onSaved(); void refresh(); } });
            }
          }}
        />
      )}
      <p className={styles.error}>{failure(setPath.error ?? setAccess.error) ?? ""}</p>
      <fieldset className={styles.group}>
        <legend>{s.access}</legend>
        <AccessChoice
          value={project.claudeAccess}
          name={`${name}-access`}
          onChange={(access) => setAccess.mutate(access, { onSuccess: () => { onSaved(); void refresh(); } })}
        />
      </fieldset>
    </section>
  );
}

function CheckpointList({ projectId, checkpoints, editable, features, onChanged }: { projectId: string; checkpoints: Checkpoint[]; editable: boolean; features: Feature[]; onChanged: () => void }) {
  const api = useApi();
  const list = useRef<HTMLOListElement>(null);
  const [drag, setDrag] = useState<{ id: string; over: number } | null>(null);
  const [said, setSaid] = useState("");
  const move = useStudioAct(({ id: cpId, to }: { id: string; to: number }) => api.moveRoadmapCheckpoint(cpId, to));

  const moveTo = (cp: Checkpoint, to: number) => {
    if (to < 1 || to > checkpoints.length || to === cp.position) return;
    move.mutate({ id: cp.id, to }, { onSuccess: () => { setSaid(t.moved(cp.code, to)); onChanged(); } });
  };
  const overIndex = (y: number) => {
    const rows = [...(list.current?.querySelectorAll<HTMLElement>("[data-checkpoint]") ?? [])];
    const index = rows.findIndex((row) => {
      const box = row.getBoundingClientRect();
      return y < box.top + box.height / 2;
    });
    return index < 0 ? rows.length : index;
  };
  const startDrag = (event: PointerEvent<HTMLButtonElement>, cp: Checkpoint) => {
    if (event.button !== 0) return;
    event.preventDefault();
    event.currentTarget.setPointerCapture(event.pointerId);
    setDrag({ id: cp.id, over: cp.position - 1 });
  };
  const moveDrag = (event: PointerEvent<HTMLButtonElement>) => {
    if (!drag) return;
    const over = overIndex(event.clientY);
    if (over !== drag.over) setDrag({ ...drag, over });
  };
  const endDrag = (event: PointerEvent<HTMLButtonElement>, cp: Checkpoint) => {
    if (!drag) return;
    const over = overIndex(event.clientY);
    setDrag(null);
    // Dropping above row `over`: past the dragged row itself, the target shifts by one.
    const to = over > cp.position - 1 ? over : over + 1;
    moveTo(cp, to);
  };

  return (
    <>
      <p className="sr-only" role="status">
        {said}
      </p>
      <ol ref={list} className={own.list} aria-label={t.title}>
        {checkpoints.map((cp, index) => (
          <CheckpointRow
            key={cp.id}
            projectId={projectId}
            cp={cp}
            index={index}
            count={checkpoints.length}
            earlier={checkpoints.slice(0, index).map((c) => c.code)}
            editable={editable}
            features={features}
            dragging={drag?.id === cp.id}
            insertBefore={drag !== null && drag.over === index && drag.id !== cp.id}
            onMove={(to) => moveTo(cp, to)}
            onChanged={onChanged}
            grip={
              editable ? (
                <button
                  type="button"
                  className={own.grip}
                  aria-label={t.drag(cp.code)}
                  tabIndex={-1}
                  onPointerDown={(event) => startDrag(event, cp)}
                  onPointerMove={moveDrag}
                  onPointerUp={(event) => endDrag(event, cp)}
                  onPointerCancel={() => setDrag(null)}
                >
                  <GripVertical size={14} aria-hidden="true" />
                </button>
              ) : null
            }
          />
        ))}
        {drag && drag.over === checkpoints.length && <li className={own.insertEnd} aria-hidden="true" />}
      </ol>
    </>
  );
}

function CheckpointRow({
  projectId,
  cp,
  index,
  count,
  earlier,
  editable,
  features,
  dragging,
  insertBefore,
  grip,
  onMove,
  onChanged,
}: {
  projectId: string;
  cp: Checkpoint;
  index: number;
  count: number;
  earlier: string[];
  editable: boolean;
  features: Feature[];
  dragging: boolean;
  insertBefore: boolean;
  grip: React.ReactNode;
  onMove: (to: number) => void;
  onChanged: () => void;
}) {
  const api = useApi();
  const details = useId();
  const [open, setOpen] = useState(false);
  const [editing, setEditing] = useState<PlanCheckpoint | null>(null);
  const [removing, setRemoving] = useState(false);
  const update = useStudioAct((value: PlanCheckpoint) => api.updateRoadmapCheckpoint(cp.id, value));
  const remove = useStudioAct(() => api.removeRoadmapCheckpoint(cp.id));
  const add = useStudioAct(() => api.addRoadmapCheckpoint(projectId, cp.id));
  const featureName = (fid: string) => features.find((f) => f.id === fid)?.name;

  return (
    <li id={`cp-${cp.code}`} data-checkpoint={cp.id} className={`${own.row} ${dragging ? own.dragging : ""}`}>
      {insertBefore && <span className={own.insert} aria-hidden="true" />}
      <div className={own.rowHead}>
        {grip}
        <code className={own.code}>{cp.code}</code>
        <div className={own.rowMain}>
          <h3 className={own.cpTitle}>{cp.title}</h3>
          {cp.goal && <p className={own.goal}>{cp.goal}</p>}
          <p className={own.meta}>
            <StatusMark status={cp.status} />
            <span>{t.difficulty[cp.difficulty]}</span>
            {cp.features.length > 0 && (
              <span>
                {cp.features.map((f) => (
                  <code key={f} className={own.mono} title={featureName(f)}>
                    {f}
                  </code>
                ))}
              </span>
            )}
          </p>
        </div>
      </div>

      <div className={styles.actions}>
        <button type="button" className="btn btn-quiet" aria-expanded={open} aria-controls={open && editing === null ? details : undefined} onClick={() => setOpen((v) => !v)}>
          {open ? t.hideDetails : t.details}
        </button>
        {editable && editing === null && (
          <>
            <button type="button" className="btn btn-quiet" onClick={() => { setEditing(ownDoD(planOf(cp))); setOpen(false); }}>
              {t.change}
            </button>
            <button type="button" className="btn btn-quiet" aria-label={t.moveUp(cp.code)} disabled={index === 0} onClick={() => onMove(index)}>
              <ChevronUp size={16} aria-hidden="true" />
            </button>
            <button type="button" className="btn btn-quiet" aria-label={t.moveDown(cp.code)} disabled={index === count - 1} onClick={() => onMove(index + 2)}>
              <ChevronDown size={16} aria-hidden="true" />
            </button>
            <button type="button" className="btn btn-quiet" aria-label={t.addAfterLabel(cp.code)} onClick={() => add.mutate(undefined, { onSuccess: onChanged })}>
              {t.addAfter}
            </button>
            <button type="button" className="btn btn-quiet" onClick={() => setRemoving(true)}>
              {t.remove}
            </button>
          </>
        )}
      </div>

      {removing && (
        <Confirm
          text={t.removeAsk(cp.code)}
          yes={t.remove}
          onYes={() => { setRemoving(false); remove.mutate(undefined, { onSuccess: onChanged }); }}
          onNo={() => setRemoving(false)}
        />
      )}

      {editing !== null ? (
        <form
          className={styles.editing}
          aria-label={`${t.change} ${cp.code}`}
          onSubmit={(event) => {
            event.preventDefault();
            update.mutate(editing, { onSuccess: () => { setEditing(null); onChanged(); } });
          }}
        >
          <CheckpointEditor value={editing} onChange={setEditing} features={features} earlier={earlier} />
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
        open && (
          <div id={details} className={own.details}>
            {cp.description && <p>{cp.description}</p>}
            <Detail title={t.fields.tasks ?? ""}>
              <ul className={own.tasks}>
                {cp.tasks
                  .filter((task) => task.parentId === null)
                  .map((task) => (
                    <li key={task.id}>
                      <code className={own.mono}>{task.code}</code> {task.title}
                      {task.featureId && <span className={styles.muted}> · {task.featureId}</span>}
                      {task.detail && <span className={styles.muted}> · {task.detail}</span>}
                      {cp.tasks.some((s) => s.parentId === task.id) && (
                        <ul>
                          {cp.tasks
                            .filter((s) => s.parentId === task.id)
                            .map((s) => (
                              <li key={s.id}>{s.title}</li>
                            ))}
                        </ul>
                      )}
                    </li>
                  ))}
              </ul>
            </Detail>
            {cp.dependsOn.length > 0 && <Detail title={t.fields.dependsOn ?? ""}>{cp.dependsOn.join(", ")}</Detail>}
            <Items title={t.fields.acceptance ?? ""} items={cp.acceptance} />
            <Detail title={t.fields.requiredTests ?? ""}>
              <ul>
                {cp.requiredTests.map((test, i) => (
                  <li key={i}>
                    {test.name} <span className={styles.muted}>({t.kinds[test.kind]})</span> <code className={own.mono}>{test.how}</code>
                  </li>
                ))}
              </ul>
            </Detail>
            <Items title={t.fields.risks ?? ""} items={cp.risks} />
            <Items title={t.fields.deliverables ?? ""} items={cp.deliverables} />
            <Items title={t.fields.doneWhen ?? ""} items={cp.doneWhen} />
          </div>
        )
      )}
    </li>
  );
}

function Detail({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className={styles.para}>
      <h4>{title}</h4>
      {children}
    </div>
  );
}

function Items({ title, items }: { title: string; items: string[] }) {
  if (items.length === 0) return null;
  return (
    <Detail title={title}>
      <ul>
        {items.map((item, i) => (
          <li key={i}>{item}</li>
        ))}
      </ul>
    </Detail>
  );
}
