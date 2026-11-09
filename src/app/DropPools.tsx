// Drop pools (docs/design.md, "Signature elements"): files dragged from Explorer over Hub dim the
// page and show where they can go: the Library, and the project, task, or learning item open on
// the page. The pool under the pointer swells; the drop goes to it (to the Library when it lands
// anywhere else). Windows tells Rust the paths; this page only learns the drop number, the
// pointer, and the names (src-tauri/src/library.rs).

import { useNavigate, useRouterState } from "@tanstack/react-router";
import { BookOpen, CheckSquare, FolderKanban, Library } from "lucide-react";
import type { LucideIcon } from "lucide-react";
import { useEffect, useRef, useState } from "react";

import { useAct, useToast } from "../components/Toast";
import type { Drag } from "../data/api";
import { useApi } from "../data/api";
import { useBackend } from "../data/backend";
import type { LinkTarget } from "../data/generated/LinkTarget";
import type { Pigment } from "../data/generated/Pigment";
import { useLearningItem, useProject, useTask } from "../data/queries";
import { id } from "../strings/id";
import styles from "./dropPools.module.css";

interface Pool {
  key: string;
  icon: LucideIcon;
  title: string;
  hint: string;
  target: LinkTarget | null;
  pigment: Pigment | null;
}

/** The pools for the page on screen. */
function usePools(): Pool[] {
  const location = useRouterState({ select: (state) => state.location });
  const search = location.search as { task?: string; item?: string };
  const projectId = /^\/proyek\/([^/]+)$/.exec(location.pathname)?.[1] ?? null;
  const taskId = location.pathname === "/proyek" && typeof search.task === "string" ? search.task : null;
  const itemId = location.pathname === "/belajar" && typeof search.item === "string" ? search.item : null;
  const project = useProject(projectId ?? "", projectId !== null);
  const task = useTask(taskId);
  const item = useLearningItem(itemId);

  const pools: Pool[] = [{ key: "library", icon: Library, title: id.drop.library, hint: id.drop.libraryHint, target: null, pigment: null }];
  if (projectId && project.data) {
    const { summary } = project.data;
    pools.push({
      key: "project",
      icon: FolderKanban,
      title: summary.name,
      hint: id.drop.project,
      target: { kind: "project", id: summary.id },
      pigment: summary.pigment,
    });
  }
  if (taskId && task.data) {
    const { card } = task.data;
    pools.push({
      key: "task",
      icon: CheckSquare,
      title: card.title,
      hint: id.drop.task,
      target: { kind: "task", id: card.id },
      pigment: card.project?.pigment ?? null,
    });
  }
  if (itemId && item.data) {
    const { card } = item.data;
    pools.push({
      key: "learning",
      icon: BookOpen,
      title: card.title,
      hint: id.drop.learning,
      target: { kind: "learning_item", id: card.id },
      pigment: card.pigment,
    });
  }
  return pools;
}

function poolAt(x: number, y: number): string | null {
  return document.elementFromPoint(x, y)?.closest<HTMLElement>("[data-pool]")?.dataset.pool ?? null;
}

export function DropPools() {
  const backend = useBackend();
  const api = useApi();
  const act = useAct();
  const toast = useToast();
  const navigate = useNavigate();
  const pools = usePools();
  const [drag, setDrag] = useState<{ count: number; names: string[] } | null>(null);
  const [over, setOver] = useState<string | null>(null);
  const latest = useRef(pools);

  useEffect(() => {
    latest.current = pools;
  });

  useEffect(() => {
    const keep = async (dropId: number, key: string) => {
      const pool = latest.current.find((p) => p.key === key) ?? latest.current[0];
      const result = await act(() => api.keepDrop(dropId, pool?.target ?? null));
      if (!result.ok || result.value.length === 0) return;
      const added = result.value;
      const fresh = added.filter((a) => !a.existed);
      const text = pool?.target ? id.drop.attached(added.length, pool.title) : fresh.length ? id.drop.saved(added.length) : id.drop.already;
      const first = added[0];
      // A single file gets "Beri catatan"; new files get "Urungkan".
      const undo = fresh.length
        ? { label: id.common.undo, run: () => void Promise.all(fresh.map((a) => api.deleteFile(a.id))) }
        : undefined;
      const note =
        added.length === 1 && first
          ? { label: id.drop.addNote, run: () => void navigate({ to: "/file", search: { file: first.id } }) }
          : undefined;
      toast({
        text: added.some((a) => a.wantsCopy) ? `${text}. ${id.drop.copying}` : text,
        action: undo ?? note,
        second: undo ? note : undefined,
      });
    };

    return backend.on("hub://drag", (payload) => {
      const event = payload as Drag;
      switch (event.phase) {
        case "enter":
          setDrag({ count: event.count, names: event.names });
          setOver(null);
          break;
        case "over":
          setOver(poolAt(event.x, event.y));
          break;
        case "drop": {
          const key = poolAt(event.x, event.y) ?? "library";
          setDrag(null);
          setOver(null);
          if (event.dropId !== null) void keep(event.dropId, key);
          break;
        }
        case "leave":
          setDrag(null);
          setOver(null);
          break;
      }
    });
  }, [backend, api, act, toast, navigate]);

  if (!drag) return null;
  return (
    <div className={styles.overlay} role="dialog" aria-modal="false" aria-label={id.drop.release}>
      <div className={styles.head}>
        <strong>{id.drop.release}</strong>
        <span>
          {id.drop.files(drag.count)}
          {drag.names.length > 0 && `: ${drag.names.join(", ")}${drag.count > drag.names.length ? ", ..." : ""}`}
        </span>
      </div>
      <div className={styles.pools}>
        {pools.map((pool) => (
          <div
            key={pool.key}
            className={`${styles.pool} ${over === pool.key ? styles.over : ""}`}
            data-pool={pool.key}
            data-pigment={pool.pigment ?? undefined}
          >
            <pool.icon size={22} strokeWidth={1.5} aria-hidden="true" />
            <strong>{pool.title}</strong>
            <span>{pool.hint}</span>
          </div>
        ))}
      </div>
    </div>
  );
}
