// Starts a Sesi Fokus (Lentera CP02) for a task or a Belajar item. The session lives in Lentera's
// capsule at the bottom right; a session already running for something else ends first (hub-core
// keeps one at a time), and so does Belajar's own timer.

import { Timer } from "lucide-react";

import { useApi } from "../data/api";
import { useSettings } from "../data/queries";
import { id } from "../strings/id";
import { useAct } from "./Toast";

export function FocusButton({ taskId, itemId, className = "btn btn-secondary" }: { taskId?: string; itemId?: string; className?: string }) {
  const api = useApi();
  const act = useAct();
  const settings = useSettings();
  const minutes = settings.data?.focusMinutes ?? 25;
  const rest = settings.data?.breakMinutes ?? 5;
  return (
    <button
      type="button"
      className={className}
      data-sound="sessionStart"
      title={`${id.focus.start}: ${id.focus.startHint(minutes, rest)}`}
      onClick={() => void act(() => api.startFocus({ taskId, itemId }))}
    >
      <Timer size={15} aria-hidden="true" />
      {id.focus.startFor(minutes)}
    </button>
  );
}
