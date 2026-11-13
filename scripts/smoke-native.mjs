// Native smoke: the real release window, driven over WebView2's DevTools port.
//
//   npm run tauri build
//   node scripts/smoke-native.mjs
//
// Starts target/release/hub.exe on a fresh demo data folder (HUB_DATA_DIR, seeded by hub-seed),
// checks Hari Ini against the core, checks a focus task, saves a line with Catat Cepat, finds it in
// Tugas, checks the week in Kalender, lets a reminder go off and snoozes it, checks the Library
// (Windows thumbnails, a Vault copy, a file moved and one deleted), times a learning session, reads
// Jejak, searches, closes the day, has ULTRON add a task through the local API (it shows at once,
// with ULTRON's mark), changes the ink, restarts Hub, and checks that what was saved survived.
// Studio end to end against a fake Gemini and the real bridge (an MCP client over stdio): an idea,
// the conversation, the PRD and the roadmap approved, CP-01 started and reported by "Claude", the
// owner's approval in Pantau; Claude Code's status in Pantau and Pengaturan from what the bridge
// reported (its session, its calls, a refusal, its goodbye); the bridge connecting by
// CLAUDE_PROJECT_DIR, answering without Gemini, and starting a stopped Hub; Lentera's reply as its
// question alone, the model menu, the glow, and the PRD's steps (L1 to L3); memory with Studio
// open and hidden. Screenshots go to the
// data folder. The debug port exists only for this run: the owner's Hub never starts with it.

import { execFileSync, spawn } from "node:child_process";
import { createServer } from "node:http";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, renameSync, unlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { setTimeout as sleep } from "node:timers/promises";

const root = resolve(import.meta.dirname, "..");
const exe = join(root, "target", "release", "hub.exe");
// Studio's bridge for Claude Code, driven here the way Claude Code drives it: MCP over stdio.
const bridgeExe = join(root, "target", "release", "hub-mcp.exe");
const seedExe = [join(root, "target", "release", "hub-seed.exe"), join(root, "target", "debug", "hub-seed.exe")].find(existsSync);
const port = 9333;
// The local API on a test port, never the owner's (47731).
const apiPort = 47781;
// A fake Gemini on loopback (HUB_GEMINI_BASE_URL is honoured only with HUB_DATA_DIR, only there),
// answering the way Google's documentation shows. The key is a test key, kept by Hub under a
// test name in Credential Manager and removed at the end.
const geminiPort = 47791;
const geminiKey = "AIzaSmokeTestKeyForHubStudio0123456789";
let geminiServer = null;
const geminiChats = [];
const geminiPrdParts = [];
const geminiStacks = [];
// A stack (R3): the seven layers, as Gemini sends them.
const SMOKE_STACK = {
  layers: ["platform", "frontend", "backend", "database", "auth", "hosting", "extras"].map((layer) => ({
    layer,
    choice: layer === "extras" ? "Tidak perlu" : `Pilihan ${layer}`,
    why: "Sederhana.",
    fit: "Cocok untuk absen.",
    ease: "Mudah.",
    cost: "Gratis.",
    alternatives: [{ name: `Lain ${layer}`, when: "Bila berubah." }],
  })),
  summary: "Stack web sederhana.",
};
const geminiRoadmaps = [];
const SMOKE_ROADMAP = {
  checkpoints: [
    { code: "CP-01", title: "Absen dengan kartu", goal: "Kartu terdaftar tercatat.", description: "", features: ["FEAT-001"],
      tasks: [{ title: "Baca kartu", detail: "", feature: "FEAT-001", subtasks: ["Uji pembaca"] }],
      dependsOn: [], difficulty: "medium", risks: [], acceptance: ["Kartu tercatat dalam 2 detik"],
      requiredTests: [{ name: "Unit test kartu", kind: "unit", how: "npm test" }], deliverables: [], doneWhen: [] },
  ],
};
const SMOKE_PRD = {
  identity: { name: "Absensi Lab", summary: "Absensi praktikum dengan kartu.", owner: "Glenvio" },
  background: { problem: "Absen manual lama.", context: "Lab TJKT", targetUsers: ["Siswa TJKT"], unmetNeed: "Absen cepat", evidence: [{ statement: "Pengamatan pemilik", source: "owner" }], assumptions: [{ id: "ASM-01", statement: "Siswa punya kartu", validate: "Tanya kesiswaan" }] },
  goals: { primary: "Absen dalam 2 menit.", benefits: ["Tepat waktu"], differentiator: "Kartu yang sudah ada", metrics: [{ metric: "Waktu absen", target: "di bawah 2 menit", how: "stopwatch" }], outOfScope: ["Penilaian"] },
  experience: { personas: [], journeys: [], mainFlows: [{ name: "Absen", steps: ["Tempel kartu"] }], alternateFlows: [], emptyStates: [], errorStates: [], accessibility: [] },
  // U5b: the user flow diagram, written in the third part.
  flow: {
    title: "Absen praktikum",
    nodes: [
      { id: "N1", kind: "start", label: "Siswa masuk lab", feature: "" },
      { id: "N2", kind: "screen", label: "Layar absen", feature: "FEAT-001" },
      { id: "N3", kind: "decision", label: "Kartu terdaftar?", feature: "" },
      { id: "N4", kind: "action", label: "Sistem mencatat hadir", feature: "FEAT-001" },
      { id: "N5", kind: "screen", label: "Pesan kartu tidak dikenal", feature: "" },
      { id: "N6", kind: "end", label: "Praktikum dimulai", feature: "" },
    ],
    edges: [
      { from: "N1", to: "N2", label: "" },
      { from: "N2", to: "N3", label: "tempel kartu" },
      { from: "N3", to: "N4", label: "Ya" },
      { from: "N3", to: "N5", label: "Tidak" },
      { from: "N5", to: "N2", label: "coba lagi" },
      { from: "N4", to: "N6", label: "" },
    ],
  },
  features: [
    { id: "FEAT-001", name: "Absen dengan kartu", description: "Tempel kartu.", rationale: "Inti masalah.", priority: "must", dependsOn: [],
      story: { asA: "siswa", want: "absen cepat", soThat: "praktikum tepat waktu" },
      acceptance: [{ id: "AC-001-1", given: "siswa terdaftar", when: "menempelkan kartu", then: "tercatat dalam 2 detik" }], tests: ["Kartu terdaftar tercatat"] },
  ],
  nfr: [{ category: "performance", requirement: "Cepat", measure: "2 detik" }],
  technical: { applicable: false, architecture: "", components: [], dataModel: "", integrations: [], technologies: [], dependencies: [], constraints: [], risks: [] },
  security: [{ id: "SEC-01", area: "data", requirement: "Rekap hanya untuk guru", verify: "Uji akses siswa" }],
  delivery: { mvp: ["FEAT-001"], later: [], order: ["FEAT-001"], testing: "Uji di lab", release: "Satu kelas", risks: [], questions: [] },
};
function startGemini() {
  geminiServer = createServer((request, response) => {
    const ok = request.headers["x-goog-api-key"] === geminiKey;
    const send = (status, body) => {
      response.writeHead(status, { "Content-Type": "application/json" });
      response.end(JSON.stringify(body));
    };
    if (!ok) return send(401, { error: { code: "authentication", message: "API key not valid." } });
    if (request.method === "GET" && request.url.startsWith("/v1beta/models/gemini-3.8-flash")) {
      return send(200, { name: "models/gemini-3.8-flash", baseModelId: "gemini-3.8-flash", version: "001", displayName: "Gemini 3.8 Flash",
        inputTokenLimit: 1048576, outputTokenLimit: 65536, supportedGenerationMethods: ["generateContent"] });
    }
    if (request.method === "GET" && request.url.startsWith("/v1beta/models")) {
      return send(200, { models: [
        { name: "models/gemini-3.8-flash", displayName: "Gemini 3.8 Flash", supportedGenerationMethods: ["generateContent"] },
        // L2: Google lists speech models too; Studio's menu leaves them out.
        { name: "models/gemini-3.8-flash-preview-tts", displayName: "Gemini 3.8 Flash Preview TTS", supportedGenerationMethods: ["generateContent"] },
        { name: "models/gemini-3.5-flash-lite", displayName: "Gemini 3.5 Flash-Lite", supportedGenerationMethods: ["generateContent"] },
      ] });
    }
    if (request.method === "POST" && request.url === "/v1beta/interactions") {
      let raw = "";
      request.on("data", (chunk) => (raw += chunk));
      request.on("end", () => {
        const body = JSON.parse(raw);
        // A tech stack (R3): asked with the owner's developer profile.
        if (body.response_format?.schema?.properties?.layers) {
          geminiStacks.push(body);
          return send(200, { id: "v1_stack", status: "completed", steps: [{ type: "model_output", content: [{ type: "text", text: JSON.stringify(SMOKE_STACK) }] }] });
        }
        // A roadmap: one checkpoint covering the PRD's one Must feature.
        if (body.response_format?.schema?.properties?.checkpoints) {
          geminiRoadmaps.push(body);
          return send(200, { id: "v1_roadmap", status: "completed", steps: [{ type: "model_output", content: [{ type: "text", text: JSON.stringify(SMOKE_ROADMAP) }] }] });
        }
        // A PRD part: the sections the schema asks for, from a small PRD that meets every rule.
        if (body.response_format) {
          geminiPrdParts.push(body);
          const wanted = Object.keys(body.response_format.schema.properties ?? {});
          const answer = Object.fromEntries(wanted.map((section) => [section, SMOKE_PRD[section]]));
          return send(200, { id: "v1_prd", status: "completed", steps: [{ type: "model_output", content: [{ type: "text", text: JSON.stringify(answer) }] }] });
        }
        if (!body.stream) {
          return send(200, { id: "v1_smoke", status: "completed", steps: [{ type: "model_output", content: [{ type: "text", text: "Siap." }] }],
            usage: { total_input_tokens: 9, total_output_tokens: 2 } });
        }
        // A chat turn: text in pieces, then (the first round) the functions catat and tanya. Since
        // F1 Hub ends the turn after `tanya` (the owner answers first), so the second round, after
        // Hub's results, should never be asked for; it answers one closing sentence if it is.
        geminiChats.push(body);
        const answered = body.input.some((step) => step.type === "function_result");
        const events = [];
        const event = (data) => events.push(`event: ${data.event_type}\ndata: ${JSON.stringify(data)}\n\n`);
        event({ index: 0, step: { type: "model_output" }, event_type: "step.start" });
        for (const text of answered ? ["Sudah aku catat."] : ["Aku tangkap idenya. ", "Siapa penggunanya?"]) {
          event({ index: 0, delta: { type: "text", text }, event_type: "step.delta" });
        }
        event({ index: 0, event_type: "step.stop" });
        if (!answered) {
          const calls = [
            ["c1", "catat", { items: [{ kind: "assumption", area: "users", topic: "Pengguna", statement: "Siswa lab TJKT" }] }],
            ["c2", "tanya", { questions: [{ area: "users", text: "Siapa penggunanya?", why: "Menentukan alur.", options: [{ label: "Siswa", recommended: true }, { label: "Guru" }] }] }],
          ];
          calls.forEach(([id, name, args], i) => {
            event({ index: i + 1, step: { type: "function_call", id, name, arguments: {} }, event_type: "step.start" });
            event({ index: i + 1, delta: { type: "arguments_delta", arguments: JSON.stringify(args) }, event_type: "step.delta" });
            event({ index: i + 1, event_type: "step.stop" });
          });
        }
        event({ interaction: { status: answered ? "completed" : "requires_action", usage: { total_input_tokens: 50, total_output_tokens: 20 } }, event_type: "interaction.completed" });
        response.writeHead(200, { "Content-Type": "text/event-stream" });
        response.end(events.join("") + "event: done\ndata: [DONE]\n\n");
      });
      return undefined;
    }
    return send(404, { error: { code: "not_found", message: "not found" } });
  });
  return new Promise((ready) => geminiServer.listen(geminiPort, "127.0.0.1", ready));
}
function stopGemini() {
  // Hub keeps its connection open; without closing it the server waits about three minutes.
  geminiServer?.closeAllConnections();
  return new Promise((done) => (geminiServer ? geminiServer.close(() => done()) : done()));
}

