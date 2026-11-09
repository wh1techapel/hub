// CP1 prototype: Hari Ini with sample data, driven by a simulated clock.
// Not app code: it shows the design (light phases, Day Arc, ink check, Tutup Hari, drop pools,
// ULTRON's thread) so it can be judged before CP2 builds the real thing.
"use strict";

// ---------- sample data (Thursday) ----------

const DAY = { wake: 5 * 60, sleep: 22 * 60 + 30 };

const EVENTS = [
  { start: 420, end: 510, title: "Matematika", sub: "Ruang 12", pigment: "lazuardi", routine: true },
  { start: 510, end: 600, title: "Bahasa Inggris", sub: "Ruang 12", pigment: "lazuardi", routine: true },
  { start: 615, end: 750, title: "Praktikum Jaringan", sub: "Lab TJKT 2, VLAN dan trunking", pigment: "toska", routine: true },
  { start: 780, end: 870, title: "Produktif TJKT", sub: "MikroTik dasar", pigment: "toska", routine: true },
  { start: 960, end: 1020, title: "Kerjakan Laporan Lab", sub: "Blok waktu untuk tugas", pigment: "kunyit" },
  { start: 1140, end: 1230, title: "Belajar Cyber", sub: "TryHackMe, jalur Jr Penetration Tester", pigment: "lavender", routine: true },
];
const REMINDERS = [{ at: 1260, title: "Kumpulkan laporan di Classroom" }];

const FOCUS = [
  { id: "f1", title: "Laporan Praktikum 5: VLAN dan Trunking", project: ["Laporan Lab Jaringan", "toska"], meta: "Tenggat Jumat 23.59", soon: true },
  { id: "f2", title: "THM: Nmap Live Host Discovery", project: ["Belajar", "lavender"], meta: "Sisa 45 m" },
  { id: "f3", title: "Hub: kunci arah desain CP1", project: ["Hub", "kunyit"], meta: "Do date hari ini", done: true },
];
const OTHER = [
  { id: "o1", title: "Beli kabel UTP cat6 2 m", project: ["Pribadi", "batu"], side: "Pribadi" },
  { id: "o2", title: "Revisi slide presentasi KIK", project: ["Sekolah", "lazuardi"], side: "Sekolah" },
];
const FILES = [
  { name: "Laporan-Praktikum-5.docx", sub: "Dokumen, diubah 2 j lalu", size: "184 KB", icon: "i-doc", pigment: "lazuardi" },
  { name: "topologi-vlan.pkt", sub: "Packet Tracer, kemarin", size: "96 KB", icon: "i-net", pigment: "toska" },
  { name: "wireshark-trunk-802.1q.png", sub: "Gambar, kemarin", size: "1.2 MB", icon: "i-image", pigment: "lavender" },
];
const TRAIL = [
  { at: "11.20", text: "Kamu menyelesaikan “Hub: kunci arah desain CP1”." },
  { at: "09.05", text: "ULTRON mencatat file “topologi-vlan.pkt” ke Laporan Lab Jaringan.", ultron: true },
  { at: "Rabu", text: "Sesi belajar 1 j 10 m di “Nmap Live Host Discovery”." },
];
const WEEK = ["on", "on", "rest", "on", "today", "", ""]; // Sen..Min, Kamis is today
const WEEK_NAMES = ["Sen", "Sel", "Rab", "Kam", "Jum", "Sab", "Min"];
const TOMORROW_SUGGESTIONS = [
  "Laporan Praktikum 5: VLAN dan Trunking",
  "THM: Nmap Live Host Discovery",
  "Baca materi subnetting /27",
  "Revisi slide presentasi KIK",
];
const INKS = [
  ["biru", "Biru Tinta", "oklch(0.5 0.16 262)"],
  ["hijau", "Hijau Botol", "oklch(0.49 0.1 165)"],
  ["sepia", "Sepia", "oklch(0.5 0.11 58)"],
  ["ungu", "Ungu Senja", "oklch(0.5 0.15 300)"],
  ["grafit", "Grafit", "oklch(0.32 0.02 265)"],
];
const PIGMENTS = ["lazuardi", "toska", "lumut", "kunyit", "terakota", "mawar", "lavender", "batu"];
const PHASES = [
  ["pagi", "Pagi", "04.30 sampai 10.00"],
  ["siang", "Siang", "10.00 sampai 15.00"],
  ["senja", "Senja", "15.00 sampai 18.30"],
  ["malam", "Malam", "18.30 sampai 04.30"],
];

