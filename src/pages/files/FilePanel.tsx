// One file: its picture, name in Hub, note, where it is (or that it is gone), what it is attached
// to, its Koleksi, and its Vault copy. Fields save as they are left, like the task panel.

import { AlertTriangle, ExternalLink, FolderOpen, MapPin, ShieldCheck, ShieldOff, Trash2, X } from "lucide-react";
import { useEffect, useState } from "react";

import { Field } from "../../components/Fields";
import { openFile } from "../../components/FileRows";
import { FileThumb } from "../../components/FileThumb";
import { useAct } from "../../components/Toast";
import { useApi } from "../../data/api";
import type { FileCard } from "../../data/generated/FileCard";
import { useCollections, useFile, useProjects } from "../../data/queries";
import { id } from "../../strings/id";
import { useNow } from "../../time/clock";
import { whenLabel } from "../../time/format";
import styles from "./files.module.css";

export function FilePanel({ fileId, onClose }: { fileId: string; onClose: () => void }) {
  const file = useFile(fileId);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !(event.target as HTMLElement | null)?.closest("[role=dialog]")) onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  return (
    <aside className={styles.panel} aria-label={id.files.detail}>
      <button type="button" className={`icon-btn ${styles.closePanel}`} aria-label={id.files.close} onClick={onClose}>
        <X size={18} aria-hidden="true" />
      </button>
      {file.data && <FileEditor key={file.data.id} file={file.data} onClose={onClose} />}
      {file.isError && <p className={styles.empty}>{id.files.stateMissing}</p>}
    </aside>
  );
}

