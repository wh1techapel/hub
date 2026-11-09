// A project's name, goal, Ruang, colour, and target date, for a new project or an edit.

import { useNavigate } from "@tanstack/react-router";
import { useState } from "react";

import { Dialog } from "../../components/Dialog";
import { Field, PigmentPicker } from "../../components/Fields";
import { useAct } from "../../components/Toast";
import { useApi } from "../../data/api";
import type { ProjectInput } from "../../data/generated/ProjectInput";
import { useAreas } from "../../data/queries";
import { id } from "../../strings/id";

export function blankProject(areaId: string | null = null): ProjectInput {
  return { name: "", description: "", areaId, pigment: "lazuardi", targetDate: null };
}

export function ProjectDialog({ projectId, initial, onClose }: { projectId?: string; initial: ProjectInput; onClose: () => void }) {
  const api = useApi();
  const act = useAct();
  const areas = useAreas();
  const navigate = useNavigate();
  const [value, setValue] = useState<ProjectInput>(initial);
  const [error, setError] = useState<string | null>(null);
  const set = (patch: Partial<ProjectInput>) => setValue((v) => ({ ...v, ...patch }));

  const save = async () => {
    const name = value.name.trim();
    if (!name) {
      setError(id.projects.noName);
      return;
    }
    const input = { ...value, name, description: value.description.trim() };
    if (projectId) {
      const result = await act(() => api.updateProject(projectId, input), { text: id.common.saved });
      if (result.ok) onClose();
      return;
    }
    const created = await act(() => api.createProject(input), { text: id.projects.created(name) });
    if (created.ok) {
      onClose();
      void navigate({ to: "/proyek/$projectId", params: { projectId: created.value } });
    }
  };

  return (
    <Dialog
      title={projectId ? id.projects.edit : id.projects.new}
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
      <Field label={id.projects.name} htmlFor="project-name" error={error}>
        <input
          id="project-name"
          className="input"
          data-autofocus
          value={value.name}
          maxLength={120}
          onChange={(e) => set({ name: e.target.value })}
          onKeyDown={(e) => {
            if (e.key === "Enter") void save();
          }}
        />
      </Field>
      <Field label={id.projects.description} htmlFor="project-goal" hint={id.projects.descriptionHint}>
        <textarea
          id="project-goal"
          className="input"
          rows={2}
          value={value.description}
          maxLength={2000}
          onChange={(e) => set({ description: e.target.value })}
        />
      </Field>
      <div className="row">
        <Field label={id.projects.area} htmlFor="project-area">
          <select id="project-area" className="input" value={value.areaId ?? ""} onChange={(e) => set({ areaId: e.target.value || null })}>
            <option value="">{id.projects.noArea}</option>
            {(areas.data ?? []).map((area) => (
              <option key={area.id} value={area.id}>
                {area.name}
              </option>
            ))}
          </select>
        </Field>
        <Field label={id.projects.targetDate} htmlFor="project-target">
          <input
            id="project-target"
            type="date"
            className="input"
            value={value.targetDate ?? ""}
            onChange={(e) => set({ targetDate: e.target.value || null })}
          />
        </Field>
      </div>
      <Field label={id.projects.pigment}>
        <PigmentPicker label={id.projects.pigment} value={value.pigment} onChange={(pigment) => set({ pigment })} />
      </Field>
    </Dialog>
  );
}