// ---------- state ----------

const state = {
  minutes: nowMinutes(),
  live: true,
  mode: "auto",
  ink: "biru",
  dayClosed: false,
  focusDone: new Set(FOCUS.filter((t) => t.done).map((t) => t.id)),
};

const $ = (id) => document.getElementById(id);
const root = document.documentElement;
const reduceMotion = matchMedia("(prefers-reduced-motion: reduce)");

// ---------- time helpers ----------

function nowMinutes() {
  const d = new Date();
  return d.getHours() * 60 + d.getMinutes();
}
const pad = (n) => String(n).padStart(2, "0");
const clock = (m) => `${pad(Math.floor(m / 60) % 24)}.${pad(m % 60)}`;
function span(m) {
  const h = Math.floor(m / 60);
  const r = m % 60;
  if (!h) return `${r} m`;
  return r ? `${h} j ${r} m` : `${h} j`;
}
function phaseAt(m) {
  if (m >= 270 && m < 600) return "pagi";
  if (m >= 600 && m < 900) return "siang";
  if (m >= 900 && m < 1110) return "senja";
  return "malam";
}
function greetingAt(m) {
  if (m >= 270 && m < 660) return "Selamat pagi";
  if (m >= 660 && m < 900) return "Selamat siang";
  if (m >= 900 && m < 1110) return "Selamat sore";
  return "Selamat malam";
}
const currentEvent = (m) => EVENTS.find((e) => m >= e.start && m < e.end);
const nextEvent = (m) => EVENTS.find((e) => e.start > m);
const isEvening = (m) => m >= 17 * 60 + 30 || m < 270;

function escapeText(s) {
  return String(s).replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]);
}

// ---------- light ----------

let shiftTimer;
function applyLight() {
  const phase = phaseAt(state.minutes);
  const shown = state.mode === "light" ? (phase === "malam" ? "siang" : phase)
    : state.mode === "dark" ? "malam" : phase;
  if (root.dataset.phase !== shown) {
    // While the light cross-fades, elements must not chase it with their own colour
    // transitions (one new transition per frame each): see "shifting" in prototype.css.
    root.dataset.shifting = "1";
    clearTimeout(shiftTimer);
    shiftTimer = setTimeout(() => delete root.dataset.shifting, 2500);
  }
  root.dataset.phase = shown;
  root.dataset.scheme = shown === "malam" ? "dark" : "light";
  root.dataset.ink = state.ink;
}

// ---------- Day Arc ----------

// A shallow circular arc from (40,160) to (960,160) peaking at y=30.
const ARC = (() => {
  const half = 460;
  const sag = 130;
  const r = (half * half + sag * sag) / (2 * sag);
  const cx = 500;
  const cy = 30 + r;
  return { r, cx, cy, a0: Math.atan2(160 - cy, 40 - cx), a1: Math.atan2(160 - cy, 960 - cx) };
})();
const frac = (m) => Math.min(1, Math.max(0, (m - DAY.wake) / (DAY.sleep - DAY.wake)));
function arcPoint(t, offset = 0) {
  const a = ARC.a0 + (ARC.a1 - ARC.a0) * t;
  const r = ARC.r + offset;
  return [ARC.cx + r * Math.cos(a), ARC.cy + r * Math.sin(a)];
}
function arcPath(t0, t1) {
  const [x0, y0] = arcPoint(t0);
  const [x1, y1] = arcPoint(t1);
  return `M${x0.toFixed(1)} ${y0.toFixed(1)} A${ARC.r.toFixed(1)} ${ARC.r.toFixed(1)} 0 0 1 ${x1.toFixed(1)} ${y1.toFixed(1)}`;
}

