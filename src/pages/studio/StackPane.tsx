// Stage 2, the tech stack (docs/studio.md, "Revision: three stages", R3). Two ways in: pick it
// yourself, or have Gemini recommend it (from Ringkasan and the developer profile). Either way the
// result is seven layers the owner can change one by one: typing a choice, or taking an
// alternative with one click. A choice of the owner's over Gemini's keeps Gemini's as an
// alternative, so nothing is lost. Approval shows the whole stack once, then the PRD is written.

import { useQueryClient } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { useId, useImperativeHandle, useState } from "react";
import type { Ref } from "react";

import { Dialog } from "../../components/Dialog";
import { useApi } from "../../data/api";
import type { StackEvent } from "../../data/api";
import { toHubError } from "../../data/backend";
import type { HubError } from "../../data/backend";
import type { StackBody } from "../../data/generated/StackBody";
import type { StackLayer } from "../../data/generated/StackLayer";
import type { StackLayerKind } from "../../data/generated/StackLayerKind";
import type { StackMode } from "../../data/generated/StackMode";
import type { StudioProject } from "../../data/generated/StudioProject";
import type { StudioStack } from "../../data/generated/StudioStack";
import { keys, useSettings, useStudioAct, useStudioStack } from "../../data/queries";
import { id } from "../../strings/id";
import { useNow } from "../../time/clock";
import { relativeDate } from "../../time/format";
import type { Approver } from "./results";
import { STEP_OF } from "./steps";
import styles from "./stack.module.css";

const t = id.studio.stack;
const LAYERS: StackLayerKind[] = ["platform", "frontend", "backend", "database", "auth", "hosting", "extras"];

/** Common choices per layer, offered as the owner types (any text is allowed). */
const SUGGEST: Record<StackLayerKind, string[]> = {
  platform: ["Web", "Web (PWA)", "Android", "Desktop (Windows)", "Bot Discord", "Bot Telegram"],
  frontend: ["HTML, CSS, JavaScript", "React + Vite", "Next.js", "Vue", "Svelte", "Flutter", "Tauri"],
  backend: ["Laravel", "Express (Node.js)", "FastAPI (Python)", "Django", "Supabase", "Firebase"],
  database: ["MySQL", "PostgreSQL", "SQLite", "MongoDB", "Firestore"],
  auth: ["Supabase Auth", "Firebase Auth", "Laravel Breeze", "Login Google (OAuth)"],
  hosting: ["Vercel", "Netlify", "Railway", "VPS", "Hosting kampus atau sekolah", "Lokal saja"],
  extras: ["Google Maps", "Leaflet + OpenStreetMap", "Notifikasi push", "Penyimpanan file", "Email"],
};

function empty(): StackBody {
  return {
    layers: LAYERS.map((layer) => ({ layer, choice: "", why: "", fit: "", ease: "", cost: "", alternatives: [], by: "owner" })),
    summary: "",
  };
}

function problem(error: HubError | null): string | null {
  if (!error) return null;
  return t.errors[error.view.field ?? ""] ?? t.errors[error.view.reason ?? ""] ?? (id.settings.errors.fallback as string);
}

/** The owner's own choice for a layer: Gemini's earlier one, if any, stays as an alternative. */
export function chooseFor(layer: StackLayer, choice: string): StackLayer {
  const text = choice.trim();
  if (text === layer.choice) return layer;
  const earlier = layer.choice && !layer.alternatives.some((a) => a.name === layer.choice) ? [{ name: layer.choice, when: t.previous }] : [];
  const alternatives = [...earlier, ...layer.alternatives.filter((a) => a.name !== text)].slice(0, 3);
  return { ...layer, choice: text, why: "", fit: "", ease: "", cost: "", alternatives, by: "owner" };
}

export interface StackWriting {
  running: boolean;
  repair: boolean;
  notice: string | null;
}

