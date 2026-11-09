// Creating or changing an event (or a task's time block). Every time in it is a field, so the
// keyboard can do everything a drag on the calendar does.

import { useState } from "react";

import { Dialog } from "../../components/Dialog";
import { Field, PigmentPicker, Switch, timeValue } from "../../components/Fields";
import { useAct } from "../../components/Toast";
import { useApi } from "../../data/api";
import type { EventInput } from "../../data/generated/EventInput";
import { id } from "../../strings/id";
import { parseClock } from "../../time/format";

export interface EventDraft extends EventInput {
  /** Present when editing. */
  id?: string;
}

export function blankEvent(date: string, startMinute: number, endMinute = startMinute + 60): EventDraft {
  return {
    title: "",
    location: "",
    notes: "",
    allDay: false,
    date,
    endDate: null,
    startMinute,
    endMinute,
    pigment: "batu",
    projectId: null,
    taskId: null,
  };
}

export function EventDialog({ draft, onClose, taskTitle }: { draft: EventDraft; onClose: () => void; taskTitle?: string }) {
  const api = useApi();
  const act = useAct();
  const [value, setValue] = useState<EventDraft>(draft);
  const [error, setError] = useState<string | null>(null);
  const editing = draft.id !== undefined;
  const set = (patch: Partial<EventDraft>) => setValue((v) => ({ ...v, ...patch }));

  const save = async () => {
    if (!value.title.trim()) {
      setError(id.calendar.noTitle);
      return;
    }
    if (!value.allDay && value.endMinute <= value.startMinute) {
      setError(id.calendar.timeOrder);
      return;
    }
    const { id: eventId, ...input } = value;
    const result = await act(() => (eventId ? api.updateEvent(eventId, input) : api.createEvent(input)), { text: id.common.saved });
    if (result.ok) onClose();
  };

  const remove = async () => {
    if (!draft.id) return;
    const eventId = draft.id;
    await act(() => api.deleteEvent(eventId), {
      text: id.common.deleted(draft.title),
      action: { label: id.common.undo, run: () => void act(() => api.restoreEvent(eventId)) },
    });
    onClose();
  };

  return (
    <Dialog
      title={editing ? id.calendar.editEvent : id.calendar.newEvent}
      description={taskTitle ? `${id.calendar.task}: ${taskTitle}` : undefined}
      onClose={onClose}
      footer={
        <>
          {editing && (
            <button type="button" className="btn btn-danger" onClick={() => void remove()} style={{ marginRight: "auto" }}>
              {id.calendar.deleteEvent}
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
      <Field label={id.calendar.titleField} htmlFor="event-title" error={error === id.calendar.noTitle ? error : null}>
        <input
          id="event-title"
          className="input"
          data-autofocus
          value={value.title}
          maxLength={200}
          onChange={(e) => set({ title: e.target.value })}
          onKeyDown={(e) => {
            if (e.key === "Enter") void save();
          }}
        />
      </Field>
      <div className="row" style={{ alignItems: "center" }}>
        <span id="event-all-day">{id.calendar.allDay}</span>
        <div style={{ flex: "none" }}>
          <Switch checked={value.allDay} labelledBy="event-all-day" onChange={(allDay) => set({ allDay })} />
        </div>
      </div>
      <div className="row">
        <Field label={id.calendar.date} htmlFor="event-date">
          <input
            id="event-date"
            type="date"
            className="input"
            value={value.date}
            onChange={(e) => e.target.value && set({ date: e.target.value })}
          />
        </Field>
        {value.allDay ? (
          <Field label={id.calendar.endDate} htmlFor="event-end-date">
            <input
              id="event-end-date"
              type="date"
              className="input"
              min={value.date}
              value={value.endDate ?? value.date}
              onChange={(e) => set({ endDate: e.target.value || null })}
            />
          </Field>
        ) : (
          <>
            <Field label={id.calendar.start} htmlFor="event-start">
              <input
                id="event-start"
                type="time"
                step={300}
                className="input"
                value={timeValue(value.startMinute)}
                onChange={(e) => {
                  const start = parseClock(e.target.value);
                  if (start !== null) set({ startMinute: start, endMinute: start + (value.endMinute - value.startMinute) });
                }}
              />
            </Field>
            <Field label={id.calendar.end} htmlFor="event-end" error={error === id.calendar.timeOrder ? error : null}>
              <input
                id="event-end"
                type="time"
                step={300}
                className="input"
                value={timeValue(value.endMinute)}
                onChange={(e) => {
                  const end = parseClock(e.target.value);
                  if (end !== null) set({ endMinute: end <= value.startMinute && end < 360 ? end + 1440 : end });
                }}
              />
            </Field>
          </>
        )}
      </div>
      <Field label={id.calendar.location} htmlFor="event-location">
        <input
          id="event-location"
          className="input"
          value={value.location}
          maxLength={200}
          onChange={(e) => set({ location: e.target.value })}
        />
      </Field>
      <Field label={id.calendar.pigment}>
        <PigmentPicker label={id.calendar.pigment} value={value.pigment} onChange={(pigment) => set({ pigment })} />
      </Field>
      <Field label={id.calendar.notes} htmlFor="event-notes">
        <textarea
          id="event-notes"
          className="input"
          value={value.notes}
          maxLength={2000}
          onChange={(e) => set({ notes: e.target.value })}
        />
      </Field>
    </Dialog>
  );
}