function renderArc() {
  const m = state.minutes;
  const t = frac(m);
  const night = m >= DAY.sleep || m < DAY.wake;
  const cur = currentEvent(m);
  const nxt = nextEvent(m);
  const parts = [];

  parts.push(`<path d="${arcPath(0, 1)}" stroke="var(--arc-future)" stroke-width="2" fill="none" stroke-linecap="round"/>`);
  if (t > 0) parts.push(`<path d="${arcPath(0, t)}" stroke="var(--arc-past)" stroke-width="2.5" fill="none" stroke-linecap="round"/>`);

  for (const e of EVENTS) {
    const t0 = frac(e.start) + 0.003;
    const t1 = frac(e.end) - 0.003;
    const past = e.end <= m;
    parts.push(`<g data-pigment="${e.pigment}"><path d="${arcPath(t0, t1)}" style="stroke: var(--pig-line)" stroke-width="9" fill="none" stroke-linecap="round" opacity="${past ? 0.45 : 1}"/></g>`);
  }
  for (const r of REMINDERS) {
    const [x, y] = arcPoint(frac(r.at), 0);
    parts.push(`<circle cx="${x.toFixed(1)}" cy="${y.toFixed(1)}" r="4" style="fill: var(--paper); stroke: var(--warn)" stroke-width="2"/>`);
  }

  // Labels: the current and the next event, outside the arc. Adjacent events would collide, so
  // the second label steps further out.
  // The sun sits on the current event, so its label also steps out.
  const sunX = night ? -Infinity : arcPoint(t)[0];
  let lastX = -Infinity;
  for (const e of [cur, nxt].filter(Boolean)) {
    const mid = (frac(e.start) + frac(e.end)) / 2;
    const midX = arcPoint(mid)[0];
    const near = Math.abs(midX - lastX) < 140 || Math.abs(midX - sunX) < 80;
    const [x, y] = arcPoint(mid, near ? 40 : 20);
    lastX = x;
    parts.push(`<text x="${x.toFixed(1)}" y="${y.toFixed(1)}" text-anchor="middle" class="${e === cur ? "label-strong" : ""}">${escapeText(e.title)}</text>`);
  }

  // Hours under the horizon, and noon inside the arc unless the sun's own time is there.
  const [lx, ly] = arcPoint(0);
  const [rx, ry] = arcPoint(1);
  parts.push(`<text x="${lx}" y="${ly + 26}" text-anchor="middle">${clock(DAY.wake)}</text>`);
  parts.push(`<text x="${rx}" y="${ry + 26}" text-anchor="middle">${clock(DAY.sleep)}</text>`);
  if (night || state.dayClosed || Math.abs(m - 720) > 75) {
    const [nx, ny] = arcPoint(frac(12 * 60), -22);
    parts.push(`<text x="${nx.toFixed(1)}" y="${ny.toFixed(1)}" text-anchor="middle">12.00</text>`);
  }

  if (state.dayClosed) {
    parts.push(`<path class="arc-close" id="arc-close" d="${arcPath(0, 1)}" pathLength="1" stroke="var(--accent)" stroke-width="2.5" fill="none" stroke-linecap="round" stroke-dasharray="1 2" style="stroke-dashoffset: ${root.dataset.closing ? 1 : 0}"/>`);
  }

  if (night || state.dayClosed) {
    const [mx, my] = m < DAY.wake ? arcPoint(0) : arcPoint(1);
    const rising = root.dataset.closing ? `style="transform: translateY(36px); opacity: 0"` : "";
    parts.push(`<g class="moon" id="moon" ${rising}>
      <circle cx="${mx}" cy="${my - 26}" r="13" fill="var(--accent-soft)"/>
      <path d="M${mx + 6} ${my - 21}a8 8 0 1 1 -5 -12a6.5 6.5 0 0 0 5 12z" fill="var(--accent)"/>
    </g>`);
  } else {
    const [sx, sy] = arcPoint(t);
    parts.push(`<circle cx="${sx.toFixed(1)}" cy="${sy.toFixed(1)}" r="16" fill="var(--accent-soft)"/>`);
    parts.push(`<circle cx="${sx.toFixed(1)}" cy="${sy.toFixed(1)}" r="7.5" fill="var(--accent)" stroke="var(--paper)" stroke-width="2.5"/>`);
    parts.push(`<text x="${sx.toFixed(1)}" y="${(sy + 34).toFixed(1)}" text-anchor="middle" class="label-strong">${clock(m)}</text>`);
  }

  $("day-arc").innerHTML = parts.join("");

  let caption;
  if (state.dayClosed) caption = `Hari ditutup<span class="sep">·</span>Besok mulai ${clock(EVENTS[0].start)} dengan ${EVENTS[0].title}`;
  else if (night) caption = `Sudah lewat waktu tidur<span class="sep">·</span>${m < DAY.wake ? "Hari ini" : "Besok"} mulai ${clock(EVENTS[0].start)} dengan ${EVENTS[0].title}`;
  else {
    const left = `Sisa hari ${span(DAY.sleep - m)}`;
    caption = nxt ? `${left}<span class="sep">·</span>Berikutnya ${nxt.title} ${clock(nxt.start)}, dalam ${span(nxt.start - m)}` : `${left}<span class="sep">·</span>Tidak ada jadwal lagi`;
  }
  $("arc-caption").innerHTML = caption;
}

