// L2: the model Lentera talks with, chosen where the owner types. A chip (or, after a failed reply,
// a "Ganti model" button) opens a menu of the text models the key can use; the choice is Hub's one
// setting (the same as Pengaturan > Integrasi AI), so every project and the next retry use it.
// Google's list is read the first time the menu opens, not on every visit to Studio.
//
// The menu follows the WAI-ARIA menu button pattern: the arrows move between models, Home and End
// jump, Enter or Space chooses, Escape closes and puts the focus back on the button, Tab or a
// click outside closes.

import { useQueryClient } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { Check, ChevronDown } from "lucide-react";
import { useEffect, useId, useLayoutEffect, useRef, useState } from "react";
import type { KeyboardEvent } from "react";

import { keys, useGeminiModels, useGeminiStatus, useSettings, useUpdateSettings } from "../../data/queries";
import { id } from "../../strings/id";
import { chatModels, groupModels, modelLabel } from "./models";
import styles from "./modelPicker.module.css";

const t = id.studio.model;
/** The menu's tallest (as in modelPicker.module.css), its shortest when room is tight, and its
 * distance from the button. */
const MENU_MAX = 420;
const MENU_MIN = 160;
const GAP = 16;

/** The part of the window a node can be seen in: the window cut by every area around it that
 * scrolls or clips. */
function visibleBand(node: HTMLElement): { top: number; bottom: number } {
  let top = 0;
  let bottom = window.innerHeight;
  for (let parent = node.parentElement; parent; parent = parent.parentElement) {
    if (getComputedStyle(parent).overflowY === "visible") continue;
    const box = parent.getBoundingClientRect();
    top = Math.max(top, box.top);
    bottom = Math.min(bottom, box.bottom);
  }
  return { top, bottom };
}

