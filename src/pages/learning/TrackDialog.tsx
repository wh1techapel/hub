// A track: its name, kind, colour, and web address; for a new one or an edit (with delete).

import { useState } from "react";

import { Dialog } from "../../components/Dialog";
import { Field, PigmentPicker, Segmented } from "../../components/Fields";
import { useAct } from "../../components/Toast";
import { useApi } from "../../data/api";
import type { Track } from "../../data/generated/Track";
import type { TrackInput } from "../../data/generated/TrackInput";
import type { TrackKind } from "../../data/generated/TrackKind";
import { id } from "../../strings/id";

const KINDS: TrackKind[] = ["platform", "ctf", "lab"];

export function TrackDialog({ track, onClose, onCreated }: { track?: Track; onClose: () => void; onCreated: (id: string) => void }) {
  const api = useApi();
  const act = useAct();
  const [value, setValue] = useState<TrackInput>(() => ({
    name: track?.name ?? "",
    kind: track?.kind ?? "platform",
    url: track?.url ?? null,
    pigment: track?.pigment ?? "lumut",
  }));
  const [error, setError] = useState<string | null>(null);

  const save = async () => {
    if (!value.name.trim()) {
      setError(id.calendar.noTitle);
      return;
    }
    const input = { ...value, name: value.name.trim(), url: value.url?.trim() || null };
    if (track) {
      const result = await act(() => api.updateTrack(track.id, input), { text: id.common.saved });
      if (result.ok) onClose();
      return;
    }
    const created = await act(() => api.createTrack(input), { text: id.common.saved });
    if (created.ok) {
      onClose();
      onCreated(created.value);
    }
  };

  const remove = async () => {
    if (!track) return;
    const result = await act(() => api.deleteTrack(track.id), {
      text: id.common.deleted(track.name),
      action: { label: id.common.undo, run: () => void act(() => api.restoreTrack(track.id)) },
    });
    if (result.ok) onClose();
  };

  return (
    <Dialog
      title={track ? id.learning.editTrack : id.learning.newTrack}
      onClose={onClose}
      footer={
        <>
          {track && (
            <button type="button" className="btn btn-danger" style={{ marginRight: "auto" }} onClick={() => void remove()}>
              {id.learning.deleteTrack}
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
      <Field label={id.learning.trackName} htmlFor="track-name" error={error}>
        <input
          id="track-name"
          className="input"
          data-autofocus
          value={value.name}
          maxLength={80}
          onChange={(e) => setValue({ ...value, name: e.target.value })}
          onKeyDown={(e) => {
            if (e.key === "Enter") void save();
          }}
        />
      </Field>
      <Field label={id.learning.trackKind}>
        <Segmented
          label={id.learning.trackKind}
          options={KINDS.map((kind) => [kind, id.learning.trackKinds[kind]] as const)}
          value={value.kind}
          onChange={(kind) => setValue({ ...value, kind })}
        />
      </Field>
      <Field label={id.learning.url} htmlFor="track-url" hint={id.learning.urlHint}>
        <input
          id="track-url"
          className="input"
          type="url"
          value={value.url ?? ""}
          maxLength={2000}
          onChange={(e) => setValue({ ...value, url: e.target.value })}
        />
      </Field>
      <Field label={id.projects.pigment}>
        <PigmentPicker label={id.projects.pigment} value={value.pigment} onChange={(pigment) => setValue({ ...value, pigment })} />
      </Field>
    </Dialog>
  );
}