// ---------- hero, now, invitation ----------

function focusDoneCount() {
  return FOCUS.filter((t) => state.focusDone.has(t.id)).length;
}

function renderHero() {
  const m = state.minutes;
  $("greeting").innerHTML = state.dayClosed
    ? `Hari ditutup. <em>Istirahat, Glenvio.</em>`
    : `${greetingAt(m)}, <em>Glenvio.</em>`;
  const cur = currentEvent(m);
  const nxt = nextEvent(m);
  const done = focusDoneCount();
  let sub;
  if (state.dayClosed) sub = "Fokus besok sudah siap. Sampai besok.";
  else if (cur) sub = `Sekarang <strong>${cur.title}</strong> sampai ${clock(cur.end)}. Fokus ${done} dari ${FOCUS.length} selesai.`;
  else if (nxt) sub = `Berikutnya <strong>${nxt.title}</strong> pukul ${clock(nxt.start)}. Fokus ${done} dari ${FOCUS.length} selesai.`;
  else sub = `Tidak ada jadwal lagi hari ini. Fokus ${done} dari ${FOCUS.length} selesai.`;
  $("hero-sub").innerHTML = sub;
}

function renderNow() {
  const m = state.minutes;
  const cur = currentEvent(m);
  const nxt = nextEvent(m);
  const e = cur || nxt;
  let html = "";
  if (e) {
    const progress = cur ? (m - cur.start) / (cur.end - cur.start) : 0;
    const right = cur
      ? `<strong>${span(cur.end - m)}</strong><span>tersisa</span>`
      : `<strong>${span(e.start - m)}</strong><span>lagi</span>`;
    html += `<div class="now-event" data-pigment="${e.pigment}">
      <span class="rail"></span>
      <div><h3>${e.title}</h3><p>${clock(e.start)} sampai ${clock(e.end)}, ${e.sub}</p></div>
      <div class="now-left">${right}</div>
    </div>`;
    if (cur) html += `<div class="bar" aria-label="Berjalan"><i style="--p:${progress.toFixed(3)}"></i></div>`;
  } else {
    html += `<div class="now-event" data-pigment="batu"><span class="rail"></span><div><h3>Tidak ada jadwal lagi</h3><p>Waktu bebas sampai ${clock(DAY.sleep)}.</p></div><div class="now-left"></div></div>`;
  }
  html += `<div class="now-sep"></div>
    <div class="now-next"><svg><use href="#i-shield"/></svg><span>Lanjutkan <strong>Nmap Live Host Discovery</strong>, berhenti di task 4.</span></div>
    <div class="now-next"><svg><use href="#i-bell"/></svg><span>${REMINDERS[0].at > m ? "Pengingat" : "Sudah diingatkan"} ${clock(REMINDERS[0].at)}: ${REMINDERS[0].title}</span></div>`;
  $("now-card").innerHTML = html;
}

function renderInvite() {
  const show = isEvening(state.minutes) && !state.dayClosed;
  $("invite").hidden = !show;
  const doneEvents = EVENTS.filter((e) => e.end <= state.minutes).length;
  const total = FOCUS.length + doneEvents;
  $("invite-title").textContent = `${focusDoneCount() + doneEvents} dari ${total} hal selesai hari ini.`;
}

// ---------- tasks and the ink check ----------

