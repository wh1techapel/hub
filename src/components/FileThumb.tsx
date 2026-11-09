// A file's picture: Windows' own thumbnail or icon (asked for only once the tile is on screen),
// with a kind glyph until it arrives or when there is none (a missing file).

import { Archive, File, FileCode, FileText, Film, Folder, Image } from "lucide-react";
import type { LucideIcon } from "lucide-react";
import { useEffect, useRef, useState } from "react";

import type { FileKind } from "../data/generated/FileKind";
import { useThumbnail } from "../data/queries";
import styles from "./fileThumb.module.css";

export const KIND_ICONS: Record<FileKind, LucideIcon> = {
  document: FileText,
  image: Image,
  code: FileCode,
  archive: Archive,
  media: Film,
  folder: Folder,
  other: File,
};

/** True once the element has come near the viewport (and stays true). */
function useSeen<T extends Element>(): [React.RefObject<T | null>, boolean] {
  const ref = useRef<T>(null);
  const [seen, setSeen] = useState(typeof IntersectionObserver === "undefined");
  useEffect(() => {
    const element = ref.current;
    if (seen || !element) return;
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((entry) => entry.isIntersecting)) {
          setSeen(true);
          observer.disconnect();
        }
      },
      { rootMargin: "200px" },
    );
    observer.observe(element);
    return () => observer.disconnect();
  }, [seen]);
  return [ref, seen];
}

export function FileThumb({
  id,
  kind,
  missing = false,
  size = "tile",
}: {
  id: string;
  kind: FileKind;
  missing?: boolean;
  size?: "tile" | "row" | "large";
}) {
  const [ref, seen] = useSeen<HTMLSpanElement>();
  const thumb = useThumbnail(id, seen && !missing);
  const Icon = KIND_ICONS[kind];
  const picture = thumb.data;
  return (
    <span ref={ref} className={`${styles.thumb} ${styles[size]}`} data-kind={kind} aria-hidden="true">
      {picture ? (
        <img src={picture.image} alt="" className={picture.kind === "thumbnail" ? styles.fill : styles.icon} draggable={false} />
      ) : (
        <Icon className={styles.glyph} strokeWidth={1.4} />
      )}
    </span>
  );
}
