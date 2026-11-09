// Integrasi AI > Claude Code (docs/studio.md, "Settings"), for the CLI and the desktop app's Code tab
// alike (they read the same registration and skill): the switch, Claude Code's three signals
// (R5: the bridge, the session, the last call), what is installed (the bridge, the `claude`
// command, the registration, the skill), Pasang after a sheet that lists exactly what changes,
// Lepas, Uji jembatan step by step, and a new token.

import { AlertTriangle, CheckCircle2, CircleSlash, XCircle } from "lucide-react";
import { useState } from "react";

import { Dialog } from "../../components/Dialog";
import { useApi } from "../../data/api";
import type { BridgeStep } from "../../data/api";
import type { Settings } from "../../data/generated/Settings";
import type { SettingsPatch } from "../../data/generated/SettingsPatch";
import { useClaudeCodeStatus, useStudioAct } from "../../data/queries";
import { ClaudeStatus } from "../studio/ClaudeStatus";
import { id } from "../../strings/id";
import styles from "./settings.module.css";

const t = id.settings.claude;

function Mark({ ok }: { ok: boolean }) {
  const Glyph = ok ? CheckCircle2 : XCircle;
  return <Glyph size={14} aria-hidden="true" className={ok ? styles.okMark : styles.badMark} />;
}

