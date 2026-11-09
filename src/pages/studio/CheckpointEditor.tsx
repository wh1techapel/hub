// One checkpoint's fields, edited in place (docs/studio.md, "Roadmap"): title, goal, the PRD
// features it covers, its tasks with their small steps, dependencies on earlier checkpoints,
// difficulty, criteria, required tests, deliverables, and the Definition of Done (the two lines
// Hub always keeps are not shown here; the hint says so).

import { useId } from "react";

import { Segmented } from "../../components/Fields";
import type { CheckpointDifficulty } from "../../data/generated/CheckpointDifficulty";
import type { Feature } from "../../data/generated/Feature";
import type { PlanCheckpoint } from "../../data/generated/PlanCheckpoint";
import type { TestKind } from "../../data/generated/TestKind";
import { DOD_REPORT, DOD_TESTS } from "./roadmapRules";
import { id } from "../../strings/id";
import styles from "./prd.module.css";
import own from "./roadmap.module.css";

const t = id.studio.roadmap;
const KINDS: TestKind[] = ["unit", "integration", "e2e", "manual", "build", "lint"];
const LEVELS: CheckpointDifficulty[] = ["low", "medium", "high"];

const lines = (text: string) => text.split("\n").map((l) => l.trim()).filter(Boolean);

/** The DoD as the owner edits it: without the two lines Hub keeps anyway. */
export function ownDoD(plan: PlanCheckpoint): PlanCheckpoint {
  return { ...plan, doneWhen: plan.doneWhen.filter((d) => d !== DOD_TESTS && d !== DOD_REPORT) };
}

function Lines({ label, value, onChange }: { label: string; value: string[]; onChange: (next: string[]) => void }) {
  const field = useId();
  return (
    <div className={styles.field}>
      <label htmlFor={field}>{label}</label>
      <textarea
        id={field}
        className={styles.input}
        rows={Math.max(2, value.length + 1)}
        defaultValue={value.join("\n")}
        onChange={(event) => onChange(lines(event.target.value))}
      />
      <span className={styles.hint}>{t.onePerLine}</span>
    </div>
  );
}

