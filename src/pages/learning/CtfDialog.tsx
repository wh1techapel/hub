// A CTF or competition: name, dates, team, and how it went. Its challenges are items of the CTF
// track that point at it.

import { useState } from "react";

import { Dialog } from "../../components/Dialog";
import { Field } from "../../components/Fields";
import { useAct } from "../../components/Toast";
import { useApi } from "../../data/api";
import type { CtfInput } from "../../data/generated/CtfInput";
import { id } from "../../strings/id";

const numberOrNull = (text: string) => (text === "" ? null : Math.max(1, Math.round(Number(text))));

export function CtfDialog({ onClose }: { onClose: () => void }) {
  const api = useApi();
  const act = useAct();
  const [value, setValue] = useState<CtfInput>({
    name: "",
    startsOn: null,
    endsOn: null,
    team: null,
    rank: null,
    participants: null,
    url: null,
    notes: "",
  });
  const [error, setError] = useState<string | null>(null);

  const save = async () => {
    if (!value.name.trim()) {
      setError(id.calendar.noTitle);
      return;
    }
    const result = await act(() => api.createCtf({ ...value, name: value.name.trim(), url: value.url?.trim() || null }), {
      text: id.common.saved,
    });
    if (result.ok) onClose();
  };

  return (
    <Dialog
      title={id.learning.newCtf}
      onClose={onClose}
      footer={
        <>
          <button type="button" className="btn btn-quiet" onClick={onClose}>
            {id.common.cancel}
          </button>
          <button type="button" className="btn btn-primary" onClick={() => void save()}>
            {id.common.save}
          </button>
        </>
      }
    >
      <Field label={id.learning.ctfName} htmlFor="ctf-name" error={error}>
        <input
          id="ctf-name"
          className="input"
          data-autofocus
          value={value.name}
          maxLength={120}
          onChange={(e) => setValue({ ...value, name: e.target.value })}
        />
      </Field>
      <div className="row">
        <Field label={id.learning.ctfStarts} htmlFor="ctf-starts">
          <input
            id="ctf-starts"
            className="input"
            type="date"
            value={value.startsOn ?? ""}
            onChange={(e) => setValue({ ...value, startsOn: e.target.value || null })}
          />
        </Field>
        <Field label={id.learning.ctfEnds} htmlFor="ctf-ends">
          <input
            id="ctf-ends"
            className="input"
            type="date"
            min={value.startsOn ?? undefined}
            value={value.endsOn ?? ""}
            onChange={(e) => setValue({ ...value, endsOn: e.target.value || null })}
          />
        </Field>
      </div>
      <div className="row">
        <Field label={id.learning.ctfTeam} htmlFor="ctf-team">
          <input
            id="ctf-team"
            className="input"
            value={value.team ?? ""}
            maxLength={80}
            onChange={(e) => setValue({ ...value, team: e.target.value || null })}
          />
        </Field>
        <Field label={id.learning.ctfRank} htmlFor="ctf-rank">
          <input
            id="ctf-rank"
            className="input"
            type="number"
            min={1}
            value={value.rank ?? ""}
            onChange={(e) => setValue({ ...value, rank: numberOrNull(e.target.value) })}
          />
        </Field>
        <Field label={id.learning.ctfParticipants} htmlFor="ctf-participants">
          <input
            id="ctf-participants"
            className="input"
            type="number"
            min={1}
            value={value.participants ?? ""}
            onChange={(e) => setValue({ ...value, participants: numberOrNull(e.target.value) })}
          />
        </Field>
      </div>
      <Field label={id.learning.url} htmlFor="ctf-url">
        <input
          id="ctf-url"
          className="input"
          type="url"
          value={value.url ?? ""}
          maxLength={2000}
          onChange={(e) => setValue({ ...value, url: e.target.value })}
        />
      </Field>
    </Dialog>
  );
}
