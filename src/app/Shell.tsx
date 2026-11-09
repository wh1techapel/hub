// The frame around every page: the sidebar (a rail in narrow windows), the top line with today's
// date and the palette, reminders waiting for the owner, and the page itself, which rises in when
// the route changes. It also follows addresses sent by the native side (a notification, the tray).
//
// P1: while Pengaturan is open the sidebar slides over to its parts, with Kembali on top, which
// returns to the page the owner left (Hari Ini when Pengaturan was opened directly).
//
// U2: while Studio is open the sidebar slides over to Studio's history the same way. The history
// folds into a rail (the choice is kept on this computer); in a window that already shows the
// rail, it opens over the page instead and closes with Esc, a click outside, or a new page.

import { Link, Outlet, useRouter, useRouterState } from "@tanstack/react-router";
import { ArrowLeft, PenLine, Search } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import type { RefObject } from "react";

import { useApi } from "../data/api";
import { useBackend } from "../data/backend";
import type { CaptureKind } from "../data/generated/CaptureKind";
import { useStudioProjects } from "../data/queries";
import { SETTINGS_PARTS } from "../pages/settings/parts";
import { placeOf } from "../sound/palette";
import { useSound } from "../sound/SoundProvider";
import { id } from "../strings/id";
import { useNow } from "../time/clock";
import { dateLabel } from "../time/format";
import { Alerts } from "./Alerts";
import { CommandPalette } from "./CommandPalette";
import { DropPools } from "./DropPools";
import { pages } from "./nav";
import type { Page } from "./nav";
import styles from "./shell.module.css";
import { CanvasContext } from "./studioCanvas";
import { StudioSide } from "./StudioSide";

/** Only the app's own addresses are followed: "/", "/proyek?view=inbox", "/proyek/<id>". */
function isAppPath(value: unknown): value is string {
  return typeof value === "string" && /^\/[\w\-/?=&.%]*$/.test(value) && value.length <= 200;
}

/** Which side the sidebar shows: Hub's pages, Studio's history, or Pengaturan's parts. */
type Side = "main" | "studio" | "settings";

export function sideOf(pathname: string): Side {
  if (pathname.startsWith("/pengaturan")) return "settings";
  if (pathname === "/studio" || pathname.startsWith("/studio/")) return "studio";
  return "main";
}

/** The window width under which the sidebar is a rail (`shell.module.css` uses the same). */
const RAIL_QUERY = "(max-width: 1100px)";
/** Where the folded history is remembered: a convenience on this computer, not Hub's data. */
const FOLD_KEY = "hub:studio-folded";

function readFolded(): boolean {
  try {
    return localStorage.getItem(FOLD_KEY) === "1";
  } catch {
    return false;
  }
}

function writeFolded(folded: boolean) {
  try {
    if (folded) localStorage.setItem(FOLD_KEY, "1");
    else localStorage.removeItem(FOLD_KEY);
  } catch {
    // Storage refused: the choice lasts until Hub closes.
  }
}

/** Whether the window is narrow enough for the rail, followed as it changes. */
function useNarrow(): boolean {
  const [narrow, setNarrow] = useState(() => typeof matchMedia === "function" && matchMedia(RAIL_QUERY).matches);
  useEffect(() => {
    if (typeof matchMedia !== "function") return;
    const query = matchMedia(RAIL_QUERY);
    const follow = () => setNarrow(query.matches);
    query.addEventListener("change", follow);
    return () => query.removeEventListener("change", follow);
  }, []);
  return narrow;
}

