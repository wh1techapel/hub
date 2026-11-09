// One Studio project as a workspace that fills the window (docs/studio.md, "The workspace", U4):
// the conversation from the idea to the build, and beside it a canvas with the document of the
// stage shown, the way Gemini's Canvas sits beside its chat. The header holds the name, the three
// stages (the stepper: which stage's document the canvas shows), and the canvas's button. Stage 1's
// document is Ringkasan, stage 2's the tech stack, stage 3's the PRD, the checkpoint plan, and
// Bangun. A wide document folds Studio's history into the rail meanwhile. In a narrow workspace
// the canvas is a sheet over the conversation.

import { useQueryClient } from "@tanstack/react-query";
import { useNavigate, useParams } from "@tanstack/react-router";
import { Check, PanelRightClose, PanelRightOpen, Trash2, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";

import { useWideCanvas } from "../../app/studioCanvas";
import { useApi } from "../../data/api";
import type { PrdEvent, RoadmapEvent } from "../../data/api";
import { toHubError } from "../../data/backend";
import type { LedgerArea } from "../../data/generated/LedgerArea";
import type { StudioStage } from "../../data/generated/StudioStage";
import { failedForGood, keys, useGeminiStatus, useStudioAct, useStudioLedger, useStudioPrd, useStudioProject, useStudioRoadmap, useStudioStack } from "../../data/queries";
import { useGlide } from "../../motion/motion";
import { id } from "../../strings/id";
import { Conversation } from "./Conversation";
import { areaAnchor, Ledger } from "./Ledger";
import { PantauPane } from "./PantauPane";
import { PrdPane } from "./PrdPane";
import type { Writing } from "./PrdPane";
import { Review } from "./Review";
import { RoadmapPane } from "./RoadmapPane";
import type { RoadmapWriting } from "./RoadmapPane";
import { resultsOf } from "./results";
import type { Approver, ResultKind } from "./results";
import { StackPane } from "./StackPane";
import { progressOf, STEPS } from "./steps";
import type { Step } from "./steps";
import styles from "./project.module.css";

const t = id.studio;

type Doc = "prd" | "roadmap" | "pantau";

const DOC_OF: Record<StudioStage, Doc> = { explore: "prd", clarify: "prd", prd: "prd", roadmap: "roadmap", build: "pantau" };
const DOCS: Doc[] = ["prd", "roadmap", "pantau"];
/** Under this width the canvas becomes a sheet over the conversation. */
const NARROW = 860;

/** A ref for an element and whether it is narrower than `NARROW`, followed as the window changes.
 * A callback ref: the element appears only once the project has loaded. */
function useNarrow(): [(node: HTMLElement | null) => void, boolean] {
  const [node, setNode] = useState<HTMLElement | null>(null);
  const [narrow, setNarrow] = useState(false);
  useEffect(() => {
    if (!node || typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(([entry]) => {
      if (entry) setNarrow(entry.contentRect.width < NARROW);
    });
    observer.observe(node);
    return () => observer.disconnect();
  }, [node]);
  return [setNode, narrow];
}

export function StudioProjectPage() {
  const params: { projectId?: string } = useParams({ strict: false });
  const projectId = params.projectId ?? "";
  const project = useStudioProject(projectId);
  const gemini = useGeminiStatus();
  const ledger = useStudioLedger(projectId);
  // U4b: the documents' state, for the result cards in the conversation.
  const stackDoc = useStudioStack(projectId);
  const prdDoc = useStudioPrd(projectId);
  const roadmapDoc = useStudioRoadmap(projectId);
  // The document in the canvas gives its approval here, for a card's Setujui.
  const approver = useRef<Approver>(null);
  const api = useApi();
  const client = useQueryClient();
  const navigate = useNavigate();
  const advance = useStudioAct((to: StudioStage) => api.advanceStudioStage(projectId, to));
  const rename = useStudioAct((name: string) => api.renameStudioProject(projectId, name));
  const remove = useStudioAct(() => api.deleteStudioProject(projectId));
  const [view, setView] = useState<Step | null>(null);
  const [doc, setDoc] = useState<Doc | null>(null);
  // The owner's choice holds for the width it was made at: a change of width starts the canvas from
  // its default again (open beside the conversation, closed as a sheet).
  const [canvasPref, setCanvasPref] = useState<{ open: boolean; narrow: boolean } | null>(null);
  const [locked, setLocked] = useState<string | null>(null);
  const [seenLedger, setSeenLedger] = useState<number | null>(null);
  const [name, setName] = useState<string | null>(null);
  const [confirming, setConfirming] = useState(false);
  // The check-answers sheet before leaving stage 1 (R2).
  const [reviewing, setReviewing] = useState(false);
  const [writing, setWriting] = useState<Writing>({ running: false, step: null, notice: null });
  const [planning, setPlanning] = useState<RoadmapWriting>({ running: false, step: null, notice: null });
  const toggle = useRef<HTMLButtonElement>(null);
  const closer = useRef<HTMLButtonElement>(null);
  const [body, narrow] = useNarrow();
  // P2b: the chosen stage and section are underlined by one line that glides.
  const steps = useGlide<HTMLDivElement>('[aria-selected="true"]', 2);
  const docTabs = useGlide<HTMLDivElement>('[aria-selected="true"]', 2);

  const progress: Step = project.data ? progressOf(project.data) : 1;
  const shown: Step = view ?? progress;
  const canvasOpen = canvasPref && canvasPref.narrow === narrow ? canvasPref.open : !narrow;
  const ledgerInView = canvasOpen && shown === 1;
  // U4: a wide document (anything but Ringkasan) beside the conversation folds the history.
  useWideCanvas(Boolean(project.data) && canvasOpen && shown !== 1 && !narrow);

  // A dot on the canvas's button when Ringkasan grew while it was out of sight. What was there when
  // the page opened, or while it was in view, counts as seen.
  const count = ledger.data?.length ?? 0;
  if (ledger.isSuccess && (seenLedger === null || (ledgerInView && seenLedger !== count))) setSeenLedger(count);
  const unseen = !ledgerInView && seenLedger !== null && count > seenLedger;

  // The sheet takes focus when it opens and gives it back when it closes.
  const sheetWasOpen = useRef(false);
  useEffect(() => {
    if (!narrow) return;
    if (canvasOpen) closer.current?.focus();
    else if (sheetWasOpen.current) toggle.current?.focus();
    sheetWasOpen.current = canvasOpen;
  }, [narrow, canvasOpen]);

  // The error page only when there is nothing to show: a refresh that fails in the background (after
  // a change from Claude, say) keeps the project and the owner's draft on screen, unless it is gone.
  if (failedForGood(project)) {
    return (
      <div className={styles.page}>
        <p>{id.settings.errors.fallback as string}</p>
      </div>
    );
  }
  if (!project.data) return null;
  const current = project.data;
  // Stage 3 opens once the stack is approved (R3), or for a project already there.
  const reachable = (step: Step) => step !== 3 || progress === 3 || current.stackStatus === "approved";
  const shownDoc: Doc = doc ?? DOC_OF[current.stage];
  const refresh = () => client.invalidateQueries({ queryKey: keys.studio });
  const setCanvas = (open: boolean) => setCanvasPref({ open, narrow });
  const saveName = () => {
    if (name !== null && name.trim() && name.trim() !== current.name) rename.mutate(name, { onSuccess: () => void refresh() });
    setName(null);
  };
  // A stage in the stepper opens its document in the canvas.
  const openStep = (step: Step) => {
    if (!reachable(step)) {
      setLocked(t.stepLocked.build ?? null);
      return;
    }
    setLocked(null);
    setView(step);
    setCanvas(true);
  };
  const openDoc = (which: Doc) => {
    setView(3);
    setDoc(which);
    setCanvas(true);
  };

  const writePrd = async () => {
    openDoc("prd");
    setWriting({ running: true, step: null, notice: null, startedAt: new Date().toISOString() });
    let notice: string | null = null;
    const onEvent = (event: PrdEvent) => {
      if (event.kind === "step") setWriting((now) => ({ ...now, step: event }));
      else if (event.state === "complete") notice = t.prd.written;
      else if (event.state === "stopped") notice = t.prd.stopped;
      else notice = t.prd.failed(event.errorKind === "bad_response" ? t.prd.badResponse : (t.chat.failed[event.errorKind ?? ""] ?? t.chat.failed.fallback ?? ""));
    };
    try {
      await api.generatePrd(current.id, onEvent);
    } catch (error) {
      const { field, reason } = toHubError(error).view;
      const why = field === "stack" ? (t.stack.errors.stack ?? "") : reason === "no_key" ? t.chat.noKey : (t.chat.failed[reason ?? ""] ?? t.chat.failed.fallback ?? "");
      notice = t.prd.failed(why);
    } finally {
      await refresh();
      setWriting({ running: false, step: null, notice });
    }
  };

  const writeRoadmap = async () => {
    openDoc("roadmap");
    setPlanning({ running: true, step: null, notice: null });
    const r = t.roadmap;
    let notice: string | null = null;
    const reasonOf = (kind: string | null) =>
      kind === "bad_response" ? r.badResponse : kind === "locked" ? (r.errors.locked ?? "") : kind === "no_key" ? t.chat.noKey : (t.chat.failed[kind ?? ""] ?? t.chat.failed.fallback ?? "");
    const onEvent = (event: RoadmapEvent) => {
      if (event.kind === "step") setPlanning((now) => ({ ...now, step: event }));
      else if (event.state === "complete") notice = r.written(event.problems);
      else if (event.state === "stopped") notice = r.stopped;
      else notice = r.failed(reasonOf(event.errorKind));
    };
    try {
      await api.generateRoadmap(current.id, onEvent);
    } catch (error) {
      notice = r.failed(reasonOf(toHubError(error).view.reason));
    } finally {
      await refresh();
      setPlanning({ running: false, step: null, notice });
    }
  };

  // A note in Ringkasan points to its message: the conversation is always there, so only a sheet
  // in the way closes.
  const showMessage = (messageId: string) => {
    if (narrow) setCanvas(false);
    requestAnimationFrame(() => document.getElementById(`message-${messageId}`)?.scrollIntoView({ block: "center" }));
  };

  // "Ubah" in the check-answers sheet: Ringkasan opens at that part, to be corrected there.
  const changePart = (area: LedgerArea) => {
    setReviewing(false);
    setView(1);
    setCanvas(true);
    requestAnimationFrame(() => {
      const section = document.getElementById(areaAnchor(area));
      section?.scrollIntoView({ block: "start" });
      section?.focus();
    });
  };

  // U4b: Buka opens the result's document; Setujui opens it and starts its own approval, with its
  // own confirmation (the document mounts with this click, so the approval waits one frame).
  const openResult = (kind: ResultKind) => {
    if (kind === "stack") openStep(2);
    else openDoc(kind === "build" ? "pantau" : kind);
  };
  const approveResult = (kind: ResultKind) => {
    openResult(kind);
    requestAnimationFrame(() => approver.current?.approve());
  };
  // U4b: approving the PRD writes the checkpoint plan at once, when there is none yet; a plan that
  // exists is never replaced by itself.
  const afterPrdApproved = () => {
    if (!roadmapDoc.data?.draft && !roadmapDoc.data?.approved) void writeRoadmap();
  };
  const results = resultsOf(current, stackDoc.data, prdDoc.data, roadmapDoc.data);

  const openQuestions = (ledger.data ?? []).filter((item) => item.kind === "question" && item.status === "active").length;
  const canvasLabel = canvasOpen ? t.canvas.hide : t.canvas.show;

  return (
    <div className={styles.page}>
      <header className={styles.header}>
        <div className={styles.titleRow}>
          <label className="sr-only" htmlFor="studio-name">
            {t.nameLabel}
          </label>
          <input
            id="studio-name"
            className={`display ${styles.name}`}
            value={name ?? current.name}
            maxLength={120}
            onChange={(event) => setName(event.target.value)}
            onBlur={saveName}
            onKeyDown={(event) => {
              if (event.key === "Enter") event.currentTarget.blur();
              if (event.key === "Escape") {
                setName(null);
                event.currentTarget.blur();
              }
            }}
          />
          <h1 className="sr-only">{current.name}</h1>
          <div ref={steps} className={styles.steps} role="tablist" aria-label={t.stages}>
            {STEPS.map(({ step, key }) => {
              // Stage 2 has no record yet (R3): a project past it skipped it, it was not done.
              const state = step < progress ? "done" : step === progress ? "current" : "later";
              const label = t.steps[key] ?? key;
              return (
                <button
                  key={key}
                  type="button"
                  role="tab"
                  id={`studio-step-${step}`}
                  aria-selected={shown === step}
                  aria-controls="studio-canvas"
                  aria-current={state === "current" ? "step" : undefined}
                  aria-disabled={!reachable(step) || undefined}
                  className={styles.step}
                  data-state={state}
                  onClick={() => openStep(step)}
                >
                  <span className={styles.stepMark} aria-hidden="true">
                    {state === "done" ? <Check size={13} strokeWidth={2.4} /> : step}
                  </span>
                  <span>{label}</span>
                  <span className="sr-only">, {t.stepState[state]}</span>
                </button>
              );
            })}
          </div>
          <div className={styles.headActions}>
            <button type="button" className={`btn btn-quiet ${styles.iconButton}`} aria-label={t.remove} title={t.remove} aria-expanded={confirming} onClick={() => setConfirming(true)}>
              <Trash2 size={17} strokeWidth={1.6} aria-hidden="true" />
            </button>
            <button
              ref={toggle}
              type="button"
              className={`btn btn-quiet ${styles.iconButton}`}
              aria-expanded={canvasOpen}
              aria-controls="studio-canvas"
              aria-label={canvasLabel}
              title={canvasLabel}
              onClick={() => setCanvas(!canvasOpen)}
            >
              {canvasOpen ? <PanelRightClose size={18} strokeWidth={1.6} aria-hidden="true" /> : <PanelRightOpen size={18} strokeWidth={1.6} aria-hidden="true" />}
              {unseen && <i className={styles.dot} aria-label={t.newThere} />}
            </button>
          </div>
        </div>
        {confirming && (
          <div className={styles.confirm} role="group" aria-labelledby="studio-remove-ask">
            <p id="studio-remove-ask">{t.removeAsk}</p>
            <button type="button" className="btn btn-danger" onClick={() => remove.mutate(undefined, { onSuccess: () => void navigate({ to: "/studio" }) })}>
              {t.removeConfirm}
            </button>
            <button type="button" className="btn btn-quiet" onClick={() => setConfirming(false)}>
              {t.cancel}
            </button>
          </div>
        )}
        {locked && (
          <p className={styles.locked} role="status">
            {locked}
          </p>
        )}
      </header>

      <div
        ref={body}
        className={styles.body}
        data-canvas={canvasOpen ? "open" : "closed"}
        data-doc={shown === 1 ? "narrow" : "wide"}
        data-narrow={narrow || undefined}
        onKeyDown={(event) => {
          if (event.key === "Escape" && !event.defaultPrevented && narrow && canvasOpen && (event.target as HTMLElement).closest("[data-canvas-part]")) {
            event.preventDefault();
            setCanvas(false);
          }
        }}
      >
        <div className={styles.chatHost}>
          <Conversation
            project={current}
            hasKey={gemini.data?.hasKey ?? false}
            onAdvance={(to) => (to === "prd" ? setReviewing(true) : advance.mutate(to, { onSuccess: () => void refresh() }))}
            results={results}
            onOpenResult={openResult}
            onApproveResult={approveResult}
          />
        </div>

        <section id="studio-canvas" role="tabpanel" aria-labelledby={`studio-step-${shown}`} className={styles.canvas} data-canvas-part>
          {(shown === 3 || narrow) && (
            <div className={styles.canvasHead}>
              {shown === 3 && (
                <div ref={docTabs} className={styles.docTabs} role="tablist" aria-label={t.docTabs}>
                  {DOCS.map((which) => (
                    <button key={which} type="button" role="tab" aria-selected={shownDoc === which} className={styles.docTab} onClick={() => openDoc(which)}>
                      {t.panes[which]}
                    </button>
                  ))}
                </div>
              )}
              {narrow && (
                <button ref={closer} type="button" className={`btn btn-quiet ${styles.close}`} aria-label={t.canvas.close} title={t.canvas.close} onClick={() => setCanvas(false)}>
                  <X size={18} strokeWidth={1.6} aria-hidden="true" />
                </button>
              )}
            </div>
          )}
          <div className={styles.canvasScroll}>
            {shown === 1 ? (
              <Ledger projectId={current.id} onShowMessage={showMessage} onReview={() => setReviewing(true)} />
            ) : shown === 2 ? (
              <StackPane project={current} hasKey={gemini.data?.hasKey ?? false} onWritePrd={() => void writePrd()} approver={approver} />
            ) : shownDoc === "pantau" ? (
              <PantauPane project={current} />
            ) : shownDoc === "roadmap" ? (
              <RoadmapPane
                project={current}
                writing={planning}
                hasKey={gemini.data?.hasKey ?? false}
                onGenerate={() => void writeRoadmap()}
                onStop={() => void api.stopStudioReply(current.id)}
                approver={approver}
              />
            ) : (
              <PrdPane
                projectId={current.id}
                writing={writing}
                openQuestions={openQuestions}
                hasKey={gemini.data?.hasKey ?? false}
                onGenerate={() => void writePrd()}
                onStop={() => void api.stopStudioReply(current.id)}
                approver={approver}
                onApproved={afterPrdApproved}
              />
            )}
          </div>
        </section>
      </div>

      {reviewing && (
        <Review
          projectId={current.id}
          onClose={() => setReviewing(false)}
          onChange={changePart}
          onGo={() => {
            setReviewing(false);
            setLocked(null);
            setView(2);
            setCanvas(true);
          }}
        />
      )}
    </div>
  );
}
