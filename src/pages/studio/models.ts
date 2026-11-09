// L2: the models Lentera can talk with. Google's list for a key holds every model that can
// `generateContent`, speech and image models among them; Studio needs a text model, so only the
// Pro, Flash, and Flash-Lite families pass. The saved model always stays in the list, even when
// Google's list does not have it (or could not be read).

import type { ModelInfo } from "../../data/generated/ModelInfo";

export type ModelFamily = "pro" | "flash" | "flash-lite";

export interface ChatModel {
  id: string;
  /** "3.5 Flash", "3 Pro Preview": the model's name without "Gemini". */
  label: string;
  family: ModelFamily | null;
}

// gemini-<version>-<family>, then at most a channel (preview, exp, latest, or a build like 001)
// and a date (05-20). Anything else (-tts, -image, -live, -native-audio) is a special model.
const TEXT_MODEL = /^gemini-(\d+(?:\.\d+)?)-(pro|flash|flash-lite)(?:-(preview|exp|latest|\d{3}))?(?:-(\d{2}-\d{2,4}))?$/;

const FAMILY_NAME: Record<ModelFamily, string> = { pro: "Pro", flash: "Flash", "flash-lite": "Flash-Lite" };
// Within a version: the everyday model first, then the careful one, then the light one.
const FAMILY_ORDER: Record<ModelFamily, number> = { flash: 0, pro: 1, "flash-lite": 2 };

interface Parsed {
  /** As written in the id ("3.5"), and as a number for sorting. */
  versionText: string;
  version: number;
  family: ModelFamily;
  channel: string;
  date: string;
}

function parse(id: string): Parsed | null {
  const match = TEXT_MODEL.exec(id);
  if (!match) return null;
  const [, versionText = "0", family, channel = "", date = ""] = match;
  return { versionText, version: Number(versionText), family: family as ModelFamily, channel, date };
}

/** The name the owner reads, from the id alone: the chip shows it before Google's list is read. */
export function modelLabel(id: string): string {
  const parsed = parse(id);
  if (!parsed) return id;
  const channel = parsed.channel === "preview" ? " Preview" : parsed.channel === "exp" ? " Eksperimen" : "";
  return `${parsed.versionText} ${FAMILY_NAME[parsed.family]}${channel}${parsed.date ? ` ${parsed.date}` : ""}`;
}

/** The models in family groups for the menu: Flash, Pro, Flash-Lite, then any the owner saved
 * that Hub does not recognise. Each group keeps the list's order (newest first). */
export function groupModels(models: readonly ChatModel[]): { family: ModelFamily | null; items: ChatModel[] }[] {
  const families: (ModelFamily | null)[] = [...(Object.keys(FAMILY_ORDER) as ModelFamily[]).sort((a, b) => FAMILY_ORDER[a] - FAMILY_ORDER[b]), null];
  return families
    .map((family) => ({ family, items: models.filter((model) => model.family === family) }))
    .filter((group) => group.items.length > 0);
}

export function isTextModel(id: string): boolean {
  return parse(id) !== null;
}

/**
 * The text models in Google's list, newest first (a stable model before its previews), with the
 * saved one kept at the top when the list lacks it.
 */
export function chatModels(models: readonly ModelInfo[] | undefined, current: string): ChatModel[] {
  const found = (models ?? [])
    .map((model) => ({ id: model.id, parsed: parse(model.id) }))
    .filter((model): model is { id: string; parsed: Parsed } => model.parsed !== null)
    .sort(
      (a, b) =>
        b.parsed.version - a.parsed.version ||
        FAMILY_ORDER[a.parsed.family] - FAMILY_ORDER[b.parsed.family] ||
        Number(a.parsed.channel !== "") - Number(b.parsed.channel !== "") ||
        a.id.localeCompare(b.id),
    )
    .map(({ id, parsed }): ChatModel => ({ id, label: modelLabel(id), family: parsed.family }));
  const unique = found.filter((model, index) => found.findIndex((other) => other.id === model.id) === index);
  if (current && !unique.some((model) => model.id === current)) {
    unique.unshift({ id: current, label: modelLabel(current), family: parse(current)?.family ?? null });
  }
  return unique;
}