export function CheckpointEditor({
  value,
  onChange,
  features,
  earlier,
}: {
  value: PlanCheckpoint;
  onChange: (next: PlanCheckpoint) => void;
  /** The approved PRD's features, except Won't. */
  features: Feature[];
  /** Codes of the checkpoints before this one. */
  earlier: string[];
}) {
  const base = useId();
  const set = (patch: Partial<PlanCheckpoint>) => onChange({ ...value, ...patch });
  const toggle = (list: string[], item: string, on: boolean) => (on ? [...list, item] : list.filter((x) => x !== item));

  return (
    <div className={styles.editor}>
      <div className={styles.field}>
        <label htmlFor={`${base}-title`}>{t.fields.title}</label>
        <input id={`${base}-title`} className={styles.input} value={value.title} maxLength={200} onChange={(event) => set({ title: event.target.value })} />
      </div>
      <div className={styles.field}>
        <label htmlFor={`${base}-goal`}>{t.fields.goal}</label>
        <input id={`${base}-goal`} className={styles.input} value={value.goal} maxLength={1000} onChange={(event) => set({ goal: event.target.value })} />
      </div>
      <div className={styles.field}>
        <label htmlFor={`${base}-desc`}>{t.fields.description}</label>
        <textarea id={`${base}-desc`} className={styles.input} rows={3} value={value.description} maxLength={4000} onChange={(event) => set({ description: event.target.value })} />
      </div>

      <fieldset className={styles.group}>
        <legend>{t.fields.features}</legend>
        {features.length === 0 && <p className={styles.hint}>{t.none}</p>}
        {features.map((f) => (
          <label key={f.id} className={styles.check}>
            <input type="checkbox" checked={value.features.includes(f.id)} onChange={(event) => set({ features: toggle(value.features, f.id, event.target.checked) })} />
            <span>
              <code className={own.mono}>{f.id}</code> {f.name} <span className={styles.hint}>({id.studio.prd.priority[f.priority]})</span>
            </span>
          </label>
        ))}
      </fieldset>

      <fieldset className={styles.group}>
        <legend>{t.fields.dependsOn}</legend>
        {earlier.length === 0 && <p className={styles.hint}>{t.none}</p>}
        {earlier.map((code) => (
          <label key={code} className={styles.check}>
            <input type="checkbox" checked={value.dependsOn.includes(code)} onChange={(event) => set({ dependsOn: toggle(value.dependsOn, code, event.target.checked) })} />
            <code className={own.mono}>{code}</code>
          </label>
        ))}
      </fieldset>

      <Segmented
        label={t.fields.difficulty ?? ""}
        options={LEVELS.map((level) => [level, t.difficulty[level] ?? level] as const)}
        value={value.difficulty}
        onChange={(difficulty) => set({ difficulty })}
      />

      <fieldset className={styles.group}>
        <legend>{t.fields.tasks}</legend>
        {value.tasks.map((task, i) => {
          const update = (patch: Partial<(typeof value.tasks)[number]>) => set({ tasks: value.tasks.map((x, j) => (j === i ? { ...x, ...patch } : x)) });
          return (
            <div key={i} className={styles.item}>
              <div className={styles.itemHead}>
                <span>
                  {t.fields.task} {i + 1}
                </span>
                <button type="button" className="btn btn-quiet" onClick={() => set({ tasks: value.tasks.filter((_, j) => j !== i) })}>
                  {t.removeItem}
                </button>
              </div>
              <div className={styles.field}>
                <label htmlFor={`${base}-task-${i}`}>{t.fields.taskTitle}</label>
                <input id={`${base}-task-${i}`} className={styles.input} value={task.title} maxLength={300} onChange={(event) => update({ title: event.target.value })} />
              </div>
              <div className={styles.field}>
                <label htmlFor={`${base}-detail-${i}`}>{t.fields.detail}</label>
                <input id={`${base}-detail-${i}`} className={styles.input} value={task.detail} maxLength={4000} onChange={(event) => update({ detail: event.target.value })} />
              </div>
              <div className={styles.field}>
                <label htmlFor={`${base}-feature-${i}`}>{t.fields.taskFeature}</label>
                <select id={`${base}-feature-${i}`} className={styles.input} value={task.feature} onChange={(event) => update({ feature: event.target.value })}>
                  <option value="">{t.noFeature}</option>
                  {features.map((f) => (
                    <option key={f.id} value={f.id}>
                      {f.id} {f.name}
                    </option>
                  ))}
                </select>
              </div>
              <Lines label={t.fields.subtasks ?? ""} value={task.subtasks} onChange={(subtasks) => update({ subtasks })} />
            </div>
          );
        })}
        <div>
          <button type="button" className="btn btn-secondary" onClick={() => set({ tasks: [...value.tasks, { code: "", title: "", detail: "", feature: "", subtasks: [] }] })}>
            {t.addTask}
          </button>
        </div>
      </fieldset>

      <Lines label={t.fields.acceptance ?? ""} value={value.acceptance} onChange={(acceptance) => set({ acceptance })} />

      <fieldset className={styles.group}>
        <legend>{t.fields.requiredTests}</legend>
        {value.requiredTests.map((test, i) => {
          const update = (patch: Partial<(typeof value.requiredTests)[number]>) =>
            set({ requiredTests: value.requiredTests.map((x, j) => (j === i ? { ...x, ...patch } : x)) });
          return (
            <div key={i} className={`${styles.item} ${own.testRow}`}>
              <div className={styles.field}>
                <label htmlFor={`${base}-tname-${i}`}>{t.fields.testName}</label>
                <input id={`${base}-tname-${i}`} className={styles.input} value={test.name} maxLength={200} onChange={(event) => update({ name: event.target.value })} />
              </div>
              <div className={styles.field}>
                <label htmlFor={`${base}-tkind-${i}`}>{t.fields.testKind}</label>
                <select id={`${base}-tkind-${i}`} className={styles.input} value={test.kind} onChange={(event) => update({ kind: event.target.value as TestKind })}>
                  {KINDS.map((kind) => (
                    <option key={kind} value={kind}>
                      {t.kinds[kind]}
                    </option>
                  ))}
                </select>
              </div>
              <div className={styles.field}>
                <label htmlFor={`${base}-thow-${i}`}>{t.fields.testHow}</label>
                <input id={`${base}-thow-${i}`} className={`${styles.input} ${own.mono}`} value={test.how} maxLength={500} onChange={(event) => update({ how: event.target.value })} />
              </div>
              <div>
                <button type="button" className="btn btn-quiet" onClick={() => set({ requiredTests: value.requiredTests.filter((_, j) => j !== i) })}>
                  {t.removeItem}
                </button>
              </div>
            </div>
          );
        })}
        <div>
          <button type="button" className="btn btn-secondary" onClick={() => set({ requiredTests: [...value.requiredTests, { name: "", kind: "unit", how: "" }] })}>
            {t.addTest}
          </button>
        </div>
      </fieldset>

      <Lines label={t.fields.risks ?? ""} value={value.risks} onChange={(risks) => set({ risks })} />
      <Lines label={t.fields.deliverables ?? ""} value={value.deliverables} onChange={(deliverables) => set({ deliverables })} />
      <div>
        <Lines label={t.fields.doneWhen ?? ""} value={value.doneWhen} onChange={(doneWhen) => set({ doneWhen })} />
        <p className={styles.hint}>{t.dodFixed}</p>
      </div>
    </div>
  );
}
