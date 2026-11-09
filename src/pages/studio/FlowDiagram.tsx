// U5b: a PRD's user flow drawn as a diagram (flowLayout.ts places it). Hub draws it itself from
// the PRD's data, as SVG elements made by React: nothing from Gemini is ever read as markup. The
// same flow follows as an ordered list for screen readers and for reading it as text.

import { useId } from "react";

import type { FlowChart } from "../../data/generated/FlowChart";
import { id } from "../../strings/id";
import { layoutFlow } from "./flowLayout";
import type { PlacedNode } from "./flowLayout";
import styles from "./prd.module.css";

const t = id.studio.prd;

/** A step's outline: a pill for the start and the end, a hexagon for a decision, a box otherwise. */
function Shape({ node }: { node: PlacedNode }) {
  const { x, y, width: w, height: h } = node;
  if (node.kind === "decision") {
    const inset = 16;
    const points = [
      [x + inset, y],
      [x + w - inset, y],
      [x + w, y + h / 2],
      [x + w - inset, y + h],
      [x + inset, y + h],
      [x, y + h / 2],
    ]
      .map((p) => p.join(","))
      .join(" ");
    return <polygon points={points} className={styles.flowDecision} />;
  }
  const radius = node.kind === "start" || node.kind === "end" ? h / 2 : node.kind === "action" ? 14 : 6;
  const kind = node.kind === "start" || node.kind === "end" ? styles.flowEnds : node.kind === "action" ? styles.flowAction : styles.flowScreen;
  return <rect x={x} y={y} width={w} height={h} rx={Math.min(radius, h / 2)} className={kind} />;
}

export function FlowDiagram({ flow }: { flow: FlowChart }) {
  const caption = useId();
  const marker = `${useId()}-arrow`;
  if (flow.nodes.length === 0) return <p className={styles.muted}>{t.flow.empty}</p>;
  const layout = layoutFlow(flow);
  const names = new Map(flow.nodes.map((node) => [node.id, node.label]));

  return (
    <figure className={styles.flowFigure} aria-labelledby={caption}>
      <figcaption id={caption} className={styles.flowCaption}>
        {t.flow.figure(flow.title.trim())}
      </figcaption>
      <div className={styles.flowScroll}>
        <svg
          role="img"
          aria-labelledby={caption}
          width={layout.width}
          height={layout.height}
          viewBox={`0 0 ${layout.width} ${layout.height}`}
          className={styles.flowSvg}
        >
          <defs>
            <marker id={marker} viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse">
              <path d="M 0 0 L 10 5 L 0 10 z" className={styles.flowArrowHead} />
            </marker>
          </defs>
          {layout.edges.map((edge, i) => (
            <g key={`${edge.from}-${edge.to}-${i}`}>
              <polyline
                points={edge.points.map((p) => p.join(",")).join(" ")}
                className={edge.back ? styles.flowBack : styles.flowEdge}
                markerEnd={`url(#${marker})`}
              />
              {edge.labelAt && (
                <text x={edge.labelAt[0] + 6} y={edge.labelAt[1]} className={styles.flowEdgeLabel}>
                  {edge.label}
                </text>
              )}
            </g>
          ))}
          {layout.nodes.map((node) => {
            const lines = node.feature ? [...node.lines, node.feature] : node.lines;
            const top = node.y + (node.height - lines.length * 18) / 2 + 13;
            const ends = node.kind === "start" || node.kind === "end";
            return (
              <g key={node.id}>
                <Shape node={node} />
                <text x={node.x + node.width / 2} y={top} textAnchor="middle" className={ends ? styles.flowTextOnAccent : styles.flowText}>
                  {lines.map((line, i) => (
                    <tspan key={i} x={node.x + node.width / 2} dy={i === 0 ? 0 : 18} className={node.feature && i === lines.length - 1 ? styles.flowFeature : undefined}>
                      {line}
                    </tspan>
                  ))}
                </text>
              </g>
            );
          })}
        </svg>
      </div>
      {layout.incomplete && <p className={styles.muted}>{t.flow.broken}</p>}
      <details className={styles.flowAsText}>
        <summary>{t.flow.asText}</summary>
        <ol>
          {flow.nodes.map((node) => (
            <li key={node.id}>
              <code>{node.id}</code> <strong>{t.flowKinds[node.kind]}</strong>: {node.label}
              {node.feature.trim() && <span className={styles.muted}> ({t.flow.handledBy(node.feature.trim())})</span>}
              {flow.edges.filter((edge) => edge.from === node.id).length > 0 && (
                <ul>
                  {flow.edges
                    .filter((edge) => edge.from === node.id)
                    .map((edge, i) => (
                      <li key={i}>{t.flow.to(edge.label.trim(), `${edge.to} ${names.get(edge.to) ?? ""}`.trim())}</li>
                    ))}
                </ul>
              )}
            </li>
          ))}
        </ol>
      </details>
    </figure>
  );
}
