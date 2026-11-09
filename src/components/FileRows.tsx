// The files attached to something (a project, a task, a learning item): open one, show it in
// Explorer, take it off, or add more. Dropping files from Explorer onto the page does the same.

import { Link } from "@tanstack/react-router";
import { FolderOpen, Paperclip, X } from "lucide-react";

import type { Api } from "../data/api";
import { useApi } from "../data/api";
import type { FileCard } from "../data/generated/FileCard";
import type { LinkTarget } from "../data/generated/LinkTarget";
import { id } from "../strings/id";
import { FileThumb } from "./FileThumb";
import styles from "./fileRows.module.css";
import { useAct } from "./Toast";
import type { ToastMessage } from "./Toast";

/** Open a file and say how it went (its Vault copy, or Explorer for a program). */
export async function openFile(api: Api, act: ReturnType<typeof useAct>, file: FileCard): Promise<void> {
  await act(
    () => api.openFile(file.id),
    (opened): ToastMessage | undefined =>
      opened === "vault" ? { text: id.files.openedVault } : opened === "revealed" ? { text: id.files.revealedProgram } : undefined,
  );
}

export function FileRows({ files, target, label }: { files: readonly FileCard[]; target: LinkTarget; label: string }) {
  const api = useApi();
  const act = useAct();

  const add = () =>
    void act(
      () => api.pickFiles(target),
      (added) => (added.length ? { text: id.drop.attached(added.length, label) } : undefined),
    );

  return (
    <div className={styles.files}>
      {files.length === 0 ? (
        <p className={styles.empty}>{id.files.dropHint}</p>
      ) : (
        <ul className={styles.rows} aria-label={id.files.section}>
          {files.map((file) => (
            <li key={file.id} className={file.state === "missing" ? styles.missing : undefined}>
              <FileThumb id={file.id} kind={file.kind} missing={file.state === "missing"} size="row" />
              <button type="button" className={styles.name} onClick={() => void openFile(api, act, file)} title={file.path}>
                <span>{file.name}</span>
                <small>{file.state === "missing" ? id.files.stateMissing : id.files.size(file.sizeBytes)}</small>
              </button>
              <Link
                to="/file"
                search={{ file: file.id }}
                className="icon-btn"
                aria-label={`${id.files.detail}: ${file.name}`}
                title={id.files.detail}
              >
                <Paperclip size={15} aria-hidden="true" />
              </Link>
              <button
                type="button"
                className="icon-btn"
                aria-label={`${id.files.reveal}: ${file.name}`}
                title={id.files.reveal}
                onClick={() => void act(() => api.revealFile(file.id))}
              >
                <FolderOpen size={15} aria-hidden="true" />
              </button>
              <button
                type="button"
                className="icon-btn"
                aria-label={id.files.detach(label)}
                title={id.files.detach(label)}
                onClick={() => void act(() => api.detachFile(file.id, target))}
              >
                <X size={15} aria-hidden="true" />
              </button>
            </li>
          ))}
        </ul>
      )}
      <button type="button" className="btn btn-secondary" onClick={add}>
        <Paperclip size={16} aria-hidden="true" />
        {id.files.add}
      </button>
    </div>
  );
}
