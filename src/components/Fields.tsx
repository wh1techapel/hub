// Form parts shared by the dialogs and panels: a labelled field, a segmented choice, the pigment
// swatches, a switch. Native inputs underneath, so the keyboard and screen readers work as usual.

import { useGlide } from "../motion/motion";
import { useId } from "react";
import type { ReactNode } from "react";

import type { Pigment } from "../data/generated/Pigment";
import { id } from "../strings/id";
import styles from "./fields.module.css";

export const PIGMENTS: Pigment[] = ["lazuardi", "toska", "lumut", "kunyit", "terakota", "mawar", "lavender", "batu"];

export function Field({
  label,
  error,
  hint,
  children,
  htmlFor,
}: {
  label: string;
  error?: string | null;
  hint?: string;
  children: ReactNode;
  htmlFor?: string;
}) {
  return (
    <div className={styles.field}>
      <label className={styles.label} htmlFor={htmlFor}>
        {label}
      </label>
      {children}
      {hint && !error && <p className={styles.hint}>{hint}</p>}
      {error && (
        <p className={styles.error} role="alert">
          {error}
        </p>
      )}
    </div>
  );
}

export function Segmented<T extends string | number>({
  label,
  options,
  value,
  onChange,
}: {
  label: string;
  options: readonly (readonly [T, string])[];
  value: T;
  onChange: (value: T) => void;
}) {
  const name = useId();
  const group = useGlide<HTMLDivElement>("input:checked");
  return (
    <div ref={group} className={styles.segmented} role="radiogroup" aria-label={label}>
      {options.map(([option, text]) => (
        <label key={String(option)} className={styles.segment}>
          <input type="radio" name={name} checked={value === option} onChange={() => onChange(option)} />
          <span>{text}</span>
        </label>
      ))}
    </div>
  );
}

export function PigmentPicker({ label, value, onChange }: { label: string; value: Pigment; onChange: (pigment: Pigment) => void }) {
  const name = useId();
  return (
    <div className={styles.pigments} role="radiogroup" aria-label={label}>
      {PIGMENTS.map((pigment) => (
        <label key={pigment} className={styles.pigment} data-pigment={pigment} title={id.pigments[pigment]}>
          <input
            type="radio"
            name={name}
            checked={value === pigment}
            onChange={() => onChange(pigment)}
            aria-label={id.pigments[pigment]}
          />
          <i aria-hidden="true" />
        </label>
      ))}
    </div>
  );
}

export function Switch({
  label,
  checked,
  onChange,
  labelledBy,
}: {
  label?: string;
  checked: boolean;
  onChange: (value: boolean) => void;
  labelledBy?: string;
}) {
  return (
    <button
      type="button"
      role="switch"
      className={styles.switch}
      aria-checked={checked}
      aria-label={label}
      aria-labelledby={labelledBy}
      onClick={() => onChange(!checked)}
    >
      <i aria-hidden="true" />
    </button>
  );
}

/** "HH:MM" from minutes. */
export function timeValue(minutes: number): string {
  const m = ((minutes % 1440) + 1440) % 1440;
  return `${String(Math.floor(m / 60)).padStart(2, "0")}:${String(m % 60).padStart(2, "0")}`;
}