function taskRow(task, quiet) {
  const done = state.focusDone.has(task.id);
  const li = document.createElement("li");
  li.className = `task${done ? " is-done" : ""}`;
  li.dataset.id = task.id;
  li.innerHTML = `
    <button class="check" type="button" aria-pressed="${done}" aria-label="Tandai selesai: ${escapeText(task.title)}">
      <svg viewBox="0 0 16 16"><path d="M3.5 8.5 6.5 11.5 12.5 5" pathLength="1"/></svg>
    </button>
    <div class="task-body">
      <span class="task-title">${escapeText(task.title)}<svg class="strike" viewBox="0 0 100 8" preserveAspectRatio="none" aria-hidden="true"><path d="M0 5 C 18 3, 36 6, 55 4 S 88 5, 100 3.5" pathLength="1"/></svg></span>
      <div class="task-meta">
        <span class="chip" data-pigment="${task.project[1]}">${escapeText(task.project[0])}</span>
        ${task.meta ? `<span class="${task.soon ? "due-soon" : ""}">${escapeText(task.meta)}</span>` : ""}
        ${task.ultron ? `<span class="ultron-mark"><svg><use href="#i-ultron"/></svg>oleh ULTRON</span>` : ""}
      </div>
    </div>`;
  li.querySelector(".check").addEventListener("click", () => toggleTask(task, li, quiet));
  return li;
}

function renderTasks() {
  const focus = $("focus-list");
  focus.replaceChildren(...FOCUS.map((t) => taskRow(t, false)));
  const other = $("other-list");
  other.replaceChildren(...OTHER.map((t) => taskRow(t, true)));
  updateFocusCount();
}

function updateFocusCount() {
  $("focus-count").textContent = `${focusDoneCount()} dari ${FOCUS.length} selesai`;
}

function toggleTask(task, li, quiet) {
  const done = !state.focusDone.has(task.id);
  if (done) state.focusDone.add(task.id);
  else state.focusDone.delete(task.id);
  li.classList.toggle("is-done", done);
  li.querySelector(".check").setAttribute("aria-pressed", String(done));
  updateFocusCount();
  renderHero();
  renderInvite();
  if (!done) return;

  if (quiet) {
    // Quiet tasks fold away after the check has been seen; the rows below glide up (FLIP).
    setTimeout(() => {
      if (!state.focusDone.has(task.id)) return;
      li.classList.add("is-leaving");
      setTimeout(() => removeWithFlip(li), reduceMotion.matches ? 0 : 240);
    }, reduceMotion.matches ? 300 : 1100);
  }
  toast(`Selesai: ${task.title}`, () => {
    state.focusDone.delete(task.id);
    if (quiet) {
      const list = $("other-list");
      const fresh = taskRow(task, true);
      fresh.classList.add("is-new");
      const index = OTHER.indexOf(task);
      list.insertBefore(fresh, list.children[index] || null);
      li.remove();
    } else {
      li.classList.remove("is-done");
      li.querySelector(".check").setAttribute("aria-pressed", "false");
    }
    updateFocusCount();
    renderHero();
    renderInvite();
  });
}

function removeWithFlip(el) {
  if (!el.isConnected) return;
  const siblings = [...el.parentElement.children].filter((c) => c !== el);
  const before = new Map(siblings.map((s) => [s, s.getBoundingClientRect().top]));
  el.remove();
  for (const s of siblings) {
    const dy = before.get(s) - s.getBoundingClientRect().top;
    if (!dy) continue;
    s.animate([{ transform: `translateY(${dy}px)` }, { transform: "none" }], {
      duration: reduceMotion.matches ? 1 : 240,
      easing: "cubic-bezier(0.2, 0, 0, 1)",
    });
  }
}

// ---------- side column ----------

function renderSchedule() {
  const m = state.minutes;
  const items = [...EVENTS.map((e) => ({ ...e, kind: "event" })), ...REMINDERS.map((r) => ({ start: r.at, title: r.title, kind: "reminder" }))]
    .sort((a, b) => a.start - b.start);
  $("schedule").innerHTML = items.map((e) => {
    if (e.kind === "reminder") {
      return `<li class="slot is-reminder"><time>${clock(e.start)}</time><div class="block"><svg><use href="#i-bell"/></svg>${escapeText(e.title)}</div></li>`;
    }
    const cls = m >= e.end ? "is-past" : m >= e.start ? "is-now" : "";
    return `<li class="slot ${cls}" data-pigment="${e.pigment}"><time>${clock(e.start)}</time>
      <div class="block ${e.routine ? "is-routine" : ""}"><strong>${escapeText(e.title)}</strong><span>${clock(e.start)} sampai ${clock(e.end)}</span></div></li>`;
  }).join("");
}

function renderWeek() {
  $("week-dots").innerHTML = WEEK.map((s, i) => `<i class="${s}" title="${WEEK_NAMES[i]}${s === "rest" ? ", hari istirahat" : ""}"></i>`).join("");
}

