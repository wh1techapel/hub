// Claude Code's three signals as a short list (claudeSignals.ts says what each means). Not a live
// region: the times in it change every minute, and the toasts already speak for what waits for
// the owner.

import { CircleAlert, CircleCheck, CircleQuestionMark, Clock } from "lucide-react";

import { useClaudeCodeStatus, useClaudeSignals } from "../../data/queries";
import { id } from "../../strings/id";
import { useNow } from "../../time/clock";
import { activitySignal, bridgeSignal, sessionSignal } from "./claudeSignals";
import type { Signal, Tone } from "./claudeSignals";
import css from "./claudeSignals.module.css";

const t = id.studio.claudeStatus;

const glyphs: Record<Tone, typeof CircleCheck> = { ok: CircleCheck, problem: CircleAlert, unknown: CircleQuestionMark, past: Clock };

/** `projectId`: one project's session and calls (Bangun); none: every session (Pengaturan). */
export function ClaudeStatus({ projectId }: { projectId: string | null }) {
  const status = useClaudeCodeStatus();
  const signals = useClaudeSignals(projectId);
  const now = useNow();
  const project = projectId !== null;
  const rows: [string, Signal][] = [
    [t.bridge, bridgeSignal(status.data, signals.data)],
    [t.session, sessionSignal(signals.data, now, project)],
    [t.activity, activitySignal(signals.data, now, project)],
  ];
  return (
    // A group, so the list has a name: a <dl> has no role of its own to carry one.
    <div role="group" aria-label={t.label}>
      <dl className={css.signals}>
        {rows.map(([term, signal]) => {
          const Glyph = glyphs[signal.tone];
          return (
            <div key={term} className={css.row} data-tone={signal.tone}>
              <dt>{term}</dt>
              <dd>
                <Glyph size={14} aria-hidden="true" className={css.glyph} />
                <span className={css.text}>
                  <span className={css.line}>
                    {signal.line}
                    {signal.code && <code className={css.code}>{signal.code}</code>}
                  </span>
                  {signal.detail && <span className={css.detail}>{signal.detail}</span>}
                </span>
              </dd>
            </div>
          );
        })}
      </dl>
    </div>
  );
}
