// Studio's home (docs/studio.md, "Studio's home", U3): after Gemini's layout, a clean page with the
// greeting in the middle by the light of the day, the question, and one pill-shaped field. Enter
// starts a project and opens its conversation with that idea sent. The projects are Studio's
// history in the sidebar (U2), not here. Without a Gemini key the field gives way to the way to
// Pengaturan. The chip under the idea chooses Lentera's model (L2).

import { Link, useNavigate } from "@tanstack/react-router";
import { ArrowUp } from "lucide-react";
import { useLayoutEffect, useRef, useState } from "react";

import { useApi } from "../../data/api";
import { useGeminiStatus, useSettings, useStudioAct } from "../../data/queries";
import { id } from "../../strings/id";
import { useNow } from "../../time/clock";
import { greeting, minutesOf, parseClock } from "../../time/format";
import { ModelPicker } from "./ModelPicker";
import { setPending } from "./pending";
import styles from "./studio.module.css";

const t = id.studio;

export function StudioPage() {
  const gemini = useGeminiStatus();
  const settings = useSettings();
  const now = useNow();
  const api = useApi();
  const navigate = useNavigate();
  const start = useStudioAct((idea: string) => api.startStudioProject(idea));
  const [idea, setIdea] = useState("");
  const field = useRef<HTMLTextAreaElement>(null);
  const hasKey = gemini.data?.hasKey ?? false;
  const wake = parseClock(settings.data?.wakeTime ?? "") ?? 300;

  // The field grows with its text, up to its CSS limit, then scrolls (as the conversation's does).
  useLayoutEffect(() => {
    const node = field.current;
    if (!node) return;
    node.style.height = "auto";
    node.style.height = `${node.scrollHeight}px`;
  }, [idea, hasKey]);

  const begin = () => {
    const text = idea.trim();
    if (!text || !hasKey || start.isPending) return;
    start.mutate(text, {
      onSuccess: (projectId) => {
        setPending(projectId, text);
        setIdea("");
        void navigate({ to: "/studio/$projectId", params: { projectId } });
      },
    });
  };

  return (
    <div className={styles.page}>
      {/* L3: the light of the day behind the page, as Gemini has its glow; only decoration. */}
      <div className={styles.glow} aria-hidden="true" data-testid="studio-glow" />
      <div className={styles.center}>
        <header className={styles.header}>
          {settings.data && (
            <p className={`display ${styles.hello}`}>
              {greeting(minutesOf(now), wake)}, <em>{settings.data.ownerName}.</em>
            </p>
          )}
          <h1 className={`display ${styles.greeting}`}>{t.greeting}</h1>
          <p className={styles.intro}>{t.intro}</p>
        </header>

        {gemini.data && !hasKey && (
          <div className={styles.setup}>
            <p>{t.noKey}</p>
            <Link to="/pengaturan/$bagian" params={{ bagian: "ai" }} className="btn btn-primary">
              {t.openSettings}
            </Link>
          </div>
        )}
        {hasKey && (
          <form
            className={styles.composer}
            // A click on the pill's own padding puts the cursor in the field.
            onClick={(event) => {
              if (event.target === event.currentTarget) field.current?.focus();
            }}
            onSubmit={(event) => {
              event.preventDefault();
              begin();
            }}
          >
            <label className="sr-only" htmlFor="studio-idea">
              {t.ideaLabel}
            </label>
            <textarea
              id="studio-idea"
              ref={field}
              className={styles.idea}
              rows={1}
              value={idea}
              maxLength={2000}
              placeholder={t.ideaPlaceholder}
              // The one thing on this page: the cursor starts here.
              autoFocus
              onChange={(event) => setIdea(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter" && !event.shiftKey && !event.nativeEvent.isComposing) {
                  event.preventDefault();
                  begin();
                }
              }}
            />
            <div className={styles.composerRow}>
              <ModelPicker placement="down" align="start" />
              <button type="submit" className={styles.send} disabled={!idea.trim() || start.isPending} aria-label={t.start} title={t.start}>
                <ArrowUp size={18} strokeWidth={2} aria-hidden="true" />
              </button>
            </div>
          </form>
        )}

        <p className={styles.privacy}>{t.privacy}</p>
      </div>
    </div>
  );
}
