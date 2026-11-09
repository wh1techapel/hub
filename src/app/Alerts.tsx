// Reminders that went off and wait for the owner, above every page until handled. The toast in
// Windows may have been missed (focus assist, away); this line is not.

import { Link } from "@tanstack/react-router";
import { Bell } from "lucide-react";

import { useAct } from "../components/Toast";
import { useApi } from "../data/api";
import { useReminders } from "../data/queries";
import { id } from "../strings/id";
import { clock, minutesOf } from "../time/format";
import styles from "./shell.module.css";

const SHOWN = 2;

export function Alerts() {
  const reminders = useReminders();
  const api = useApi();
  const act = useAct();
  const fired = (reminders.data ?? []).filter((r) => r.state === "fired");
  if (fired.length === 0) return null;

  return (
    <section className={styles.alerts} aria-label={id.reminders.alerts(fired.length)} aria-live="polite">
      {fired.slice(0, SHOWN).map((reminder) => (
        <div key={reminder.id} className={styles.alert}>
          <Bell size={16} strokeWidth={1.8} aria-hidden="true" />
          <span className={styles.alertText}>
            <strong>{reminder.text}</strong>
            <small>{clock(minutesOf(new Date(reminder.dueAt)))}</small>
          </span>
          <button type="button" className="btn btn-quiet" onClick={() => void act(() => api.snoozeReminder(reminder.id, 10))}>
            {id.reminders.snooze}
          </button>
          <button type="button" className="btn btn-primary" onClick={() => void act(() => api.doneReminder(reminder.id))}>
            {id.reminders.done}
          </button>
        </div>
      ))}
      {fired.length > SHOWN && (
        <Link to="/kalender" search={{ view: "pengingat" }} className={styles.alertMore}>
          {id.common.more(fired.length - SHOWN)}
        </Link>
      )}
    </section>
  );
}
