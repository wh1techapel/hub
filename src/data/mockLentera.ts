// Lentera's brain in the demo backend (CP03): a pretend Gemini that streams a short answer and
// proposes cards, a rough copy of Dari Pesan's cleaning, and Simpan through the demo's own
// handlers. Enough for the browser preview and the tests; the real rules are in hub-ai
// (lentera.rs, group_message.rs) and their tests.
//
// The pretend Gemini: a line becomes one task per part joined by " dan " (due tomorrow, at 19:00
// when the line says "jam 7"); "ingatkan ..." becomes a reminder; a line with "gagal" fails as
// offline; one with "lama" streams until Hentikan. A group message becomes one task per line that
// mentions "tugas" or "PR".


import type { DeskEvent } from "./api";
import { HubError } from "./backend";
import type { Cleaned } from "./generated/Cleaned";
import type { DeskMode } from "./generated/DeskMode";
import type { Proposal } from "./generated/Proposal";

interface Deps {
  hasKey: () => boolean;
  today: () => string;
  addDays: (date: string, days: number) => string;
  /** The demo's own handlers (create_task, create_reminder, create_event, save_capture). */
  call: (command: "create_task" | "create_reminder" | "create_event" | "save_capture", args: Record<string, unknown>) => unknown;
}

const HEADER = /^\[?\d{1,2}[/.-]\d{1,2}[/.-]\d{2,4}[, ]+\d{1,2}[.:]\d{2}(?:[.:]\d{2})?\]?\s*(?:-\s*)?([^:]{1,64}):\s*/;
const PHONE = /(\+62|62|0)\d[\d -]{7,}\d/g;
const EMAIL = /[\w.+-]+@[\w-]+\.[\w.]+/g;

export function cleanMessage(raw: string): Cleaned {
  const senders = new Set<string>();
  let phones = 0;
  let emails = 0;
  let times = 0;
  const lines = raw.split(/\r?\n/).map((line) => {
    const header = HEADER.exec(line);
    let text = line;
    if (header) {
      senders.add(header[1] ?? "");
      times += 1;
      text = `- ${line.slice(header[0].length)}`;
    }
    text = text.replace(PHONE, () => {
      phones += 1;
      return "[nomor]";
    });
    return text.replace(EMAIL, () => {
      emails += 1;
      return "[email]";
    });
  });
  const text = lines.join("\n").trim();
  return { text, senders: senders.size, phones, emails, mentions: 0, times, dropped: 0, tooLong: text.length > 8000 };
}

function fail(reason: string): never {
  throw new HubError({ kind: "invalid", field: "gemini", reason, message: reason });
}

const pause = (ms: number) => new Promise((done) => setTimeout(done, ms));

export function lenteraMock(deps: Deps) {
  let stopped = false;

  const task = (title: string, due: string, time: string | null): Proposal => ({
    kind: "task",
    title,
    notes: "",
    doDate: null,
    dueDate: due,
    dueTime: time,
    priority: 0,
    estimateMin: null,
    project: null,
    area: null,
  });

  function propose(text: string, mode: DeskMode): Proposal[] {
    const tomorrow = deps.addDays(deps.today(), 1);
    if (mode === "message") {
      return text
        .split("\n")
        .map((line) => line.replace(/^[-\s]+/, "").trim())
        .filter((line) => /tugas|\bPR\b/i.test(line))
        .map((line) => task(line.slice(0, 80), tomorrow, null));
    }
    if (/^ingatkan/i.test(text)) return [{ kind: "reminder", text: text.replace(/^ingatkan( aku)?\s*/i, ""), date: tomorrow, time: "07:00" }];
    return text
      .split(" dan ")
      .map((part) => part.trim())
      .filter(Boolean)
      .map((part) => task(part.replace(/\s*(besok|jam 7)\b/gi, "").trim() || part, tomorrow, /jam 7/i.test(part) ? "19:00" : null));
  }

  return {
    handlers: {
      clean_lentera_message: ({ text }: Record<string, unknown>) => cleanMessage(text as string),
      stop_lentera: () => {
        stopped = true;
      },
      save_lentera_proposal: ({ proposal }: Record<string, unknown>) => {
        const p = proposal as Proposal;
        switch (p.kind) {
          case "task": {
            if (!p.title.trim()) fail("format");
            const id = deps.call("create_task", {
              input: {
                title: p.title,
                notes: p.notes,
                priority: p.priority,
                doDate: p.doDate,
                dueDate: p.dueDate,
                dueTime: p.dueTime,
                estimateMin: p.estimateMin,
                projectId: p.project?.id ?? null,
                areaId: p.area?.id ?? null,
                parentId: null,
              },
            }) as string;
            return { kind: "task", id, title: p.title, date: p.dueDate ?? p.doDate };
          }
          case "reminder": {
            const [h, m] = p.time.split(":").map(Number);
            const id = deps.call("create_reminder", { input: { text: p.text, date: p.date, minute: (h ?? 0) * 60 + (m ?? 0), taskId: null } }) as string;
            return { kind: "reminder", id, title: p.text, date: p.date };
          }
          case "event": {
            const minute = (clock: string | null) => (clock ? Number(clock.slice(0, 2)) * 60 + Number(clock.slice(3)) : 0);
            const start = minute(p.start);
            const id = deps.call("create_event", {
              input: {
                title: p.title,
                location: p.location,
                notes: "",
                allDay: !p.start,
                date: p.date,
                endDate: null,
                startMinute: start,
                endMinute: p.end ? minute(p.end) : start + 60,
                pigment: "batu",
                projectId: p.project?.id ?? null,
                taskId: null,
              },
            }) as string;
            return { kind: "event", id, title: p.title, date: p.date };
          }
          case "note": {
            return deps.call("save_capture", { text: p.text, kind: "note" });
          }
        }
      },
    },
    streams: {
      ask_lentera: async ({ text, mode }: Record<string, unknown>, onEvent: (event: DeskEvent) => void) => {
        if (!deps.hasKey()) fail("no_key");
        const line = String(text);
        const kind = mode as DeskMode;
        if (/gagal/i.test(line)) {
          await pause(5);
          throw new HubError({ kind: "internal", field: "gemini", reason: "offline", message: "offline" });
        }
        stopped = false;
        const sent = kind === "message" ? cleanMessage(line).text : line.trim();
        onEvent({ kind: "started", sent });
        if (/lama/i.test(line)) {
          onEvent({ kind: "delta", text: "Sebentar, " });
          // Hentikan sets `stopped` while this waits (TypeScript cannot see the other call).
          const isStopped = () => stopped;
          for (let i = 0; i < 400 && !isStopped(); i += 1) await pause(5);
          onEvent({ kind: "finished", memory: "Sebentar,", stopped: true });
          return undefined;
        }
        await pause(5);
        const proposals = propose(sent, kind);
        const reply = proposals.length ? "Ini usulanku." : "Tidak ada yang perlu dicatat.";
        for (const piece of reply.split(/(?<= )/)) {
          onEvent({ kind: "delta", text: piece });
          await pause(1);
        }
        if (proposals.length) onEvent({ kind: "proposals", proposals });
        onEvent({ kind: "finished", memory: `${reply}\nUsulan: ${proposals.length}`, stopped: false });
        return undefined;
      },
    },
  };
}

