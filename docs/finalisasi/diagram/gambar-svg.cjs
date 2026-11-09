// Two diagrams Mermaid cannot lay out for an A4 page, drawn by hand as SVG:
//   02-kasus-penggunaan.svg  (UML use case: actors, the Hub boundary, the use cases)
//   04-alur-studio.svg       (Hub Studio's three stages as columns)
// Run: node gambar-svg.cjs  (writes the .svg files beside it; render-mermaid.js turns them into PNG)
const fs = require("fs");
const path = require("path");

const FONT = "font-family=\"Arial, sans-serif\"";
const esc = (s) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;");
const lines = (x, y, text, size = 15, weight = "normal", anchor = "middle") => {
  const parts = text.split("\n");
  const top = y - ((parts.length - 1) * size * 1.25) / 2;
  return parts.map((t, i) => `<text x="${x}" y="${top + i * size * 1.25 + size * 0.35}" ${FONT} font-size="${size}" font-weight="${weight}" text-anchor="${anchor}" fill="#222">${esc(t)}</text>`).join("");
};
const defs = `<defs><marker id="arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="8" markerHeight="8" orient="auto-start-reverse"><path d="M0,0 L10,5 L0,10 z" fill="#555"/></marker></defs>`;
const svg = (w, h, body) => `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}"><rect width="${w}" height="${h}" fill="#fff"/>${defs}${body}</svg>`;

// ---------- use case ----------
function actor(x, y, label) {
  return `<g stroke="#333" stroke-width="2" fill="none"><circle cx="${x}" cy="${y - 38}" r="12"/><line x1="${x}" y1="${y - 26}" x2="${x}" y2="${y + 8}"/><line x1="${x - 20}" y1="${y - 14}" x2="${x + 20}" y2="${y - 14}"/><line x1="${x}" y1="${y + 8}" x2="${x - 16}" y2="${y + 34}"/><line x1="${x}" y1="${y + 8}" x2="${x + 16}" y2="${y + 34}"/></g>${lines(x, y + 56, label, 15, "bold")}`;
}
function useCase(cx, cy, text, rx = 150, ry = 30) {
  return `<ellipse cx="${cx}" cy="${cy}" rx="${rx}" ry="${ry}" fill="#eef2f8" stroke="#555" stroke-width="1.5"/>${lines(cx, cy, text, 14)}`;
}
const link = (x1, y1, x2, y2) => `<line x1="${x1}" y1="${y1}" x2="${x2}" y2="${y2}" stroke="#666" stroke-width="1.3"/>`;

function useCaseDiagram() {
  const left = [
    "Mengelola tugas\ndan proyek",
    "Mengatur jadwal,\nrutinitas, pengingat",
    "Catat Cepat",
    "Tutup Hari dan\nmemilih fokus",
    "Mengelola berkas",
    "Mencatat kegiatan\nbelajar",
    "Melihat Jejak\ndan mencari",
    "Merancang proyek\ndi Studio",
    "Menyetujui PRD\ndan checkpoint",
  ];
  const right = [
    { text: "Membaca jadwal dan\ndaftar tugas", actor: 0 },
    { text: "Membuat tugas, pengingat,\ncatatan; mencari berkas", actor: 0 },
    { text: "Menyusun pertanyaan, tech\nstack, PRD, rencana", actor: 1 },
    { text: "Membaca PRD dan checkpoint,\nmengirim laporan", actor: 2 },
  ];
  const W = 1120, H = 800, top = 70, step = 76;
  let body = `<rect x="230" y="20" width="660" height="${H - 40}" rx="6" fill="none" stroke="#333" stroke-width="2"/>${lines(560, 44, "Hub", 18, "bold")}`;
  const pY = top + (step * (left.length - 1)) / 2;
  body += actor(110, pY, "Pengguna");
  left.forEach((t, i) => {
    const y = top + 20 + i * step;
    body += link(132, pY - 14, 260, y) + useCase(410, y, t);
  });
  const actors = [
    { y: 205, label: "ULTRON" },
    { y: 435, label: "Lentera\n(Google Gemini)" },
    { y: 620, label: "Claude Code" },
  ];
  const ys = [150, 260, 435, 620];
  right.forEach((r, i) => {
    body += link(1010 - 22, actors[r.actor].y - 14, 860, ys[i]) + useCase(715, ys[i], r.text, 150, 32);
  });
  actors.forEach((a) => (body += actor(1010, a.y, a.label)));
  return svg(W, H, body);
}

// ---------- Studio flow ----------
function node(n) {
  const { x, y, w = 250, h = 54, text, shape = "box" } = n;
  if (shape === "diamond") {
    const hw = w / 2, hh = h / 2;
    return `<polygon points="${x},${y - hh} ${x + hw},${y} ${x},${y + hh} ${x - hw},${y}" fill="#fff7e0" stroke="#555" stroke-width="1.5"/>${lines(x, y, text, 13)}`;
  }
  const r = shape === "round" ? h / 2 : 6;
  return `<rect x="${x - w / 2}" y="${y - h / 2}" width="${w}" height="${h}" rx="${r}" fill="${shape === "round" ? "#e6f0e6" : "#eef2f8"}" stroke="#555" stroke-width="1.5"/>${lines(x, y, text, 13)}`;
}
const arrow = (pts, label, lx, ly) =>
  `<polyline points="${pts.map((p) => p.join(",")).join(" ")}" fill="none" stroke="#555" stroke-width="1.5" marker-end="url(#arrow)"/>` +
  (label ? `<rect x="${lx - 4 - label.length * 3.6}" y="${ly - 10}" width="${label.length * 7.2 + 8}" height="18" fill="#fff"/>${lines(lx, ly, label, 12, "normal")}` : "");

