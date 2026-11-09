// Checks WCAG contrast of every text colour in design/tokens.css against every paper it can sit
// on, in every light phase and every accent ink, and over the glow on Studio's home (L3).
// Run: node design/check-contrast.mjs
// Exits 1 when a pair is under its minimum (4.5:1 for text).

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const css = readFileSync(fileURLToPath(new URL("./tokens.css", import.meta.url)), "utf8");

/** Declarations of the first rule whose selector is exactly `selector`. */
function block(selector) {
  const escaped = selector.replace(/[[\]().*+?^$|\\]/g, "\\$&");
  const found = new RegExp(`(^|\\n)${escaped}\\s*\\{`).exec(css);
  if (!found) throw new Error(`no rule for ${selector}`);
  const start = found.index;
  const body = css.slice(css.indexOf("{", start) + 1, css.indexOf("}", start));
  const decls = {};
  for (const m of body.matchAll(/--([\w-]+):\s*([^;]+);/g)) decls[m[1]] = m[2].trim();
  return decls;
}

const root = block(":root");

function resolve(value, scope) {
  const v = value.match(/^var\(--([\w-]+)\)$/);
  if (!v) return value;
  const next = scope[v[1]] ?? root[v[1]];
  if (!next) throw new Error(`unresolved ${value}`);
  return resolve(next, scope);
}