export function ModelPicker({
  placement,
  align = "start",
  variant = "chip",
}: {
  /** Where the menu opens: above the button (an input at the bottom) or under it. */
  placement: "up" | "down";
  align?: "start" | "end";
  /** "switch": the "Ganti model" button shown after a reply failed for its model. */
  variant?: "chip" | "switch";
}) {
  const settings = useSettings();
  const status = useGeminiStatus();
  const client = useQueryClient();
  const save = useUpdateSettings();
  const [open, setOpen] = useState(false);
  const [fit, setFit] = useState<{ side: "up" | "down"; maxHeight: number | null }>({ side: placement, maxHeight: null });
  const [asked, setAsked] = useState(false);
  const models = useGeminiModels(asked && (status.data?.hasKey ?? false));
  const root = useRef<HTMLDivElement>(null);
  const button = useRef<HTMLButtonElement>(null);
  const menu = useRef<HTMLDivElement>(null);
  const menuId = useId();

  const current = settings.data?.geminiModel ?? status.data?.model ?? "";
  const items = chatModels(models.data, current);

  const menuItems = () => Array.from(menu.current?.querySelectorAll<HTMLElement>('[role="menuitemradio"], [role="menuitem"]') ?? []);
  const focusItem = (index: number) => {
    const list = menuItems();
    if (list.length === 0) return;
    list[(index + list.length) % list.length]?.focus();
  };

  const show = () => {
    setAsked(true);
    setOpen(true);
    save.reset();
  };
  const hide = (refocus: boolean) => {
    setOpen(false);
    if (refocus) button.current?.focus();
  };

  // On opening, the focus goes to the chosen model (or the first one).
  useEffect(() => {
    if (!open) return;
    const list = menuItems();
    const chosen = list.find((item) => item.getAttribute("aria-checked") === "true");
    // The menu scrolls itself into view (below); the focus does not scroll on its own.
    (chosen ?? list[0])?.focus({ preventScroll: true });
    // Only when it opens: the list arriving later must not pull the focus back.
  }, [open]);

  // Where the menu opens, measured within the window and every scrolling area around the button
  // (the conversation clips what rises above its top): the asked side when it fits, else the
  // other side when it fits, else the roomier side with the menu made as tall as that room (its
  // list scrolls inside), so opening it never scrolls the page away from the input.
  const count = items.length;
  useLayoutEffect(() => {
    if (!open || !button.current || !menu.current) return;
    const at = button.current.getBoundingClientRect();
    const room = visibleBand(button.current);
    const space = { down: room.bottom - at.bottom - GAP, up: at.top - room.top - GAP };
    const needed = Math.min(menu.current.scrollHeight, MENU_MAX, window.innerHeight * 0.6);
    const other = placement === "down" ? "up" : "down";
    const side = needed <= space[placement] ? placement : needed <= space[other] ? other : space.up > space.down ? "up" : "down";
    setFit({ side, maxHeight: needed <= space[side] ? null : Math.max(MENU_MIN, Math.floor(space[side])) });
  }, [open, count, placement]);

  // Only when even the roomier side is shorter than a usable menu does it hang past an edge; then
  // it is brought into view.
  useEffect(() => {
    if (open) menu.current?.scrollIntoView({ block: "nearest" });
  }, [open, fit, count]);

  // A press outside closes it, without taking the focus anywhere.
  useEffect(() => {
    if (!open) return;
    const away = (event: PointerEvent) => {
      if (root.current && !root.current.contains(event.target as Node)) setOpen(false);
    };
    document.addEventListener("pointerdown", away);
    return () => document.removeEventListener("pointerdown", away);
  }, [open]);

  const choose = (model: string) => {
    if (model === current) {
      hide(true);
      return;
    }
    save.mutate(
      { geminiModel: model },
      {
        onSuccess: () => {
          void client.invalidateQueries({ queryKey: keys.gemini });
          hide(true);
        },
      },
    );
  };

  const onButtonKey = (event: KeyboardEvent<HTMLButtonElement>) => {
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      show();
    }
  };

  const onMenuKey = (event: KeyboardEvent<HTMLDivElement>) => {
    const list = menuItems();
    const at = list.indexOf(document.activeElement as HTMLElement);
    switch (event.key) {
      case "ArrowDown":
        event.preventDefault();
        focusItem(at + 1);
        break;
      case "ArrowUp":
        event.preventDefault();
        focusItem(at - 1);
        break;
      case "Home":
        event.preventDefault();
        focusItem(0);
        break;
      case "End":
        event.preventDefault();
        focusItem(list.length - 1);
        break;
      case "Escape":
        event.preventDefault();
        event.stopPropagation();
        hide(true);
        break;
      case "Tab":
        setOpen(false);
        break;
    }
  };

  const label = variant === "switch" ? t.switch : current ? modelLabel(current) : t.none;

  return (
    <div className={styles.picker} ref={root}>
      <button
        ref={button}
        type="button"
        className={variant === "switch" ? "btn btn-secondary" : styles.chip}
        aria-haspopup="menu"
        aria-expanded={open}
        aria-controls={open ? menuId : undefined}
        // The chip names the model; its accessible name says what it is for.
        aria-label={variant === "chip" ? t.chipLabel(label) : undefined}
        title={variant === "chip" ? t.chipTitle : undefined}
        onClick={() => (open ? hide(false) : show())}
        onKeyDown={onButtonKey}
      >
        <span>{label}</span>
        {variant === "chip" && <ChevronDown size={14} strokeWidth={1.8} aria-hidden="true" />}
      </button>
      {open && (
        <div
          ref={menu}
          id={menuId}
          className={styles.menu}
          data-placement={fit.side}
          style={fit.maxHeight === null ? undefined : { maxHeight: fit.maxHeight }}
          data-align={align}
          role="menu"
          aria-label={t.menuLabel}
          aria-busy={save.isPending || models.isFetching}
          onKeyDown={onMenuKey}
        >
          {/* Tidied after the owner's list of 13 models: one group per family, its hint said
              once in the group's head, each model one line. */}
          {groupModels(items).map((group) => {
            const family = group.family ?? "other";
            const nameId = `${menuId}-${family}`;
            const hintId = `${nameId}-hint`;
            return (
              <div
                key={family}
                role="group"
                aria-labelledby={nameId}
                aria-describedby={group.family ? hintId : undefined}
                className={styles.group}
              >
                <div className={styles.groupHead}>
                  <span className={styles.groupName} id={nameId}>
                    {t.families[family]}
                  </span>
                  {group.family && (
                    <span className={styles.groupHint} id={hintId}>
                      {t.hints[group.family]}
                    </span>
                  )}
                </div>
                {group.items.map((model) => (
                  <button
                    key={model.id}
                    type="button"
                    role="menuitemradio"
                    aria-checked={model.id === current}
                    tabIndex={-1}
                    className={styles.item}
                    disabled={save.isPending}
                    onClick={() => choose(model.id)}
                  >
                    <span className={styles.name}>{model.label}</span>
                    <Check className={styles.check} size={16} strokeWidth={2} aria-hidden="true" />
                  </button>
                ))}
              </div>
            );
          })}
          {models.isFetching && !models.data && (
            <p className={styles.state} role="status">
              {t.loading}
            </p>
          )}
          {models.isError && <p className={styles.state}>{t.listFailed}</p>}
          {save.isError && (
            <p className={styles.error} role="alert">
              {t.saveFailed}
            </p>
          )}
          <div className={styles.divider} role="separator" />
          <Link to="/pengaturan/$bagian" params={{ bagian: "ai" }} role="menuitem" tabIndex={-1} className={styles.item} onClick={() => setOpen(false)}>
            <span className={styles.name}>{t.settings}</span>
          </Link>
        </div>
      )}
    </div>
  );
}
