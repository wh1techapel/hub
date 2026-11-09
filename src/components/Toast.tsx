// A short note at the bottom of the window, with at most two actions (Urungkan, and one more such
// as Beri catatan). It leaves by itself. One region for the whole window, so any page can say something.

import { createContext, useCallback, useContext, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import type { ReactNode } from "react";

import { toHubError } from "../data/backend";
import { leave } from "../motion/motion";
import { id } from "../strings/id";
import styles from "./toast.module.css";

export interface ToastMessage {
  text: string;
  action?: { label: string; run: () => void };
  second?: { label: string; run: () => void };
}

type Show = (message: ToastMessage) => void;

const VISIBLE_MS = 5000;
const ToastContext = createContext<Show | null>(null);

export function ToastProvider({ children }: { children: ReactNode }) {
  const [message, setMessage] = useState<(ToastMessage & { key: number }) | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const counter = useRef(0);

  const show = useCallback<Show>((next) => {
    clearTimeout(timer.current);
    counter.current += 1;
    setMessage({ ...next, key: counter.current });
    timer.current = setTimeout(() => setMessage(null), VISIBLE_MS);
  }, []);

  useEffect(() => () => clearTimeout(timer.current), []);

  return (
    <ToastContext.Provider value={show}>
      {children}
      <div className={styles.region} role="status" aria-live="polite">
        {message && (
          <Note key={message.key}>
            <span>{message.text}</span>
            {message.action && (
              <button
                type="button"
                onClick={() => {
                  message.action?.run();
                  setMessage(null);
                }}
              >
                {message.action.label}
              </button>
            )}
            {message.second && (
              <button
                type="button"
                onClick={() => {
                  message.second?.run();
                  setMessage(null);
                }}
              >
                {message.second.label}
              </button>
            )}
          </Note>
        )}
      </div>
    </ToastContext.Provider>
  );
}

/** One toast. P2b: when it goes (by itself, Urungkan, or a newer one), it slides away, not vanishes. */
function Note({ children }: { children: ReactNode }) {
  const note = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    const node = note;
    return () => leave([node.current], [[{ opacity: 1, transform: "none" }, { opacity: 0, transform: "translateY(8px)" }]], { pin: true });
  }, []);
  return (
    <div ref={note} className={styles.toast}>
      {children}
    </div>
  );
}

export function useToast(): Show {
  const show = useContext(ToastContext);
  if (!show) throw new Error("useToast outside ToastProvider");
  return show;
}

/** What `act` gives back: the call's value, or that it failed (already said in a toast). Many commands
 *  return nothing, so a plain `undefined` could not tell success from failure. */
export type Outcome<T> = { ok: true; value: T } | { ok: false };

/** Run a core call; a refusal or failure is said in a toast instead of thrown. */
export function useAct() {
  const toast = useToast();
  return useMemo(
    () =>
      async <T,>(work: () => Promise<T>, after?: ToastMessage | ((value: T) => ToastMessage | undefined)): Promise<Outcome<T>> => {
        try {
          const value = await work();
          const message = typeof after === "function" ? after(value) : after;
          if (message) toast(message);
          return { ok: true, value };
        } catch (error) {
          const view = toHubError(error).view;
          toast({ text: view.kind === "invalid" ? `${id.common.failed} (${view.field ?? ""})` : id.common.failed });
          return { ok: false };
        }
      },
    [toast],
  );
}
