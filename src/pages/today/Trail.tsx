// The last few things that changed, in plain sentences. ULTRON's rows carry its crimson mark.

import type { ActivityItem } from "../../data/generated/ActivityItem";
import { id, trailSentence } from "../../strings/id";
import { whenLabel } from "../../time/format";
import styles from "./today.module.css";

export function Trail({ items, now }: { items: readonly ActivityItem[]; now: Date }) {
  if (items.length === 0) return <p className={styles.empty}>{id.today.trailEmpty}</p>;
  return (
    <ul className={styles.trail}>
      {items.map((item) => (
        <li key={item.id} className={item.actor === "ultron" ? styles.byUltron : undefined}>
          <div>
            {trailSentence(item)}
            <time dateTime={item.at}>{whenLabel(item.at, now)}</time>
          </div>
        </li>
      ))}
    </ul>
  );
}
