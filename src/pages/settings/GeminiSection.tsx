// Integrasi AI > Google Gemini: the key (kept by Windows Credential Manager; the page only learns
// whether one is saved), the model, and "Uji koneksi" with the problem named in plain words.

import { AlertTriangle, CheckCircle2, Eye, EyeOff } from "lucide-react";
import { useState } from "react";

import type { HubError } from "../../data/backend";
import type { CheckReport } from "../../data/generated/CheckReport";
import type { Settings } from "../../data/generated/Settings";
import type { SettingsPatch } from "../../data/generated/SettingsPatch";
import { useCheckGemini, useDeleteGeminiKey, useGeminiModels, useGeminiStatus, useSaveGeminiKey } from "../../data/queries";
import { isTextModel } from "../studio/models";
import { id } from "../../strings/id";
import styles from "./settings.module.css";

const t = id.settings.gemini;

function keyError(error: HubError | null): string | null {
  if (!error) return null;
  const { field, reason } = error.view;
  if (field === "keyStore") return t.storeFailed;
  const messages = id.settings.errors.key;
  if (field === "key" && typeof messages === "object" && reason && messages[reason]) return messages[reason];
  return id.settings.errors.fallback as string;
}

function outcomeText(report: CheckReport, model: string): string {
  const say = t.outcome[report.outcome] ?? t.outcome["unexpected"];
  return say ? say(report.model?.displayName ?? model) : "";
}