function fileRow(f, isNew) {
  return `<li class="file${isNew ? " is-new" : ""}" data-pigment="${f.pigment}">
    <span class="file-glyph"><svg><use href="#${f.icon}"/></svg></span>
    <div style="min-width:0"><div class="file-name">${escapeText(f.name)}</div><div class="file-sub">${escapeText(f.sub)}</div></div>
    <span class="file-size">${escapeText(f.size)}</span></li>`;
}

function renderFiles() {
  $("files").innerHTML = FILES.slice(0, 4).map((f, i) => fileRow(f, f.isNew && i === 0)).join("");
  for (const f of FILES) f.isNew = false;
}

function renderTrail() {
  $("trail").innerHTML = TRAIL.slice(0, 4).map((t, i) =>
    `<li class="${t.ultron ? "by-ultron" : ""}${t.isNew && i === 0 ? " is-new" : ""}"><div>${escapeText(t.text)}<time>${escapeText(t.at)}</time></div></li>`
  ).join("");
  for (const t of TRAIL) t.isNew = false;
}

// ---------- toast ----------

let toastTimer;
function toast(text, undo) {
  const el = $("toast");
  el.replaceChildren();
  const span = document.createElement("span");
  span.textContent = text;
  el.append(span);
  if (undo) {
    const b = document.createElement("button");
    b.type = "button";
    b.textContent = "Urungkan";
    b.addEventListener("click", () => { undo(); el.hidden = true; });
    el.append(b);
  }
  el.hidden = false;
  el.style.animation = "none";
  void el.offsetWidth;
  el.style.animation = "";
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => { el.hidden = true; }, 5000);
}

// ---------- Tutup Hari ----------

let lastFocus;
function openSheet() {
  lastFocus = document.activeElement;
  const doneTasks = FOCUS.filter((t) => state.focusDone.has(t.id)).map((t) => t.title);
  const doneEvents = EVENTS.filter((e) => e.end <= state.minutes).map((e) => e.title);
  const doneRows = [...doneTasks.map((t) => [t, "i-check", ""]), ...doneEvents.map((t) => [t, "i-calendar", "is-event"])];
  $("done-list").innerHTML = doneRows.map(([t, icon, cls], i) =>
    `<li class="${cls}" style="animation-delay:${Math.min(i, 8) * 30}ms"><svg><use href="#${icon}"/></svg>${escapeText(t)}</li>`).join("") || "<li>Belum ada yang selesai, dan itu tidak apa-apa.</li>";

  const slipped = FOCUS.filter((t) => !state.focusDone.has(t.id));
  $("slipped").innerHTML = slipped.map((t) => `<div class="slip"><span>${escapeText(t.title)}</span>
    <span class="choices" role="group" aria-label="Pindahkan ${escapeText(t.title)}">
      <button type="button" aria-pressed="true">Besok</button><button type="button" aria-pressed="false">Pilih tanggal</button><button type="button" aria-pressed="false">Lepaskan</button>
    </span></div>`).join("") || "<p>Tidak ada yang tertunda.</p>";
  for (const group of $("slipped").querySelectorAll(".choices")) {
    group.addEventListener("click", (ev) => {
      if (ev.target.tagName !== "BUTTON") return;
      for (const b of group.children) b.setAttribute("aria-pressed", String(b === ev.target));
    });
  }

  const preselected = new Set(slipped.map((t) => t.title));
  $("tomorrow").innerHTML = TOMORROW_SUGGESTIONS.map((t) =>
    `<button type="button" class="pick" aria-pressed="${preselected.has(t)}">${escapeText(t)}</button>`).join("");
  updateTomorrowCount();

  $("scrim").hidden = false;
  $("sheet").hidden = false;
  $("sheet-close").focus();
}

function updateTomorrowCount() {
  const n = $("tomorrow").querySelectorAll('[aria-pressed="true"]').length;
  $("tomorrow-count").textContent = `${n} dari 3`;
}

function closeSheet() {
  $("scrim").hidden = true;
  $("sheet").hidden = true;
  if (lastFocus) lastFocus.focus();
}

