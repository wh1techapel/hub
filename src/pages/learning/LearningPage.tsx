// Belajar: the week against the goal in active days, the timer, the tracks as tabs with their
// items, and on the side the twenty-week heatmap and the domains. An item opens beside the list
// (its address carries it, like a task).

import { useSound } from "../../sound/SoundProvider";
import { useNavigate, useSearch } from "@tanstack/react-router";
import { ExternalLink, Pause, Play, Plus, Settings2, Trophy, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";

import { FocusButton } from "../../components/FocusButton";
import { useAct, useToast } from "../../components/Toast";
import { useApi } from "../../data/api";
import type { LearningCard } from "../../data/generated/LearningCard";
import type { LearningKind } from "../../data/generated/LearningKind";
import type { LearningStatus } from "../../data/generated/LearningStatus";
import type { Timer } from "../../data/generated/Timer";
import type { Track } from "../../data/generated/Track";
import type { TrackKind } from "../../data/generated/TrackKind";
import { useCtfEvents, useLearning, useLearningOverview, useTracks } from "../../data/queries";
import { id } from "../../strings/id";
import { useNow } from "../../time/clock";
import { clockOf, useElapsed } from "../../time/elapsed";
import { isoDate, relativeDate, span } from "../../time/format";
import { CtfDialog } from "./CtfDialog";
import { DomainBars, Heatmap, WeekDots } from "./charts";
import { ItemPanel } from "./ItemPanel";
import styles from "./learning.module.css";
import { TrackDialog } from "./TrackDialog";

export interface LearningSearch {
  track?: string;
  item?: string;
}

const FIRST_KIND: Record<TrackKind, LearningKind> = { platform: "room", ctf: "challenge", lab: "practical" };
const STATUS_ORDER: LearningStatus[] = ["doing", "todo", "done", "dropped"];

function hostOf(url: string): string {
  try {
    return new URL(url).hostname;
  } catch {
    return url;
  }
}

export function TimerBar({ timer }: { timer: Timer }) {
  const api = useApi();
  const act = useAct();
  const elapsed = useElapsed(timer.startedAt);
  return (
    <div className={styles.timer} data-pigment={timer.pigment} role="status">
      <span className={styles.timerPulse} aria-hidden="true" />
      <div className={styles.timerText}>
        <small>
          {id.learning.timerOn} · {timer.trackName}
        </small>
        <strong>{timer.itemTitle}</strong>
      </div>
      <span className={styles.timerClock} aria-label={span(Math.floor(elapsed / 60))}>
        {clockOf(elapsed)}
      </span>
      <button
        type="button"
        className="btn btn-primary"
        data-sound="sessionStop"
        onClick={() =>
          void act(
            () => api.stopTimer(),
            (minutes) => (minutes ? { text: id.learning.timerStopped(span(minutes)) } : undefined),
          )
        }
      >
        <Pause size={15} aria-hidden="true" />
        {id.learning.stop}
      </button>
      <button
        type="button"
        className="icon-btn"
        aria-label={id.learning.discard}
        title={id.learning.discard}
        onClick={() => void act(() => api.discardTimer(), { text: id.learning.timerDiscarded })}
      >
        <X size={16} aria-hidden="true" />
      </button>
    </div>
  );
}

export function LearningPage() {
  const search: LearningSearch = useSearch({ strict: false });
  const navigate = useNavigate();
  const now = useNow();
  const todayIso = isoDate(now);
  const overview = useLearningOverview();
  const tracks = useTracks();
  const [dialog, setDialog] = useState<{ kind: "track"; track?: Track } | { kind: "ctf" } | null>(null);

  const trackList = tracks.data ?? [];
  const track = trackList.find((t) => t.id === search.track) ?? trackList[0];
  const go = (next: LearningSearch) => void navigate({ to: "/belajar", search: next });
  const view = overview.data;
  const left = view ? Math.max(0, view.goalDays - view.activeDays) : 0;
  // P2: the week's goal reached while the owner is here is a small celebration, once.
  const sound = useSound();
  const before = useRef<number | null>(null);
  useEffect(() => {
    if (!view) return;
    const was = before.current;
    before.current = view.activeDays;
    if (was !== null && was < view.goalDays && view.activeDays >= view.goalDays) sound.play("hero");
  }, [view, sound]);

  return (
    <div className={styles.page}>
      <header className={styles.header}>
        <h1 className={`display ${styles.title}`}>{id.learning.title}</h1>
        {view && (
          <section className={styles.week} aria-label={id.learning.week}>
            <WeekDots days={view.days} todayIso={todayIso} />
            <div className={styles.weekText}>
              <strong>{id.learning.activeDays(view.activeDays, view.goalDays)}</strong>
              <span>
                {left === 0 ? id.learning.goalMet : id.learning.goalLeft(left)}{" "}
                {view.minutesWeek > 0 && `· ${id.learning.minutesWeek(span(view.minutesWeek))}`}
              </span>
            </div>
          </section>
        )}
      </header>

      {view?.timer && <TimerBar timer={view.timer} />}

      <div className={styles.columns}>
        <section className={styles.main} aria-label={id.learning.tracks}>
          <div className={styles.tabs} role="tablist" aria-label={id.learning.tracks}>
            {trackList.map((t) => (
              <button
                key={t.id}
                type="button"
                role="tab"
                aria-selected={t.id === track?.id}
                className={styles.tab}
                data-pigment={t.pigment}
                onClick={() => go({ track: t.id })}
              >
                <i aria-hidden="true" />
                {t.name}
                <small>
                  {t.done}/{t.items}
                </small>
              </button>
            ))}
            <button
              type="button"
              className={`${styles.tab} ${styles.tabAdd}`}
              onClick={() => setDialog({ kind: "track" })}
              aria-label={id.learning.newTrack}
              title={id.learning.newTrack}
            >
              <Plus size={16} aria-hidden="true" />
            </button>
          </div>

          {tracks.isSuccess && trackList.length === 0 && <p className={styles.quiet}>{id.learning.emptyTracks}</p>}
          {track && (
            <TrackView
              track={track}
              selected={search.item ?? null}
              onOpen={(itemId) => go({ track: track.id, item: itemId })}
              onEdit={() => setDialog({ kind: "track", track })}
              onNewCtf={() => setDialog({ kind: "ctf" })}
            />
          )}
        </section>

        {search.item ? (
          <ItemPanel itemId={search.item} onClose={() => go({ track: track?.id })} />
        ) : (
          <aside className={styles.side}>
            {view?.current && !view.timer && (
              <CurrentItem item={view.current} onOpen={() => go({ track: view.current?.trackId, item: view.current?.id })} />
            )}
            <section aria-labelledby="heatmap-title" className={styles.block}>
              <h2 className="section-label" id="heatmap-title">
                {id.learning.heatmap}
              </h2>
              {view && <Heatmap cells={view.heatmap} todayIso={todayIso} />}
            </section>
            <section aria-labelledby="domains-title" className={styles.block}>
              <h2 className="section-label" id="domains-title">
                {id.learning.domainsTitle}
              </h2>
              {view && <DomainBars domains={view.domains} />}
            </section>
          </aside>
        )}
      </div>

      {dialog?.kind === "track" && (
        <TrackDialog track={dialog.track} onClose={() => setDialog(null)} onCreated={(trackId) => go({ track: trackId })} />
      )}
      {dialog?.kind === "ctf" && <CtfDialog onClose={() => setDialog(null)} />}
    </div>
  );
}

function CurrentItem({ item, onOpen }: { item: LearningCard; onOpen: () => void }) {
  const api = useApi();
  const act = useAct();
  return (
    <section className={styles.current} data-pigment={item.pigment} aria-labelledby="current-title">
      <h2 className="section-label" id="current-title">
        {id.learning.current}
      </h2>
      <button type="button" className={styles.currentTitle} onClick={onOpen}>
        {item.title}
      </button>
      <span className={styles.currentMeta}>
        {item.trackName} · {item.minutes > 0 ? span(item.minutes) : id.learning.noTime}
      </span>
      <div className={styles.currentButtons}>
        <button type="button" className="btn btn-primary" data-sound="sessionStart" onClick={() => void act(() => api.startTimer(item.id))}>
          <Play size={15} aria-hidden="true" />
          {id.learning.resume}
        </button>
        <FocusButton itemId={item.id} />
      </div>
    </section>
  );
}

function TrackView({
  track,
  selected,
  onOpen,
  onEdit,
  onNewCtf,
}: {
  track: Track;
  selected: string | null;
  onOpen: (itemId: string) => void;
  onEdit: () => void;
  onNewCtf: () => void;
}) {
  const api = useApi();
  const act = useAct();
  const toast = useToast();
  const now = useNow();
  const items = useLearning({ trackId: track.id });
  const ctf = useCtfEvents();
  const overview = useLearningOverview();
  const [title, setTitle] = useState("");
  const timing = overview.data?.timer?.itemId ?? null;
  const list = items.data ?? [];

  const add = async () => {
    const text = title.trim();
    if (!text) return;
    setTitle("");
    const created = await act(() =>
      api.createLearning({
        trackId: track.id,
        title: text,
        kind: FIRST_KIND[track.kind],
        domain: null,
        difficulty: null,
        status: null,
        url: null,
        points: null,
        progress: null,
        notes: "",
        ctfEventId: null,
      }),
    );
    if (created.ok) {
      toast({ text: id.common.saved, action: { label: id.common.edit, run: () => onOpen(created.value) } });
    }
  };

  return (
    <div className={styles.track}>
      <div className={styles.trackHead}>
        <span>{id.learning.trackKinds[track.kind]}</span>
        {track.url && (
          <button type="button" className="btn btn-quiet" onClick={() => void act(() => api.openLink("track", track.id))}>
            <ExternalLink size={14} aria-hidden="true" />
            {hostOf(track.url)}
          </button>
        )}
        <button type="button" className="icon-btn" aria-label={id.learning.editTrack} title={id.learning.editTrack} onClick={onEdit}>
          <Settings2 size={16} aria-hidden="true" />
        </button>
      </div>

      {track.kind === "ctf" && (
        <section className={styles.ctfList} aria-label={id.learning.ctfEvents}>
          {(ctf.data ?? []).map((event) => (
            <article key={event.id} className={styles.ctf}>
              <Trophy size={18} aria-hidden="true" />
              <div>
                <strong>{event.name}</strong>
                <span>
                  {[
                    event.startsOn ? relativeDate(event.startsOn, now) : null,
                    event.rank ? id.learning.ctfOf(event.rank, event.participants) : null,
                    id.learning.ctfSolved(event.solved, event.challenges, event.points),
                  ]
                    .filter(Boolean)
                    .join(" · ")}
                </span>
              </div>
            </article>
          ))}
          <button type="button" className="btn btn-quiet" onClick={onNewCtf}>
            <Plus size={15} aria-hidden="true" />
            {id.learning.newCtf}
          </button>
        </section>
      )}

      <div className={styles.add}>
        <input
          className="input"
          aria-label={id.learning.newItem}
          placeholder={`${id.learning.newItem} di ${track.name}`}
          value={title}
          maxLength={200}
          onChange={(e) => setTitle(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") void add();
          }}
        />
      </div>

      {items.isSuccess && list.length === 0 && <p className={styles.quiet}>{id.learning.emptyTrack}</p>}
      {STATUS_ORDER.map((status) => {
        const group = list.filter((item) => item.status === status);
        if (group.length === 0) return null;
        return (
          <section key={status} aria-labelledby={`status-${status}`} className={styles.group}>
            <h3 className="section-label" id={`status-${status}`}>
              {id.learning.statuses[status]} <em>{group.length}</em>
            </h3>
            <ul className={styles.items}>
              {group.map((item) => (
                <li key={item.id} className={item.id === selected ? styles.selected : undefined}>
                  <button
                    type="button"
                    className={styles.itemBody}
                    onClick={() => onOpen(item.id)}
                    aria-current={item.id === selected ? "true" : undefined}
                  >
                    <strong>{item.title}</strong>
                    <span className={styles.itemMeta}>
                      <span className="chip" data-pigment={item.pigment}>
                        {id.learning.domains[item.domain]}
                      </span>
                      <span>{id.learning.kinds[item.kind]}</span>
                      {item.difficulty && <span>{id.learning.difficulties[item.difficulty]}</span>}
                      {item.points !== null && <span>{item.points} poin</span>}
                      <span>{item.minutes > 0 ? span(item.minutes) : id.learning.noTime}</span>
                      {item.source === "ultron" && <span className="ultron-mark">{id.task.byUltron}</span>}
                    </span>
                    {item.progress !== null && item.status !== "done" && (
                      <span
                        className={styles.progress}
                        role="progressbar"
                        aria-label={`${id.learning.progress} ${item.title}`}
                        aria-valuenow={item.progress}
                        aria-valuemin={0}
                        aria-valuemax={100}
                      >
                        <i style={{ transform: `scaleX(${item.progress / 100})` }} />
                      </span>
                    )}
                  </button>
                  {status !== "done" && status !== "dropped" && (
                    <button
                      type="button"
                      className={`btn ${timing === item.id ? "btn-secondary" : "btn-quiet"}`}
                      disabled={timing === item.id}
                      data-sound="sessionStart"
                      onClick={() => void act(() => api.startTimer(item.id))}
                      aria-label={`${id.learning.resume}: ${item.title}`}
                    >
                      <Play size={14} aria-hidden="true" />
                      {timing === item.id ? id.learning.timerOn : id.learning.resume}
                    </button>
                  )}
                </li>
              ))}
            </ul>
          </section>
        );
      })}
    </div>
  );
}