export function StackPane({
  project,
  hasKey,
  onWritePrd,
  approver,
}: {
  project: StudioProject;
  hasKey: boolean;
  /** Approved, and the PRD should be written now. */
  onWritePrd: () => void;
  /** U4b: Setujui on the stack card in the conversation opens this pane's own approval. */
  approver?: Ref<Approver>;
}) {
  const api = useApi();
  const client = useQueryClient();
  const stack = useStudioStack(project.id);
  const settings = useSettings();
  const now = useNow();
  const [writing, setWriting] = useState<StackWriting>({ running: false, repair: false, notice: null });
  const [ask, setAsk] = useState("");
  const [approving, setApproving] = useState(false);
  const save = useStudioAct(({ body, mode }: { body: StackBody; mode: StackMode }) => api.saveStudioStack(project.id, body, mode));
  const approve = useStudioAct(() => api.approveStudioStack(project.id));
  const refresh = () => client.invalidateQueries({ queryKey: keys.studio });
  const current = stack.data ?? null;
  const beforePrd = STEP_OF[project.stage] < 3;
  // U4b: from the card, the same sheet as the button, listing the whole stack before approving.
  useImperativeHandle(approver, () => ({
    approve: () => {
      if (current?.status === "draft" && current.body.layers.every((layer) => layer.choice)) setApproving(true);
    },
  }));
  const profileEmpty = settings.data ? !settings.data.devKnows.trim() && !settings.data.devDeploy.trim() : false;

  const recommend = async (words: string) => {
    setWriting({ running: true, repair: false, notice: null });
    // Filled by the events; read once the request ends.
    const result: { notice: string | null; done: boolean } = { notice: null, done: false };
    const onEvent = (event: StackEvent) => {
      if (event.kind === "step") setWriting((w) => ({ ...w, repair: event.repair }));
      else if (event.state === "complete") Object.assign(result, { notice: t.written, done: true });
      else if (event.state === "stopped") result.notice = t.stopped;
      else result.notice = t.failed(event.errorKind === "bad_response" ? t.badResponse : (id.studio.chat.failed[event.errorKind ?? ""] ?? id.studio.chat.failed.fallback ?? ""));
    };
    try {
      await api.recommendStudioStack(project.id, words, onEvent);
      if (result.done) setAsk("");
    } catch (error) {
      const reason = toHubError(error).view.reason;
      result.notice = t.failed(reason === "no_key" ? id.studio.chat.noKey : (id.studio.chat.failed[reason ?? ""] ?? id.studio.chat.failed.fallback ?? ""));
    } finally {
      await refresh();
      setWriting({ running: false, repair: false, notice: result.notice });
    }
  };

  const keep = (body: StackBody) => save.mutate({ body, mode: current?.mode ?? "own" }, { onSuccess: () => void refresh() });

  return (
    <section className={styles.stack} aria-labelledby="stack-title">
      <header className={styles.head}>
        <h2 className={styles.title} id="stack-title">
          {t.title}
          {current && (
            <span className={styles.status} data-status={current.status}>
              {current.status === "approved" && current.approvedAt ? t.approvedOn(relativeDate(current.approvedAt.slice(0, 10), now)) : t.statuses[current.status]}
            </span>
          )}
        </h2>
        <p className={styles.intro}>{t.intro}</p>
      </header>

      {!current && !writing.running && (
        <div className={styles.ways}>
          <button type="button" className={styles.way} onClick={() => keep(empty())}>
            <strong>{t.own}</strong>
            <span>{t.ownHint}</span>
          </button>
          <button type="button" className={styles.way} disabled={!hasKey} onClick={() => void recommend("")}>
            <strong>{t.agent}</strong>
            <span>{hasKey ? t.agentHint : id.studio.chat.noKey}</span>
          </button>
        </div>
      )}
      {!current && profileEmpty && (
        <p className={styles.note}>
          {t.noProfile} <Link to="/pengaturan/$bagian" params={{ bagian: "ai" }}>{t.openProfile}</Link>
        </p>
      )}

      {writing.running && (
        <div className={styles.working} role="status">
          <span>{writing.repair ? t.repairing : t.working}</span>
          <button type="button" className="btn btn-quiet" onClick={() => void api.stopStudioReply(project.id)}>
            {t.stop}
          </button>
        </div>
      )}
      {writing.notice && !writing.running && (
        <p className={styles.notice} role="status">
          {writing.notice}
        </p>
      )}
      {problem(save.error ?? approve.error) && (
        <p className={styles.error} role="alert">
          {problem(save.error ?? approve.error)}
        </p>
      )}

      {current && (
        <Layers
          stack={current}
          disabled={writing.running}
          onChange={(layer) => keep({ ...current.body, layers: current.body.layers.map((l) => (l.layer === layer.layer ? layer : l)) })}
        />
      )}

      {current && (
        <div className={styles.foot}>
          {current.status === "approved" ? (
            <>
              <p className={styles.note}>{t.approvedNote}</p>
              {beforePrd && (
                <button type="button" className="btn btn-primary" onClick={onWritePrd}>
                  {t.writePrd}
                </button>
              )}
            </>
          ) : (
            <Approve stack={current} onApprove={() => setApproving(true)} />
          )}
          {hasKey && (
            <form
              className={styles.ask}
              onSubmit={(event) => {
                event.preventDefault();
                if (ask.trim()) void recommend(ask);
              }}
            >
              <label className={styles.askLabel} htmlFor="stack-ask">
                {current.mode === "gemini" ? t.askLabel : t.fromAgent}
              </label>
              <div className={styles.askRow}>
                <input
                  id="stack-ask"
                  className={styles.input}
                  value={ask}
                  maxLength={1000}
                  placeholder={t.askPlaceholder}
                  disabled={writing.running}
                  onChange={(event) => setAsk(event.target.value)}
                />
                <button type="submit" className="btn btn-secondary" disabled={writing.running || !ask.trim()}>
                  {t.askSend}
                </button>
              </div>
            </form>
          )}
        </div>
      )}

      {approving && current && (
        <Dialog
          title={t.sheetTitle}
          description={t.sheetIntro}
          onClose={() => setApproving(false)}
          footer={
            <>
              <button type="button" className="btn btn-quiet" onClick={() => setApproving(false)}>
                {t.back}
              </button>
              <button
                type="button"
                className="btn btn-primary"
                data-autofocus
                onClick={() =>
                  approve.mutate(undefined, {
                    onSuccess: () => {
                      setApproving(false);
                      void refresh();
                      if (beforePrd) onWritePrd();
                    },
                  })
                }
              >
                {beforePrd ? t.approveAndPrd : t.approveOnly}
              </button>
            </>
          }
        >
          <dl className={styles.sheet}>
            {current.body.layers.map((layer) => (
              <div key={layer.layer} className={styles.sheetRow}>
                <dt>{t.layers[layer.layer]}</dt>
                <dd>
                  {layer.choice}
                  <span className={styles.by}>{t.by[layer.by]}</span>
                </dd>
              </div>
            ))}
          </dl>
        </Dialog>
      )}
    </section>
  );
}