function closeDay() {
  closeSheet();
  state.dayClosed = true;
  root.dataset.closing = "1";
  renderAll();
  // Flush the start state (no ink, moon below the horizon), then let the ink run along the arc
  // and the moon rise. A forced style read, not requestAnimationFrame, which a hidden page skips.
  delete root.dataset.closing;
  const ink = $("arc-close");
  const moon = $("moon");
  ink?.getBoundingClientRect();
  if (ink) ink.style.strokeDashoffset = "0";
  if (moon) { moon.style.transform = "none"; moon.style.opacity = "1"; }
  TRAIL.unshift({ at: clock(state.minutes), text: "Kamu menutup hari. Fokus besok sudah dipilih.", isNew: true });
  renderTrail();
}

// ---------- ULTRON acting ----------

function traceThread(el) {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("class", "thread");
  svg.setAttribute("aria-hidden", "true");
  svg.innerHTML = `<rect x="1" y="1" width="calc(100% - 2px)" height="calc(100% - 2px)" rx="8" pathLength="1"/>`;
  el.style.position = "relative";
  el.append(svg);
  // SVG rect does not take calc() in every engine: size it from the box.
  const box = el.getBoundingClientRect();
  const rect = svg.querySelector("rect");
  rect.setAttribute("width", String(box.width + 14));
  rect.setAttribute("height", String(box.height + 6));
  setTimeout(() => svg.remove(), 1400);
}

let ultronCount = 0;
function ultronActs() {
  ultronCount += 1;
  const task = { id: `u${ultronCount}`, title: "Bawa kabel UTP ke lab besok", project: ["Sekolah", "lazuardi"], meta: "Besok 06.30", ultron: true };
  OTHER.unshift(task);
  const li = taskRow(task, true);
  li.classList.add("is-new");
  $("other-list").prepend(li);
  traceThread(li);
  TRAIL.unshift({ at: clock(state.minutes), text: "ULTRON menambahkan tugas “Bawa kabel UTP ke lab besok” dari suaramu.", ultron: true, isNew: true });
  renderTrail();
  traceThread($("trail").firstElementChild);
}

// ---------- drop pools ----------

const drop = $("drop");
let dragDepth = 0;

function hasFiles(ev) {
  return [...(ev.dataTransfer?.types || [])].includes("Files");
}
function poolAt(ev) {
  return ev.target instanceof Element ? ev.target.closest(".pool") : null;
}
function setOver(pool) {
  for (const p of drop.querySelectorAll(".pool")) p.classList.toggle("is-over", p === pool);
}

window.addEventListener("dragenter", (ev) => {
  if (!hasFiles(ev)) return;
  ev.preventDefault();
  dragDepth += 1;
  drop.hidden = false;
});
window.addEventListener("dragover", (ev) => {
  if (!hasFiles(ev)) return;
  ev.preventDefault();
  setOver(poolAt(ev));
});
window.addEventListener("dragleave", () => {
  dragDepth = Math.max(0, dragDepth - 1);
  if (!dragDepth) { drop.hidden = true; setOver(null); }
});
window.addEventListener("drop", (ev) => {
  if (!hasFiles(ev)) return;
  ev.preventDefault();
  const target = poolAt(ev)?.dataset.target || "library";
  const files = [...ev.dataTransfer.files].map((f) => ({ name: f.name, size: f.size }));
  dragDepth = 0;
  finishDrop(files, target);
});

function humanSize(bytes) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

function finishDrop(files, target) {
  drop.hidden = true;
  setOver(null);
  if (!files.length) return;
  const where = { library: "Library", project: "Laporan Lab Jaringan", task: "Laporan Praktikum 5" }[target];
  for (const f of files.reverse()) {
    FILES.unshift({ name: f.name, sub: `Baru, ke ${where}`, size: humanSize(f.size), icon: /\.(png|jpe?g|gif|webp)$/i.test(f.name) ? "i-image" : "i-doc", pigment: "toska", isNew: true });
  }
  renderFiles();
  TRAIL.unshift({ at: clock(state.minutes), text: `${files.length} file masuk ke ${where}.`, isNew: true });
  renderTrail();
  toast(`${files.length} file masuk ke ${where}. Disimpan sebagai referensi.`);
}

function demoDrop() {
  drop.hidden = false;
  const pools = drop.querySelectorAll(".pool");
  setTimeout(() => setOver(pools[1]), 500);
  setTimeout(() => finishDrop([{ name: "topologi-vlan-final.pkt", size: 101_376 }], "project"), 1500);
}

// ---------- foundations ----------

