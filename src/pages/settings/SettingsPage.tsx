// Pengaturan, in five parts since P1 (`parts.ts`), each its own page at `/pengaturan/<slug>`:
// Tampilan (the light, the ink), Hari & kebiasaan (the owner's name and day, the Tutup Hari nudge,
// the Belajar goal), Integrasi AI (Gemini, the developer profile, Claude Code), ULTRON (the local
// API), and Sistem & data (starting with Windows, the capture hotkey, the export, where the data
// lives). While Pengaturan is open the sidebar lists the parts (Shell).
// Every control saves on its own; a refusal is said next to the control that caused it.

import { useParams } from "@tanstack/react-router";
import { useEffect, useState } from "react";

import type { HubError } from "../../data/backend";
import type { Ink } from "../../data/generated/Ink";
import type { Settings } from "../../data/generated/Settings";
import type { SettingsPatch } from "../../data/generated/SettingsPatch";
import type { ThemeMode } from "../../data/generated/ThemeMode";
import {
  useApiStatus,
  useExportData,
  useInfo,
  useRotateApiToken,
  useSettings,
  useShell,
  useUpdateSettings,
} from "../../data/queries";
import { id } from "../../strings/id";
import { relativeDate, whenLabel } from "../../time/format";
import { useNow } from "../../time/clock";
import { ClaudeCodeSection } from "./ClaudeCodeSection";
import { DevProfileSection } from "./DevProfileSection";
import { GeminiSection } from "./GeminiSection";
import type { Place } from "../../sound/palette";
import { useGlide } from "../../motion/motion";
import { useSound } from "../../sound/SoundProvider";
import { SETTINGS_PARTS } from "./parts";
import type { SettingsPart } from "./parts";
import styles from "./settings.module.css";

const MODES: ThemeMode[] = ["auto", "system", "light", "dark"];
const INKS: Ink[] = ["biru", "hijau", "sepia", "ungu", "grafit"];

function errorText(error: HubError | null, field: string): string | null {
  if (!error) return null;
  const { field: failed, reason } = error.view;
  if (failed !== field) return null;
  const messages = id.settings.errors[field];
  if (typeof messages === "object" && reason && messages[reason]) return messages[reason];
  return id.settings.errors.fallback as string;
}

export function SettingsPage() {
  const settings = useSettings();
  const params: { bagian?: string } = useParams({ strict: false });
  // The router sends any other address to the first part.
  const part = SETTINGS_PARTS.find((p) => p.slug === params.bagian);
  if (!settings.data || !part) return null;
  return <SettingsForm settings={settings.data} part={part} />;
}

