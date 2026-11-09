// The shape of each PRD section for the editor: which fields, of what kind, and how a new item
// gets its id. Labels come from id.ts. The core checks everything again when it is saved.

import type { PrdSection } from "../../data/generated/PrdSection";
import { id } from "../../strings/id";

const f = id.studio.prd.fields;
const label = (key: string) => f[key] ?? key;

export type Field =
  | { key: string; label: string; kind: "text" | "long" | "lines" | "bool" | "id" }
  | { key: string; label: string; kind: "select"; options: { value: string; label: string }[] }
  | { key: string; label: string; kind: "group"; fields: Field[] }
  | { key: string; label: string; kind: "list"; itemLabel: string; fields: Field[]; make: (items: Record<string, unknown>[], parent: Record<string, unknown>) => Record<string, unknown> };

type Item = Record<string, unknown>;

/** The next number after the highest `PREFIX-NN` in `items`. */
function nextId(items: Item[], prefix: string, digits: number): string {
  const top = items.reduce((max, item) => {
    const n = Number((typeof item.id === "string" ? item.id : "").slice(prefix.length));
    return Number.isFinite(n) && n > max ? n : max;
  }, 0);
  return `${prefix}${String(top + 1).padStart(digits, "0")}`;
}

const flow = (key: string): Field => ({
  key,
  label: label(key),
  kind: "list",
  itemLabel: label("flowName"),
  fields: [
    { key: "name", label: label("flowName"), kind: "text" },
    { key: "steps", label: label("steps"), kind: "lines" },
  ],
  make: () => ({ name: "", steps: [] }),
});

const select = (key: string, options: Record<string, string>): Field => ({
  key,
  label: label(key),
  kind: "select",
  options: Object.entries(options).map(([value, text]) => ({ value, label: text })),
});