function renderFoundations() {
  $("f-phases").innerHTML = PHASES.map(([id, name, hours]) => `
    <div class="f-phase" data-phase="${id}" data-scheme="${id === "malam" ? "dark" : "light"}">
      <h3>${name}</h3><p>${hours}</p>
      <div class="swatches"><i style="background:var(--paper)" title="paper"></i><i style="background:var(--paper-2)" title="paper-2"></i><i style="background:var(--surface)" title="surface"></i><i style="background:var(--ink)" title="ink"></i><i style="background:var(--accent)" title="accent"></i></div>
      <div class="sample"><b>Laporan Lab</b> dikumpulkan Jumat. <u>Tutup Hari</u></div>
    </div>`).join("");
  $("f-inks").innerHTML = INKS.map(([id, name]) => `<div class="f-ink" data-ink="${id}" data-scheme="${root.dataset.scheme}"><i></i>${name}</div>`).join("");
  $("f-pigments").innerHTML = PIGMENTS.map((p) => `<div class="f-pig" data-pigment="${p}">${p[0].toUpperCase() + p.slice(1)}<span>Rutinitas, proyek, jadwal</span></div>`).join("");
}

// ---------- dock ----------

function renderDock() {
  $("time").value = String(state.minutes);
  $("time-out").textContent = clock(state.minutes);
  $("live-btn").setAttribute("aria-pressed", String(state.live));
  $("ink-picker").innerHTML = INKS.map(([id, name, swatch]) =>
    `<button type="button" role="radio" aria-checked="${id === state.ink}" aria-label="${name}" title="${name}" data-ink-pick="${id}" style="--swatch:${swatch}"></button>`).join("");
}

$("time").addEventListener("input", (ev) => {
  state.live = false;
  state.minutes = Number(ev.target.value);
  renderAll();
});
$("live-btn").addEventListener("click", () => {
  state.live = true;
  state.minutes = nowMinutes();
  renderAll();
});
$("mode").addEventListener("change", (ev) => {
  state.mode = ev.target.value;
  renderAll();
});
$("ink-picker").addEventListener("click", (ev) => {
  const id = ev.target.dataset?.inkPick;
  if (!id) return;
  state.ink = id;
  renderAll();
});
$("demo-drop").addEventListener("click", demoDrop);
$("demo-ultron").addEventListener("click", ultronActs);
$("demo-foundations").addEventListener("click", (ev) => {
  const open = $("foundations").hidden;
  $("foundations").hidden = !open;
  ev.currentTarget.setAttribute("aria-pressed", String(open));
  if (open) renderFoundations();
});

$("invite-btn").addEventListener("click", openSheet);
$("sheet-cancel").addEventListener("click", closeSheet);
$("scrim").addEventListener("click", closeSheet);
$("sheet-close").addEventListener("click", closeDay);
$("tomorrow").addEventListener("click", (ev) => {
  const b = ev.target.closest(".pick");
  if (!b) return;
  const on = b.getAttribute("aria-pressed") === "true";
  const count = $("tomorrow").querySelectorAll('[aria-pressed="true"]').length;
  if (!on && count >= 3) { toast("Fokus paling banyak tiga. Lepaskan satu dulu."); return; }
  b.setAttribute("aria-pressed", String(!on));
  updateTomorrowCount();
});
document.addEventListener("keydown", (ev) => {
  if (ev.key === "Escape" && !$("sheet").hidden) closeSheet();
});

// ---------- clock ----------

let ticker;
function scheduleTick() {
  clearTimeout(ticker);
  if (document.hidden) return;
  const msToNextMinute = 60_000 - (Date.now() % 60_000);
  ticker = setTimeout(() => {
    if (state.live) {
      state.minutes = nowMinutes();
      renderAll();
    }
    scheduleTick();
  }, msToNextMinute + 50);
}
document.addEventListener("visibilitychange", () => {
  if (!document.hidden && state.live) { state.minutes = nowMinutes(); renderAll(); }
  scheduleTick();
});

function renderAll() {
  applyLight();
  renderHero();
  renderArc();
  renderNow();
  renderInvite();
  renderSchedule();
  renderDock();
  if (!$("foundations").hidden) renderFoundations();
  $("date-line").textContent = "Kamis, 22 Oktober";
}

renderTasks();
renderWeek();
renderFiles();
renderTrail();
renderAll();
scheduleTick();