if (!existsSync(exe)) throw new Error(`build first: ${exe} is missing`);
if (!seedExe) throw new Error("build hub-seed first: cargo build -p hub-core --bin hub-seed");

const dataDir = mkdtempSync(join(tmpdir(), "hub-smoke-"));
console.log(`data: ${dataDir}`);
console.log(execFileSync(seedExe, [dataDir], { encoding: "utf8" }).trim());

const results = [];
function check(name, ok, detail = "") {
  results.push({ name, ok });
  console.log(`${ok ? "PASS" : "FAIL"} ${name}${detail ? `: ${detail}` : ""}`);
}

function launch() {
  return spawn(exe, [], {
    env: {
      ...process.env,
      HUB_DATA_DIR: dataDir,
      HUB_API_PORT: String(apiPort),
      HUB_GEMINI_BASE_URL: `http://127.0.0.1:${geminiPort}`,
      WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port} --remote-allow-origins=*`,
    },
    stdio: "ignore",
  });
}

/** An MCP client over the bridge's stdio (newline-delimited JSON-RPC), as Claude Code is. */
function mcp(cwd, extra = [], projectDir = null) {
  // The folder is the working directory unless CLAUDE_PROJECT_DIR names one (as Claude Code does);
  // a smoke run from inside Claude Code must not pass its own on.
  const env = { ...process.env, HUB_DATA_DIR: dataDir, HUB_API_PORT: String(apiPort), HUB_GEMINI_BASE_URL: `http://127.0.0.1:${geminiPort}` };
  delete env.CLAUDE_PROJECT_DIR;
  if (projectDir) env.CLAUDE_PROJECT_DIR = projectDir;
  const child = spawn(bridgeExe, ["--data-dir", dataDir, ...extra], {
    cwd,
    env,
    stdio: ["pipe", "pipe", "ignore"],
  });
  let buffer = "";
  let next = 1;
  const waiting = new Map();
  child.stdout.setEncoding("utf8");
  child.stdout.on("data", (chunk) => {
    buffer += chunk;
    let at;
    while ((at = buffer.indexOf("\n")) >= 0) {
      const line = buffer.slice(0, at);
      buffer = buffer.slice(at + 1);
      try {
        const message = JSON.parse(line);
        waiting.get(message.id)?.(message);
        waiting.delete(message.id);
      } catch {
        // not a JSON line
      }
    }
  });
  const send = (method, params) =>
    new Promise((resolve, reject) => {
      const id = next++;
      const timer = setTimeout(() => reject(new Error(`mcp ${method} timed out`)), 25_000);
      waiting.set(id, (message) => {
        clearTimeout(timer);
        resolve(message);
      });
      child.stdin.write(JSON.stringify({ jsonrpc: "2.0", id, method, params }) + "\n");
    });
  return {
    async start() {
      await send("initialize", { protocolVersion: "2025-11-25", capabilities: {}, clientInfo: { name: "hub-smoke", version: "1" } });
      child.stdin.write(JSON.stringify({ jsonrpc: "2.0", method: "notifications/initialized" }) + "\n");
    },
    async tool(name, args = {}) {
      return (await send("tools/call", { name, arguments: args })).result;
    },
    /** Killed, as when Claude Code is ended without closing the bridge's input. */
    close() {
      child.kill();
    },
    /** Claude Code closing the bridge's input: the bridge says goodbye to Hub, then exits. */
    async goodbye() {
      const exited = new Promise((done) => (child.exitCode !== null ? done() : child.once("exit", done)));
      child.stdin.end();
      await Promise.race([exited, sleep(5_000)]);
      if (child.exitCode === null) child.kill();
    },
  };
}

/** Only the test Hub (target\\release\\hub.exe) is ever stopped this way, never the owner's. */
function stopTestHubs() {
  const script = `Get-CimInstance Win32_Process -Filter "Name='hub.exe'" | Where-Object { $_.ExecutablePath -eq '${exe}' } | ForEach-Object { Stop-Process -Id $_.ProcessId -Force }`;
  try {
    execFileSync("powershell", ["-NoProfile", "-Command", script], { stdio: "ignore" });
  } catch {
    // nothing to stop
  }
}

/** Private working set (Task Manager's Memory column) of hub.exe and its WebView2 processes, in MB. */
function memoryMB(hubPid) {
  const script = `$ids = @(${hubPid}) + @(Get-CimInstance Win32_Process | Where-Object { $_.Name -eq 'msedgewebview2.exe' -and $_.CommandLine -like '*${dataDir}*' } | ForEach-Object { [int]$_.ProcessId });
    $perf = Get-CimInstance Win32_PerfFormattedData_PerfProc_Process;
    [math]::Round((($perf | Where-Object { $ids -contains $_.IDProcess } | Measure-Object -Property WorkingSetPrivate -Sum).Sum) / 1MB, 1)`;
  try {
    return Number(execFileSync("powershell", ["-NoProfile", "-Command", script], { encoding: "utf8" }).trim());
  } catch {
    return 0;
  }
}

/** Close the window as the owner does (WM_CLOSE): Hub hides it to the tray. */
function hideWindow(hubPid) {
  const script = `Add-Type -Namespace W -Name U -MemberDefinition '[DllImport("user32.dll")] public static extern bool PostMessage(System.IntPtr h, uint m, System.IntPtr w, System.IntPtr l);';
    $h = (Get-Process -Id ${hubPid}).MainWindowHandle; [void][W.U]::PostMessage($h, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero)`;
  try {
    execFileSync("powershell", ["-NoProfile", "-Command", script], { stdio: "ignore" });
  } catch {
    // the window is already gone
  }
}

function stop(child) {
  try {
    execFileSync("taskkill", ["/PID", String(child.pid), "/T", "/F"], { stdio: "ignore" });
  } catch {
    // already gone
  }
}

async function attach() {
  for (let attempt = 0; attempt < 100; attempt += 1) {
    try {
      const pages = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
      const page = pages.find((p) => p.type === "page");
      if (page) return connect(page.webSocketDebuggerUrl);
    } catch {
      // not up yet
    }
    await sleep(100);
  }
  throw new Error("the window never offered a DevTools page");
}

function connect(url) {
  const socket = new WebSocket(url);
  let id = 0;
  const waiting = new Map();
  socket.addEventListener("message", (event) => {
    const message = JSON.parse(event.data);
    const pending = waiting.get(message.id);
    if (!pending) return;
    waiting.delete(message.id);
    if (message.error) pending.reject(new Error(message.error.message));
    else pending.resolve(message.result);
  });
  const send = (method, params = {}) =>
    new Promise((resolveCall, rejectCall) => {
      id += 1;
      waiting.set(id, { resolve: resolveCall, reject: rejectCall });
      socket.send(JSON.stringify({ id, method, params }));
    });
  const evaluate = async (expression) => {
    const result = await send("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true });
    if (result.exceptionDetails) throw new Error(result.exceptionDetails.exception?.description ?? "evaluate failed");
    return result.result.value;
  };
  const screenshot = async (name) => {
    const { data } = await send("Page.captureScreenshot", { format: "png" });
    const file = join(dataDir, `${name}.png`);
    writeFileSync(file, Buffer.from(data, "base64"));
    return file;
  };
  const close = () => socket.close();
  return new Promise((resolveOpen, rejectOpen) => {
    socket.addEventListener("open", () => resolveOpen({ evaluate, screenshot, close }));
    socket.addEventListener("error", () => rejectOpen(new Error("DevTools socket failed")));
  });
}

async function waitFor(page, expression, label, timeout = 8000) {
  const start = Date.now();
  while (Date.now() - start < timeout) {
    try {
      if (await page.evaluate(expression)) return true;
    } catch {
      // the page may be reloading
    }
    await sleep(100);
  }
  throw new Error(`timed out waiting for ${label}`);
}

// Page-side helpers. The core is asked through Tauri's own bridge, so the checks compare the
// window with what the core says rather than with numbers that depend on the weekday.
const core = (command, args = {}) => `window.__TAURI_INTERNALS__.invoke(${JSON.stringify(command)}, ${JSON.stringify(args)})`;
// An input or a textarea, typed into the way React notices; a missing field says what was on screen
// instead of failing with "Illegal invocation".
const typeInto = (selector, text) => `(() => {
  const el = document.querySelector(${JSON.stringify(selector)});
  if (!el) throw new Error(${JSON.stringify(`${selector} is not on the page`)} + ": " + (document.querySelector("main") ?? document.body).innerText.slice(0, 200));
  Object.getOwnPropertyDescriptor(Object.getPrototypeOf(el), "value").set.call(el, ${JSON.stringify(text)});
  el.focus();
  el.dispatchEvent(new Event("input", { bubbles: true }));
  return true;
})()`;
const pressEnter = (selector) =>
  `document.querySelector(${JSON.stringify(selector)}).dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }))`;
const clickButton = (text, scope = "document") =>
  `[...${scope}.querySelectorAll("button")].find((b) => b.textContent.trim() === ${JSON.stringify(text)}).click()`;
// A section of Studio's stage 3 (PRD, Rencana checkpoint, Bangun): the stage first, then its tab.
const openStudioDoc = (name) => `(async () => {
  document.querySelector("#studio-step-3").click();
  await new Promise((done) => setTimeout(done, 100));
  [...document.querySelectorAll("[role=tab]")].find((b) => b.textContent.trim() === ${JSON.stringify(name)}).click();
  return true;
})()`;
const isoDay = (d) => `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;

let child = launch();
try {
  let page = await attach();
  await waitFor(page, "!!document.querySelector('h1')", "Hari Ini");
  const view = await page.evaluate(core("get_today"));
  const timed = view.items.filter((item) => !item.allDay).length;
  const first = await page.evaluate(`({
    heading: document.querySelector('h1').textContent,
    phase: document.documentElement.dataset.phase,
    active: document.querySelector('aside a[data-status=active]')?.textContent,
    checks: document.querySelectorAll('[aria-label^="Tandai selesai"], [aria-label^="Batalkan selesai"]').length,
    rows: document.querySelectorAll('[aria-labelledby=schedule-title] ol > li').length,
    arc: document.querySelector('svg[role=img]')?.getAttribute('aria-label'),
    visible: document.visibilityState,
    fonts: [...document.fonts].filter(f => f.status === 'loaded').map(f => f.family),
  })`);
  check("Hari Ini renders", /^Selamat (pagi|siang|sore|malam), Glenvio\.$/.test(first.heading), first.heading);
  check("light applied", ["pagi", "siang", "senja", "malam"].includes(first.phase), first.phase);
  check("sidebar marks Hari Ini", first.active === "Hari Ini");
  check("focus and today's tasks shown", first.checks === view.focus.length + view.also.length, `${first.checks} checks`);
  check(
    "schedule matches the core",
    first.rows === view.items.length + view.reminders.length,
    `${first.rows} rows, ${view.items.length + view.reminders.length} in the core`,
  );
  check("Day Arc labelled", (first.arc ?? "").includes(`${timed} jadwal hari ini`), first.arc);
  check("window is visible", first.visible === "visible");
  check(
    "own fonts loaded",
    ["Fraunces Variable", "Instrument Sans Variable"].every((f) => first.fonts.includes(f)),
    first.fonts.join(", "),
  );
  const shell = await page.evaluate(core("get_shell"));
  console.log(`info: hotkey ${shell.hotkey} ${shell.hotkeyOk ? "registered" : "taken by another app (the owner's Hub may be running)"}`);
  console.log(`screenshot: ${await page.screenshot("today")}`);

  await page.evaluate(`document.querySelector('[aria-label="Tandai selesai: THM: Nmap Live Host Discovery"]').click()`);
  await waitFor(
    page,
    `document.querySelector('[aria-label="Batalkan selesai: THM: Nmap Live Host Discovery"]')?.getAttribute('aria-pressed') === 'true'`,
    "the check",
  );
  check("a focus task can be checked", true);
  // Jejak shows only the latest few, so it is read now, before the later steps push this out.
  await waitFor(
    page,
    `[...document.querySelectorAll('section ul li')].some((li) => li.textContent.includes('menyelesaikan') && li.textContent.includes('Nmap'))`,
    "the check in Jejak",
  );
  check("Jejak records the check", true);

  // Catat Cepat from the sidebar: the core reads "besok" as tomorrow.
  await page.evaluate(`document.querySelector('aside button[title="Catat cepat"]').click()`);
  await waitFor(page, "!!document.querySelector('[role=dialog] input')", "the capture line");
  await page.evaluate(typeInto("[role=dialog] input", "beli tinta printer besok"));
  await sleep(400);
  await page.evaluate(pressEnter("[role=dialog] input"));
  await waitFor(page, "!document.querySelector('[role=dialog]')", "the capture to close");
  const upcoming = await page.evaluate(core("list_tasks", { view: "upcoming" }));
  const captured = upcoming.find((task) => task.title === "beli tinta printer");
  check(
    "Catat Cepat saves a task for tomorrow",
    Boolean(captured?.doDate && captured.doDate > view.date),
    JSON.stringify(captured?.doDate),
  );

  // Proyek & Tugas (R6): the new task waits under Mendatang.
  await page.evaluate("location.hash = '#/proyek?view=upcoming'");
  await waitFor(page, `!!document.querySelector('[role=button][aria-label="beli tinta printer"]')`, "the task in Mendatang");
  check("Proyek & Tugas lists the captured task", true);
  // The sidebar's groups (R6): the main pages, a line, File and Jejak.
  const sidebar = await page.evaluate("[...document.querySelectorAll('aside [data-pane=main] nav > *')].map((el) => (el.tagName === 'HR' ? '|' : el.textContent.trim())).join(',')");
  check("the sidebar groups its pages", sidebar === "Hari Ini,Kalender,Proyek & Tugas,Studio,Belajar,|,File,Jejak", sidebar);
  console.log(`screenshot: ${await page.screenshot("tasks")}`);

  // Proyek: the project counts every task; each milestone groups its own tasks and counts them, and
  // only a milestone without tasks has a tick of its own.
  const lab = (await page.evaluate(core("list_projects"))).find((p) => p.name === "Laporan Lab Jaringan");
  const labDetail = await page.evaluate(core("get_project", { id: lab.id }));
  await page.evaluate(`location.hash = '#/proyek/${lab.id}'`);
  await waitFor(page, `document.querySelectorAll('section[aria-labelledby^="milestone-"]').length === ${labDetail.milestones.length}`, "the project's milestones");
  const shown = await page.evaluate(`({
    progress: document.querySelector('[role=progressbar][aria-label="Kemajuan Laporan Lab Jaringan"]')?.getAttribute('aria-valuenow'),
    groups: [...document.querySelectorAll('section[aria-labelledby^="milestone-"]')].map((s) => ({
      meta: s.querySelector('p').textContent,
      tasks: s.querySelectorAll('[data-row]').length,
      tick: !!s.querySelector('input[type=checkbox]'),
      done: s.dataset.done,
    })),
  })`);
  check("the project counts its tasks", shown.progress === String(lab.progress.percent) && lab.progress.total === lab.openTasks + lab.progress.done, JSON.stringify(lab.progress));
  const fits = labDetail.milestones.every((m, i) => {
    const g = shown.groups[i];
    const open = labDetail.tasks.filter((t) => t.milestoneId === m.id && t.status !== "done").length;
    return g.meta.includes(m.tasksTotal ? `${m.tasksDone} dari ${m.tasksTotal} tugas` : "Belum ada tugas") && g.tasks === open && g.tick === (m.tasksTotal === 0) && g.done === String(m.done);
  });
  check("each milestone groups and counts its tasks", fits && labDetail.milestones.some((m) => m.done && m.tasksTotal > 0), JSON.stringify(shown.groups));
  console.log(`screenshot: ${await page.screenshot("project")}`);

  // Kalender: every timed item of the week is on the grid.
  await page.evaluate("location.hash = '#/kalender'");
  await waitFor(page, "document.querySelectorAll('[aria-label*=\" sampai \"]').length > 0", "the week grid");
  // P2b: the chosen view's pill glides there in the real window (mid-way it is neither here nor there).
  const glide = await page.evaluate(`(async () => {
    const group = document.querySelector('[role=radiogroup][aria-label=Kalender][data-glide]');
    const clip = () => getComputedStyle(group, '::before').clipPath;
    const from = clip();
    [...group.querySelectorAll('input')][2].click();
    await new Promise((done) => setTimeout(done, 90));
    const mid = clip();
    await new Promise((done) => setTimeout(done, 450));
    const to = clip();
    [...group.querySelectorAll('input')][1].click();
    return { from, mid, to };
  })()`);
  check("the chosen view's pill glides", glide.from !== glide.to && glide.mid !== glide.from && glide.mid !== glide.to, JSON.stringify(glide));
  await waitFor(page, "location.hash.includes('view=week') && document.querySelectorAll('[aria-label*=\" sampai \"]').length > 0", "the week grid again");
  const monday = new Date(`${view.date}T00:00`);
  monday.setDate(monday.getDate() - ((monday.getDay() + 6) % 7));
  const week = await page.evaluate(core("get_calendar", { from: isoDay(monday), days: 7 }));
  const weekTimed = week.reduce((sum, day) => sum + day.items.filter((item) => !item.allDay).length, 0);
  const onGrid = await page.evaluate(
    `[...document.querySelectorAll('button[aria-label]')].filter((b) => /, \\d\\d\\.\\d\\d sampai \\d\\d\\.\\d\\d/.test(b.getAttribute('aria-label'))).length`,
  );
  check("Kalender shows the week's timed items", onGrid === weekTimed && weekTimed > 0, `${onGrid} on the grid, ${weekTimed} in the core`);
  console.log(`screenshot: ${await page.screenshot("calendar")}`);

  // A reminder a minute from now goes off by itself and waits above the page until handled.
  const now = new Date();
  const minute = now.getHours() * 60 + now.getMinutes() + 1;
  if (minute < 1440) {
    await page.evaluate(core("create_reminder", { input: { text: "Smoke: minum air", date: isoDay(now), minute, taskId: null } }));
    await waitFor(
      page,
      `[...document.querySelectorAll('strong')].some((s) => s.textContent === 'Smoke: minum air')`,
      "the reminder to go off",
      90_000,
    );
    check("a reminder goes off on time", true);
    console.log(`screenshot: ${await page.screenshot("reminder")}`);
    await page.evaluate(`(() => {
      const row = [...document.querySelectorAll('strong')].find((s) => s.textContent === 'Smoke: minum air').closest('span').parentElement;
      ${clickButton("Tunda 10 m", "row")};
    })()`);
    await waitFor(page, `![...document.querySelectorAll('strong')].some((s) => s.textContent === 'Smoke: minum air')`, "the snooze");
    const reminders = await page.evaluate(core("list_reminders"));
    check("Tunda 10 m snoozes it", reminders.find((r) => r.text === "Smoke: minum air")?.state === "snoozed");
  } else {
    console.log("skip: the reminder check needs a minute left in the day");
  }

  // File: the demo files (hub-seed puts real ones in <data>\demo-files) with Windows' own pictures.
  const counts = await page.evaluate(core("file_counts"));
  await page.evaluate("location.hash = '#/file'");
  await waitFor(page, "!!document.querySelector('ul[aria-label=\"Semua file\"] li')", "the Library");
  const tiles = await page.evaluate("document.querySelectorAll('ul[aria-label=\"Semua file\"] > li').length");
  check("File lists the Library", tiles === counts.all && counts.all === 4, `${tiles} tiles, ${counts.all} in the core`);
  const files = await page.evaluate(core("list_files", { query: {} }));
  const byName = (name) => files.find((f) => f.name === name);
  const picture = await page.evaluate(core("file_thumbnail", { id: byName("topologi-vlan.png").id }));
  check("Windows makes the thumbnail", picture?.kind === "thumbnail" && picture.image.startsWith("data:image/png;base64,"), picture?.kind);
  await waitFor(page, "[...document.querySelectorAll('ul[aria-label=\"Semua file\"] img')].length > 0", "pictures on the tiles");
  console.log(`screenshot: ${await page.screenshot("files")}`);

  const sheet = byName("nmap-cheatsheet.pdf");
  await page.evaluate(core("copy_to_vault", { id: sheet.id }));
  const kept = await page.evaluate(core("get_file", { id: sheet.id }));
  const vaultFiles = existsSync(join(dataDir, "vault"))
    ? readdirSync(join(dataDir, "vault"), { recursive: true }).filter((f) => String(f).endsWith("nmap-cheatsheet.pdf"))
    : [];
  check("Simpan salinan copies into the Vault", kept.vaulted && vaultFiles.length === 1, vaultFiles.join(", "));

  // Moved on the same drive: found again by its file id. Deleted: missing.
  const writeup = byName("writeup-lame.md");
  mkdirSync(join(dataDir, "arsip"), { recursive: true });
  renameSync(writeup.path, join(dataDir, "arsip", "lame-final.md"));
  await page.evaluate(core("check_file", { id: writeup.id }));
  const moved = await page.evaluate(core("get_file", { id: writeup.id }));
  check("a moved file is found again by its id", moved.state === "moved" && moved.path.endsWith("lame-final.md"), moved.path);
  const docx = byName("Laporan Praktikum 5 - VLAN.docx");
  unlinkSync(docx.path);
  await page.evaluate(core("check_file", { id: docx.id }));
  check("a deleted file is marked missing", (await page.evaluate(core("get_file", { id: docx.id }))).state === "missing");

  // Belajar: Lanjutkan starts the timer in the real window, Berhenti writes a session.
  await page.evaluate("location.hash = '#/belajar'");
  await waitFor(page, `!!document.querySelector('[aria-label^="Lanjutkan: Nmap Live Host Discovery"]')`, "the TryHackMe items");
  const room = (await page.evaluate(core("list_learning", { query: {} }))).find((i) => i.title === "Nmap Live Host Discovery");
  await page.evaluate(`document.querySelector('[aria-label^="Lanjutkan: Nmap Live Host Discovery"]').click()`);
  await waitFor(page, "[...document.querySelectorAll('small')].some((s) => s.textContent.startsWith('Sedang belajar'))", "the timer");
  console.log(`screenshot: ${await page.screenshot("learning")}`);
  await sleep(1500);
  await page.evaluate(clickButton("Berhenti"));
  await waitFor(
    page,
    "![...document.querySelectorAll('small')].some((s) => s.textContent.startsWith('Sedang belajar'))",
    "the timer to stop",
  );
  const studied = (await page.evaluate(core("list_learning", { query: {} }))).find((i) => i.title === "Nmap Live Host Discovery");
  check("the timer writes a session", studied.sessions === room.sessions + 1, `${room.sessions} then ${studied.sessions} sessions`);

  // Jejak and search.
  await page.evaluate("location.hash = '#/jejak'");
  await waitFor(page, "document.querySelectorAll('ul[aria-label=\"Minggu ini\"] > li').length === 5", "the week's measures");
  check("Jejak shows the week against the average", true);
  console.log(`screenshot: ${await page.screenshot("trail")}`);
  const hits = await page.evaluate(core("search", { query: "nmap" }));
  const kinds = [...new Set(hits.map((h) => h.kind))].sort();
  check(
    "search finds tasks, files, and learning items",
    ["file", "learning_item", "task"].every((k) => kinds.includes(k)),
    kinds.join(", "),
  );

  // Tutup Hari: the defaults move what slipped to tomorrow.
  await page.evaluate("location.hash = '#/?review=1'");
  await waitFor(
    page,
    "[...document.querySelectorAll('[role=dialog] button')].some((b) => b.textContent.trim() === 'Tutup hari')",
    "the Tutup Hari sheet",
  );
  console.log(`screenshot: ${await page.screenshot("review")}`);
  await page.evaluate(clickButton("Tutup hari"));
  await waitFor(page, "document.querySelector('h1')?.textContent.startsWith('Hari ditutup.')", "the closed day");
  const closed = await page.evaluate(core("get_today"));
  check("Tutup Hari closes the day", closed.closed === true);
  await sleep(2200);
  console.log(`screenshot: ${await page.screenshot("closed")}`);

  // Studio and Gemini: the page asks for a key, the key goes to Credential Manager (never the
  // database or the log), Uji koneksi reaches the fake Gemini, and with Gemini gone the rest of Hub
  // works and the check says offline.
  await startGemini();
  await page.evaluate("location.hash = '#/studio'");
  await waitFor(page, "document.querySelector('h1')?.textContent === 'Apa yang ingin kamu bangun hari ini?'", "Studio");
  check("Studio asks for a Gemini key first", await page.evaluate("document.body.textContent.includes('Lentera butuh API key dari Google AI Studio')"));
  console.log(`screenshot: ${await page.screenshot("studio")}`);
  // P1: Pengaturan in parts; the Gemini key is in Integrasi AI.
  await page.evaluate("location.hash = '#/pengaturan/ai'");
  await waitFor(page, "!!document.querySelector('#gemini-key')", "the Gemini key field");
  await page.evaluate(typeInto("#gemini-key", geminiKey));
  await sleep(200);
  await page.evaluate("document.querySelector('#gemini-key').form.querySelector('button[type=submit]').click()");
  await waitFor(page, "document.body.textContent.includes('Tersimpan di Windows Credential Manager.')", "the saved key");
  check("the Gemini key is saved", (await page.evaluate(core("gemini_status"))).hasKey === true);
  await page.evaluate(clickButton("Uji koneksi"));
  await waitFor(page, "document.body.textContent.includes('Terhubung. Gemini 3.8 Flash siap dipakai.')", "Uji koneksi");
  check("Uji koneksi reaches Gemini", true);
  console.log(`screenshot: ${await page.screenshot("gemini")}`);
  const traces = ["hub.db", "hub.db-wal", "hub.log"]
    .map((name) => join(dataDir, name))
    .filter(existsSync)
    .filter((file) => readFileSync(file).includes(geminiKey));
  check("the key is in no file of Hub's", traces.length === 0, traces.join(", "));
  // A conversation: the idea starts a project, the reply streams in, Gemini's note lands in
  // Ringkasan and its question shows as choices; the request carried nothing but the project.
  await page.evaluate("location.hash = '#/studio'");
  await waitFor(page, "!!document.querySelector('#studio-idea')", "the idea field");
  // L3: the glow by the light of the day, drawn by radial lights that move, hidden from assistive
  // technology.
  const glow = await page.evaluate(`(() => {
    const g = document.querySelector("[data-testid=studio-glow]");
    if (!g) return null;
    const s = getComputedStyle(g, "::before");
    return { hidden: g.getAttribute("aria-hidden"), lights: s.backgroundImage.split("radial-gradient").length - 1, moving: s.animationName !== "none" };
  })()`);
  check("Studio's home has the glow by the light of the day", glow?.hidden === "true" && glow.lights === 2 && glow.moving, JSON.stringify(glow));
  // After the page's rise and the sidebar's slide, so the picture shows the light at rest.
  await sleep(1200);
  console.log(`screenshot: ${await page.screenshot("studio-glow")}`);
  // L2: the model chip opens Google's text models (the speech model left out), and the choice is
  // Hub's one setting.
  const chipName = () => page.evaluate("document.querySelector('button[aria-haspopup=menu]')?.getAttribute('aria-label')");
  check("the model chip names the model", (await chipName()) === "Model: 3.8 Flash. Ganti model", String(await chipName()));
  await page.evaluate("document.querySelector('button[aria-haspopup=menu]').click()");
  await waitFor(page, "document.querySelectorAll('[role=menu] [role=menuitemradio]').length === 2", "the model menu with Google's list");
  const models = await page.evaluate("[...document.querySelectorAll('[role=menu] [role=menuitemradio]')].map((i) => i.textContent.trim())");
  check("the model menu lists the text models only", JSON.stringify(models) === JSON.stringify(["3.8 Flash", "3.5 Flash-Lite"]), JSON.stringify(models));
  await sleep(400);
  console.log(`screenshot: ${await page.screenshot("studio-models")}`);
  await page.evaluate("[...document.querySelectorAll('[role=menu] [role=menuitemradio]')][1].click()");
  await waitFor(page, `document.querySelector('button[aria-haspopup=menu]')?.getAttribute('aria-label') === "Model: 3.5 Flash-Lite. Ganti model"`, "the chosen model");
  check("the chosen model is saved for every project", (await page.evaluate(core("get_settings"))).geminiModel === "gemini-3.5-flash-lite");
  await page.evaluate("document.querySelector('button[aria-haspopup=menu]').click()");
  await waitFor(page, "document.querySelectorAll('[role=menu] [role=menuitemradio]').length === 2", "the model menu again");
  await page.evaluate("[...document.querySelectorAll('[role=menu] [role=menuitemradio]')][0].click()");
  await waitFor(page, `document.querySelector('button[aria-haspopup=menu]')?.getAttribute('aria-label') === "Model: 3.8 Flash. Ganti model"`, "the model back");
  await page.evaluate(typeInto("#studio-idea", "Absensi lab pakai kartu"));
  await sleep(200);
  await page.evaluate("document.querySelector('#studio-idea').form.querySelector('button[type=submit]').click()");
  // L1: a reply that asks is only its questions, under Lentera's name; the words the fake Gemini
  // writes around them (both rounds) are not shown.
  await waitFor(page, "[...document.querySelectorAll('input[type=radio]')].some((r) => r.value === 'Siswa')", "the question's choices", 15_000);
  check("Gemini's question shows as choices", true);
  check(
    "Studio starts a project and Lentera's reply is its question alone",
    await page.evaluate("document.body.textContent.includes('Pertanyaan dari Lentera') && !document.body.textContent.includes('Aku tangkap idenya.') && !document.body.textContent.includes('Sudah aku catat.')"),
  );
  await waitFor(page, "document.body.textContent.includes('Siswa lab TJKT')", "the note in Ringkasan");
  check("Gemini's note is in Ringkasan", true);
  // U2: in Studio the sidebar is Studio's history, the new project marked current.
  const side = await page.evaluate(`({
    mode: document.querySelector('[data-mode]')?.dataset.mode,
    current: document.querySelector('#studio-history [aria-current=page]')?.textContent.trim(),
  })`);
  check("the sidebar slid to Studio's history, the project current", side.mode === "studio" && side.current === "Absensi lab pakai kartu", JSON.stringify(side));
  console.log(`screenshot: ${await page.screenshot("studio-chat")}`);
  const projects = await page.evaluate(core("list_studio_projects"));
  const ledger = await page.evaluate(core("studio_ledger", { projectId: projects[0].id }));
  check("the note is Gemini's, from that reply", ledger.length === 1 && ledger[0].statedBy === "gemini" && ledger[0].messageId !== null);
  // R2: the note keeps its part of the idea, and Ringkasan counts that part as partly clear.
  check("the note is placed under Pengguna", ledger[0].area === "users", String(ledger[0].area));
  check(
    "Yang sudah jelas shows Pengguna as partly clear",
    await page.evaluate("[...document.querySelectorAll('li')].some((li) => li.textContent.startsWith('Pengguna') && li.textContent.includes('1 asumsi') && li.textContent.endsWith('sebagian'))"),
  );
  const sent = JSON.stringify(geminiChats);
  check(
    "the request held the project and no other Hub data",
    geminiChats.every((b) => b.store === false) && sent.includes("Absensi lab") && !sent.includes("Laporan Praktikum") && !sent.includes("nmap"),
  );
  // F1: a reply that asks ends its turn; Gemini is not asked to go on before the owner answers.
  check("the reply that asked ended its turn: one request, no second round", geminiChats.length === 1 && !sent.includes("function_result"), `${geminiChats.length} requests`);
  // Stage 2 (R3): the PRD waits for an approved stack; Gemini recommends one with the profile, the
  // owner approves it, and the PRD is written from it.
  await page.evaluate(core("update_settings", { patch: { devKnows: "PHP dan MySQL", devCost: "free" } }));
  await page.evaluate(`document.querySelector("#studio-step-3").click()`);
  check("stage 3 waits for the approved stack", await page.evaluate("document.body.textContent.includes('Setujui tech stack dulu di Tahap 2')"));
  await page.evaluate(`document.querySelector("#studio-step-2").click()`);
  await waitFor(page, "[...document.querySelectorAll('button')].some((b) => b.textContent.includes('Rekomendasikan dengan Lentera'))", "stage 2");
  await page.evaluate("[...document.querySelectorAll('button')].find((b) => b.textContent.includes('Rekomendasikan dengan Lentera')).click()");
  await waitFor(page, "document.body.textContent.includes('Rekomendasi tersusun')", "the recommended stack", 15_000);
  const stackSent = JSON.stringify(geminiStacks);
  check("the recommendation carried the profile and the project, structured", geminiStacks.length === 1 && stackSent.includes("PHP dan MySQL") && stackSent.includes("Absensi lab") && geminiStacks[0].store === false);
  await page.evaluate(clickButton("Setujui tech stack"));
  await waitFor(page, "!!document.querySelector('[role=dialog]')", "the approval sheet");
  // L3: the writing's steps are watched as they change.
  await page.evaluate(`(() => {
    window.__prdSteps = [];
    new MutationObserver(() => {
      const list = document.querySelector('[aria-label="Bagian PRD"]');
      if (list) window.__prdSteps.push([...list.children].map((li) => li.dataset.state).join(","));
    }).observe(document.body, { subtree: true, childList: true, attributes: true, attributeFilter: ["data-state"] });
  })()`);
  await page.evaluate(clickButton("Setujui dan susun PRD"));
  await waitFor(page, "document.body.textContent.includes('PRD tersusun.')", "the PRD", 20_000);
  const prdSteps = await page.evaluate("[...new Set(window.__prdSteps)]");
  check("the PRD's writing showed its parts as steps", prdSteps.includes("running,waiting,waiting") && prdSteps.some((s) => s.startsWith("done")), JSON.stringify(prdSteps));
  check("the written PRD fades in", await page.evaluate("!!document.querySelector('#studio-canvas section[class*=arrived]')"));
  const keptStack = await page.evaluate(core("studio_stack", { projectId: projects[0].id }));
  check("the approved stack is kept, each layer Gemini's", keptStack?.status === "approved" && keptStack.body.layers.length === 7 && keptStack.body.layers.every((l) => l.by === "gemini"), keptStack?.status);
  const prdView = await page.evaluate(core("studio_prd", { projectId: projects[0].id }));
  check(
    "Susun PRD writes three parts into version 1 with no problem left",
    geminiPrdParts.length === 3 && prdView.versions.length === 1 && prdView.problems.length === 0 && prdView.draft.features[0].id === "FEAT-001",
    `${geminiPrdParts.length} parts, ${prdView.problems.length} problems`,
  );
  check("each part asked for JSON against its own schema", geminiPrdParts.every((b) => b.response_format.mime_type === "application/json" && b.store === false));
  // R4: the technologies are the approved stack's, and security is a section of its own.
  check(
    "the PRD takes its technologies from the approved stack and has security",
    prdView.draft.technical.fromStack === true && prdView.draft.technical.technologies.includes("Backend: Pilihan backend") && prdView.draft.security.length === 1,
    JSON.stringify(prdView.draft.technical.technologies),
  );
  // U4a: the conversation stays beside the canvas, and the wide document folded the history.
  const layout = await page.evaluate(`({
    log: !!document.querySelector('[role=log]'),
    canvas: !!document.querySelector('#studio-canvas'),
    folded: document.querySelector('aside')?.parentElement?.hasAttribute('data-folded') ?? false,
  })`);
  check("the PRD sits in the canvas beside the conversation, the history folded", layout.log && layout.canvas && layout.folded, JSON.stringify(layout));
  // U5b: the PRD's user flow is drawn in the real window, with its text alongside.
  const drawn = await page.evaluate(`(() => {
    const figure = document.querySelector('#studio-canvas figure');
    const svg = figure?.querySelector('svg');
    return { steps: svg?.querySelectorAll('rect, polygon').length ?? 0, arrows: svg?.querySelectorAll('polyline').length ?? 0, width: svg?.getBoundingClientRect().width ?? 0, text: figure?.querySelectorAll('ol > li').length ?? 0 };
  })()`);
  check("the user flow is drawn as a diagram", drawn.steps === 6 && drawn.arrows === 6 && drawn.width > 0 && drawn.text === 6, JSON.stringify(drawn));
  console.log(`screenshot: ${await page.screenshot("studio-flow")}`);
  // U4b: the PRD's card is in the conversation; the document's own Setujui approves (in the canvas,
  // since the card has one too), and the plan is written at once.
  check("the PRD's card waits in the conversation", await page.evaluate("!!document.querySelector('[role=log] article[aria-label^=\"Hasil: PRD\"]')"));
  await page.evaluate(clickButton("Setujui", "document.querySelector('#studio-canvas')"));
  await waitFor(page, "document.body.textContent.includes('Roadmap tersusun.')", "the approval and the plan written at once", 20_000);
  const approvedView = await page.evaluate(core("studio_prd", { projectId: projects[0].id }));
  check("the PRD is approved as version 1", approvedView.approved?.number === 1, JSON.stringify(approvedView.approved?.status));
  console.log(`screenshot: ${await page.screenshot("studio-prd")}`);

  // The roadmap: written from the approved PRD as soon as it was approved, approved with the folder
  // (the data folder stands in for a repository: the folder dialog is native and not driven here),
  // then started in Pantau.
  const studioId = projects[0].id;
  const roadmapView = await page.evaluate(core("studio_roadmap", { projectId: studioId }));
  check(
    "the plan is asked for once, right after the approval, against the roadmap schema, with the approved PRD fenced",
    geminiRoadmaps.length === 1 && geminiRoadmaps[0].store === false && geminiRoadmaps[0].input.includes("<prd>") && roadmapView.problems.length === 0,
    `${geminiRoadmaps.length} requests, ${roadmapView.problems.length} problems`,
  );
  check(
    "the draft holds Hub's codes and the DoD lines",
    roadmapView.draft?.checkpoints[0]?.tasks[0]?.code === "CP-01.T01" && roadmapView.draft.checkpoints[0].doneWhen.length === 2,
    JSON.stringify(roadmapView.draft?.checkpoints[0]?.tasks.map((t) => t.code)),
  );
  const folder = await page.evaluate(core("check_repo_folder", { path: dataDir }));
  check("a folder is checked without reading it", folder.exists && !folder.git, JSON.stringify(folder));
  await page.evaluate(core("approve_roadmap", { projectId: studioId, input: { link: { kind: "new" }, repoPath: dataDir, claudeAccess: "report" } }));
  const built = await page.evaluate(core("get_studio_project", { id: studioId }));
  check("approval links a new Proyek, the folder, and Claude's access", built.stage === "build" && built.projectId !== null && built.claudeAccess === "report", built.stage);
  await page.evaluate(openStudioDoc("Bangun"));
  await waitFor(page, "[...document.querySelectorAll('button')].some((b) => b.textContent.trim() === 'Mulai CP-01')", "Pantau");
  await page.evaluate(clickButton("Mulai CP-01"));
  await waitFor(page, "[...document.querySelectorAll('button')].some((b) => b.textContent.trim() === 'Kerjakan sendiri')", "CP-01 ready");
  await page.evaluate(clickButton("Buat prompt implementasi"));
  await waitFor(page, "!!document.querySelector('textarea[readonly]')", "the prompt");
  const promptText = await page.evaluate("document.querySelector('textarea[readonly]').value");
  check("the prompt names the project, the folder, and CP-01", promptText.includes(`/hub-connect ${studioId} CP-01`) && promptText.includes(dataDir), promptText.split(String.fromCharCode(10))[0]);
  // R4: "Mandiri", the whole of it in one text.
  await page.evaluate("[...document.querySelectorAll('input[type=radio]')].find((r) => r.value === 'standalone').click()");
  await waitFor(page, "document.querySelector('textarea[readonly]').value.includes('tanpa Hub dan tanpa MCP')", "the standalone prompt");
  const standaloneText = await page.evaluate("document.querySelector('textarea[readonly]').value");
  check(
    "the standalone prompt carries the checkpoint, the stack, and the PRD",
    standaloneText.includes("## Checkpoint CP-01") && standaloneText.includes("Backend: Pilihan backend") && standaloneText.includes("## Persyaratan keamanan") && !standaloneText.includes("/hub-connect"),
  );
  const link = await page.evaluate(core("studio_link", { projectId: built.projectId }));
  check("the Proyek knows CP-01 is ready", link?.activeCode === "CP-01" && link.activeStatus === "ready", JSON.stringify(link));

  // Claude Code through the real bridge (an MCP client over stdio, as Claude Code is): switched
  // on in Hub, connected from the repository folder, CP-01 started and reported. Pantau shows each
  // step as it happens, without a reload.
  await page.evaluate(core("update_settings", { patch: { claudeEnabled: true } }));
  for (let i = 0; i < 25 && !existsSync(join(dataDir, "claude-code.json")); i += 1) await sleep(200);
  check("switching Claude Code on writes its own token file", existsSync(join(dataDir, "claude-code.json")) && existsSync(join(dataDir, "api.json")));
  const claude = mcp(dataDir, ["--no-start"]);
  await claude.start();
  const seen = await claude.tool("hub_status");
  check("Claude Code reaches Hub through the bridge", seen?.isError !== true && seen?.structuredContent?.projects === 1, JSON.stringify(seen?.structuredContent ?? seen).slice(0, 160));
  const connected = await claude.tool("hub_connect");
  check("hub_connect finds the project from the folder", connected?.structuredContent?.project?.id === studioId);
  // R5: Pantau's three signals come from what the bridge reported (its hello and each call).
  const claudeGroup = "(document.querySelector('[role=group][aria-labelledby=pantau-claude]')?.textContent ?? '')";
  await waitFor(page, `${claudeGroup}.includes('Terbuka sekarang') && ${claudeGroup}.includes('Menyambung ke proyek, ')`, "Pantau's Claude Code signals");
  check("Pantau says Claude Code is open in the project's folder, and its last call", true);
  await claude.tool("hub_start_checkpoint", { code: "CP-01" });
  await waitFor(page, "[...document.querySelectorAll('[data-status=in_progress]')].length > 0", "CP-01 in progress in Pantau");
  check("Claude's start shows in Pantau without a reload", true);
  const report = await claude.tool("hub_submit_report", {
    checkpoint: "CP-01",
    summary: "Kartu terdaftar tercatat.",
    changes: [{ path: "src/kartu.rs", what: "pembaca kartu" }],
    tests: [{ name: "Unit test kartu", command: "npm test", result: "passed", exit_code: 0, duration_ms: 900, output_tail: "1 passed" }],
  });
  check("Claude's report is accepted", report?.isError !== true, JSON.stringify(report?.structuredContent ?? report).slice(0, 160));
  await waitFor(page, "document.body.textContent.includes('Laporan CP-01 menunggu tinjauanmu.')", "the report waiting in Pantau");
  check("the report waits for the owner in Pantau without a reload", true);
  await waitFor(page, "!!document.querySelector('[aria-label=\"1 hal menunggu kamu\"]')", "the sidebar count");
  check("the sidebar counts what Claude left waiting", true);
  // A refusal shows in Pantau's Claude line (shown while a checkpoint is still open).
  const outside = mcp(join(dataDir, ".."), ["--no-start"]);
  await outside.start();
  const refusedConnect = await outside.tool("hub_connect", { project_id: studioId });
  check("a bridge outside the repository folder is refused", refusedConnect?.isError === true && refusedConnect?.structuredContent?.error?.field === "folder");
  await outside.goodbye();
  await waitFor(page, `${claudeGroup}.includes('Menyambung ke proyek gagal') && ${claudeGroup}.includes('Claude Code tidak dibuka di folder repo proyek ini.')`, "the refusal in Pantau");
  check("Pantau says why a connection was refused", true);
  console.log(`screenshot: ${await page.screenshot("studio-claude")}`);
  console.log(`screenshot: ${await page.screenshot("studio-pantau")}`);
  // The owner approves the report in Pantau: the idea has become an approved checkpoint, and
  // Claude reads it as done; nothing waits any more.
  await page.evaluate(clickButton("Setujui checkpoint"));
  await waitFor(page, "document.body.textContent.includes('CP-01 disetujui.')", "the approval in Pantau");
  await waitFor(page, "document.body.textContent.includes('Semua checkpoint selesai.')", "Pantau's last line");
  const doneLink = await page.evaluate(core("studio_link", { projectId: built.projectId }));
  check("the owner's approval completes CP-01 and the Proyek knows", doneLink?.completed === 1 && doneLink.total === 1, JSON.stringify(doneLink));
  const read = await claude.tool("hub_get_checkpoint", { code: "CP-01" });
  check("Claude reads CP-01 as completed", read?.structuredContent?.checkpoint?.status === "completed", JSON.stringify(read?.structuredContent?.checkpoint?.status ?? read).slice(0, 200));
  await waitFor(page, "!document.querySelector('[aria-label=\"1 hal menunggu kamu\"]')", "the sidebar count gone");
  check("nothing waits for the owner after the approval", true);
  console.log(`screenshot: ${await page.screenshot("studio-approved")}`);
  await claude.goodbye();
  const afterGoodbye = await page.evaluate(core("claude_signals", { projectId: studioId }));
  check(
    "the bridge says goodbye when Claude Code closes it, and Hub keeps its calls",
    afterGoodbye?.session?.state === "closed" && afterGoodbye.open === 0 && afterGoodbye.activity?.tool === "hub_get_checkpoint" && afterGoodbye.session.calls >= 5,
    JSON.stringify({ state: afterGoodbye?.session?.state, open: afterGoodbye?.open, calls: afterGoodbye?.session?.calls, activity: afterGoodbye?.activity }),
  );
  // Claude's desktop app may start the bridge outside the session's folder; Claude Code names
  // the folder in CLAUDE_PROJECT_DIR and the bridge goes by it.
  const desktop = mcp(join(dataDir, ".."), ["--no-start"], join(dataDir, "vault"));
  await desktop.start();
  const named = await desktop.tool("hub_connect");
  check("a bridge started elsewhere connects by the folder Claude Code names", named?.structuredContent?.project?.id === studioId, JSON.stringify(named?.structuredContent ?? named).slice(0, 160));
  await desktop.goodbye();

  await stopGemini();
  // Flow 12: with Gemini gone, Claude Code still reaches Hub.
  const offline = mcp(dataDir, ["--no-start"]);
  await offline.start();
  const stillThere = await offline.tool("hub_status");
  check("without Gemini the bridge still answers", stillThere?.isError !== true && stillThere?.structuredContent?.projects === 1, JSON.stringify(stillThere?.structuredContent ?? stillThere).slice(0, 160));
  await offline.goodbye();
  await waitFor(page, "!!document.querySelector('#studio-input')", "the chat input");
  await page.evaluate(typeInto("#studio-input", "Siswa"));
  await sleep(200);
  await page.evaluate("document.querySelector('#studio-input').form.querySelector('button[type=submit]').click()");
  await waitFor(page, "document.body.textContent.includes('Tidak ada koneksi ke Google.')", "the failed reply", 15_000);
  check("without Gemini a reply fails plainly and offers Coba lagi", await page.evaluate("[...document.querySelectorAll('button')].some((b) => b.textContent.trim() === 'Coba lagi')"));
  await page.evaluate("location.hash = '#/pengaturan/ai'");
  await waitFor(page, "[...document.querySelectorAll('button')].some((b) => b.textContent.trim() === 'Uji koneksi')", "Pengaturan again");
  await page.evaluate(clickButton("Uji koneksi"));
  await waitFor(page, "document.body.textContent.includes('Tidak ada koneksi ke Google.')", "the offline outcome");
  check("without Gemini, Uji koneksi says offline", true);
  await page.evaluate("location.hash = '#/'");
  await waitFor(page, "/^(Selamat|Hari ditutup)/.test(document.querySelector('h1')?.textContent ?? '')", "Hari Ini without Gemini");
  check("Hub works on without Gemini", true);
  await page.evaluate(core("gemini_delete_key"));
  check("the test key is removed from Credential Manager", (await page.evaluate(core("gemini_status"))).hasKey === false);

  // ULTRON through the local API: a task it adds is in Tugas at once, with its crimson mark.
  const api = JSON.parse(readFileSync(join(dataDir, "api.json"), "utf8"));
  check("api.json names the test port", api.url === `http://127.0.0.1:${apiPort}` && api.token.length === 64, api.url);
  const status = await page.evaluate(core("get_api_status"));
  check("the API runs on the test port", status.running && status.port === apiPort, JSON.stringify(status));
  const made = await fetch(`${api.url}/v1/tasks`, {
    method: "POST",
    headers: { Authorization: `Bearer ${api.token}`, "Content-Type": "application/json" },
    body: JSON.stringify({ title: "Kirim laporan PKL", doDate: isoDay(new Date()) }),
  });
  check("ULTRON's request is accepted", made.status === 201, String(made.status));
  const refused = await fetch(`${api.url}/v1/tasks`, { headers: { Authorization: `Bearer ${api.token}`, Origin: "https://evil.example" } });
  check("a web page's request is refused", refused.status === 403, String(refused.status));
  // An address from before R6 still works: Tugas leads to every open task in Proyek & Tugas.
  await page.evaluate("location.hash = '#/tugas'");
  await waitFor(
    page,
    `[...document.querySelectorAll('li')].some((li) => li.textContent.includes('Kirim laporan PKL') && li.textContent.includes('oleh ULTRON'))`,
    "ULTRON's task with its mark",
  );
  check("ULTRON's task shows with its mark", true);
  console.log(`screenshot: ${await page.screenshot("ultron-task")}`);

  // P1: Pengaturan in five parts. Its old address opens the first part, and the sidebar slides to
  // the parts; Kembali returns to the page before (Proyek & Tugas, opened just above).
  await page.evaluate("location.hash = '#/pengaturan'");
  await waitFor(page, "document.querySelector('h1')?.textContent === 'Tampilan' && location.hash === '#/pengaturan/tampilan'", "Pengaturan's first part");
  const sides = await page.evaluate("({ mode: document.querySelector('aside [data-mode]')?.dataset.mode, parts: [...document.querySelectorAll('aside [data-pane=settings] nav a')].map((a) => a.textContent.trim()).join(','), mainInert: document.querySelector('aside [data-pane=main]')?.hasAttribute('inert') })");
  check(
    "Pengaturan opens on its first part and the sidebar shows its parts",
    sides.mode === "settings" && sides.parts === "Tampilan,Hari & kebiasaan,Integrasi AI,ULTRON,Sistem & data" && sides.mainInert === true,
    JSON.stringify(sides),
  );
  await page.evaluate(`[...document.querySelectorAll('input[name=ink]')].find(i => i.value === 'hijau').click()`);
  await waitFor(page, "document.documentElement.dataset.ink === 'hijau'", "the new ink");
  check("Tampilan changes the ink", true);
  // P2: a sample of Studio's sound in the real window: notes are made, then the audio rests.
  await page.evaluate(`(() => {
    const base = BaseAudioContext.prototype;
    const made = base.createOscillator;
    window.__hubNotes = { count: 0, context: null };
    base.createOscillator = function (...args) {
      window.__hubNotes.count += 1;
      window.__hubNotes.context = this;
      return made.apply(this, args);
    };
  })()`);
  await page.evaluate(`[...document.querySelectorAll('[role=group][aria-labelledby=sound-try] button')].find((b) => b.textContent.trim() === 'Studio').click()`);
  await sleep(300);
  const playing = await page.evaluate("({ notes: window.__hubNotes.count, state: window.__hubNotes.context?.state })");
  check("Coba suara plays Studio's sound", playing.notes > 0 && playing.state === "running", JSON.stringify(playing));
  await waitFor(page, "window.__hubNotes.context?.state === 'suspended'", "the audio resting", 6000);
  check("the audio rests once the sound has rung out", true);
  await page.evaluate("location.hash = '#/pengaturan/ultron'");
  await waitFor(page, `document.body.textContent.includes('Aktif di 127.0.0.1:${apiPort}.')`, "the API's state in Pengaturan");
  check("ULTRON says where the API runs", true);
  await page.evaluate("location.hash = '#/pengaturan/sistem'");
  await waitFor(page, "[...document.querySelectorAll('dl')].some((dl) => dl.querySelector('dt')?.textContent === 'Folder data')", "Sistem & data");
  const info = await page.evaluate("[...document.querySelectorAll('dl')].find((dl) => dl.querySelector('dt')?.textContent === 'Folder data')?.querySelector('code')?.textContent");
  check("Sistem & data shows the data folder", info === dataDir, info);
  // R5: Pengaturan shows every session; the last call was the bridge without Gemini.
  await page.evaluate("location.hash = '#/pengaturan/ai'");
  await waitFor(page, "(document.querySelector('[role=group][aria-label=\"Status Claude Code\"]')?.textContent ?? '').includes('Mengecek Hub, ')", "Claude Code's signals in Pengaturan");
  check("Pengaturan shows Claude Code's last call", true);
  console.log(`screenshot: ${await page.screenshot("settings")}`);
  await page.evaluate("[...document.querySelectorAll('aside button')].find((b) => b.textContent.trim() === 'Kembali').click()");
  await waitFor(page, "location.hash.startsWith('#/proyek') && document.querySelector('aside [data-mode]')?.dataset.mode === 'main'", "Kembali to the page before");
  check("Kembali returns to the page before Pengaturan", true);

  page.close();

  // Restart: what was saved must come back.
  stop(child);
  await sleep(800);
  child = launch();
  page = await attach();
  await waitFor(page, "!!document.querySelector('h1')", "Hari Ini after restart");
  const after = await page.evaluate(`({
    heading: document.querySelector('h1').textContent,
    ink: document.documentElement.dataset.ink,
    checked: document.querySelector('[aria-label="Batalkan selesai: THM: Nmap Live Host Discovery"]')?.getAttribute('aria-pressed'),
    trail: [...document.querySelectorAll('section ul li')].map(li => li.textContent).find(t => t.includes('menutup hari')),
  })`);
  check("ink survives a restart", after.ink === "hijau", after.ink);
  check("the check survives a restart", after.checked === "true");
  check("the closed day survives a restart", after.heading.startsWith("Hari ditutup."), after.heading);
  // Read from Jejak itself: Hari Ini lists only the latest few rows, and Studio adds many.
  const trailAfter = await page.evaluate(core("get_trail", { query: { limit: 200 } }));
  check("Jejak kept the close after a restart", trailAfter.items.some((item) => item.verb === "closed"), `${trailAfter.items.length} rows`);
  check("Hari Ini still reads its recent rows after Studio wrote some", after.heading.length > 0 && !(await page.evaluate("document.body.textContent.includes('tidak bisa dimuat')")));
  // Flow 3: the conversation (and the note Gemini made) came back after the restart.
  const keptMessages = await page.evaluate(core("studio_messages", { projectId: studioId }));
  const keptLedger = await page.evaluate(core("studio_ledger", { projectId: studioId }));
  check("the Studio conversation survives a restart", keptMessages.some((m) => m.text === "Absensi lab pakai kartu") && keptMessages.some((m) => m.text.includes("Aku tangkap idenya")) && keptLedger.some((item) => item.statement === "Siswa lab TJKT"), `${keptMessages.length} messages`);
  // Memory in this fresh window: Hari Ini, then the Studio project (idea to an approved checkpoint,
  // Pantau shown), then the window hidden on Studio. Against Hub's target for an open window (under
  // 180 MB); the debug port is on for both, so the difference is what Studio costs.
  // scripts/measure.ps1 has the numbers without the port.
  await sleep(8000);
  const todayOpen = memoryMB(child.pid);
  await page.evaluate(`location.hash = '#/studio/${studioId}'`);
  await waitFor(page, "!!document.querySelector('#studio-input')", "the Studio project after the restart");
  await sleep(8000);
  const studioOpen = memoryMB(child.pid);
  check("memory with Studio open stays under 180 MB", studioOpen > 0 && studioOpen < 180, `${studioOpen} MB (Hari Ini ${todayOpen} MB)`);
  hideWindow(child.pid);
  await sleep(10_000);
  const studioHidden = memoryMB(child.pid);
  check("memory with Studio hidden is measured", studioHidden > 0, `${studioHidden} MB`);
  page.close();

  // With Hub stopped, the bridge starts it into the tray and answers (Claude Code opened first).
  stop(child);
  await sleep(1500);
  const waking = mcp(dataDir);
  await waking.start();
  const woke = await waking.tool("hub_status");
  check("the bridge starts a stopped Hub and answers", woke?.isError !== true && woke?.structuredContent?.projects === 1, JSON.stringify(woke?.structuredContent ?? woke).slice(0, 160));
  waking.close();
  stopTestHubs();
} catch (error) {
  check("smoke ran to the end", false, error.stack ?? error.message);
} finally {
  stop(child);
}

const failed = results.filter((r) => !r.ok).length;
console.log(`${results.length - failed}/${results.length} passed`);
process.exit(failed ? 1 : 0);
