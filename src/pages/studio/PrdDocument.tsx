// A PRD set for reading: section titles in the display face, paragraphs at a reading measure, ids
// in mono, assumptions and missing evidence marked, technical items marked as proposals.

import type { ReactNode } from "react";

import type { Flow } from "../../data/generated/Flow";
import type { Prd } from "../../data/generated/Prd";
import type { PrdSection } from "../../data/generated/PrdSection";
import { id } from "../../strings/id";
import { FlowDiagram } from "./FlowDiagram";
import styles from "./prd.module.css";

const t = id.studio.prd;
const f = t.fields;

function Para({ label, text }: { label: string; text: string }) {
  if (!text.trim()) return null;
  return (
    <div className={styles.para}>
      <h4>{label}</h4>
      <p>{text}</p>
    </div>
  );
}

function Bullets({ label, items, mono = false }: { label: string; items: string[]; mono?: boolean }) {
  if (items.length === 0) return null;
  return (
    <div className={styles.para}>
      <h4>{label}</h4>
      <ul>
        {items.map((item, index) => (
          <li key={index}>{mono ? <code>{item}</code> : item}</li>
        ))}
      </ul>
    </div>
  );
}

function Flows({ label, flows }: { label: string; flows: Flow[] }) {
  if (flows.length === 0) return null;
  return (
    <div className={styles.para}>
      <h4>{label}</h4>
      {flows.map((flow, index) => (
        <div key={index} className={styles.flow}>
          <p className={styles.flowName}>{flow.name}</p>
          <ol>
            {flow.steps.map((step, i) => (
              <li key={i}>{step}</li>
            ))}
          </ol>
        </div>
      ))}
    </div>
  );
}

