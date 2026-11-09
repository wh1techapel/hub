// Hari Ini: the greeting, the Day Arc, what is happening now, the day's focus and other tasks,
// the schedule, and the last few things that changed. Reads one view from the core; the clock
// decides what is "now". From the evening it invites Tutup Hari.

import { useNavigate, useSearch } from "@tanstack/react-router";
import { Moon, Sparkles } from "lucide-react";

import { TaskList } from "../../components/TaskList";
import { useAct } from "../../components/Toast";
import { useApi } from "../../data/api";
import type { TaskCard } from "../../data/generated/TaskCard";
import { failedForGood, useSettings, useToday } from "../../data/queries";
import { id } from "../../strings/id";
import { useNow } from "../../time/clock";
import { clock, greeting, isoDate, minutesOf, parseClock, span } from "../../time/format";
import { DayArc } from "./DayArc";
import { momentAt } from "./arcGeometry";
import { NowCard } from "./NowCard";
import { ReviewSheet } from "./ReviewSheet";
import { Schedule } from "./Schedule";
import { LearningCardToday, RecentFiles } from "./TodayCards";
import styles from "./today.module.css";
import { Trail } from "./Trail";

/** Tutup Hari is offered from 18.30 (CONTEXT.md). */
const INVITE_FROM = 18 * 60 + 30;