// OKLCH or #hex to linear sRGB, then WCAG relative luminance.
function toLinearRgb(color) {
  const hex = color.match(/^#([0-9a-f]{6})$/i);
  if (hex) {
    return [0, 2, 4].map((i) => {
      const c = parseInt(hex[1].slice(i, i + 2), 16) / 255;
      return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
    });
  }
  const m = color.match(/^oklch\(\s*([\d.]+)\s+([\d.]+)\s+([\d.]+)/);
  if (!m) throw new Error(`cannot read colour ${color}`);
  const [L, C, H] = m.slice(1).map(Number);
  const a = C * Math.cos((H * Math.PI) / 180);
  const b = C * Math.sin((H * Math.PI) / 180);
  const l = (L + 0.3963377774 * a + 0.2158037573 * b) ** 3;
  const mm = (L - 0.1055613458 * a - 0.0638541728 * b) ** 3;
  const s = (L - 0.0894841775 * a - 1.291485548 * b) ** 3;
  const rgb = [
    4.0767416621 * l - 3.3077115913 * mm + 0.2309699292 * s,
    -1.2684380046 * l + 2.6097574011 * mm - 0.3413193965 * s,
    -0.0041960863 * l - 0.7034186147 * mm + 1.707614701 * s,
  ];
  return rgb.map((c) => Math.min(1, Math.max(0, c)));
}

// A translucent colour laid over a paper, the way the browser blends it (in sRGB, not linear).
function over(color, alpha, paper) {
  const gamma = (c) => (c <= 0.0031308 ? 12.92 * c : 1.055 * c ** (1 / 2.4) - 0.055);
  const linear = (c) => (c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4);
  // The paper may itself be a blend already (linear sRGB), as under the glow's stacked lights.
  const [top, bottom] = [toLinearRgb(color), Array.isArray(paper) ? paper : toLinearRgb(paper)];
  return top.map((c, i) => linear(alpha * gamma(c) + (1 - alpha) * gamma(bottom[i])));
}

function luminance(color) {
  const [r, g, b] = Array.isArray(color) ? color : toLinearRgb(color);
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

function contrast(fg, bg) {
  const [hi, lo] = [luminance(fg), luminance(bg)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}

const phases = ["pagi", "siang", "senja", "malam"];
const inks = ["biru", "hijau", "sepia", "ungu", "grafit"];
const pigments = { lazuardi: 250, toska: 195, lumut: 140, kunyit: 85, terakota: 45, mawar: 10, lavender: 300, batu: 265 };

let failures = 0;
let checked = 0;
let lowest = { ratio: Infinity, label: "" };

function expect(label, fg, bg, min = 4.5) {
  const ratio = contrast(fg, bg);
  checked += 1;
  if (ratio < lowest.ratio) lowest = { ratio, label };
  if (ratio < min) {
    failures += 1;
    console.log(`FAIL ${ratio.toFixed(2)} < ${min}  ${label}`);
  }
}

for (const phase of phases) {
  const p = block(`[data-phase="${phase}"]`);
  const schemeName = phase === "malam" ? "dark" : "light";
  const scheme = block(`[data-scheme="${schemeName}"]`);
  const papers = ["paper", "paper-2", "surface"].map((k) => [k, resolve(p[k], p)]);
  const texts = ["ink", "ink-2", "ink-3"].map((k) => [k, resolve(p[k], p)]);
  texts.push(["warn", scheme.warn], ["danger", scheme.danger], ["ultron", resolve(scheme.ultron, scheme)]);

  for (const [pk, paper] of papers) {
    for (const [tk, text] of texts) expect(`${phase}: ${tk} on ${pk}`, text, paper);
  }

  // The soft tints (a notice, a selected row) are the warn or accent colour at a low alpha over a
  // paper; text on them is ink or ink-2 (ink-3, the faintest, is kept off them). The alpha comes
  // from the token itself.
  const alphaOf = (token) => Number(scheme[token].match(/\/\s*([\d.]+)\s*\)$/)[1]);
  const softs = { "warn-soft": [scheme.warn, alphaOf("warn-soft")] };
  for (const ink of inks) {
    const accent = block(`[data-ink="${ink}"]`)[`ink-accent-${schemeName}`];
    softs[`accent-soft/${ink}`] = [accent, alphaOf("accent-soft")];
  }
  for (const [sk, [tint, alpha]] of Object.entries(softs)) {
    for (const [pk, paper] of papers) {
      const soft = over(tint, alpha, paper);
      for (const [tk, text] of texts.slice(0, 2)) expect(`${phase}: ${tk} on ${sk} over ${pk}`, text, soft);
    }
  }

  for (const ink of inks) {
    const i = block(`[data-ink="${ink}"]`);
    const accent = i[`ink-accent-${schemeName}`];
    for (const [pk, paper] of papers) expect(`${phase}/${ink}: accent on ${pk}`, accent, paper);
    expect(`${phase}/${ink}: on-accent on accent`, scheme["on-accent"], accent);
  }

  // L3: the glow behind Studio's home. Text there is ink or ink-2 (ink-3 is kept off it), and is
  // checked against the worst place: all four lights at their brightest on the same spot, for
  // every accent ink.
  const glowAccent = scheme["glow-accent"].match(/^oklch\(from var\(--accent\) ([\d.]+) c h \/ ([\d.]+)\)$/);
  if (!glowAccent) throw new Error(`cannot read glow-accent in ${schemeName}`);
  for (const ink of inks) {
    const accent = block(`[data-ink="${ink}"]`)[`ink-accent-${schemeName}`];
    const [, , c, h] = accent.match(/^oklch\(\s*([\d.]+)\s+([\d.]+)\s+([\d.]+)/);
    const lights = ["glow-a", "glow-b", "glow-c"].map((k) => p[k]);
    lights.push(`oklch(${glowAccent[1]} ${c} ${h} / ${glowAccent[2]})`);
    let lit = toLinearRgb(papers[0][1]);
    for (const light of lights) {
      const [, color, alpha] = light.match(/^(oklch\([^/]+)\/\s*([\d.]+)\)$/);
      lit = over(`${color.trim()})`, Number(alpha), lit);
    }
    for (const [tk, text] of texts.slice(0, 2)) expect(`${phase}/${ink}: ${tk} on the glow`, text, lit);
  }

  for (const [name, hue] of Object.entries(pigments)) {
    const pig = name === "batu" ? block('[data-pigment="batu"]') : {};
    const c = (role) => Number(pig[`pig-${role}-c`] ?? scheme[`pig-${role}-c`]);
    const fill = `oklch(${scheme["pig-fill-l"]} ${c("fill")} ${hue})`;
    const text = `oklch(${scheme["pig-text-l"]} ${c("text")} ${hue})`;
    expect(`${phase}/${name}: pigment text on pigment fill`, text, fill);
  }
}

console.log(`${checked} pairs checked, lowest ${lowest.ratio.toFixed(2)}:1 (${lowest.label}).`);
if (failures) {
  console.log(`${failures} pair(s) under the minimum.`);
  process.exit(1);
}
console.log("All text pairs meet 4.5:1.");