function SettingsForm({ settings, part }: { settings: Settings; part: SettingsPart }) {
  const update = useUpdateSettings();
  const info = useInfo();
  const shell = useShell();
  const exportData = useExportData();
  const now = useNow();
  const [name, setName] = useState(settings.ownerName);
  const [lastField, setLastField] = useState<string | null>(null);
  const error = update.error;

  const save = (patch: SettingsPatch, field: string) => {
    setLastField(field);
    update.mutate(patch);
  };
  const saveName = () => {
    if (name.trim() !== settings.ownerName) save({ ownerName: name }, "ownerName");
  };
  const failedField = lastField && error ? lastField : null;
  const modes = useGlide<HTMLDivElement>("input:checked");
  const goal = useGlide<HTMLDivElement>("input:checked");

  return (
    <div className={styles.page}>
      <p className={styles.eyebrow}>{id.settings.title}</p>
      <h1 className={`display ${styles.title}`} id="settings-title">
        {part.label}
      </h1>

      {part.slug === "tampilan" && (
        <>
          <section className={styles.section} aria-labelledby="look-title">
            <h2 className="section-label" id="look-title">
              {id.settings.lookLight}
            </h2>
            <div className={styles.field}>
              <span className={styles.label} id="mode-label">
                {id.settings.light}
              </span>
              <div ref={modes} className={styles.segmented} role="radiogroup" aria-labelledby="mode-label">
                {MODES.map((mode) => (
                  <label key={mode} className={styles.segment}>
                    <input
                      type="radio"
                      name="theme-mode"
                      value={mode}
                      checked={settings.themeMode === mode}
                      onChange={() => save({ themeMode: mode }, "themeMode")}
                    />
                    <span>{id.settings.modes[mode]}</span>
                  </label>
                ))}
              </div>
              <p className={styles.hint}>{id.settings.lightHint}</p>
            </div>

            <div className={styles.field}>
              <span className={styles.label} id="ink-label">
                {id.settings.ink}
              </span>
              <div className={styles.inks} role="radiogroup" aria-labelledby="ink-label">
                {INKS.map((ink) => (
                  <label key={ink} className={styles.ink} data-ink={ink}>
                    <input
                      type="radio"
                      name="ink"
                      value={ink}
                      checked={settings.ink === ink}
                      onChange={() => save({ ink }, "ink")}
                    />
                    <i aria-hidden="true" />
                    <span>{id.settings.inks[ink]}</span>
                  </label>
                ))}
              </div>
            </div>
          </section>
          <SoundSection settings={settings} save={save} />
        </>
      )}

      {part.slug === "hari" && (
        <>
          <section className={styles.section} aria-labelledby="day-title">
            <h2 className="section-label" id="day-title">
              {id.settings.day}
            </h2>
            <div className={styles.field}>
              <label className={styles.label} htmlFor="owner-name">
                {id.settings.name}
              </label>
              <input
                id="owner-name"
                className={styles.input}
                value={name}
                maxLength={60}
                aria-invalid={failedField === "ownerName"}
                aria-describedby="owner-name-error"
                onChange={(event) => setName(event.target.value)}
                onBlur={saveName}
                onKeyDown={(event) => {
                  if (event.key === "Enter") saveName();
                }}
              />
              <p className={styles.error} id="owner-name-error">
                {failedField === "ownerName" ? errorText(error, "ownerName") : ""}
              </p>
            </div>
            <div className={styles.times}>
              <div className={styles.field}>
                <label className={styles.label} htmlFor="wake-time">
                  {id.settings.wake}
                </label>
                <input
                  id="wake-time"
                  type="time"
                  step={300}
                  className={styles.input}
                  defaultValue={settings.wakeTime}
                  onChange={(event) => {
                    if (event.target.value) save({ wakeTime: event.target.value }, "wakeTime");
                  }}
                />
              </div>
              <div className={styles.field}>
                <label className={styles.label} htmlFor="sleep-time">
                  {id.settings.sleep}
                </label>
                <input
                  id="sleep-time"
                  type="time"
                  step={300}
                  className={styles.input}
                  defaultValue={settings.sleepTime}
                  aria-describedby="sleep-time-error"
                  onChange={(event) => {
                    if (event.target.value) save({ sleepTime: event.target.value }, "sleepTime");
                  }}
                />
              </div>
            </div>
            <p className={styles.error} id="sleep-time-error">
              {failedField === "wakeTime" || failedField === "sleepTime"
                ? (errorText(error, "sleepTime") ?? errorText(error, "wakeTime"))
                : ""}
            </p>
            <p className={styles.hint}>{id.settings.dayHint}</p>
          </section>

          <section className={styles.section} aria-labelledby="evening-title">
            <h2 className="section-label" id="evening-title">
              {id.settings.evening}
            </h2>
            <div className={styles.switchRow}>
              <div>
                <span className={styles.label} id="nudge-label">
                  {id.settings.nudge}
                </span>
                <p className={styles.hint}>{id.settings.nudgeHint}</p>
              </div>
              <button
                type="button"
                role="switch"
                className={styles.switch}
                aria-checked={settings.closeNudge}
                aria-labelledby="nudge-label"
                onClick={() => save({ closeNudge: !settings.closeNudge }, "closeNudge")}
              >
                <i aria-hidden="true" />
              </button>
            </div>
            {settings.closeNudge && (
              <div className={styles.field}>
                <label className={styles.label} htmlFor="nudge-time">
                  {id.settings.nudgeTime}
                </label>
                <input
                  id="nudge-time"
                  type="time"
                  step={300}
                  className={`${styles.input} ${styles.timeInput}`}
                  defaultValue={settings.closeNudgeTime}
                  aria-describedby="nudge-time-error"
                  onChange={(event) => {
                    if (event.target.value) save({ closeNudgeTime: event.target.value }, "closeNudgeTime");
                  }}
                />
                <p className={styles.error} id="nudge-time-error">
                  {failedField === "closeNudgeTime" ? errorText(error, "closeNudgeTime") : ""}
                </p>
              </div>
            )}
          </section>

          <section className={styles.section} aria-labelledby="learning-goal-title">
            <h2 className="section-label" id="learning-goal-title">
              {id.settings.learning}
            </h2>
            <div className={styles.field}>
              <span className={styles.label} id="goal-label">
                {id.learning.goal}
              </span>
              <div ref={goal} className={styles.segmented} role="radiogroup" aria-labelledby="goal-label">
                {[1, 2, 3, 4, 5, 6, 7].map((days) => (
                  <label key={days} className={styles.segment}>
                    <input
                      type="radio"
                      name="learning-goal"
                      value={days}
                      checked={settings.learningGoalDays === days}
                      onChange={() => save({ learningGoalDays: days }, "learningGoalDays")}
                    />
                    <span>{days}</span>
                  </label>
                ))}
              </div>
              <p className={styles.hint}>{id.learning.goalHint}</p>
            </div>
          </section>

          <FocusSection settings={settings} save={save} failed={failedField && FOCUS_FIELDS.some(([field]) => field === failedField) ? { field: failedField, text: errorText(error, failedField) ?? "" } : null} />
        </>
      )}

      {part.slug === "ultron" && (
        <UltronSection settings={settings} save={save} failed={failedField === "apiPort" ? errorText(error, "apiPort") : null} />
      )}

      {part.slug === "ai" && (
        <>
          <GeminiSection settings={settings} save={save} failed={failedField === "geminiModel" ? errorText(error, "geminiModel") : null} />

          <DevProfileSection
            settings={settings}
            save={save}
            failed={failedField && ["devKnows", "devLearning", "devDeploy"].includes(failedField) ? { field: failedField, text: errorText(error, failedField) ?? "" } : null}
          />

          <ClaudeCodeSection settings={settings} save={save} />
        </>
      )}

      {part.slug === "sistem" && (
        <section className={styles.section} aria-labelledby="settings-title">
          <div className={styles.switchRow}>
            <div>
              <span className={styles.label} id="autostart-label">
                {id.settings.autostart}
              </span>
              <p className={styles.hint}>{id.settings.autostartHint}</p>
            </div>
            <button
              type="button"
              role="switch"
              className={styles.switch}
              aria-checked={settings.autostart}
              aria-labelledby="autostart-label"
              onClick={() => save({ autostart: !settings.autostart }, "autostart")}
            >
              <i aria-hidden="true" />
            </button>
          </div>
          {failedField === "autostart" && <p className={styles.error}>{id.settings.errors.fallback as string}</p>}
          {shell.data && (
            <div className={styles.field}>
              <span className={styles.label}>{id.settings.hotkey}</span>
              <p className={shell.data.hotkeyOk ? styles.hint : styles.error}>
                {shell.data.hotkeyOk ? id.settings.hotkeyOk(shell.data.hotkey) : id.settings.hotkeyBusy(shell.data.hotkey)}
              </p>
            </div>
          )}
          {shell.data && (
            <div className={styles.field}>
              <span className={styles.label}>{id.settings.lenteraHotkey}</span>
              <p className={shell.data.lenteraHotkeyOk ? styles.hint : styles.error}>
                {shell.data.lenteraHotkeyOk ? id.settings.lenteraHotkeyOk(shell.data.lenteraHotkey) : id.settings.hotkeyBusy(shell.data.lenteraHotkey)}
              </p>
            </div>
          )}
          <div className={styles.field}>
            <div>
              <button type="button" className="btn btn-secondary" disabled={exportData.isPending} onClick={() => exportData.mutate()}>
                {id.settings.export}
              </button>
            </div>
            <p className={exportData.isError ? styles.error : styles.hint} aria-live="polite">
              {exportData.isError
                ? id.settings.exportFailed
                : exportData.data
                  ? id.settings.exported(exportData.data)
                  : id.settings.exportHint}
            </p>
          </div>
          {info.data && (
            <dl className={styles.info}>
              <dt>{id.settings.dataDir}</dt>
              <dd>
                <code>{info.data.dataDir}</code>
              </dd>
              <dt>{id.settings.lastBackup}</dt>
              <dd>{info.data.lastBackup ? relativeDate(info.data.lastBackup, now) : id.settings.noBackup}</dd>
              <dt>{id.settings.version}</dt>
              <dd>
                {info.data.version} <span className={styles.muted}>(skema {info.data.schemaVersion})</span>
              </dd>
            </dl>
          )}
        </section>
      )}
    </div>
  );
}