function studioFlow() {
  const W = 1120, H = 760;
  const cols = [{ x: 190, title: "Tahap 1: Mau buat apa?" }, { x: 560, title: "Tahap 2: Tech stack" }, { x: 930, title: "Tahap 3: PRD dan bangun" }];
  let body = "";
  cols.forEach((c) => {
    body += `<rect x="${c.x - 175}" y="15" width="350" height="${H - 30}" rx="8" fill="#fafafa" stroke="#bbb"/>${lines(c.x, 42, c.title, 16, "bold")}`;
  });
  const [x1, x2, x3] = cols.map((c) => c.x);
  const n = {
    a: { x: x1, y: 100, text: "Pengguna menulis ide", shape: "round" },
    b: { x: x1, y: 190, text: "Lentera mengajukan\nsatu set pertanyaan" },
    c: { x: x1, y: 280, text: "Pengguna menjawab" },
    d: { x: x1, y: 390, w: 210, h: 100, text: "Lima bagian ide\nsudah jelas?", shape: "diamond" },
    e: { x: x2, y: 100, text: "Dipilih sendiri atau\ndirekomendasikan Lentera" },
    f: { x: x2, y: 220, w: 190, h: 90, text: "Tech stack\ndisetujui?", shape: "diamond" },
    g: { x: x3, y: 100, text: "PRD disusun tiga bagian,\ndiperiksa dan diperbaiki Hub" },
    h: { x: x3, y: 205, w: 180, h: 80, text: "PRD\ndisetujui?", shape: "diamond" },
    i: { x: x3, y: 305, text: "Rencana checkpoint\ndisusun dan disetujui" },
    k: { x: x3, y: 395, text: "Pengguna memulai\ncheckpoint" },
    l: { x: x3, y: 485, text: "Claude Code bekerja lewat\nMCP, mengirim laporan" },
    m: { x: x3, y: 590, w: 190, h: 86, text: "Laporan\nditinjau", shape: "diamond" },
    o: { x: x3, y: 690, text: "Semua checkpoint selesai", shape: "round" },
  };
  Object.values(n).forEach((v) => (body += node(v)));
  const bottom = (v) => [v.x, v.y + (v.h ?? 54) / 2];
  const topOf = (v) => [v.x, v.y - (v.h ?? 54) / 2];
  // Stage 1.
  body += arrow([bottom(n.a), topOf(n.b)]) + arrow([bottom(n.b), topOf(n.c)]) + arrow([bottom(n.c), topOf(n.d)]);
  body += arrow([[x1 - 105, 390], [x1 - 150, 390], [x1 - 150, 190], [x1 - 125, 190]], "belum", x1 - 150, 300);
  body += arrow([[x1 + 105, 390], [x1 + 165, 390], [x1 + 165, 500], [x2 - 180, 500], [x2 - 180, 100], [x2 - 125, 100]], "ya", x1 + 165, 450);
  // Stage 2.
  body += arrow([bottom(n.e), topOf(n.f)]);
  body += arrow([[x2 - 95, 220], [x2 - 150, 220], [x2 - 150, 120], [x2 - 125, 120]], "ubah", x2 - 150, 175);
  body += arrow([[x2 + 95, 220], [x2 + 170, 220], [x2 + 170, 30], [x3 - 190, 30], [x3 - 190, 100], [x3 - 125, 100]], "ya", x2 + 170, 260);
  // Stage 3.
  body += arrow([bottom(n.g), topOf(n.h)]);
  body += arrow([[x3 - 90, 205], [x3 - 150, 205], [x3 - 150, 115], [x3 - 125, 115]], "revisi", x3 - 150, 165);
  body += arrow([bottom(n.h), topOf(n.i)], "ya", x3 + 14, 255);
  body += arrow([bottom(n.i), topOf(n.k)]) + arrow([bottom(n.k), topOf(n.l)]) + arrow([bottom(n.l), topOf(n.m)]);
  body += arrow([[x3 + 95, 590], [x3 + 150, 590], [x3 + 150, 485], [x3 + 125, 485]], "ubah", x3 + 150, 540);
  body += arrow([[x3 - 95, 590], [x3 - 158, 590], [x3 - 158, 395], [x3 - 125, 395]], "berikutnya", x3 - 158, 500);
  body += arrow([bottom(n.m), topOf(n.o)], "selesai", x3 + 30, 645);
  return svg(W, H, body);
}

const here = __dirname;
fs.writeFileSync(path.join(here, "02-kasus-penggunaan.svg"), useCaseDiagram());
fs.writeFileSync(path.join(here, "04-alur-studio.svg"), studioFlow());
console.log("written");