/** A section's fields. Features and NFRs are lists at their root (key ""). */
export const SECTION_FIELDS: Record<PrdSection, Field[]> = {
  identity: [
    { key: "name", label: label("name"), kind: "text" },
    { key: "summary", label: label("summary"), kind: "long" },
    { key: "owner", label: label("owner"), kind: "text" },
  ],
  background: [
    { key: "problem", label: label("problem"), kind: "long" },
    { key: "context", label: label("context"), kind: "long" },
    { key: "targetUsers", label: label("targetUsers"), kind: "lines" },
    { key: "unmetNeed", label: label("unmetNeed"), kind: "long" },
    {
      key: "evidence",
      label: label("evidence"),
      kind: "list",
      itemLabel: label("evidence"),
      fields: [{ key: "statement", label: label("statement"), kind: "long" }, select("source", id.studio.prd.sources)],
      make: () => ({ statement: "", source: "none" }),
    },
    {
      key: "assumptions",
      label: label("assumptions"),
      kind: "list",
      itemLabel: id.studio.prd.assumption,
      fields: [
        { key: "id", label: "ID", kind: "id" },
        { key: "statement", label: label("statement"), kind: "long" },
        { key: "validate", label: label("validate"), kind: "text" },
      ],
      make: (items) => ({ id: nextId(items, "ASM-", 2), statement: "", validate: "" }),
    },
  ],
  goals: [
    { key: "primary", label: label("primary"), kind: "long" },
    { key: "benefits", label: label("benefits"), kind: "lines" },
    { key: "differentiator", label: label("differentiator"), kind: "long" },
    {
      key: "metrics",
      label: label("metrics"),
      kind: "list",
      itemLabel: label("metric"),
      fields: [
        { key: "metric", label: label("metric"), kind: "text" },
        { key: "target", label: label("target"), kind: "text" },
        { key: "how", label: label("how"), kind: "text" },
      ],
      make: () => ({ metric: "", target: "", how: "" }),
    },
    { key: "outOfScope", label: label("outOfScope"), kind: "lines" },
  ],
  experience: [
    {
      key: "personas",
      label: label("personas"),
      kind: "list",
      itemLabel: label("personas"),
      fields: [
        { key: "name", label: label("personaName"), kind: "text" },
        { key: "need", label: label("need"), kind: "text" },
        { key: "context", label: label("context"), kind: "text" },
      ],
      make: () => ({ name: "", need: "", context: "" }),
    },
    flow("journeys"),
    flow("mainFlows"),
    flow("alternateFlows"),
    { key: "emptyStates", label: label("emptyStates"), kind: "lines" },
    { key: "errorStates", label: label("errorStates"), kind: "lines" },
    { key: "accessibility", label: label("accessibility"), kind: "lines" },
  ],
  features: [
    {
      key: "",
      label: label("feature"),
      kind: "list",
      itemLabel: label("feature"),
      fields: [
        { key: "id", label: "ID", kind: "id" },
        { key: "name", label: label("featureName"), kind: "text" },
        { key: "description", label: label("description"), kind: "long" },
        { key: "rationale", label: label("rationale"), kind: "long" },
        select("priority", id.studio.prd.priority),
        { key: "dependsOn", label: label("dependsOn"), kind: "lines" },
        {
          key: "story",
          label: label("story"),
          kind: "group",
          fields: [
            { key: "asA", label: label("asA"), kind: "text" },
            { key: "want", label: label("want"), kind: "text" },
            { key: "soThat", label: label("soThat"), kind: "text" },
          ],
        },
        {
          key: "acceptance",
          label: label("acceptance"),
          kind: "list",
          itemLabel: label("acceptance"),
          fields: [
            { key: "id", label: "ID", kind: "id" },
            { key: "given", label: label("given"), kind: "text" },
            { key: "when", label: label("when"), kind: "text" },
            { key: "then", label: label("then"), kind: "text" },
          ],
          make: (items, parent) => {
            const number = (typeof parent.id === "string" ? parent.id : "FEAT-000").replace("FEAT-", "");
            return { id: nextId(items, `AC-${number}-`, 1), given: "", when: "", then: "" };
          },
        },
        { key: "tests", label: label("tests"), kind: "lines" },
      ],
      make: (items) => ({
        id: nextId(items, "FEAT-", 3),
        name: "",
        description: "",
        rationale: "",
        priority: "should",
        dependsOn: [],
        story: { asA: "", want: "", soThat: "" },
        acceptance: [],
        tests: [],
      }),
    },
  ],
  nfr: [
    {
      key: "",
      label: label("nfrItem"),
      kind: "list",
      itemLabel: label("nfrItem"),
      fields: [select("category", id.studio.prd.categories), { key: "requirement", label: label("requirement"), kind: "text" }, { key: "measure", label: label("measure"), kind: "text" }],
      make: () => ({ category: "performance", requirement: "", measure: "" }),
    },
  ],
  technical: [
    { key: "applicable", label: label("applicable"), kind: "bool" },
    { key: "architecture", label: label("architecture"), kind: "long" },
    { key: "components", label: label("components"), kind: "lines" },
    { key: "dataModel", label: label("dataModel"), kind: "long" },
    { key: "integrations", label: label("integrations"), kind: "lines" },
    { key: "technologies", label: label("technologies"), kind: "lines" },
    { key: "dependencies", label: label("dependencies"), kind: "lines" },
    { key: "constraints", label: label("constraints"), kind: "lines" },
    { key: "risks", label: label("techRisks"), kind: "lines" },
  ],
  // U5b: the user flow diagram, its steps and arrows edited as rows.
  flow: [
    { key: "title", label: label("flowTitle"), kind: "text" },
    {
      key: "nodes",
      label: label("flowNodes"),
      kind: "list",
      itemLabel: label("flowNode"),
      fields: [
        { key: "id", label: "ID", kind: "id" },
        { key: "kind", label: label("flowKind"), kind: "select", options: Object.entries(id.studio.prd.flowKinds).map(([value, text]) => ({ value, label: text })) },
        { key: "label", label: label("flowLabel"), kind: "text" },
        { key: "feature", label: label("flowFeature"), kind: "text" },
      ],
      make: (items) => ({ id: nextId(items, "N", 1), kind: "screen", label: "", feature: "" }),
    },
    {
      key: "edges",
      label: label("flowEdges"),
      kind: "list",
      itemLabel: label("flowEdge"),
      fields: [
        { key: "from", label: label("flowFrom"), kind: "text" },
        { key: "to", label: label("flowTo"), kind: "text" },
        { key: "label", label: label("flowEdgeLabel"), kind: "text" },
      ],
      make: () => ({ from: "", to: "", label: "" }),
    },
  ],
  // R4: security requirements, a list at the section's root like the NFRs.
  security: [
    {
      key: "",
      label: label("securityItem"),
      kind: "list",
      itemLabel: label("securityItem"),
      fields: [
        { key: "id", label: "ID", kind: "id" },
        { key: "area", label: label("securityArea"), kind: "select", options: Object.entries(id.studio.prd.securityAreas).map(([value, text]) => ({ value, label: text })) },
        { key: "requirement", label: label("requirement"), kind: "long" },
        { key: "verify", label: label("verify"), kind: "text" },
      ],
      make: (items) => ({ id: nextId(items, "SEC-", 2), area: "data", requirement: "", verify: "" }),
    },
  ],
  delivery: [
    { key: "mvp", label: label("mvp"), kind: "lines" },
    { key: "later", label: label("later"), kind: "lines" },
    { key: "order", label: label("order"), kind: "lines" },
    { key: "testing", label: label("testing"), kind: "long" },
    { key: "release", label: label("release"), kind: "long" },
    {
      key: "risks",
      label: label("risks"),
      kind: "list",
      itemLabel: label("risk"),
      fields: [
        { key: "risk", label: label("risk"), kind: "text" },
        { key: "impact", label: label("impact"), kind: "text" },
        { key: "mitigation", label: label("mitigation"), kind: "text" },
      ],
      make: () => ({ risk: "", impact: "", mitigation: "" }),
    },
    {
      key: "questions",
      label: label("questions"),
      kind: "list",
      itemLabel: label("question"),
      fields: [
        { key: "id", label: "ID", kind: "id" },
        { key: "question", label: label("question"), kind: "text" },
        { key: "owner", label: label("answerer"), kind: "text" },
      ],
      make: (items) => ({ id: nextId(items, "Q-", 2), question: "", owner: "" }),
    },
  ],
};

export const SECTIONS: PrdSection[] = ["identity", "background", "goals", "experience", "flow", "features", "nfr", "technical", "security", "delivery"];

/** The section a problem's place belongs to (same names as the core's). */
export function sectionOf(at: string): PrdSection {
  const head = at.split(/[.[]/)[0] ?? "";
  if (head.startsWith("FEAT-") || head.startsWith("AC-")) return "features";
  if (head.startsWith("ASM-")) return "background";
  if (head.startsWith("Q-")) return "delivery";
  if (head.startsWith("SEC-")) return "security";
  return (SECTIONS as string[]).includes(head) ? (head as PrdSection) : "delivery";
}

export { nextId };