/** ULTRON's door into Hub: the switch, the port, what is happening, and a new token. */
/** Sesi Fokus's lengths (Lentera CP02), with what the core allows (settings.rs). */
const FOCUS_FIELDS: readonly ["focusMinutes" | "breakMinutes" | "longBreakMinutes" | "longBreakEvery", number, number][] = [
  ["focusMinutes", 5, 90],
  ["breakMinutes", 1, 30],
  ["longBreakMinutes", 5, 60],
  ["longBreakEvery", 2, 8],
];

function FocusSection({
  settings,
  save,
  failed,
}: {
  settings: Settings;
  save: (patch: SettingsPatch, field: string) => void;
  failed: { field: string; text: string } | null;
}) {
  return (
    <section className={styles.section} aria-labelledby="focus-title">
      <h2 className="section-label" id="focus-title">
        {id.focus.settings.title}
      </h2>
      <div className={styles.times}>
        {FOCUS_FIELDS.map(([field, min, max]) => (
          <div key={field} className={styles.field}>
            <label className={styles.label} htmlFor={`focus-${field}`}>
              {id.focus.settings[field]}
            </label>
            <input
              id={`focus-${field}`}
              type="number"
              inputMode="numeric"
              min={min}
              max={max}
              className={styles.input}
              defaultValue={settings[field]}
              aria-invalid={failed?.field === field || undefined}
              aria-describedby="focus-error"
              onChange={(event) => {
                const value = Number(event.target.value);
                if (Number.isInteger(value) && value >= min && value <= max && value !== settings[field]) save({ [field]: value }, field);
              }}
            />
          </div>
        ))}
      </div>
      <p className={styles.error} id="focus-error">
        {failed?.text ?? ""}
      </p>
      <p className={styles.hint}>{id.focus.settings.hint}</p>
    </section>
  );
}

