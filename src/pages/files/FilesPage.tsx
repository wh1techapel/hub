// File: the Library. Kinds, Koleksi, and projects on the left; the files as tiles with Windows'
// own pictures, or as a list; one file's detail beside them. Its address carries the filter and
// the open file, so a toast, the palette, or ULTRON can open one.

import { useNavigate, useSearch } from "@tanstack/react-router";
import { AlertTriangle, FolderKanban, LayoutGrid, List, Plus, ShieldCheck } from "lucide-react";
import { useState } from "react";

import { KIND_ICONS, FileThumb } from "../../components/FileThumb";
import { openFile } from "../../components/FileRows";
import { useAct } from "../../components/Toast";
import { useApi } from "../../data/api";
import type { FileCard } from "../../data/generated/FileCard";
import type { FileKind } from "../../data/generated/FileKind";
import type { FileQuery } from "../../data/generated/FileQuery";
import type { Pigment } from "../../data/generated/Pigment";
import { useCollections, useFileCounts, useFiles, useProjects } from "../../data/queries";
import { id } from "../../strings/id";
import { useNow } from "../../time/clock";
import { whenLabel } from "../../time/format";
import { FilePanel } from "./FilePanel";
import styles from "./files.module.css";

const KINDS: FileKind[] = ["document", "image", "code", "archive", "media", "folder", "other"];
const NEW_PIGMENTS: Pigment[] = ["lumut", "lavender", "kunyit", "toska", "mawar", "lazuardi", "terakota"];

export interface FilesSearch {
  kind?: FileKind;
  collection?: string;
  project?: string;
  missing?: true;
  file?: string;
  view?: "grid" | "list";
}