/** One section's body. */
export function SectionBody({ prd, section }: { prd: Prd; section: PrdSection }): ReactNode {
  switch (section) {
    case "identity":
      return (
        <>
          <p className={styles.lead}>{prd.identity.summary}</p>
          <Para label={f.owner ?? ""} text={prd.identity.owner} />
        </>
      );
    case "background": {
      const b = prd.background;
      return (
        <>
          <Para label={f.problem ?? ""} text={b.problem} />
          <Para label={f.context ?? ""} text={b.context} />
          <Bullets label={f.targetUsers ?? ""} items={b.targetUsers} />
          <Para label={f.unmetNeed ?? ""} text={b.unmetNeed} />
          {b.evidence.length > 0 && (
            <div className={styles.para}>
              <h4>{f.evidence}</h4>
              <ul>
                {b.evidence.map((e, i) => (
                  <li key={i}>
                    {e.statement}
                    {e.source === "none" && <span className={styles.warnTag}> · {t.unknownEvidence}</span>}
                  </li>
                ))}
              </ul>
            </div>
          )}
          {b.assumptions.length > 0 && (
            <div className={styles.para}>
              <h4>{f.assumptions}</h4>
              <ul>
                {b.assumptions.map((a) => (
                  <li key={a.id}>
                    <code>{a.id}</code> <span className={styles.warnTag}>{t.assumption}</span> {a.statement}
                    {a.validate && <span className={styles.muted}> ({f.validate}: {a.validate})</span>}
                  </li>
                ))}
              </ul>
            </div>
          )}
        </>
      );
    }
    case "goals": {
      const g = prd.goals;
      return (
        <>
          <Para label={f.primary ?? ""} text={g.primary} />
          <Bullets label={f.benefits ?? ""} items={g.benefits} />
          <Para label={f.differentiator ?? ""} text={g.differentiator} />
          {g.metrics.length > 0 && (
            <div className={styles.para}>
              <h4>{f.metrics}</h4>
              <ul>
                {g.metrics.map((m, i) => (
                  <li key={i}>
                    {m.metric}: <strong>{m.target}</strong>
                    {m.how && <span className={styles.muted}> ({m.how})</span>}
                  </li>
                ))}
              </ul>
            </div>
          )}
          <Bullets label={f.outOfScope ?? ""} items={g.outOfScope} />
        </>
      );
    }
    case "experience": {
      const x = prd.experience;
      return (
        <>
          {x.personas.length > 0 && (
            <div className={styles.para}>
              <h4>{f.personas}</h4>
              <ul>
                {x.personas.map((p, i) => (
                  <li key={i}>
                    <strong>{p.name}</strong>
                    {p.need && `: ${p.need}`}
                    {p.context && <span className={styles.muted}> ({p.context})</span>}
                  </li>
                ))}
              </ul>
            </div>
          )}
          <Flows label={f.journeys ?? ""} flows={x.journeys} />
          <Flows label={f.mainFlows ?? ""} flows={x.mainFlows} />
          <Flows label={f.alternateFlows ?? ""} flows={x.alternateFlows} />
          <Bullets label={f.emptyStates ?? ""} items={x.emptyStates} />
          <Bullets label={f.errorStates ?? ""} items={x.errorStates} />
          <Bullets label={f.accessibility ?? ""} items={x.accessibility} />
        </>
      );
    }
    // U5b: the user flow as a diagram Hub draws.
    case "flow":
      return <FlowDiagram flow={prd.flow} />;
    case "features":
      return (
        <ol className={styles.features}>
          {prd.features.map((feature) => (
            <li key={feature.id} className={styles.feature} id={`prd-${feature.id}`}>
              <p className={styles.featureHead}>
                <code>{feature.id}</code> <strong>{feature.name}</strong> <span className={styles.priority}>{t.priority[feature.priority]}</span>
              </p>
              <p>{feature.description}</p>
              {feature.rationale && <p className={styles.muted}>{feature.rationale}</p>}
              <p className={styles.story}>{t.storyLine(feature.story.asA, feature.story.want, feature.story.soThat)}</p>
              {feature.dependsOn.length > 0 && (
                <p className={styles.muted}>
                  {t.dependsOn}: {feature.dependsOn.map((d) => <code key={d}>{d} </code>)}
                </p>
              )}
              {feature.acceptance.length > 0 && (
                <div className={styles.para}>
                  <h4>{f.acceptance}</h4>
                  <ul className={styles.criteria}>
                    {feature.acceptance.map((c) => (
                      <li key={c.id}>
                        <code>{c.id}</code>
                        <span>
                          <em>Given</em> {c.given}, <em>When</em> {c.when}, <em>Then</em> {c.then}
                        </span>
                      </li>
                    ))}
                  </ul>
                </div>
              )}
              <Bullets label={f.tests ?? ""} items={feature.tests} />
            </li>
          ))}
        </ol>
      );
    case "nfr":
      return (
        <ul className={styles.plain}>
          {prd.nfr.map((n, i) => (
            <li key={i}>
              <strong>{t.categories[n.category]}</strong>: {n.requirement}
              {n.measure && <span className={styles.muted}> ({f.measure}: {n.measure})</span>}
            </li>
          ))}
        </ul>
      );
    case "technical": {
      const x = prd.technical;
      if (!x.applicable) return <p className={styles.muted}>{t.notApplicable}</p>;
      return (
        <>
          {x.fromStack ? (
            <>
              <div className={styles.para}>
                <h4>
                  {f.technologies} <span className={styles.muted}>({t.stackTag})</span>
                </h4>
                <ul>
                  {x.technologies.map((item, index) => (
                    <li key={index}>{item}</li>
                  ))}
                </ul>
              </div>
              <p className={styles.warnTag}>{t.onStack}</p>
            </>
          ) : (
            <p className={styles.warnTag}>{t.proposalTag}</p>
          )}
          <Para label={f.architecture ?? ""} text={x.architecture} />
          <Bullets label={f.components ?? ""} items={x.components} />
          <Para label={f.dataModel ?? ""} text={x.dataModel} />
          <Bullets label={f.integrations ?? ""} items={x.integrations} />
          {!x.fromStack && <Bullets label={f.technologies ?? ""} items={x.technologies} />}
          <Bullets label={f.dependencies ?? ""} items={x.dependencies} />
          <Bullets label={f.constraints ?? ""} items={x.constraints} />
          <Bullets label={f.techRisks ?? ""} items={x.risks} />
        </>
      );
    }
    case "security":
      return (
        <ul className={styles.plain}>
          {prd.security.map((s) => (
            <li key={s.id}>
              <code>{s.id}</code> <strong>{t.securityAreas[s.area]}</strong>: {s.requirement}
              {s.verify && <span className={styles.muted}> ({f.verify}: {s.verify})</span>}
            </li>
          ))}
        </ul>
      );
    case "delivery": {
      const d = prd.delivery;
      return (
        <>
          <Bullets label={f.mvp ?? ""} items={d.mvp} mono />
          <Bullets label={f.later ?? ""} items={d.later} mono />
          <Bullets label={f.order ?? ""} items={d.order} mono />
          <Para label={f.testing ?? ""} text={d.testing} />
          <Para label={f.release ?? ""} text={d.release} />
          {d.risks.length > 0 && (
            <div className={styles.para}>
              <h4>{f.risks}</h4>
              <ul>
                {d.risks.map((r, i) => (
                  <li key={i}>
                    {r.risk}
                    {r.impact && <span className={styles.muted}> ({f.impact}: {r.impact})</span>}
                    {r.mitigation && <span> · {f.mitigation}: {r.mitigation}</span>}
                  </li>
                ))}
              </ul>
            </div>
          )}
          {d.questions.length > 0 && (
            <div className={styles.para}>
              <h4>{f.questions}</h4>
              <ul>
                {d.questions.map((q) => (
                  <li key={q.id}>
                    <code>{q.id}</code> {q.question}
                    {q.owner && <span className={styles.muted}> ({q.owner})</span>}
                  </li>
                ))}
              </ul>
            </div>
          )}
        </>
      );
    }
  }
}