function UltronSection({
  settings,
  save,
  failed,
}: {
  settings: Settings;
  save: (patch: SettingsPatch, field: string) => void;
  failed: string | null;
}) {
  const status = useApiStatus();
  const rotate = useRotateApiToken();
  const now = useNow();
  const [port, setPort] = useState(String(settings.apiPort));
  const savePort = () => {
    const value = Number(port);
    if (port.trim() && value !== settings.apiPort) save({ apiPort: Number.isInteger(value) ? value : 0 }, "apiPort");
  };
  const state = status.data;
  let line: string;
  if (!settings.apiEnabled) line = id.settings.apiOff;
  else if (state?.problem) line = id.settings.apiProblem[state.problem]?.(state.port) ?? id.settings.errors.fallback as string;
  else if (!state?.running) line = id.settings.apiStarting;
  else line = id.settings.apiOn(state.port);

  return (
    <section className={styles.section} aria-labelledby="settings-title">
      <div className={styles.switchRow}>
        <div>
          <span className={styles.label} id="api-label">
            {id.settings.api}
          </span>
          <p className={styles.hint}>{id.settings.apiHint}</p>
        </div>
        <button
          type="button"
          role="switch"
          className={styles.switch}
          aria-checked={settings.apiEnabled}
          aria-labelledby="api-label"
          onClick={() => save({ apiEnabled: !settings.apiEnabled }, "apiEnabled")}
        >
          <i aria-hidden="true" />
        </button>
      </div>
      <div className={styles.apiState} role="status">
        <p className={settings.apiEnabled && state?.problem ? styles.error : styles.hint}>{line}</p>
        {settings.apiEnabled && state?.running && (
          <p className={styles.hint}>
            {state.listeners > 0
              ? id.settings.apiListening
              : state.lastUsedAt
                ? id.settings.apiLastUsed(whenLabel(state.lastUsedAt, now))
                : id.settings.apiNeverUsed}
          </p>
        )}
      </div>
      {settings.apiEnabled && (
        <>
          <div className={styles.field}>
            <label className={styles.label} htmlFor="api-port">
              {id.settings.apiPort}
            </label>
            <input
              id="api-port"
              inputMode="numeric"
              className={`${styles.input} ${styles.timeInput}`}
              value={port}
              maxLength={5}
              aria-invalid={failed !== null}
              aria-describedby="api-port-hint api-port-error"
              onChange={(event) => setPort(event.target.value.replace(/[^0-9]/g, ""))}
              onBlur={savePort}
              onKeyDown={(event) => {
                if (event.key === "Enter") savePort();
              }}
            />
            <p className={styles.error} id="api-port-error">
              {failed ?? ""}
            </p>
            <p className={styles.hint} id="api-port-hint">
              {id.settings.apiPortHint}
            </p>
          </div>
          <div className={styles.field}>
            <div>
              <button type="button" className="btn btn-secondary" disabled={rotate.isPending} onClick={() => rotate.mutate()}>
                {id.settings.rotate}
              </button>
            </div>
            <p className={styles.hint} aria-live="polite">
              {rotate.isSuccess ? id.settings.rotated : id.settings.rotateHint}
            </p>
          </div>
        </>
      )}
    </section>
  );
}