function Approve({ stack, onApprove }: { stack: StudioStack; onApprove: () => void }) {
  const missing = stack.body.layers.filter((l) => !l.choice).length;
  return (
    <div className={styles.approve}>
      <button type="button" className="btn btn-primary" disabled={missing > 0} aria-describedby={missing > 0 ? "stack-missing" : undefined} onClick={onApprove}>
        {t.approve}
      </button>
      {missing > 0 && (
        <p className={styles.note} id="stack-missing">
          {t.unchosen(missing)}
        </p>
      )}
    </div>
  );
}

function Layers({ stack, disabled, onChange }: { stack: StudioStack; disabled: boolean; onChange: (layer: StackLayer) => void }) {
  return (
    <>
      {stack.body.summary && <p className={styles.summary}>{stack.body.summary}</p>}
      <ol className={styles.layers}>
        {stack.body.layers.map((layer) => (
          <Layer key={`${layer.layer}-${layer.choice}`} layer={layer} disabled={disabled} onChange={onChange} />
        ))}
      </ol>
    </>
  );
}

function Layer({ layer, disabled, onChange }: { layer: StackLayer; disabled: boolean; onChange: (layer: StackLayer) => void }) {
  const base = useId();
  const [draft, setDraft] = useState(layer.choice);
  const name = t.layers[layer.layer] ?? layer.layer;
  const notes = [
    [t.why, layer.why],
    [t.fit, layer.fit],
    [t.ease, layer.ease],
    [t.cost, layer.cost],
  ].filter(([, text]) => text);
  const commit = (value: string) => {
    if (value.trim() !== layer.choice) onChange(chooseFor(layer, value));
  };

  return (
    <li className={styles.layer} data-empty={!layer.choice || undefined}>
      <div className={styles.layerRow}>
        <label className={styles.layerName} htmlFor={`${base}-choice`}>
          {name}
        </label>
        <input
          id={`${base}-choice`}
          className={styles.choice}
          list={`${base}-list`}
          value={draft}
          maxLength={120}
          placeholder={t.choicePlaceholder}
          aria-label={t.choiceLabel(name)}
          disabled={disabled}
          onChange={(event) => setDraft(event.target.value)}
          onBlur={() => commit(draft)}
          onKeyDown={(event) => {
            if (event.key === "Enter") {
              event.preventDefault();
              event.currentTarget.blur();
            }
            if (event.key === "Escape") setDraft(layer.choice);
          }}
        />
        <datalist id={`${base}-list`}>
          {[...SUGGEST[layer.layer], t.none].map((option) => (
            <option key={option} value={option} />
          ))}
        </datalist>
        {layer.choice && <span className={styles.by}>{t.by[layer.by]}</span>}
      </div>
      {notes.length > 0 && (
        <details className={styles.notes}>
          <summary>{t.details}</summary>
          <dl>
            {notes.map(([label, text]) => (
              <div key={label}>
                <dt>{label}</dt>
                <dd>{text}</dd>
              </div>
            ))}
          </dl>
        </details>
      )}
      {layer.alternatives.length > 0 && (
        <div className={styles.alternatives}>
          <span className={styles.altLabel}>{t.alternatives}</span>
          {layer.alternatives.map((alternative) => (
            <button
              key={alternative.name}
              type="button"
              className={styles.alt}
              disabled={disabled}
              title={alternative.when || undefined}
              aria-label={t.use(alternative.name)}
              onClick={() => onChange(chooseFor(layer, alternative.name))}
            >
              {alternative.name}
            </button>
          ))}
        </div>
      )}
    </li>
  );
}
