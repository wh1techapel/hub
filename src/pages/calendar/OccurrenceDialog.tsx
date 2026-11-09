// One occurrence of a routine: skip it, put a moved one back, or change the routine itself.

import { Dialog } from "../../components/Dialog";
import { useAct } from "../../components/Toast";
import { useApi } from "../../data/api";
import type { CalendarItem } from "../../data/generated/CalendarItem";
import { id } from "../../strings/id";
import { clock, relativeDate } from "../../time/format";

interface Props {
  item: CalendarItem;
  today: Date;
  onEditRoutine: (routineId: string) => void;
  onClose: () => void;
}

export function OccurrenceDialog({ item, today, onEditRoutine, onClose }: Props) {
  const api = useApi();
  const act = useAct();
  const routineId = item.routineId ?? "";
  const ruleDate = item.ruleDate ?? "";
  const when = `${relativeDate(ruleDate, today)}, ${clock(item.startMinute)} sampai ${clock(item.endMinute)}`;

  const skip = async () => {
    await act(() => api.skipOccurrence(routineId, ruleDate), {
      text: id.calendar.skipped,
      action: { label: id.common.undo, run: () => void act(() => api.restoreOccurrence(routineId, ruleDate)) },
    });
    onClose();
  };

  const restore = async () => {
    await act(() => api.restoreOccurrence(routineId, ruleDate), { text: id.common.saved });
    onClose();
  };

  return (
    <Dialog
      title={item.title}
      description={`${id.calendar.routine} · ${when}${item.location ? ` · ${item.location}` : ""}${item.moved ? ` · ${id.calendar.moved}` : ""}`}
      onClose={onClose}
      footer={
        <>
          <button type="button" className="btn btn-quiet" onClick={onClose}>
            {id.common.close}
          </button>
          <button type="button" className="btn btn-secondary" onClick={() => onEditRoutine(routineId)}>
            {id.calendar.editRoutine}
          </button>
        </>
      }
    >
      <div className="row">
        <button type="button" className="btn btn-secondary" data-autofocus onClick={() => void skip()}>
          {id.calendar.skip}
        </button>
        {item.moved && (
          <button type="button" className="btn btn-secondary" onClick={() => void restore()}>
            {id.calendar.restore}
          </button>
        )}
      </div>
    </Dialog>
  );
}