const TRY: Place[] = ["today", "work", "studio", "learning", "capture"];

/** P2: the switch, Hub's own volume (saved when the owner lets go), and a sample of each place. */
function SoundSection({ settings, save }: { settings: Settings; save: (patch: SettingsPatch, field: string) => void }) {
  const t = id.settings.sound;
  const sound = useSound();
  const [volume, setVolume] = useState(settings.soundVolume);
  useEffect(() => {
    if (volume === settings.soundVolume) return;
    const wait = window.setTimeout(() => save({ soundVolume: volume }, "soundVolume"), 350);
    return () => window.clearTimeout(wait);
  }, [volume, settings.soundVolume, save]);

  return (
    <section className={styles.section} aria-labelledby="sound-title">
      <h2 className="section-label" id="sound-title">
        {t.title}
      </h2>
      <div className={styles.switchRow}>
        <div>
          <span className={styles.label} id="sound-label">
            {t.switchLabel}
          </span>
          <p className={styles.hint}>{t.hint}</p>
        </div>
        <button
          type="button"
          role="switch"
          className={styles.switch}
          aria-checked={settings.soundEnabled}
          aria-labelledby="sound-label"
          onClick={() => save({ soundEnabled: !settings.soundEnabled }, "soundEnabled")}
        >
          <i aria-hidden="true" />
        </button>
      </div>
      <div className={styles.field}>
        <label className={styles.label} htmlFor="sound-volume">
          {t.volume}
        </label>
        <div className={styles.volume}>
          <input
            id="sound-volume"
            type="range"
            min={0}
            max={100}
            step={5}
            value={volume}
            disabled={!settings.soundEnabled}
            aria-valuetext={t.volumeValue(volume)}
            onChange={(event) => setVolume(Number(event.target.value))}
          />
          <output htmlFor="sound-volume" className={styles.muted}>
            {t.volumeValue(volume)}
          </output>
        </div>
      </div>
      <div className={styles.field}>
        <span className={styles.label} id="sound-try">
          {t.tryLabel}
        </span>
        <div className={styles.actions} role="group" aria-labelledby="sound-try">
          {TRY.map((place) => (
            <button key={place} type="button" className="btn btn-quiet" onClick={() => sound.preview(place)}>
              {t.places[place]}
            </button>
          ))}
        </div>
        <p className={styles.hint}>{t.tryHint}</p>
      </div>
    </section>
  );
}
