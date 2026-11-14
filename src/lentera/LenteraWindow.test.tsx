import { QueryClientProvider } from "@tanstack/react-query";
import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";

import { BackendContext } from "../data/backend";
import { createMockBackend } from "../data/mock";
import type { MockBackend } from "../data/mock";
import { createQueryClient } from "../data/queries";
import { SoundProvider } from "../sound/SoundProvider";
import { a11yProblems } from "../test/a11y";
import { LenteraWindow, SAVED_MS } from "./LenteraWindow";

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

function renderLentera() {
  const backend: MockBackend = createMockBackend();
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
  const stage = screen.getByRole("main", { name: "Lentera" });
  return { backend, stage, played };
}

const line = () => screen.getByRole("textbox", { name: "Tulis untuk Lentera" });

/** The owner clicks another application: the window loses the keyboard, and keeps it lost. */
function clickElsewhere() {
  const focus = vi.spyOn(document, "hasFocus").mockReturnValue(false);
  fireEvent.blur(window);
  return focus;
}
const called = (backend: MockBackend, call: string) => backend.calls.includes(call);

async function opened() {
  const result = renderLentera();
  await waitFor(() => expect(result.stage).toHaveAttribute("data-view", "bar"));
  await waitFor(() => expect(line()).toHaveFocus());
  return result;
}