export function Shell() {
  const [palette, setPalette] = useState<{ startWith: CaptureKind | null } | null>(null);
  const pathname = useRouterState({ select: (state) => state.location.pathname });
  const href = useRouterState({ select: (state) => state.location.href });
  const router = useRouter();
  const backend = useBackend();
  const api = useApi();
  const now = useNow();
  const main = useRef<HTMLElement>(null);
  const sidebar = useRef<HTMLElement>(null);
  const mainPane = useRef<HTMLDivElement>(null);
  const studioPane = useRef<HTMLDivElement>(null);
  const settingsPane = useRef<HTMLDivElement>(null);
  const sideNow = sideOf(pathname);
  const inSettings = sideNow === "settings";
  const inStudio = sideNow === "studio";
  // The sounds follow the place (P2).
  const sound = useSound();
  useEffect(() => {
    sound.setPlace(placeOf(pathname));
  }, [sound, pathname]);
  // The last page outside Pengaturan: where its Kembali returns. The last page outside Studio and
  // Pengaturan: where Studio's Kembali returns.
  const outside = useRef("/");
  const outsideStudio = useRef("/");
  useEffect(() => {
    if (!inSettings) outside.current = href;
    if (sideNow === "main") outsideStudio.current = href;
  }, [inSettings, sideNow, href]);
  // Focus never stays in the side that slid away (a browser lets go of focus inside what becomes
  // inert): when the sidebar changes sides with focus in it, focus goes to the item the side that
  // came in marks as current. That waits for the route to settle: `/pengaturan` alone is sent on to
  // its first part a moment later. Focus elsewhere (the page, the palette) is left alone, and
  // nothing moves when the app first opens.
  const side = useRef<Side>(sideNow);
  const owed = useRef(false);
  useEffect(() => {
    const paneOf: Record<Side, RefObject<HTMLDivElement | null>> = { main: mainPane, studio: studioPane, settings: settingsPane };
    const shown = paneOf[sideNow].current;
    if (side.current !== sideNow) {
      const gone = paneOf[side.current].current;
      side.current = sideNow;
      const active = document.activeElement;
      owed.current = gone !== null && (active === null || active === document.body || gone.contains(active));
    }
    if (!owed.current || !shown) return;
    const current = shown.querySelector<HTMLElement>("[aria-current=page]");
    if (current || pathname !== "/pengaturan") {
      (current ?? shown.querySelector<HTMLElement>("a, button"))?.focus();
      owed.current = false;
    }
  }, [sideNow, pathname]);

  // U2: Studio's history folded into the rail (kept), or opened over the page in a narrow window.
  // U4: it also folds while a project shows a wide document in its canvas, until the owner opens
  // it again; that is not kept.
  const narrow = useNarrow();
  const [folded, setFolded] = useState(readFolded);
  const [peek, setPeek] = useState(false);
  const [canvasWide, setCanvasWide] = useState(false);
  const [canvasUnfolded, setCanvasUnfolded] = useState(false);
  if (!canvasWide && canvasUnfolded) setCanvasUnfolded(false);
  const railed = folded || (canvasWide && !canvasUnfolded);
  const historyOpen = inStudio && (narrow ? peek : !railed);
  // A new page, or leaving Studio, or the window changing size closes what was opened over the page.
  const [peekAt, setPeekAt] = useState({ pathname, narrow });
  if (peekAt.pathname !== pathname || peekAt.narrow !== narrow) {
    setPeekAt({ pathname, narrow });
    if (peek) setPeek(false);
  }
  const unfold = () => {
    if (folded) {
      setFolded(false);
      writeFolded(false);
    }
    if (canvasWide) setCanvasUnfolded(true);
  };
  const toggleHistory = () => {
    if (narrow) setPeek((open) => !open);
    else if (railed) unfold();
    // Folding again beside a wide document only undoes the opening; the kept choice stays.
    else if (canvasWide) setCanvasUnfolded(false);
    else {
      setFolded(true);
      writeFolded(true);
    }
  };
  const searchHistory = () => {
    if (narrow) setPeek(true);
    else unfold();
    requestAnimationFrame(() => document.getElementById("studio-search")?.focus());
  };
  useEffect(() => {
    if (!peek) return;
    const close = (returnFocus: boolean) => {
      setPeek(false);
      // Focus inside what closes goes back to the button that opened it.
      if (returnFocus) requestAnimationFrame(() => sidebar.current?.querySelector<HTMLElement>('[aria-controls="studio-history"]')?.focus());
    };
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !event.defaultPrevented) close(sidebar.current?.contains(document.activeElement) ?? false);
    };
    const onPointer = (event: PointerEvent) => {
      if (!sidebar.current?.contains(event.target as Node)) close(false);
    };
    window.addEventListener("keydown", onKey);
    document.addEventListener("pointerdown", onPointer);
    return () => {
      window.removeEventListener("keydown", onKey);
      document.removeEventListener("pointerdown", onPointer);
    };
  }, [peek]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        setPalette((open) => (open ? null : { startWith: null }));
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  // Where the window was asked to open before this page existed, and where it is sent later.
  useEffect(() => {
    const go = (path: unknown) => {
      if (isAppPath(path)) router.history.push(path);
    };
    void api.takeStartPath().then(go, () => undefined);
    return backend.on("hub://navigate", go);
  }, [api, backend, router]);

  // A new page starts at its top.
  useEffect(() => {
    if (main.current) main.current.scrollTop = 0;
  }, [pathname]);

  const settings = pages.find((page) => page.group === "foot");
  // Studio is a workspace: its home (U3, the field in the window's middle) and a project fill the
  // window and scroll inside themselves.
  const workspace = /^\/studio(\/[^/]+)?$/.test(pathname);
  // What waits for the owner in Studio (Claude's questions, blockers, reports), on Studio's entry.
  const studio = useStudioProjects();
  const waiting = (studio.data ?? []).reduce((sum, project) => sum + project.waiting, 0);

  return (
    <div className={styles.app} data-folded={(inStudio && !narrow && railed) || undefined} data-peek={(inStudio && narrow && peek) || undefined}>
      <aside className={styles.sidebar} aria-label={id.nav.label} ref={sidebar}>
        <div className={styles.wordmark} aria-label={id.app.name}>
          hub<span aria-hidden="true">.</span>
        </div>
        <div className={styles.panes} data-mode={sideNow}>
          <div className={styles.pane} data-pane="main" ref={mainPane} inert={sideNow !== "main"} aria-hidden={sideNow !== "main" || undefined}>
            <button type="button" className={styles.capture} onClick={() => setPalette({ startWith: "task" })} title={id.nav.capture}>
              <PenLine size={17} strokeWidth={1.7} aria-hidden="true" />
              <span>{id.nav.capture}</span>
            </button>
            <nav className={styles.nav}>
              {pages
                .filter((page) => page.group === "main")
                .map((page) => (
                  <NavLink key={page.path} page={page} waiting={page.path === "/studio" ? waiting : 0} />
                ))}
              <hr className={styles.divider} />
              {pages
                .filter((page) => page.group === "more")
                .map((page) => (
                  <NavLink key={page.path} page={page} waiting={0} />
                ))}
            </nav>
            {settings && (
              <div className={styles.foot}>
                <Link
                  to={settings.path}
                  className={styles.item}
                  activeProps={{ className: `${styles.item} ${styles.active}` }}
                  title={settings.label}
                >
                  <settings.icon size={18} strokeWidth={1.6} aria-hidden="true" />
                  <span>{settings.label}</span>
                </Link>
              </div>
            )}
          </div>
          <div className={`${styles.pane} ${styles.studioPane}`} data-pane="studio" ref={studioPane} inert={!inStudio} aria-hidden={!inStudio || undefined}>
            <StudioSide
              projects={studio.data ?? []}
              expanded={historyOpen}
              onBack={() => router.history.push(outsideStudio.current)}
              onToggle={toggleHistory}
              onSearch={searchHistory}
            />
          </div>
          <div className={styles.pane} data-pane="settings" ref={settingsPane} inert={!inSettings} aria-hidden={!inSettings || undefined}>
            <button type="button" className={styles.item} title={id.settings.back} onClick={() => router.history.push(outside.current)}>
              <ArrowLeft size={18} strokeWidth={1.6} aria-hidden="true" />
              <span>{id.settings.back}</span>
            </button>
            <nav className={styles.nav} aria-labelledby="settings-parts-label">
              <p className={styles.paneLabel} id="settings-parts-label">
                {id.settings.title}
              </p>
              {SETTINGS_PARTS.map((part) => (
                <Link
                  key={part.slug}
                  to="/pengaturan/$bagian"
                  params={{ bagian: part.slug }}
                  className={styles.item}
                  activeProps={{ className: `${styles.item} ${styles.active}` }}
                  title={part.label}
                >
                  <part.icon size={18} strokeWidth={1.6} aria-hidden="true" />
                  <span>{part.label}</span>
                </Link>
              ))}
            </nav>
          </div>
        </div>
      </aside>

      <main className={styles.main} ref={main} data-workspace={workspace || undefined}>
        <header className={styles.topbar}>
          <div className={styles.date}>{dateLabel(now)}</div>
          <button type="button" className={styles.paletteHint} onClick={() => setPalette({ startWith: null })}>
            <Search size={15} strokeWidth={1.6} aria-hidden="true" />
            <span>{id.palette.open}</span>
            <kbd>Ctrl K</kbd>
          </button>
        </header>
        <Alerts />
        <div key={pathname} className={styles.page}>
          <CanvasContext.Provider value={setCanvasWide}>
            <Outlet />
          </CanvasContext.Provider>
        </div>
      </main>

      {palette && <CommandPalette startWith={palette.startWith} onClose={() => setPalette(null)} />}
      <DropPools />
    </div>
  );
}

/** One page in the sidebar; Studio carries what waits for the owner. */
function NavLink({ page, waiting }: { page: Page; waiting: number }) {
  return (
    <Link
      to={page.path}
      className={styles.item}
      activeProps={{ className: `${styles.item} ${styles.active}` }}
      activeOptions={{ exact: page.path === "/", includeSearch: false }}
      title={page.label}
    >
      <page.icon size={18} strokeWidth={1.6} aria-hidden="true" />
      <span>{page.label}</span>
      {waiting > 0 && (
        <span className={styles.badge} aria-label={id.nav.waiting(waiting)}>
          {waiting}
        </span>
      )}
    </Link>
  );
}
