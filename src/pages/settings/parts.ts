// Pengaturan's parts (P1, the owner's choice of five): each has its own address
// (`/pengaturan/<slug>`), its own page, and its place in the sidebar while Pengaturan is open.

import { HardDrive, Palette, Radio, Sparkles, Sunrise } from "lucide-react";
import type { LucideIcon } from "lucide-react";

import { id } from "../../strings/id";

export type SettingsSlug = "tampilan" | "hari" | "ai" | "ultron" | "sistem";

export interface SettingsPart {
  slug: SettingsSlug;
  label: string;
  icon: LucideIcon;
}

export const SETTINGS_PARTS: readonly SettingsPart[] = [
  { slug: "tampilan", label: id.settings.parts.tampilan, icon: Palette },
  { slug: "hari", label: id.settings.parts.hari, icon: Sunrise },
  { slug: "ai", label: id.settings.parts.ai, icon: Sparkles },
  { slug: "ultron", label: id.settings.parts.ultron, icon: Radio },
  { slug: "sistem", label: id.settings.parts.sistem, icon: HardDrive },
];

/** Where `/pengaturan` alone (an address from before P1) opens. */
export const FIRST_PART: SettingsSlug = "tampilan";

export function isSettingsSlug(value: unknown): value is SettingsSlug {
  return SETTINGS_PARTS.some((part) => part.slug === value);
}