export function ClaudeCodeSection({ settings, save }: { settings: Settings; save: (patch: SettingsPatch, field: string) => void }) {
  const api = useApi();
  const status = useClaudeCodeStatus();
  const [sheet, setSheet] = useState(false);
  const [removing, setRemoving] = useState(false);
  const [steps, setSteps] = useState<BridgeStep[] | null>(null);
  const [notice, setNotice] = useState("");
  // Without the `claude` command, Pasang writes the skill and the sheet stays open on the command.
  const [skillWritten, setSkillWritten] = useState(false);
  const refresh = () => void status.refetch();
  const closeSheet = () => {
    setSheet(false);
    setSkillWritten(false);
  };
  const install = useStudioAct(() => api.installClaudeCode());
  const remove = useStudioAct(() => api.removeClaudeCode());
  const test = useStudioAct(() => api.testClaudeBridge());
  const rotate = useStudioAct(() => api.rotateClaudeToken());
  const s = status.data;
  const error = install.error ?? remove.error ?? rotate.error;
  const errorText = error ? (t.errors[error.view.field ?? ""] ?? t.errors.fallback) : "";
  const installed = s?.registration === "ours" && s.skill === "ours";
  // The bridge registered by hand (the sheet's command run in a terminal) without the skill:
  // Claude Code has Hub's tools but no /hub-connect.
  const half = s?.registration === "ours" && s.skill === "missing";
  const copy = async (text: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setNotice(t.copied);
    } catch {
      setNotice(t.errors.fallback ?? "");
    }
  };


  return (
    <section className={styles.section} aria-labelledby="claude-title">
      <h2 className="section-label" id="claude-title">
        {t.title}
      </h2>
      <p className={styles.hint}>{t.intro}</p>
      <div className={styles.switchRow}>
        <div>
          <span className={styles.label} id="claude-label">
            {t.switchLabel}
          </span>
          <p className={styles.hint}>{t.switchHint}</p>
        </div>
        <button
          type="button"
          role="switch"
          className={styles.switch}
          aria-checked={settings.claudeEnabled}
          aria-labelledby="claude-label"
          onClick={() => save({ claudeEnabled: !settings.claudeEnabled }, "claudeEnabled")}
        >
          <i aria-hidden="true" />
        </button>
      </div>
      <p className={styles.hint} role="status">
        {settings.claudeEnabled ? "" : t.off}
      </p>
      <h3 className={styles.label}>
        {t.status}
      </h3>
      <ClaudeStatus projectId={null} />

      {s && (
        <dl className={styles.info} aria-label={t.parts}>
          <dt>{t.bridge}</dt>
          <dd>
            <Mark ok={s.bridge !== null} /> {s.bridge ? t.found : t.missing}
          </dd>
          <dt>{t.cli}</dt>
          <dd>
            <Mark ok={s.cli !== null} /> {s.cli ? t.found : t.missing}
          </dd>
          <dt>{t.registration}</dt>
          <dd>
            <Mark ok={s.registration === "ours"} /> {t.registrations[s.registration]}
          </dd>
          <dt>{t.skill}</dt>
          <dd>
            <Mark ok={s.skill === "ours"} /> {t.skills[s.skill]}
          </dd>
        </dl>
      )}
      {s && !s.bridge && <p className={styles.error}>{t.noBridge}</p>}
      {half && (
        <p className={styles.outcomeBad}>
          <AlertTriangle size={15} strokeWidth={1.8} aria-hidden="true" /> {t.skillMissing}
        </p>
      )}

      <div className={styles.actions}>
        <button type="button" className="btn btn-primary" disabled={!s?.bridge} onClick={() => setSheet(true)}>
          {installed ? t.reinstall : half ? t.complete : t.install}
        </button>
        {(s?.registration === "ours" || s?.skill === "ours") && (
          <button type="button" className="btn btn-quiet" onClick={() => setRemoving(true)}>
            {t.remove}
          </button>
        )}
        <button
          type="button"
          className="btn btn-secondary"
          disabled={!s?.bridge || !settings.claudeEnabled || test.isPending}
          onClick={() => test.mutate(undefined, { onSuccess: setSteps })}
        >
          {test.isPending ? t.testing : t.test}
        </button>
      </div>

      {removing && (
        <div className={styles.confirm} role="group" aria-labelledby="claude-remove-ask">
          <p id="claude-remove-ask">{t.removeAsk}</p>
          <div className={styles.actions}>
            <button
              type="button"
              className="btn btn-danger"
              disabled={remove.isPending}
              onClick={() => remove.mutate(undefined, { onSuccess: () => { setRemoving(false); setNotice(t.removed); refresh(); } })}
            >
              {t.remove}
            </button>
            <button type="button" className="btn btn-quiet" onClick={() => setRemoving(false)}>
              {t.cancel}
            </button>
          </div>
        </div>
      )}

      {steps && (
        <ol className={styles.steps} aria-label={t.test}>
          {steps.map((step) => (
            <li key={step.step}>
              <Mark ok={step.ok} /> {t.steps[step.step]}: {step.ok ? t.ok : t.failed}
              {step.detail && <span className={styles.muted}> ({step.detail})</span>}
            </li>
          ))}
        </ol>
      )}

      <div role="status" className={styles.hint}>
        {notice}
      </div>
      {errorText && <p className={styles.error}>{errorText}</p>}

      {settings.claudeEnabled && (
        <div className={styles.field}>
          <div>
            <button type="button" className="btn btn-secondary" disabled={rotate.isPending} onClick={() => rotate.mutate(undefined, { onSuccess: () => setNotice(t.rotated) })}>
              {t.rotate}
            </button>
          </div>
          <p className={styles.hint}>{t.rotateHint}</p>
        </div>
      )}

      {sheet && s && (
        <Dialog
          title={t.sheetTitle}
          description={t.sheetIntro}
          width="wide"
          onClose={closeSheet}
          footer={
            <>
              <button type="button" className="btn btn-quiet" onClick={closeSheet}>
                {t.cancel}
              </button>
              {s.cli ? (
                <button
                  type="button"
                  className="btn btn-primary"
                  disabled={install.isPending}
                  onClick={() => install.mutate(undefined, { onSuccess: () => { closeSheet(); setNotice(t.installed); refresh(); } })}
                >
                  {t.confirm}
                </button>
              ) : (
                <>
                  <button type="button" className="btn btn-secondary" onClick={refresh}>
                    {t.check}
                  </button>
                  <button
                    type="button"
                    className="btn btn-primary"
                    disabled={install.isPending || s.skill === "ours"}
                    onClick={() => install.mutate(undefined, { onSuccess: () => { setSkillWritten(true); refresh(); } })}
                  >
                    {t.writeSkill}
                  </button>
                </>
              )}
            </>
          }
        >
          <ul className={styles.changes}>
            <li>
              <p>{s.cli ? t.sheetCommand : t.noCli}</p>
              {s.commands.map((command) => (
                <div key={command} className={styles.commandRow}>
                  <code className={styles.command}>{command}</code>
                  <button type="button" className="btn btn-quiet" data-sound="copy" onClick={() => void copy(command)}>
                    {t.copy}
                  </button>
                </div>
              ))}
            </li>
            <li>{t.sheetSkill(s.skillPath)}</li>
            {!settings.claudeEnabled && <li>{t.sheetSwitch}</li>}
          </ul>
          <div className={styles.hint}>
            <p className={styles.after}>{t.sheetAfter}</p>
            <ul className={styles.afterSteps}>
              <li>
                <strong>{t.sheetCli}</strong> {t.sheetCliStep}
              </li>
              <li>
                <strong>{t.sheetDesktop}</strong> {t.sheetDesktopStep}
              </li>
            </ul>
            <p className={styles.after}>{t.sheetChat}</p>
          </div>
          <div role="status">
            {skillWritten && (
              <p className={styles.outcomeOk}>
                <CheckCircle2 size={15} strokeWidth={1.8} aria-hidden="true" /> {t.skillWritten}
              </p>
            )}
          </div>
          {errorText && (
            <p className={styles.error} role="alert">
              <CircleSlash size={14} aria-hidden="true" /> {errorText}
            </p>
          )}
        </Dialog>
      )}
    </section>
  );
}
