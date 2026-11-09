import { expect } from "vitest";

// The accessibility pass from CP5, as a check a test can run on whatever the page shows now:
// one h1, unique ids, every id an attribute points at exists, and every control has a name.
// Returns the problems as sentences, so a failing test lists all of them at once.

const CONTROLS = [
  "button",
  "a[href]",
  "input:not([type=hidden])",
  "textarea",
  "select",
  "[role=tab]",
  "[role=switch]",
  "[role=radio]",
  "[role=checkbox]",
  "[role=dialog]",
  "[role=tablist]",
  "[role=form]",
  "form[aria-label], form[aria-labelledby]",
].join(", ");

const POINTERS = ["aria-labelledby", "aria-describedby", "aria-controls"];

function describe(element: Element): string {
  const id = element.id ? `#${element.id}` : "";
  const text = element.textContent.trim().slice(0, 40);
  return `<${element.tagName.toLowerCase()}${id}${element.getAttribute("role") ? ` role=${element.getAttribute("role")}` : ""}> "${text}"`;
}

export function a11yProblems(root: Document = document): string[] {
  const problems: string[] = [];
  const h1 = root.querySelectorAll("h1").length;
  if (h1 !== 1) problems.push(`${h1} h1 elements (one expected)`);

  const seen = new Map<string, number>();
  for (const element of root.querySelectorAll("[id]")) seen.set(element.id, (seen.get(element.id) ?? 0) + 1);
  for (const [id, count] of seen) if (count > 1) problems.push(`id "${id}" used ${count} times`);

  for (const element of root.querySelectorAll(POINTERS.map((p) => `[${p}]`).join(", "))) {
    for (const pointer of POINTERS) {
      for (const id of (element.getAttribute(pointer) ?? "").split(/\s+/).filter(Boolean)) {
        if (!root.getElementById(id)) problems.push(`${describe(element)} ${pointer} points at missing "${id}"`);
      }
    }
  }
  for (const label of root.querySelectorAll("label[for]")) {
    const target = label.getAttribute("for") ?? "";
    if (!root.getElementById(target)) problems.push(`label "${label.textContent.trim()}" is for missing "${target}"`);
  }

  for (const element of root.querySelectorAll(CONTROLS)) {
    try {
      expect(element).toHaveAccessibleName();
    } catch {
      problems.push(`${describe(element)} has no accessible name`);
    }
  }
  return problems;
}
