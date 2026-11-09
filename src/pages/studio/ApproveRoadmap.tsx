// Approving the roadmap asks once (docs/studio.md, "Roadmap"): which Hub Proyek it belongs to
// (a new one, or one that exists), the repository folder (Windows' folder dialog, or typed and
// checked; a folder without `.git` is a warning, not a refusal), and Claude Code's access.

import { useId, useState } from "react";

import { Dialog } from "../../components/Dialog";
import { useApi } from "../../data/api";
import type { ClaudeAccess } from "../../data/generated/ClaudeAccess";
import type { FolderCheck } from "../../data/generated/FolderCheck";
import type { StudioProject } from "../../data/generated/StudioProject";
import { useProjects, useStudioAct } from "../../data/queries";
import { id } from "../../strings/id";
import styles from "./prd.module.css";
import own from "./roadmap.module.css";

const t = id.studio.roadmap;
const a = t.approval;
const ACCESS: ClaudeAccess[] = ["none", "read", "report"];

export function FolderNote({ folder }: { folder: FolderCheck | null }) {
  if (!folder) return null;
  if (!folder.exists) return <p className={styles.error} role="alert">{a.folderMissing}</p>;
  if (!folder.git) return <p className={own.warn}>{a.noGit}</p>;
  return <p className={styles.hint}>{a.gitOk}</p>;
}

/** The folder dialog, or a typed path checked by Hub. */
export function FolderChooser({ folder, onFolder }: { folder: FolderCheck | null; onFolder: (folder: FolderCheck | null) => void }) {
  const api = useApi();
  const typed = useId();
  const [text, setText] = useState("");
  const [bad, setBad] = useState(false);
  const pick = async () => {
    const picked = await api.pickRepoFolder();
    if (picked) {
      setBad(false);
      onFolder(picked);
    }
  };
  const check = async () => {
    try {
      setBad(false);
      onFolder(await api.checkRepoFolder(text));
    } catch {
      setBad(true);
      onFolder(null);
    }
  };
  return (
    <div className={styles.field}>
      {folder && <code className={own.path}>{folder.path}</code>}
      <div className={styles.actions}>
        <button type="button" className="btn btn-secondary" onClick={() => void pick()}>
          {folder ? a.pickAgain : a.pick}
        </button>
      </div>
      <label htmlFor={typed} className={styles.hint}>
        {a.orType}
      </label>
      <div className={styles.saveRow}>
        <input id={typed} className={`${styles.input} ${own.mono}`} value={text} placeholder="C:\Users\kamu\Projects\…" onChange={(event) => setText(event.target.value)} />
        <button type="button" className="btn btn-quiet" disabled={!text.trim()} onClick={() => void check()}>
          {a.check}
        </button>
      </div>
      {bad && <p className={styles.error} role="alert">{t.errors.repoPath}</p>}
      <FolderNote folder={folder} />
    </div>
  );
}

export function AccessChoice({ value, onChange, name }: { value: ClaudeAccess; onChange: (access: ClaudeAccess) => void; name: string }) {
  return (
    <div className={own.choices} role="radiogroup" aria-label={a.access}>
      {ACCESS.map((level) => (
        <label key={level} className={own.choice}>
          <input type="radio" name={name} checked={value === level} onChange={() => onChange(level)} />
          <span>
            <strong>{a.accessLevels[level]}</strong>
            <span className={styles.hint}>{a.accessHints[level]}</span>
          </span>
        </label>
      ))}
    </div>
  );
}

export function ApproveRoadmapDialog({
  project,
  proyekName,
  onClose,
  onApproved,
}: {
  project: StudioProject;
  /** The name a new Proyek gets (hub-core `roadmap::proyek_name`: the PRD's product name). */
  proyekName: string;
  onClose: () => void;
  onApproved: () => void;
}) {
  const api = useApi();
  const projects = useProjects();
  const base = useId();
  const [link, setLink] = useState<"new" | "existing">(project.projectId ? "existing" : "new");
  const [existing, setExisting] = useState(project.projectId ?? "");
  const [folder, setFolder] = useState<FolderCheck | null>(project.repoPath ? { path: project.repoPath, exists: true, git: true } : null);
  const [access, setAccess] = useState<ClaudeAccess>(project.claudeAccess);
  const approve = useStudioAct(() =>
    api.approveRoadmap(project.id, {
      link: link === "new" ? { kind: "new" } : { kind: "existing", projectId: existing },
      repoPath: folder?.path ?? "",
      claudeAccess: access,
    }),
  );
  const others = (projects.data ?? []).filter((p) => p.status !== "archived");
  const ready = folder?.exists === true && (link === "new" || existing !== "") && !approve.isPending;
  const error = approve.error ? (t.errors[approve.error.view.field ?? ""] ?? t.errors.fallback) : null;

  return (
    <Dialog
      title={a.title}
      description={a.intro}
      width="wide"
      onClose={onClose}
      footer={
        <>
          <button type="button" className="btn btn-quiet" onClick={onClose}>
            {t.cancel}
          </button>
          <button type="button" className="btn btn-primary" disabled={!ready} onClick={() => approve.mutate(undefined, { onSuccess: onApproved })}>
            {a.confirm}
          </button>
        </>
      }
    >
      <div className={styles.editor}>
        <fieldset className={styles.group}>
          <legend>{a.link}</legend>
          <label className={own.choice}>
            <input type="radio" name={`${base}-link`} checked={link === "new"} onChange={() => setLink("new")} data-autofocus />
            <span>
              <strong>{a.linkNew(proyekName)}</strong>
              <span className={styles.hint}>{a.linkNewHint}</span>
            </span>
          </label>
          <label className={own.choice}>
            <input type="radio" name={`${base}-link`} checked={link === "existing"} onChange={() => setLink("existing")} />
            <span>
              <strong>{project.projectName ? a.linkKeep(project.projectName) : a.linkExisting}</strong>
            </span>
          </label>
          {link === "existing" && (
            <div className={styles.field}>
              <label htmlFor={`${base}-project`}>{a.chooseProject}</label>
              {others.length === 0 ? (
                <p className={styles.hint}>{a.noProjects}</p>
              ) : (
                <select id={`${base}-project`} className={styles.input} value={existing} onChange={(event) => setExisting(event.target.value)}>
                  <option value="" disabled>
                    {a.chooseProject}
                  </option>
                  {others.map((p) => (
                    <option key={p.id} value={p.id}>
                      {p.name}
                    </option>
                  ))}
                </select>
              )}
            </div>
          )}
        </fieldset>

        <fieldset className={styles.group}>
          <legend>{a.folder}</legend>
          <p className={styles.hint}>{a.folderHint}</p>
          <FolderChooser folder={folder} onFolder={setFolder} />
        </fieldset>

        <fieldset className={styles.group}>
          <legend>{a.access}</legend>
          <p className={styles.hint}>{a.accessAsk}</p>
          <AccessChoice value={access} onChange={setAccess} name={`${base}-access`} />
        </fieldset>
        {error && (
          <p className={styles.error} role="alert">
            {error}
          </p>
        )}
      </div>
    </Dialog>
  );
}