export function GeminiSection({
  settings,
  save,
  failed,
}: {
  settings: Settings;
  save: (patch: SettingsPatch, field: string) => void;
  failed: string | null;
}) {
  const status = useGeminiStatus();
  const hasKey = status.data?.hasKey ?? false;
  const models = useGeminiModels(hasKey);
  const saveKey = useSaveGeminiKey();
  const removeKey = useDeleteGeminiKey();
  const check = useCheckGemini();
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState("");
  const [visible, setVisible] = useState(false);
  const [confirming, setConfirming] = useState(false);

  const showField = !hasKey || editing;
  const keyProblem = keyError(saveKey.error);

  const submitKey = () => {
    if (!draft.trim()) return;
    saveKey.mutate(draft, {
      onSuccess: () => {
        setDraft("");
        setVisible(false);
        setEditing(false);
        check.reset();
      },
    });
  };
  const remove = () =>
    removeKey.mutate(undefined, {
      onSuccess: () => {
        setConfirming(false);
        check.reset();
      },
    });

  // The saved model stays in the list even when Google's list does not have it (or failed). Only
  // text models are offered (L2): speech and image models cannot hold Studio's conversation.
  const options = models.data?.filter((model) => isTextModel(model.id)).map((model) => ({ id: model.id, name: model.displayName })) ?? [];
  if (!options.some((option) => option.id === settings.geminiModel)) options.unshift({ id: settings.geminiModel, name: settings.geminiModel });

  const report = check.data;
  const connected = report?.outcome === "connected";

  return (
    <section className={styles.section} aria-labelledby="ai-title">
      <h2 className="section-label" id="ai-title">
        {t.title}
      </h2>
      <p className={styles.hint}>{t.intro}</p>

      {showField ? (
        <form
          className={styles.field}
          onSubmit={(event) => {
            event.preventDefault();
            submitKey();
          }}
        >
          <label className={styles.label} htmlFor="gemini-key">
            {t.key}
          </label>
          <div className={styles.keyRow}>
            <input
              id="gemini-key"
              name="gemini-api-key"
              className={styles.input}
              type={visible ? "text" : "password"}
              value={draft}
              autoComplete="off"
              spellCheck={false}
              maxLength={200}
              aria-invalid={keyProblem !== null}
              aria-describedby="gemini-key-hint gemini-key-error"
              onChange={(event) => setDraft(event.target.value)}
            />
            <button
              type="button"
              className={`btn btn-quiet ${styles.iconButton}`}
              aria-pressed={visible}
              aria-label={visible ? t.hide : t.show}
              title={visible ? t.hide : t.show}
              onClick={() => setVisible((shown) => !shown)}
            >
              {visible ? <EyeOff size={16} strokeWidth={1.5} aria-hidden="true" /> : <Eye size={16} strokeWidth={1.5} aria-hidden="true" />}
            </button>
          </div>
          <p className={styles.error} id="gemini-key-error">
            {keyProblem ?? ""}
          </p>
          <p className={styles.hint} id="gemini-key-hint">
            {t.keyHint}
          </p>
          <div className={styles.actions}>
            <button type="submit" className="btn btn-primary" disabled={saveKey.isPending || !draft.trim()}>
              {t.save}
            </button>
            {hasKey && (
              <button
                type="button"
                className="btn btn-quiet"
                onClick={() => {
                  setEditing(false);
                  setDraft("");
                  setVisible(false);
                  saveKey.reset();
                }}
              >
                {t.cancel}
              </button>
            )}
          </div>
        </form>
      ) : (
        <div className={styles.field}>
          <span className={styles.label}>{t.key}</span>
          <p className={styles.hint}>{t.keySaved}</p>
          {confirming ? (
            <div className={styles.confirm} role="group" aria-labelledby="gemini-remove-ask">
              <p id="gemini-remove-ask">{t.removeAsk}</p>
              <div className={styles.actions}>
                <button type="button" className="btn btn-danger" disabled={removeKey.isPending} onClick={remove}>
                  {t.removeConfirm}
                </button>
                <button type="button" className="btn btn-quiet" onClick={() => setConfirming(false)}>
                  {t.cancel}
                </button>
              </div>
            </div>
          ) : (
            <div className={styles.actions}>
              <button type="button" className="btn btn-secondary" onClick={() => setEditing(true)}>
                {t.change}
              </button>
              <button type="button" className="btn btn-danger" onClick={() => setConfirming(true)}>
                {t.remove}
              </button>
            </div>
          )}
          {removeKey.isError && <p className={styles.error}>{keyError(removeKey.error)}</p>}
        </div>
      )}

      <div className={styles.field}>
        <label className={styles.label} htmlFor="gemini-model">
          {t.model}
        </label>
        <select
          id="gemini-model"
          className={styles.input}
          value={settings.geminiModel}
          aria-invalid={failed !== null}
          aria-describedby="gemini-model-hint gemini-model-error"
          onChange={(event) => {
            save({ geminiModel: event.target.value }, "geminiModel");
            check.reset();
          }}
        >
          {options.map((option) => (
            <option key={option.id} value={option.id}>
              {option.name === option.id ? option.id : `${option.name} (${option.id})`}
            </option>
          ))}
        </select>
        <p className={styles.error} id="gemini-model-error">
          {failed ?? ""}
        </p>
        <p className={styles.hint} id="gemini-model-hint">
          {models.isError ? t.modelsFailed : t.modelHint}
        </p>
      </div>

      <div className={styles.field}>
        <div>
          <button type="button" className="btn btn-secondary" disabled={!hasKey || check.isPending} onClick={() => check.mutate()}>
            {check.isPending ? t.checking : t.check}
          </button>
        </div>
        <div role="status" aria-live="polite" className={styles.outcome}>
          {report ? (
            <>
              <p className={connected ? styles.outcomeOk : styles.outcomeBad}>
                {connected ? (
                  <CheckCircle2 size={16} strokeWidth={1.5} aria-hidden="true" />
                ) : (
                  <AlertTriangle size={16} strokeWidth={1.5} aria-hidden="true" />
                )}
                <span>{outcomeText(report, settings.geminiModel)}</span>
              </p>
              {report.retryAfter !== null && <p className={styles.hint}>{t.waitSeconds(report.retryAfter)}</p>}
              {!connected && report.detail && <p className={styles.hint}>{t.googleSaid(report.detail)}</p>}
            </>
          ) : check.isError ? (
            <p className={styles.outcomeBad}>
              <AlertTriangle size={16} strokeWidth={1.5} aria-hidden="true" />
              <span>{t.storeFailed}</span>
            </p>
          ) : (
            <p className={styles.hint}>{t.checkHint}</p>
          )}
        </div>
      </div>

      <p className={styles.note}>{t.privacy}</p>
    </section>
  );
}
