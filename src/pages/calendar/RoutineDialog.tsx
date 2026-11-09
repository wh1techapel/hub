// Creating or changing a routine: which days, what time, for how long, from when until when.
// The school timetable is entered here, one lesson at a time (owner's choice, CONTEXT.md).

import { useState } from "react";

import { Dialog } from "../../components/Dialog";
import { Field, PigmentPicker, Segmented, timeValue } from "../../components/Fields";
import { useAct } from "../../components/Toast";
import { useApi } from "../../data/api";
import type { Day } from "../../data/generated/Day";
import type { Routine } from "../../data/generated/Routine";
import type { RoutineInput } from "../../data/generated/RoutineInput";
import type { RoutineKind } from "../../data/generated/RoutineKind";
import { id } from "../../strings/id";
import { isoDate, parseClock } from "../../time/format";
import styles from "./calendar.module.css";

const DAYS: Day[] = ["mo", "tu", "we", "th", "fr", "sa", "su"];
const KINDS: RoutineKind[] = ["lesson", "habit", "study", "other"];

export function blankRoutine(date: string, startMinute: number, day: Day): RoutineInput {
  return {
    title: "",
    kind: "lesson",
    rule: { freq: "weekly", interval: 1, byDay: [day] },
    startTime: timeValue(startMinute),
    durationMin: 90,
    startsOn: date,
    endsOn: null,
    location: "",
    pigment: "lazuardi",
    projectId: null,
  };
}

export function RoutineDialog({ routine, draft, onClose }: { routine?: Routine; draft?: RoutineInput; onClose: () => void }) {
  const api = useApi();
  const act = useAct();
  const initial: RoutineInput = routine
    ? {
        title: routine.title,
        kind: routine.kind,
        rule: routine.rule,
        startTime: routine.startTime,
        durationMin: routine.durationMin,
        startsOn: routine.startsOn,
        endsOn: routine.endsOn,
        location: routine.location,
        pigment: routine.pigment,
        projectId: routine.projectId,
      }
    : (draft ?? blankRoutine(isoDate(new Date()), 420, "mo"));
  const [value, setValue] = useState<RoutineInput>(initial);
  const [error, setError] = useState<string | null>(null);
  const set = (patch: Partial<RoutineInput>) => setValue((v) => ({ ...v, ...patch }));
  const start = parseClock(value.startTime) ?? 0;
  const end = start + value.durationMin;

  const toggleDay = (day: Day) => {
    const byDay = value.rule.byDay.includes(day) ? value.rule.byDay.filter((d) => d !== day) : [...value.rule.byDay, day];
    set({ rule: { ...value.rule, byDay } });
  };

  const save = async () => {
    if (!value.title.trim()) {
      setError(id.calendar.noTitle);
      return;
    }
    const result = await act(() => (routine ? api.updateRoutine(routine.id, value) : api.createRoutine(value)), { text: id.common.saved });
    if (result.ok) onClose();
  };

  const remove = async () => {
    if (!routine) return;
    await act(() => api.deleteRoutine(routine.id), { text: id.common.deleted(routine.title) });
    onClose();
  };

  return (
    <Dialog
      title={routine ? id.calendar.editRoutine : id.calendar.newRoutine}
      onClose={onClose}
      width="wide"
      footer={
        <>
          {routine && (
            <button type="button" className="btn btn-danger" style={{ marginRight: "auto" }} onClick={() => void remove()}>
              {id.calendar.deleteRoutine}
            </button>
          )}
          <button type="button" className="btn btn-quiet" onClick={onClose}>
            {id.common.cancel}
          </button>
          <button type="button" className="btn btn-primary" onClick={() => void save()}>
            {id.common.save}
          </button>
        </>
      }
    >
      <Field label={id.calendar.titleField} htmlFor="routine-title" error={error}>
        <input
          id="routine-title"
          className="input"
          data-autofocus
          value={value.title}
          maxLength={200}
          onChange={(e) => set({ title: e.target.value })}
        />
      </Field>
      <Field label={id.calendar.kind}>
        <Segmented
          label={id.calendar.kind}
          options={KINDS.map((k) => [k, id.calendar.kinds[k]] as const)}
          value={value.kind}
          onChange={(kind) => set({ kind })}
        />
      </Field>
      <Field label={id.calendar.days}>
        <div className={styles.days} role="group" aria-label={id.calendar.days}>
          {DAYS.map((day) => (
            <button
              key={day}
              type="button"
              aria-pressed={value.rule.byDay.includes(day)}
              className={styles.dayToggle}
              onClick={() => toggleDay(day)}
              title={id.weekdays.long[day]}
            >
              {id.weekdays.short[day]}
            </button>
          ))}
        </div>
      </Field>
      <div className="row">
        <Field label={id.calendar.start} htmlFor="routine-start">
          <input
            id="routine-start"
            type="time"
            step={300}
            className="input"
            value={value.startTime}
            onChange={(e) => e.target.value && set({ startTime: e.target.value })}
          />
        </Field>
        <Field label={id.calendar.end} htmlFor="routine-end">
          <input
            id="routine-end"
            type="time"
            step={300}
            className="input"
            value={timeValue(end)}
            onChange={(e) => {
              const minutes = parseClock(e.target.value);
              if (minutes !== null && minutes > start) set({ durationMin: minutes - start });
            }}
          />
        </Field>
      </div>
      <div className="row">
        <Field label={id.calendar.startsOn} htmlFor="routine-from">
          <input
            id="routine-from"
            type="date"
            className="input"
            value={value.startsOn}
            onChange={(e) => e.target.value && set({ startsOn: e.target.value })}
          />
        </Field>
        <Field label={id.calendar.endsOn} htmlFor="routine-until">
          <input
            id="routine-until"
            type="date"
            className="input"
            min={value.startsOn}
            value={value.endsOn ?? ""}
            onChange={(e) => set({ endsOn: e.target.value || null })}
          />
        </Field>
      </div>
      <Field label={id.calendar.location} htmlFor="routine-location">
        <input
          id="routine-location"
          className="input"
          value={value.location}
          maxLength={200}
          onChange={(e) => set({ location: e.target.value })}
        />
      </Field>
      <Field label={id.calendar.pigment}>
        <PigmentPicker label={id.calendar.pigment} value={value.pigment} onChange={(pigment) => set({ pigment })} />
      </Field>
    </Dialog>
  );
}
