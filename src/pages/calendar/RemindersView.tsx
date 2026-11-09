// Pengingat (in Kalender since R6, `/kalender?view=pengingat`): what went off and waits for the
// owner, what is still to come, and this week's handled ones. New reminders come from Catat Cepat
// ("ingatkan aku ... jam 7 malam") or the line at the top.

import { CaptureForm } from "../../components/CaptureForm";
import { useAct, useToast } from "../../components/Toast";
import { useApi } from "../../data/api";
import type { ReminderItem } from "../../data/generated/ReminderItem";
import { useReminders } from "../../data/queries";
import { id } from "../../strings/id";
import { useNow } from "../../time/clock";
import { clock, isoDate, minutesOf, relativeDate } from "../../time/format";
import styles from "./reminders.module.css";

function when(item: ReminderItem, now: Date): string {
  const at = new Date(item.dueAt);
  return `${relativeDate(isoDate(at), now)}, ${clock(minutesOf(at))}`;
}

export function RemindersView() {
  const reminders = useReminders();
  const now = useNow();
  const api = useApi();
  const act = useAct();
  const toast = useToast();
  const list = reminders.data ?? [];
  const fired = list.filter((r) => r.state === "fired");
  const coming = list.filter((r) => r.state === "scheduled" || r.state === "snoozed");
  const handled = list.filter((r) => r.state === "done" || r.state === "cancelled");

  const row = (item: ReminderItem, actions: "fired" | "coming" | "handled") => (
    <li key={item.id} className={styles.reminder}>
      <div>
        <span className={styles.reminderText}>{item.text}</span>
        <span className={styles.reminderWhen}>
          {when(item, now)}
          {actions === "handled" && ` · ${item.state === "done" ? id.reminders.handledDone : id.reminders.cancelled}`}
          {item.source === "ultron" && <span className="ultron-mark"> {id.task.byUltron}</span>}
        </span>
      </div>
      {actions === "fired" && (
        <div className={styles.reminderActions}>
          <button type="button" className="btn btn-secondary" onClick={() => void act(() => api.snoozeReminder(item.id, 10))}>
            {id.reminders.snooze}
          </button>
          <button type="button" className="btn btn-primary" onClick={() => void act(() => api.doneReminder(item.id))}>
            {id.reminders.done}
          </button>
        </div>
      )}
      {actions === "coming" && (
        <button
          type="button"
          className="btn btn-quiet"
          onClick={() => void act(() => api.cancelReminder(item.id), { text: id.reminders.cancelled })}
        >
          {id.reminders.cancel}
        </button>
      )}
    </li>
  );

  return (
    <div className={styles.reminders}>
      <div className={styles.add}>
        <CaptureForm kinds={["reminder"]} compact onSaved={() => toast({ text: id.capture.saved.reminder })} />
      </div>
      {list.length === 0 && !reminders.isPending && <p className={styles.empty}>{id.reminders.empty}</p>}
      {fired.length > 0 && (
        <section aria-labelledby="fired-title">
          <h3 className="section-label" id="fired-title">
            {id.reminders.fired}
          </h3>
          <ul className={styles.reminderList}>{fired.map((r) => row(r, "fired"))}</ul>
        </section>
      )}
      {coming.length > 0 && (
        <section aria-labelledby="coming-title">
          <h3 className="section-label" id="coming-title">
            {id.reminders.upcoming}
          </h3>
          <ul className={styles.reminderList}>{coming.map((r) => row(r, "coming"))}</ul>
        </section>
      )}
      {handled.length > 0 && (
        <section aria-labelledby="handled-title">
          <h3 className="section-label" id="handled-title">
            {id.reminders.handled}
          </h3>
          <ul className={`${styles.reminderList} ${styles.quietList}`}>{handled.map((r) => row(r, "handled"))}</ul>
        </section>
      )}
    </div>
  );
}
