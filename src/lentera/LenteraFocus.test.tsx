import { QueryClientProvider } from "@tanstack/react-query";
import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";

import { BackendContext } from "../data/backend";
import type { Command } from "../data/backend";
import { createMockBackend } from "../data/mock";
import type { MockBackend } from "../data/mock";
import { createQueryClient } from "../data/queries";
import { SoundProvider } from "../sound/SoundProvider";
import { a11yProblems } from "../test/a11y";
import { LenteraWindow } from "./LenteraWindow";

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

/** Lentera's window; `forBar: false` is a window Hub built for a session's capsule. */
function renderLentera(options: { forBar?: boolean; backend?: MockBackend } = {}) {
  const backend = options.backend ?? createMockBackend();
  if (options.forBar === false) {
    const call = backend.call;
    backend.call = <T,>(command: Command, args?: Record<string, unknown>) =>
      command === "take_lentera_open" ? Promise.resolve(false as T) : call<T>(command, args);
  }
  const played: string[] = [];
  render(
    <BackendContext.Provider value={backend}>
      <QueryClientProvider client={createQueryClient()}>
        <SoundProvider engine={{ play: (cue, place) => void played.push(`${cue}@${place}`), setVolume: () => undefined }} place="capture">
          <LenteraWindow />
        </SoundProvider>
      </QueryClientProvider>
    </BackendContext.Provider>,
  );
  return { backend, stage: screen.getByRole("main", { name: "Lentera" }), played };
}

const capsuleButton = () => screen.getByRole("button", { name: /Buka Lentera$/ });
/** The capsule's own controls (the bar's card, inert while folded, has the same names). */
const inCapsule = () => within(capsuleButton().parentElement!);
const status = () => screen.getByRole("status");

describe("Sesi Fokus in Lentera's window", () => {
  it("starts from the bar: the card shows the block, and Esc folds the bar into the session's capsule", async () => {
    const user = userEvent.setup();
    const { backend, stage, played } = renderLentera();
    await waitFor(() => expect(stage).toHaveAttribute("data-view", "bar"));
    await user.click(screen.getByRole("button", { name: /Mulai sesi fokus/ }));
    const card = await screen.findByRole("article", { name: "Sesi fokus" });
    expect(within(card).getByText("25:00")).toBeInTheDocument();
    expect(card).toHaveTextContent("Fokus · Blok 1 dari 4");
    await waitFor(() => expect(status()).toHaveTextContent("Sesi fokus dimulai, 25 menit."));
    expect(played).toContain("sessionStart@capture");
    expect(a11yProblems()).toEqual([]);

    await user.keyboard("{Escape}");
    await waitFor(() => expect(stage).toHaveAttribute("data-view", "capsule"));
    // The window stays for the capsule; the keyboard goes back to where the owner was.
    expect(backend.calls).toContain("return_lentera_focus");
    expect(backend.calls.some((call) => call.startsWith("hide_lentera"))).toBe(false);
    expect(capsuleButton()).toHaveAccessibleName(/^Fokus 2[45]:\d\d\. Buka Lentera$/);
  });

  it("a window built for a session shows the capsule, without opening the bar", async () => {
    const backend = createMockBackend();
    await backend.call("start_focus", { input: {} });
    const { stage } = renderLentera({ backend, forBar: false });
    await waitFor(() => expect(stage).toHaveAttribute("data-view", "capsule"));
    expect(capsuleButton()).toHaveAccessibleName(/^Fokus 2[45]:\d\d\. Buka Lentera$/);
    expect(inCapsule().getByRole("button", { name: "Jeda" })).toBeInTheDocument();
  });

  it("the capsule's one action pauses the block, and then resumes it", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend();
    await backend.call("start_focus", { input: {} });
    const { stage } = renderLentera({ backend, forBar: false });
    await waitFor(() => expect(stage).toHaveAttribute("data-view", "capsule"));
    await user.click(inCapsule().getByRole("button", { name: "Jeda" }));
    await waitFor(() => expect(capsuleButton()).toHaveAccessibleName(/^Dijeda 2[45]:\d\d\. Buka Lentera$/));
    expect(status()).toHaveTextContent("Sesi fokus dijeda.");
    await user.click(inCapsule().getByRole("button", { name: "Lanjutkan" }));
    await waitFor(() => expect(inCapsule().getByRole("button", { name: "Jeda" })).toBeInTheDocument());
    expect(status()).toHaveTextContent("Sesi fokus dilanjutkan.");
  });

  it("Selipkan tucks the capsule away until the next phase: the window hides", async () => {
    const user = userEvent.setup();
    const { backend, stage } = renderLentera();
    await waitFor(() => expect(stage).toHaveAttribute("data-view", "bar"));
    await user.click(screen.getByRole("button", { name: /Mulai sesi fokus/ }));
    await user.click(await screen.findByRole("button", { name: /Selipkan/ }));
    await waitFor(() => expect(stage).toHaveAttribute("data-view", "hidden"));
    await waitFor(() => expect(backend.calls).toContain('hide_lentera {"giveBack":true}'));
    expect(backend.calls).toContain('tuck_focus {"tucked":true}');
  });

  it("Hentikan ends the session; with nothing left to show the window hides", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend();
    await backend.call("start_focus", { input: {} });
    const { stage, played } = renderLentera({ backend, forBar: false });
    await waitFor(() => expect(stage).toHaveAttribute("data-view", "capsule"));
    await user.click(capsuleButton());
    await waitFor(() => expect(stage).toHaveAttribute("data-view", "bar"));
    await user.click(within(screen.getByRole("article", { name: "Sesi fokus" })).getByRole("button", { name: /Hentikan/ }));
    await waitFor(() => expect(screen.queryByRole("article", { name: "Sesi fokus" })).not.toBeInTheDocument());
    expect(status()).toHaveTextContent("Sesi fokus selesai.");
    expect(played).toContain("sessionStop@capture");
    // The bar is still open (the owner is in it); closed, the window hides.
    await user.keyboard("{Escape}");
    await waitFor(() => expect(backend.calls.some((call) => call.startsWith("hide_lentera"))).toBe(true));
  });

  it("a block that ends is said and heard once, and its break counts down", async () => {
    // A minute lasts a second here: a 5-minute block ends in 5 s (so the break is said as 0 minutes).
    const backend = createMockBackend({ settings: { focusMinutes: 5, breakMinutes: 1 }, focusSecondsPerMinute: 1 });
    await backend.call("start_focus", { input: {} });
    const { played } = renderLentera({ backend, forBar: false });
    await waitFor(() => expect(capsuleButton()).toHaveAccessibleName(/^Fokus 0:0[45]\. Buka Lentera$/));
    await waitFor(() => expect(status()).toHaveTextContent(/^Blok selesai. Istirahat/), { timeout: 7000 });
    expect(played.filter((cue) => cue === "complete@capture")).toHaveLength(1);
    expect(inCapsule().getByRole("button", { name: "Lewati istirahat" })).toBeInTheDocument();
    await waitFor(() => expect(status()).toHaveTextContent("Istirahat selesai."), { timeout: 3000 });
    await act(() => backend.call("next_focus"));
    await waitFor(() => expect(inCapsule().getByRole("button", { name: "Jeda" })).toBeInTheDocument());
  }, 15_000);
});
