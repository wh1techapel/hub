// The pages, in sidebar order. The command palette and the router read the same list.
//
// R6: the main pages first (Hari Ini, Kalender, Proyek & Tugas, Studio, Belajar), then the
// secondary ones under a divider (File, Jejak), and Pengaturan at the bottom. Tugas and Pengingat
// are no longer pages of their own: their old addresses still work (router.tsx).

import { CalendarDays, FileText, FolderKanban, Footprints, PenTool, Settings, ShieldCheck, Sun } from "lucide-react";
import type { LucideIcon } from "lucide-react";

import { id } from "../strings/id";

export type PagePath = "/" | "/kalender" | "/proyek" | "/studio" | "/belajar" | "/file" | "/jejak" | "/pengaturan";

/** Where the sidebar shows a page: the main list, the secondary one under the divider, or the foot. */
export type PageGroup = "main" | "more" | "foot";

export interface Page {
  path: PagePath;
  label: string;
  icon: LucideIcon;
  /** Extra words the palette matches. */
  keywords: string;
  group: PageGroup;
}

export const pages: readonly Page[] = [
  { path: "/", label: id.nav.today, icon: Sun, keywords: "dashboard beranda hari ini fokus jadwal", group: "main" },
  { path: "/kalender", label: id.nav.calendar, icon: CalendarDays, keywords: "jadwal rutinitas minggu bulan pengingat reminder", group: "main" },
  { path: "/proyek", label: id.nav.work, icon: FolderKanban, keywords: "project kemajuan tugas todo kotak masuk ruang", group: "main" },
  { path: "/studio", label: id.nav.studio, icon: PenTool, keywords: "ide produk prd roadmap lentera gemini claude bangun", group: "main" },
  { path: "/belajar", label: id.nav.learning, icon: ShieldCheck, keywords: "cyber tryhackme hack the box ctf lab", group: "main" },
  { path: "/file", label: id.nav.files, icon: FileText, keywords: "library dokumen vault", group: "more" },
  { path: "/jejak", label: id.nav.trail, icon: Footprints, keywords: "riwayat aktivitas insight", group: "more" },
  { path: "/pengaturan", label: id.nav.settings, icon: Settings, keywords: "settings tema tinta cahaya nama", group: "foot" },
];

/** Lower case, accents removed: "Pengaturan" matches "pengat". */
export function normalize(text: string): string {
  return text.normalize("NFD").replace(/\p{Diacritic}/gu, "").toLowerCase();
}

export function matchPages(query: string): Page[] {
  const words = normalize(query).split(/\s+/).filter(Boolean);
  if (words.length === 0) return [...pages];
  return pages.filter((page) => {
    const haystack = normalize(`${page.label} ${page.keywords}`);
    return words.every((word) => haystack.includes(word));
  });
}
