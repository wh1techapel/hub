// Editing one PRD section in place: fields by kind (a line, a paragraph, one item per line, a
// choice, a switch, a nested group, a list of items with add, remove, and reorder). Ids are set
// by Hub when an item is added and are not edited. Saved as a whole section; the core checks it.

import { useState } from "react";

import { id } from "../../strings/id";
import type { Field } from "./prdFields";
import styles from "./prd.module.css";

const t = id.studio.prd;
type Item = Record<string, unknown>;

function asText(value: unknown): string {
  return typeof value === "string" ? value : "";
}

function asLines(value: unknown): string {
  return Array.isArray(value) ? value.map(String).join("\n") : "";
}

function FieldView({ field, value, parent, onChange, path }: { field: Field; value: unknown; parent: Item; onChange: (next: unknown) => void; path: string }) {
  const inputId = `prd-${path}`;
  switch (field.kind) {
    case "id":
      return (
        <p className={styles.idLine}>
          <code>{asText(value)}</code>
        </p>
      );
    case "text":
      return (
        <div className={styles.field}>
          <label htmlFor={inputId}>{field.label}</label>
          <input id={inputId} className={styles.input} value={asText(value)} onChange={(event) => onChange(event.target.value)} />
        </div>
      );
    case "long":
      return (
        <div className={styles.field}>
          <label htmlFor={inputId}>{field.label}</label>
          <textarea id={inputId} className={styles.input} rows={3} value={asText(value)} onChange={(event) => onChange(event.target.value)} />
        </div>
      );
    case "lines":
      return (
        <LinesField
          inputId={inputId}
          label={field.label}
          value={asLines(value)}
          onChange={(text) => onChange(text.split("\n").map((line) => line.trim()).filter((line) => line.length > 0))}
        />
      );
    case "bool":
      return (
        <label className={styles.check}>
          <input type="checkbox" checked={value === true} onChange={(event) => onChange(event.target.checked)} />
          <span>{field.label}</span>
        </label>
      );
    case "select":
      return (
        <div className={styles.field}>
          <label htmlFor={inputId}>{field.label}</label>
          <select id={inputId} className={styles.input} value={asText(value)} onChange={(event) => onChange(event.target.value)}>
            {field.options.map((option) => (
              <option key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </select>
        </div>
      );
    case "group": {
      const group = (value ?? {}) as Item;
      return (
        <fieldset className={styles.group}>
          <legend>{field.label}</legend>
          {field.fields.map((sub) => (
            <FieldView
              key={sub.key}
              field={sub}
              value={group[sub.key]}
              parent={group}
              path={`${path}-${sub.key}`}
              onChange={(next) => onChange({ ...group, [sub.key]: next })}
            />
          ))}
        </fieldset>
      );
    }
    case "list": {
      const items = (Array.isArray(value) ? value : []) as Item[];
      const set = (next: Item[]) => onChange(next);
      return (
        <fieldset className={styles.group}>
          <legend>{field.label}</legend>
          {items.map((item, index) => (
            <div key={typeof item.id === "string" ? item.id : index} className={styles.item}>
              <div className={styles.itemHead}>
                <span>
                  {field.itemLabel} {typeof item.id === "string" ? item.id : index + 1}
                </span>
                <div className={styles.itemActions}>
                  <button type="button" className="btn btn-quiet" disabled={index === 0} onClick={() => set(items.map((x, i) => (i === index - 1 ? item : i === index ? (items[index - 1] as Item) : x)))}>
                    {t.moveUp}
                  </button>
                  <button
                    type="button"
                    className="btn btn-quiet"
                    disabled={index === items.length - 1}
                    onClick={() => set(items.map((x, i) => (i === index + 1 ? item : i === index ? (items[index + 1] as Item) : x)))}
                  >
                    {t.moveDown}
                  </button>
                  <button type="button" className="btn btn-danger" onClick={() => set(items.filter((_, i) => i !== index))}>
                    {t.remove}
                  </button>
                </div>
              </div>
              {field.fields.map((sub) => (
                <FieldView
                  key={sub.key}
                  field={sub}
                  value={item[sub.key]}
                  parent={item}
                  path={`${path}-${index}-${sub.key}`}
                  onChange={(next) => set(items.map((x, i) => (i === index ? { ...x, [sub.key]: next } : x)))}
                />
              ))}
            </div>
          ))}
          <div>
            <button type="button" className="btn btn-quiet" onClick={() => set([...items, field.make(items, parent)])}>
              {t.add} {field.itemLabel.toLowerCase()}
            </button>
          </div>
        </fieldset>
      );
    }
  }
}

/** Lines keep the owner's typing (blank lines included) until the field is left. */
function LinesField({ inputId, label, value, onChange }: { inputId: string; label: string; value: string; onChange: (text: string) => void }) {
  const [draft, setDraft] = useState<string | null>(null);
  return (
    <div className={styles.field}>
      <label htmlFor={inputId}>{label}</label>
      <textarea
        id={inputId}
        className={styles.input}
        rows={3}
        value={draft ?? value}
        aria-describedby={`${inputId}-hint`}
        onChange={(event) => {
          setDraft(event.target.value);
          onChange(event.target.value);
        }}
        onBlur={() => setDraft(null)}
      />
      <small id={`${inputId}-hint`} className={styles.hint}>
        {t.onePerLine}
      </small>
    </div>
  );
}

export function SectionEditor({ fields, value, onChange, section }: { fields: Field[]; value: unknown; onChange: (next: unknown) => void; section: string }) {
  const root = fields[0];
  // Features and NFRs: the section is the list itself.
  if (fields.length === 1 && root?.key === "") {
    return <FieldView field={root} value={value} parent={{}} path={section} onChange={onChange} />;
  }
  const object = (value ?? {}) as Item;
  return (
    <div className={styles.editor}>
      {fields.map((field) => (
        <FieldView
          key={field.key}
          field={field}
          value={object[field.key]}
          parent={object}
          path={`${section}-${field.key}`}
          onChange={(next) => onChange({ ...object, [field.key]: next })}
        />
      ))}
    </div>
  );
}
