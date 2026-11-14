// Hari Ini's side cards for CP4: Belajar (the week against the goal, what to pick up again, the
// running timer) and the files added most recently.

import { Link } from "@tanstack/react-router";
import { Play } from "lucide-react";

import { openFile } from "../../components/FileRows";
import { FileThumb } from "../../components/FileThumb";
import { FocusButton } from "../../components/FocusButton";
import { useAct } from "../../components/Toast";
import { useApi } from "../../data/api";
import type { FileCard } from "../../data/generated/FileCard";
import type { LearningGlance } from "../../data/generated/LearningGlance";
import { id } from "../../strings/id";
import { span } from "../../time/format";
import { WeekDots } from "../learning/charts";
import { TimerBar } from "../learning/LearningPage";
import styles from "./today.module.css";

export function LearningCardToday({ glance, todayIso }: { glance: LearningGlance; todayIso: string }) {
  const api = useApi();
  const act = useAct();
  const current = glance.current;
  return (
    <div className={styles.learning}>
      <div className={styles.learningWeek}>
        <WeekDots days={glance.days} todayIso={todayIso} />
        <span>
          {id.learning.activeDays(glance.activeDays, glance.goalDays)}
          {glance.minutesWeek > 0 && ` · ${span(glance.minutesWeek)}`}
        </span>
      </div>
      {glance.timer ? (
        <TimerBar timer={glance.timer} />
      ) : current ? (
        <div className={styles.learningItem} data-pigment={current.pigment}>
          <Link to="/belajar" search={{ track: current.trackId, item: current.id }} className={styles.learningTitle}>
            {current.title}
          </Link>
          <span>{current.trackName}</span>
          <div className={styles.learningButtons}>
            <button type="button" className="btn btn-secondary" data-sound="sessionStart" onClick={() => void act(() => api.startTimer(current.id))}>
              <Play size={14} aria-hidden="true" />
              {id.learning.resume}
            </button>
            <FocusButton itemId={current.id} className="btn btn-quiet" />
          </div>
        </div>
      ) : (
        <p className={styles.empty}>{id.learning.currentEmpty}</p>
      )}
    </div>
  );
}

export function RecentFiles({ files }: { files: readonly FileCard[] }) {
  const api = useApi();
  const act = useAct();
  if (files.length === 0) return <p className={styles.empty}>{id.files.recentEmpty}</p>;
  return (
    <ul className={styles.recentFiles}>
      {files.map((file) => (
        <li key={file.id}>
          <FileThumb id={file.id} kind={file.kind} missing={file.state === "missing"} size="row" />
          <button type="button" onClick={() => void openFile(api, act, file)} title={file.path}>
            <span>{file.name}</span>
            <small>{file.links[0]?.title ?? (file.state === "missing" ? id.files.stateMissing : id.files.size(file.sizeBytes))}</small>
          </button>
          <Link to="/file" search={{ file: file.id }} className={styles.recentMore} aria-label={`${id.files.detail}: ${file.name}`}>
            ›
          </Link>
        </li>
      ))}
    </ul>
  );
}