function FileEditor({ file, onClose }: { file: FileCard; onClose: () => void }) {
  const api = useApi();
  const act = useAct();
  const now = useNow();
  const projects = useProjects();
  const collections = useCollections();
  const [name, setName] = useState(file.name);
  const [note, setNote] = useState(file.note);
  const [typing, setTyping] = useState(false);
  const [seen, setSeen] = useState(file);
  // A change from elsewhere replaces what is shown, unless the owner is typing (as in TaskPanel).
  if (file !== seen) {
    setSeen(file);
    if (!typing) {
      setName(file.name);
      setNote(file.note);
    }
  }

  const save = () => {
    setTyping(false);
    if (!name.trim() || (name === file.name && note === file.note)) return;
    void act(() => api.updateFile(file.id, { name, note }));
  };

  const missing = file.state === "missing";
  const attachedProjects = new Set(file.links.filter((l) => l.kind === "project").map((l) => l.id));
  const inCollection = new Set(file.collections.map((c) => c.id));

  const toggleCollection = (collectionId: string) => {
    const next = inCollection.has(collectionId) ? [...inCollection].filter((c) => c !== collectionId) : [...inCollection, collectionId];
    void act(() => api.setFileCollections(file.id, next));
  };

  const forget = async () => {
    const fileId = file.id;
    const result = await act(() => api.deleteFile(fileId), {
      text: id.files.forgotten(file.name),
      action: { label: id.common.undo, run: () => void act(() => api.restoreFile(fileId)) },
    });
    if (result.ok) onClose();
  };

  return (
    <div className={styles.editor}>
      <FileThumb id={file.id} kind={file.kind} missing={missing} size="large" />

      {missing && (
        <div className={styles.banner} role="status">
          <AlertTriangle size={18} aria-hidden="true" />
          <div>
            <strong>{id.files.stateMissing}</strong>
            <p>{id.files.stateMissingHint}</p>
            <button
              type="button"
              className="btn btn-secondary"
              onClick={() =>
                void act(
                  () => api.relocateFile(file.id),
                  (done) => (done ? { text: id.files.located } : undefined),
                )
              }
            >
              <MapPin size={15} aria-hidden="true" />
              {id.files.locate}
            </button>
          </div>
        </div>
      )}
      {file.state === "moved" && <p className={styles.note}>{id.files.stateMoved}</p>}

      <div className={styles.buttons}>
        <button type="button" className="btn btn-primary" disabled={missing && !file.vaulted} onClick={() => void openFile(api, act, file)}>
          <ExternalLink size={15} aria-hidden="true" />
          {id.files.open}
        </button>
        <button
          type="button"
          className="btn btn-secondary"
          disabled={missing && !file.vaulted}
          onClick={() => void act(() => api.revealFile(file.id))}
        >
          <FolderOpen size={15} aria-hidden="true" />
          {id.files.reveal}
        </button>
      </div>

      <Field label={id.files.name} htmlFor="file-name" hint={id.files.nameHint}>
        <input
          id="file-name"
          className="input"
          value={name}
          maxLength={260}
          onFocus={() => setTyping(true)}
          onChange={(e) => setName(e.target.value)}
          onBlur={save}
          onKeyDown={(e) => {
            if (e.key === "Enter") e.currentTarget.blur();
          }}
        />
      </Field>

      <Field label={id.files.note} htmlFor="file-note">
        <textarea
          id="file-note"
          className="input"
          rows={3}
          value={note}
          maxLength={4000}
          placeholder={id.files.notePlaceholder}
          onFocus={() => setTyping(true)}
          onChange={(e) => setNote(e.target.value)}
          onBlur={save}
        />
      </Field>

      <section className={styles.block} aria-labelledby="file-links">
        <h3 className="section-label" id="file-links">
          {id.files.attachedTo}
        </h3>
        {file.links.length === 0 ? (
          <p className={styles.note}>{id.files.notAttached}</p>
        ) : (
          <ul className={styles.links}>
            {file.links.map((link) => (
              <li key={`${link.kind}-${link.id}`} className="chip" data-pigment={link.pigment ?? "batu"}>
                <span>
                  {id.files.linkKinds[link.kind]}: {link.title}
                </span>
                <button
                  type="button"
                  aria-label={id.files.detach(link.title)}
                  onClick={() => void act(() => api.detachFile(file.id, { kind: link.kind, id: link.id }))}
                >
                  <X size={12} aria-hidden="true" />
                </button>
              </li>
            ))}
          </ul>
        )}
        <label className="sr-only" htmlFor="file-attach">
          {id.files.attach}
        </label>
        <select
          id="file-attach"
          className="input"
          value=""
          onChange={(e) => {
            if (e.target.value) void act(() => api.attachFile(file.id, { kind: "project", id: e.target.value }));
          }}
        >
          <option value="">{id.files.attach}</option>
          {(projects.data ?? [])
            .filter((p) => p.status === "active" && !attachedProjects.has(p.id))
            .map((p) => (
              <option key={p.id} value={p.id}>
                {p.name}
              </option>
            ))}
        </select>
      </section>

      {(collections.data ?? []).length > 0 && (
        <section className={styles.block} aria-labelledby="file-collections">
          <h3 className="section-label" id="file-collections">
            {id.files.collections}
          </h3>
          <div className={styles.collectionToggles}>
            {(collections.data ?? []).map((collection) => (
              <button
                key={collection.id}
                type="button"
                className={styles.toggle}
                aria-pressed={inCollection.has(collection.id)}
                data-pigment={collection.pigment}
                onClick={() => toggleCollection(collection.id)}
              >
                {collection.name}
              </button>
            ))}
          </div>
        </section>
      )}

      <section className={styles.block} aria-labelledby="file-vault">
        <h3 className="section-label" id="file-vault">
          {id.files.vault}
        </h3>
        <p className={styles.note}>{id.files.vaultHint}</p>
        {file.vaulted ? (
          <button
            type="button"
            className="btn btn-quiet"
            onClick={() => void act(() => api.removeVaultCopy(file.id), { text: id.files.vaultRemoved })}
          >
            <ShieldOff size={15} aria-hidden="true" />
            {id.files.vaultRemove}
          </button>
        ) : (
          <button
            type="button"
            className="btn btn-secondary"
            disabled={missing || file.kind === "folder"}
            onClick={() => void act(() => api.copyToVault(file.id), { text: id.files.vaultSaved })}
          >
            <ShieldCheck size={15} aria-hidden="true" />
            {id.files.vaultKeep}
          </button>
        )}
      </section>

      <dl className={styles.facts}>
        <dt>{id.files.path}</dt>
        <dd className={styles.path}>{file.path}</dd>
        <dt>{id.files.addedLabel}</dt>
        <dd>
          {whenLabel(file.addedAt, now)}
          {file.sizeBytes !== null && ` · ${id.files.size(file.sizeBytes)}`}
          {file.source === "ultron" && <span className="ultron-mark"> {id.task.byUltron}</span>}
        </dd>
      </dl>

      <div className={styles.forget}>
        <button type="button" className="btn btn-quiet" onClick={() => void forget()}>
          <Trash2 size={15} aria-hidden="true" />
          {id.files.forget}
        </button>
        <p className={styles.note}>{id.files.forgetHint}</p>
      </div>
    </div>
  );
}
