// How far a project has come, as ink filling a thin line in the project's colour.

import type { Progress } from "../../data/generated/Progress";
import styles from "./projects.module.css";

export function ProgressBar({ progress, label, large = false }: { progress: Progress; label: string; large?: boolean }) {
  return (
    <div
      className={`${styles.progress} ${large ? styles.progressLarge : ""}`}
      role="progressbar"
      aria-label={label}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={progress.percent}
      aria-valuetext={`${progress.percent}%`}
    >
      <i style={{ transform: `scaleX(${progress.percent / 100})` }} />
    </div>
  );
}
