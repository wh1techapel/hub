// Lentera's light: a small orb made of the lights of Studio's glow (tokens.css --glow-a to --glow-c
// and the owner's accent), so it follows the day the way the glow does. Decorative: whatever it sits
// beside names Lentera in words. An <i>, like an icon, so the sidebar's rail (which hides an item's
// text spans) keeps it.

import styles from "./lenteraMark.module.css";

export function LenteraMark({ size = "small" }: { size?: "small" | "large" }) {
  return <i className={`${styles.mark} ${size === "large" ? styles.large : ""}`} aria-hidden="true" />;
}
