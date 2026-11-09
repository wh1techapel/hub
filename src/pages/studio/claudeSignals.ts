// What Hub says about Claude Code, in three signals (docs/studio.md, "Claude's status", R5): the
// bridge (installed, registered, the skill), the session (as the bridge reports it), and the last
// call. Each from real data; "Status belum diketahui" when Hub has none. Shared by Bangun (one
// project) and Pengaturan (every session).

import type { ClaudeCodeStatus } from "../../data/api";
import type { ClaudeSignals } from "../../data/generated/ClaudeSignals";
import { id } from "../../strings/id";
import { whenLabel } from "../../time/format";

const t = id.studio.claudeStatus;

/** Good, in the way, not known, or over: each drawn with its own glyph, never colour alone. */
export type Tone = "ok" | "problem" | "unknown" | "past";

export interface Signal {
  tone: Tone;
  line: string;
  detail: string;
  /** The tool's own name, shown as code beside the words. */
  code?: string;
}

const checking: Signal = { tone: "unknown", line: t.checking, detail: "" };

/** Installed, registered, and the skill: what Pengaturan checks, and the switch in Hub. A session
 * Hub heard from proves the registration when the `claude` command cannot be asked. */
export function bridgeSignal(status: ClaudeCodeStatus | undefined, signals: ClaudeSignals | undefined): Signal {
  if (!status) return checking;
  const missing: string[] = [];
  if (!status.bridge) missing.push(t.parts.bridge);
  if (status.registration === "missing" || status.registration === "other") missing.push(t.parts.registration);
  if (status.skill !== "ours") missing.push(t.parts.skill);
  if (missing.length > 0) return { tone: "problem", line: t.bridgeMissing, detail: t.missingDetail(missing.join(", ")) };
  if (!status.enabled) return { tone: "problem", line: t.bridgeOff, detail: t.bridgeOffDetail };
  if (status.registration === "unknown") {
    return signals?.session ? { tone: "ok", line: t.bridgeReady, detail: t.bridgeProven } : { tone: "unknown", line: t.unknown, detail: t.bridgeUnknownDetail };
  }
  return { tone: "ok", line: t.bridgeReady, detail: t.bridgeReadyDetail };
}

function clientName(client: string, version: string): string {
  const name = client === "claude-code" ? "Claude Code" : client;
  return [name, version].filter(Boolean).join(" ");
}

export function sessionSignal(signals: ClaudeSignals | undefined, now: Date, project: boolean): Signal {
  if (!signals) return checking;
  const session = signals.session;
  if (!session) return { tone: "unknown", line: t.unknown, detail: project ? t.noSessionProject : t.noSession };
  switch (session.state) {
    case "open":
      return {
        tone: "ok",
        line: signals.open > 1 ? t.openMany(signals.open) : t.open,
        detail: t.openDetail(whenLabel(session.startedAt, now), session.folder, clientName(session.client, session.clientVersion)),
      };
    case "closed":
      return { tone: "past", line: t.closed(whenLabel(session.endedAt ?? session.seenAt, now)), detail: session.folder };
    case "quiet":
      return { tone: "problem", line: t.quiet(whenLabel(session.seenAt, now)), detail: t.quietDetail };
  }
}

export function activitySignal(signals: ClaudeSignals | undefined, now: Date, project: boolean): Signal {
  if (!signals) return checking;
  const call = signals.activity;
  if (!call) return { tone: "unknown", line: t.unknown, detail: project ? t.noCallsProject : t.noCalls };
  const name = t.tools[call.tool] ?? call.tool;
  const when = whenLabel(call.at, now);
  if (call.ok) return { tone: "ok", line: t.called(name, when), detail: "", code: call.tool };
  const reason = call.reason ? (t.reasons[call.reason] ?? t.reasonOther(call.reason)) : t.noReason;
  return { tone: "problem", line: t.failed(name, when), detail: reason, code: call.tool };
}