export function FilesPage() {
  const search: FilesSearch = useSearch({ strict: false });
  const navigate = useNavigate();
  const api = useApi();
  const act = useAct();
  const now = useNow();
  const counts = useFileCounts();
  const collections = useCollections();
  const projects = useProjects();
  const [naming, setNaming] = useState<string | null>(null);

  const query: FileQuery = {
    kind: search.kind,
    collectionId: search.collection,
    projectId: search.project,
    state: search.missing ? "missing" : undefined,
  };
  const files = useFiles(query);
  const view = search.view ?? "grid";
  const filtered = Boolean(search.kind || search.collection || search.project || search.missing);

  const go = (next: Partial<FilesSearch>, keepFilter = false) => {
    const base: FilesSearch = keepFilter ? { ...search } : { view: search.view };
    void navigate({ to: "/file", search: { ...base, ...next } });
  };
  const open = (file: FileCard) => go({ file: file.id }, true);
  const close = () => go({ file: undefined }, true);

  const title = search.kind
    ? id.files.kinds[search.kind]
    : search.missing
      ? id.files.missing
      : search.collection
        ? (collections.data?.find((c) => c.id === search.collection)?.name ?? id.files.collections)
        : search.project
          ? (projects.data?.find((p) => p.id === search.project)?.name ?? id.files.projects)
          : id.files.all;

  const createCollection = async () => {
    const name = naming?.trim();
    if (!name) {
      setNaming(null);
      return;
    }
    const pigment = NEW_PIGMENTS[(collections.data?.length ?? 0) % NEW_PIGMENTS.length] ?? "batu";
    const created = await act(() => api.createCollection({ name, pigment }));
    setNaming(null);
    if (created.ok) go({ collection: created.value });
  };

  const kindCount = (kind: FileKind) => counts.data?.kinds.find((k) => k.kind === kind)?.count ?? 0;
  const place = (active: boolean) => (active ? "page" : undefined);

  return (
    <div className={styles.page}>
      <nav className={styles.places} aria-label={id.files.places}>
        <h1 className={`display ${styles.title}`}>{id.files.title}</h1>
        <button type="button" className={styles.place} aria-current={place(!filtered)} onClick={() => go({})}>
          <LayoutGrid size={17} strokeWidth={1.6} aria-hidden="true" />
          <span>{id.files.all}</span>
          <em>{counts.data?.all || ""}</em>
        </button>
        {KINDS.filter((kind) => kindCount(kind) > 0 || search.kind === kind).map((kind) => {
          const Icon = KIND_ICONS[kind];
          return (
            <button
              key={kind}
              type="button"
              className={styles.place}
              aria-current={place(search.kind === kind)}
              onClick={() => go({ kind })}
            >
              <Icon size={17} strokeWidth={1.6} aria-hidden="true" />
              <span>{id.files.kinds[kind]}</span>
              <em>{kindCount(kind)}</em>
            </button>
          );
        })}
        {(counts.data?.missing ?? 0) > 0 && (
          <button
            type="button"
            className={`${styles.place} ${styles.warnPlace}`}
            aria-current={place(Boolean(search.missing))}
            onClick={() => go({ missing: true })}
          >
            <AlertTriangle size={17} strokeWidth={1.6} aria-hidden="true" />
            <span>{id.files.missing}</span>
            <em>{counts.data?.missing}</em>
          </button>
        )}

        <h2 className={`section-label ${styles.railHead}`}>{id.files.collections}</h2>
        {(collections.data ?? []).map((collection) => (
          <button
            key={collection.id}
            type="button"
            className={styles.place}
            aria-current={place(search.collection === collection.id)}
            onClick={() => go({ collection: collection.id })}
          >
            <i className={styles.dot} data-pigment={collection.pigment} aria-hidden="true" />
            <span>{collection.name}</span>
            <em>{collection.files || ""}</em>
          </button>
        ))}
        {naming === null ? (
          <button type="button" className={`${styles.place} ${styles.quietPlace}`} onClick={() => setNaming("")}>
            <Plus size={16} strokeWidth={1.6} aria-hidden="true" />
            <span>{id.files.newCollection}</span>
          </button>
        ) : (
          <input
            className={`input ${styles.newCollection}`}
            aria-label={id.files.collectionName}
            placeholder={id.files.collectionName}
            value={naming}
            maxLength={80}
            autoFocus
            onChange={(e) => setNaming(e.target.value)}
            onBlur={() => void createCollection()}
            onKeyDown={(e) => {
              if (e.key === "Enter") void createCollection();
              if (e.key === "Escape") setNaming(null);
            }}
          />
        )}

        {(projects.data ?? []).some((p) => p.status === "active") && (
          <h2 className={`section-label ${styles.railHead}`}>{id.files.projects}</h2>
        )}
        {(projects.data ?? [])
          .filter((p) => p.status === "active")
          .map((project) => (
            <button
              key={project.id}
              type="button"
              className={styles.place}
              aria-current={place(search.project === project.id)}
              onClick={() => go({ project: project.id })}
            >
              <FolderKanban size={16} strokeWidth={1.6} aria-hidden="true" data-pigment={project.pigment} className={styles.projectIcon} />
              <span>{project.name}</span>
            </button>
          ))}
      </nav>

      <section className={styles.main} aria-labelledby="files-title">
        <header className={styles.head}>
          <div>
            <h2 className="display" id="files-title">
              {title}
            </h2>
            <p>{id.files.dropHint}</p>
          </div>
          <div className={styles.actions}>
            <div className={styles.views} role="group" aria-label={id.files.view}>
              <button
                type="button"
                className="icon-btn"
                aria-pressed={view === "grid"}
                aria-label={id.files.grid}
                title={id.files.grid}
                onClick={() => go({ view: "grid" }, true)}
              >
                <LayoutGrid size={17} aria-hidden="true" />
              </button>
              <button
                type="button"
                className="icon-btn"
                aria-pressed={view === "list"}
                aria-label={id.files.list}
                title={id.files.list}
                onClick={() => go({ view: "list" }, true)}
              >
                <List size={17} aria-hidden="true" />
              </button>
            </div>
            <button
              type="button"
              className="btn btn-primary"
              onClick={() =>
                void act(
                  () => api.pickFiles(null),
                  (added) => (added.length ? { text: id.drop.saved(added.length) } : undefined),
                )
              }
            >
              <Plus size={16} aria-hidden="true" />
              {id.files.add}
            </button>
          </div>
        </header>

        {files.isSuccess && files.data.length === 0 && <p className={styles.empty}>{filtered ? id.files.emptyFilter : id.files.empty}</p>}

        {view === "grid" ? (
          <ul className={styles.grid} aria-label={title}>
            {(files.data ?? []).map((file) => (
              <li key={file.id}>
                <button
                  type="button"
                  className={styles.tile}
                  aria-current={search.file === file.id ? "true" : undefined}
                  aria-label={[file.name, file.state === "missing" ? id.files.missing : null, file.vaulted ? id.files.vaulted : null]
                    .filter(Boolean)
                    .join(", ")}
                  onClick={() => open(file)}
                  onDoubleClick={() => void openFile(api, act, file)}
                >
                  <FileThumb id={file.id} kind={file.kind} missing={file.state === "missing"} />
                  <span className={styles.tileName} title={file.name}>
                    {file.name}
                  </span>
                  <span className={styles.tileMeta}>
                    {file.state === "missing" ? (
                      <span className={styles.warn}>
                        <AlertTriangle size={12} aria-hidden="true" /> {id.files.missing}
                      </span>
                    ) : (
                      <span>{id.files.size(file.sizeBytes) || id.files.kinds[file.kind]}</span>
                    )}
                    {file.vaulted && <ShieldCheck size={13} aria-hidden="true" className={styles.vault} />}
                  </span>
                </button>
              </li>
            ))}
          </ul>
        ) : (
          <ul className={styles.list} aria-label={title}>
            {(files.data ?? []).map((file) => (
              <li key={file.id}>
                <button
                  type="button"
                  className={styles.row}
                  aria-current={search.file === file.id ? "true" : undefined}
                  onClick={() => open(file)}
                  onDoubleClick={() => void openFile(api, act, file)}
                >
                  <FileThumb id={file.id} kind={file.kind} missing={file.state === "missing"} size="row" />
                  <span className={styles.rowName}>
                    <strong>{file.name}</strong>
                    <small>{file.links.map((link) => link.title).join(" · ") || file.path}</small>
                  </span>
                  <span className={styles.rowMeta}>
                    {file.state === "missing" ? <span className={styles.warn}>{id.files.missing}</span> : id.files.size(file.sizeBytes)}
                  </span>
                  <span className={styles.rowMeta}>{whenLabel(file.addedAt, now)}</span>
                </button>
              </li>
            ))}
          </ul>
        )}
      </section>

      {search.file && <FilePanel fileId={search.file} onClose={close} />}
    </div>
  );
}
