// Integrasi AI > Profil developer (R3): what Gemini reads when it recommends a tech stack in Studio,
// filled once. Each line is saved when it loses focus; the cost is a choice of three.

import { useState } from "react";

import type { DevCost } from "../../data/generated/DevCost";
import type { Settings } from "../../data/generated/Settings";
import type { SettingsPatch } from "../../data/generated/SettingsPatch";
import { id } from "../../strings/id";
import styles from "./settings.module.css";

const t = id.settings.profile;
const COSTS: DevCost[] = ["free", "low", "flexible"];

type Line = "devKnows" | "devLearning" | "devDeploy";

export function DevProfileSection({
  settings,
  save,
  failed,
}: {
  settings: Settings;
  save: (patch: SettingsPatch, field: string) => void;
  failed: { field: string; text: string } | null;
}) {
  const [lines, setLines] = useState<Record<Line, string>>({
    devKnows: settings.devKnows,
    devLearning: settings.devLearning,
    devDeploy: settings.devDeploy,
  });

  const field = (key: Line, label: string, hint: string) => (
    <div className={styles.field}>
      <label className={styles.label} htmlFor={`profile-${key}`}>
        {label}
      </label>
      <input
        id={`profile-${key}`}
        className={styles.input}
        value={lines[key]}
        maxLength={300}
        aria-describedby={`profile-${key}-hint profile-${key}-error`}
        aria-invalid={failed?.field === key || undefined}
        onChange={(event) => setLines((now) => ({ ...now, [key]: event.target.value }))}
        onBlur={() => {
          if (lines[key].trim() !== settings[key]) save({ [key]: lines[key] }, key);
        }}
      />
      <p className={styles.error} id={`profile-${key}-error`}>
        {failed?.field === key ? failed.text : ""}
      </p>
      <p className={styles.hint} id={`profile-${key}-hint`}>
        {hint}
      </p>
    </div>
  );

  return (
    <section className={styles.section} aria-labelledby="profile-title">
      <h2 className="section-label" id="profile-title">
        {t.title}
      </h2>
      <p className={styles.hint}>{t.intro}</p>
      {field("devKnows", t.knows, t.knowsHint)}
      {field("devLearning", t.learning, t.learningHint)}
      <div className={styles.field}>
        <p className={styles.label} id="profile-cost">
          {t.cost}
        </p>
        <div className={styles.segmented} role="radiogroup" aria-labelledby="profile-cost">
          {COSTS.map((cost) => (
            <label key={cost} className={styles.segment}>
              <input type="radio" name="dev-cost" value={cost} checked={settings.devCost === cost} onChange={() => save({ devCost: cost }, "devCost")} />
              <span>{t.costs[cost]}</span>
            </label>
          ))}
        </div>
      </div>
      {field("devDeploy", t.deploy, t.deployHint)}
    </section>
  );
}
