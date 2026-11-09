// A modal sheet: labelled, keeps focus inside, Escape and the scrim close it, and focus goes back
// to where it was.

import { leave } from "../motion/motion";
import { useSound } from "../sound/SoundProvider";
import { useEffect, useId, useLayoutEffect, useRef } from "react";
import type { ReactNode } from "react";

import styles from "./dialog.module.css";

interface Props {
  title: string;
  onClose: () => void;
  children: ReactNode;
  footer?: ReactNode;
  width?: "narrow" | "wide";
  description?: string;
}

const FOCUSABLE =
  'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

export function Dialog({ title, onClose, children, footer, width = "narrow", description }: Props) {
  const titleId = useId();
  const panel = useRef<HTMLDivElement>(null);
  const scrim = useRef<HTMLDivElement>(null);
  // P2b: when it closes, by any way, it settles away instead of vanishing.
  useLayoutEffect(() => {
    const [backdrop, sheet] = [scrim, panel];
    return () =>
      leave(
        [backdrop.current, sheet.current],
        [
          [{ opacity: 1 }, { opacity: 0 }],
          [
            { opacity: 1, transform: "translate(-50%, -50%)" },
            { opacity: 0, transform: "translate(-50%, calc(-50% + 8px)) scale(0.98)" },
          ],
        ],
      );
  }, []);
  const returnTo = useRef<Element | null>(null);

  const sound = useSound();
  useEffect(() => {
    sound.play("open");
    return () => sound.play("close");
  }, [sound]);

  useEffect(() => {
    returnTo.current = document.activeElement;
    const first = panel.current?.querySelector<HTMLElement>("[data-autofocus]") ?? panel.current?.querySelector<HTMLElement>(FOCUSABLE);
    first?.focus();
    return () => {
      if (returnTo.current instanceof HTMLElement) returnTo.current.focus();
    };
  }, []);

  const onKeyDown = (event: React.KeyboardEvent) => {
    if (event.key === "Escape") {
      event.stopPropagation();
      onClose();
      return;
    }
    if (event.key !== "Tab" || !panel.current) return;
    const items = [...panel.current.querySelectorAll<HTMLElement>(FOCUSABLE)];
    const first = items[0];
    const last = items[items.length - 1];
    if (!first || !last) return;
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first.focus();
    }
  };

  return (
    <>
      <div ref={scrim} className={styles.scrim} onMouseDown={onClose} />
      <div
        ref={panel}
        className={`${styles.panel} ${width === "wide" ? styles.wide : ""}`}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        onKeyDown={onKeyDown}
      >
        <header className={styles.head}>
          <h2 id={titleId}>{title}</h2>
          {description && <p>{description}</p>}
        </header>
        <div className={styles.body}>{children}</div>
        {footer && <footer className={styles.foot}>{footer}</footer>}
      </div>
    </>
  );
}
