// Atur Ruang: rename, recolour, add, or remove the life areas projects are grouped by. Removing a
// Ruang keeps its projects and tasks; they lose only the grouping, so it asks once first.

import { Trash2 } from "lucide-react";
import { useState } from "react";

import { Dialog } from "../../components/Dialog";
import { PigmentPicker } from "../../components/Fields";
import { useAct } from "../../components/Toast";
import { useApi } from "../../data/api";
import type { Area } from "../../data/generated/Area";
import type { Pigment } from "../../data/generated/Pigment";
import { useAreas } from "../../data/queries";
import { id } from "../../strings/id";
import styles from "./projects.module.css";

export function AreasDialog({ onClose }: { onClose: () => void }) {
  const areas = useAreas();
  const api = useApi();
  const act = useAct();
  const [name, setName] = useState("");
  const [pigment, setPigment] = useState<Pigment>("toska");

  const add = async () => {
    const text = name.trim();
    if (!text) return;
    const created = await act(() => api.createArea({ name: text, pigment }));
    if (created.ok) setName("");
  };

  return (
    <Dialog
      title={id.projects.areas}
      description={id.projects.areasHint}
      onClose={onClose}
      width="wide"
      footer={
        <button type="button" className="btn btn-primary" onClick={onClose}>
          {id.common.close}
        </button>
      }
    >
      <ul className={styles.areaList}>
        {(areas.data ?? []).map((area) => (
          <AreaRow key={area.id} area={area} />
        ))}
      </ul>
      <div className={styles.areaAdd}>
        <input
          className="input"
          placeholder={id.projects.newArea}
          aria-label={id.projects.newArea}
          value={name}
          maxLength={40}
          onChange={(e) => setName(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") void add();
          }}
        />
        <PigmentPicker label={id.projects.pigment} value={pigment} onChange={setPigment} />
        <button type="button" className="btn btn-secondary" onClick={() => void add()} disabled={!name.trim()}>
          {id.common.add}
        </button>
      </div>
    </Dialog>
  );
}

function AreaRow({ area }: { area: Area }) {
  const api = useApi();
  const act = useAct();
  const [name, setName] = useState(area.name);
  const [confirming, setConfirming] = useState(false);

  const save = (patch: { name?: string; pigment?: Pigment }) => {
    const next = { name: (patch.name ?? name).trim(), pigment: patch.pigment ?? area.pigment };
    if (!next.name || (next.name === area.name && next.pigment === area.pigment)) return;
    void act(() => api.updateArea(area.id, next));
  };

  return (
    <li className={styles.areaRow} data-pigment={area.pigment}>
      <i className={styles.areaDot} aria-hidden="true" />
      <input
        className="input"
        aria-label={id.projects.areaName}
        value={name}
        maxLength={40}
        onChange={(e) => setName(e.target.value)}
        onBlur={() => save({})}
        onKeyDown={(e) => {
          if (e.key === "Enter") e.currentTarget.blur();
        }}
      />
      <PigmentPicker label={`${id.projects.pigment} ${area.name}`} value={area.pigment} onChange={(pigment) => save({ pigment })} />
      {confirming ? (
        <span className={styles.confirm}>
          <span>{id.projects.deleteAreaConfirm(area.openProjects)}</span>
          <button type="button" className="btn btn-quiet" onClick={() => setConfirming(false)}>
            {id.common.cancel}
          </button>
          <button
            type="button"
            className="btn btn-danger"
            onClick={() => void act(() => api.deleteArea(area.id), { text: id.common.deleted(area.name) })}
          >
            {id.common.delete}
          </button>
        </span>
      ) : (
        <button
          type="button"
          className="icon-btn"
          aria-label={`${id.projects.deleteArea} ${area.name}`}
          onClick={() => setConfirming(true)}
        >
          <Trash2 size={16} aria-hidden="true" />
        </button>
      )}
    </li>
  );
}