describe("Lentera's window", () => {
  it("is shown once drawn and opens as the bar, with the line ready and an honest note without a key", async () => {
    const { backend, stage } = await opened();
    expect(called(backend, "window_ready")).toBe(true);
    expect(screen.getByRole("heading", { level: 1, name: "Lentera" })).toBeInTheDocument();
    const replies = screen.getByRole("region", { name: "Balasan Lentera" });
    // No Gemini key in the demo by default: the line is Catat Cepat's, and the bar says so (CP03).
    expect(replies).toHaveTextContent("Lentera belum terhubung ke Gemini");
    // The capsule is out of reach while the bar is open.
    expect(stage.querySelector("[inert]")).toHaveTextContent("Buka Lentera");
    expect(a11yProblems()).toEqual([]);
  });

  it("Enter saves the line the way Catat Cepat does; the capsule says so, gives the keyboard back, and leaves", async () => {
    const user = userEvent.setup();
    const { backend, stage, played } = await opened();
    await user.type(line(), "beli tinta printer besok{Enter}");
    await waitFor(() => expect(stage).toHaveAttribute("data-view", "capsule"));
    expect(backend.calls.some((call) => call.startsWith("save_capture") && call.includes("beli tinta printer besok"))).toBe(true);
    expect(called(backend, "return_lentera_focus")).toBe(true);
    expect(played).toContain("saved@capture");
    // Said once to a screen reader, and shown in the capsule.
    expect(screen.getByRole("status")).toHaveTextContent("Tugas tersimpan: beli tinta printer");
    expect(screen.getByRole("button", { name: /Tugas tersimpan: beli tinta printer\. Buka Lentera/ })).toBeInTheDocument();
    // After a moment it leaves on its own, without pulling the keyboard anywhere.
    await waitFor(() => expect(stage).toHaveAttribute("data-view", "hidden"), { timeout: SAVED_MS + 1000 });
    await waitFor(() => expect(called(backend, 'hide_lentera {"giveBack":false}')).toBe(true));
  });

  it("Esc clears the line first, then closes and gives the keyboard back", async () => {
    const user = userEvent.setup();
    const { backend, stage } = await opened();
    await user.type(line(), "rapat osis");
    await user.keyboard("{Escape}");
    expect(line()).toHaveValue("");
    expect(stage).toHaveAttribute("data-view", "bar");
    await user.keyboard("{Escape}");
    expect(stage).toHaveAttribute("data-view", "hidden");
    await waitFor(() => expect(called(backend, 'hide_lentera {"giveBack":true}')).toBe(true));
  });

  it("the close button closes from anywhere in the bar", async () => {
    const user = userEvent.setup();
    const { backend, stage } = await opened();
    await user.click(screen.getByRole("button", { name: "Tutup Lentera" }));
    expect(stage).toHaveAttribute("data-view", "hidden");
    await waitFor(() => expect(called(backend, 'hide_lentera {"giveBack":true}')).toBe(true));
  });

  it("a click elsewhere hides an empty bar", async () => {
    const { backend, stage } = await opened();
    clickElsewhere();
    await waitFor(() => expect(stage).toHaveAttribute("data-view", "hidden"));
    await waitFor(() => expect(called(backend, 'hide_lentera {"giveBack":false}')).toBe(true));
  });

  it("a click elsewhere keeps an unsaved line in the capsule; clicking it brings the line back", async () => {
    const user = userEvent.setup();
    const { backend, stage } = await opened();
    await user.type(line(), "rapat osis jumat");
    const focus = clickElsewhere();
    await waitFor(() => expect(stage).toHaveAttribute("data-view", "capsule"));
    // The capsule clicked: the window has the keyboard again.
    focus.mockReturnValue(true);
    expect(backend.calls.some((call) => call.startsWith("hide_lentera"))).toBe(false);
    const capsule = screen.getByRole("button", { name: /Draf: rapat osis jumat\. Buka Lentera/ });
    await user.click(capsule);
    expect(stage).toHaveAttribute("data-view", "bar");
    await waitFor(() => expect(line()).toHaveFocus());
    expect(line()).toHaveValue("rapat osis jumat");
  });

  it("the draft capsule's one action lets the line go and hides Lentera", async () => {
    const user = userEvent.setup();
    const { backend, stage } = await opened();
    await user.type(line(), "rapat osis jumat");
    clickElsewhere();
    await user.click(await screen.findByRole("button", { name: "Buang draf" }));
    expect(stage).toHaveAttribute("data-view", "hidden");
    await waitFor(() => expect(called(backend, 'hide_lentera {"giveBack":true}')).toBe(true));
    // Called again: a fresh line.
    act(() => backend.send("hub://lentera-open", null));
    await waitFor(() => expect(line()).toHaveValue(""));
  });

  it("a blur that does not last (a new window settling its focus) changes nothing", async () => {
    const { backend, stage } = await opened();
    fireEvent.blur(window);
    await new Promise((done) => setTimeout(done, 300));
    expect(stage).toHaveAttribute("data-view", "bar");
    expect(backend.calls.some((call) => call.startsWith("hide_lentera"))).toBe(false);
  });

  it("the shortcut pressed again closes it, and calling it again opens the bar", async () => {
    const { backend, stage } = await opened();
    act(() => backend.send("hub://lentera-close", null));
    expect(stage).toHaveAttribute("data-view", "hidden");
    act(() => backend.send("hub://lentera-open", null));
    expect(stage).toHaveAttribute("data-view", "bar");
    await waitFor(() => expect(line()).toHaveFocus());
  });

  it("follows the owner's light while it lives on, hidden or shown", async () => {
    const { backend } = await opened();
    await act(() => backend.call("update_settings", { patch: { themeMode: "dark" } }));
    await waitFor(() => expect(document.documentElement).toHaveAttribute("data-scheme", "dark"));
    await act(() => backend.call("update_settings", { patch: { themeMode: "light" } }));
    await waitFor(() => expect(document.documentElement).toHaveAttribute("data-scheme", "light"));
  });

  it("Tab stays inside the bar", async () => {
    await opened();
    // The close button is the bar's first control, the line its last.
    const close = screen.getByRole("button", { name: "Tutup Lentera" });
    close.focus();
    expect(fireEvent.keyDown(close, { key: "Tab", shiftKey: true })).toBe(false);
    expect(line()).toHaveFocus();
    const lastKind = within(screen.getByRole("radiogroup")).getAllByRole("radio").at(-1)!;
    lastKind.focus();
    // Tab from inside goes on as usual (only the ends come round).
    expect(fireEvent.keyDown(lastKind, { key: "Tab" })).toBe(true);
  });

  it("says where it takes the mouse: the whole bar while it is open", async () => {
    vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue({ left: 32, top: 120, width: 432, height: 264, right: 464, bottom: 384, x: 32, y: 120, toJSON: () => ({}) });
    const { backend } = await opened();
    await waitFor(() => expect(called(backend, 'set_lentera_shape {"shape":{"x":32,"y":120,"width":432,"height":264,"radius":22}}')).toBe(true));
  });
});