export function TodayPage() {
  const now = useNow();
  const todayIso = isoDate(now);
  const settings = useSettings();
  const today = useToday(todayIso);
  const api = useApi();
  const act = useAct();
  const navigate = useNavigate();
  const search: { review?: true } = useSearch({ strict: false });

  const view = today.data;
  if (!settings.data) return null;
  const wake = parseClock(settings.data.wakeTime) ?? 300;
  const sleep = parseClock(settings.data.sleepTime) ?? 1350;
  const minute = minutesOf(now);

  // A refresh that fails keeps the day on screen.
  if (failedForGood(today)) {
    return (
      <div className={styles.page} role="alert">
        <p>{id.today.loadError}</p>
        <button type="button" className="btn btn-secondary" onClick={() => void today.refetch()}>
          {id.today.retry}
        </button>
      </div>
    );
  }
  if (!view) return null;

  const moment = momentAt(view.items, minute);
  const timed = view.items.filter((item) => !item.allDay);
  const focusDone = view.focus.filter((task) => task.status === "done").length;
  const itemsDone = timed.filter((item) => item.endMinute <= minute).length;
  const tallyTotal = view.focus.length + timed.length;
  const inviting = !view.closed && minute >= INVITE_FROM;
  const openTask = (task: TaskCard) => void navigate({ to: "/proyek", search: { view: "anytime", task: task.id } });
  const openReview = () => void navigate({ to: "/", search: { review: true } });
  const closeReview = () => void navigate({ to: "/", search: {}, replace: true });
  const takeSuggestions = () =>
    void act(() =>
      api.setFocus(
        todayIso,
        view.suggestions.map((t) => t.id),
      ),
    );

  const lead = moment.current
    ? id.today.nowUntil(moment.current.title, clock(moment.current.endMinute))
    : moment.next
      ? id.today.nextAt(moment.next.title, clock(moment.next.startMinute))
      : null;

  return (
    <div className={styles.page}>
      <section className={styles.hero}>
        {view.closed ? (
          <>
            <h1 className={`display ${styles.greeting}`}>
              {id.greeting.closed} <em>{id.greeting.rest(settings.data.ownerName)}</em>
            </h1>
            <p className={styles.heroSub}>{id.today.closedLine}</p>
          </>
        ) : (
          <>
            <h1 className={`display ${styles.greeting}`}>
              {greeting(minute, wake)}, <em>{settings.data.ownerName}.</em>
            </h1>
            <p className={styles.heroSub}>
              {lead ? (
                <>
                  {lead.lead}
                  <strong>{lead.title}</strong>
                  {lead.tail}
                </>
              ) : timed.length ? (
                id.today.nothingLeft
              ) : (
                id.today.emptyDay
              )}
              {view.focus.length > 0 && id.today.focusTally(focusDone, view.focus.length)}
            </p>
            {view.load.over && <p className={styles.over}>{id.today.overFull(span(view.load.scheduledMin + view.load.plannedMin))}</p>}
          </>
        )}
      </section>

      <DayArc events={view.items} reminders={view.reminders} minute={minute} wake={wake} sleep={sleep} closed={view.closed} />

      {inviting && (
        <section className={styles.evening} aria-labelledby="invite-title">
          <div>
            <strong id="invite-title">{tallyTotal > 0 ? id.today.eveningTally(focusDone + itemsDone, tallyTotal) : id.review.title}</strong>
            <span>{id.today.eveningInvite}</span>
          </div>
          <button type="button" className="btn btn-primary" onClick={openReview}>
            <Moon size={16} aria-hidden="true" />
            {id.today.closeDay}
          </button>
        </section>
      )}

      <div className={styles.columns}>
        <div className={styles.colMain}>
          <section aria-labelledby="now-title">
            <h2 className="section-label" id="now-title">
              {id.today.now}
            </h2>
            <NowCard moment={moment} hasEvents={timed.length > 0} reminders={view.reminders} minute={minute} sleep={sleep} />
          </section>

          <section aria-labelledby="focus-title">
            <div className={styles.sectionHead}>
              <h2 className="section-label" id="focus-title">
                {id.today.focus}
              </h2>
              {view.focus.length > 0 && <span className={styles.count}>{id.today.focusCount(focusDone, view.focus.length)}</span>}
            </div>
            {view.focus.length > 0 ? (
              <TaskList tasks={view.focus} label={id.today.focus} today={now} todayIso={todayIso} onOpen={openTask} />
            ) : view.suggestions.length > 0 ? (
              <div className={styles.suggest}>
                <p className={styles.suggestHead}>
                  <Sparkles size={15} aria-hidden="true" />
                  <strong>{id.today.suggestions}</strong>
                  <span>{id.today.suggestionsHint}</span>
                </p>
                <TaskList tasks={view.suggestions} label={id.today.suggestions} today={now} todayIso={todayIso} quiet onOpen={openTask} />
                <button type="button" className="btn btn-secondary" onClick={takeSuggestions}>
                  {id.today.useSuggestions}
                </button>
              </div>
            ) : (
              <p className={styles.empty}>{id.today.noSuggestions}</p>
            )}
          </section>

          <section aria-labelledby="also-title">
            <div className={styles.sectionHead}>
              <h2 className="section-label" id="also-title">
                {id.today.also}
              </h2>
            </div>
            {view.also.length > 0 ? (
              <TaskList tasks={view.also} label={id.today.also} today={now} todayIso={todayIso} quiet reorderable onOpen={openTask} />
            ) : (
              <p className={styles.empty}>{id.today.alsoEmpty}</p>
            )}
          </section>
        </div>

        <div className={styles.colSide}>
          <section aria-labelledby="schedule-title">
            <div className={styles.sectionHead}>
              <h2 className="section-label" id="schedule-title">
                {id.today.schedule}
              </h2>
            </div>
            <Schedule events={view.items} reminders={view.reminders} minute={minute} />
          </section>

          <section aria-labelledby="learning-title">
            <div className={styles.sectionHead}>
              <h2 className="section-label" id="learning-title">
                {id.learning.title}
              </h2>
            </div>
            <LearningCardToday glance={view.learning} todayIso={todayIso} />
          </section>

          <section aria-labelledby="files-title">
            <div className={styles.sectionHead}>
              <h2 className="section-label" id="files-title">
                {id.files.recent}
              </h2>
            </div>
            <RecentFiles files={view.files} />
          </section>

          <section aria-labelledby="trail-title">
            <div className={styles.sectionHead}>
              <h2 className="section-label" id="trail-title">
                {id.today.trail}
              </h2>
            </div>
            <Trail items={view.recent} now={now} />
          </section>
        </div>
      </div>

      {search.review && <ReviewSheet date={todayIso} now={now} onClose={closeReview} />}
    </div>
  );
}
