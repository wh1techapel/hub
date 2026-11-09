import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";

import { createMockBackend } from "../data/mock";
import { renderApp } from "../test/renderApp";

afterEach(() => {
  vi.useRealTimers();
});

// P2: what the app asks the engine to play, as "cue@place" (the engine itself is tested apart).
describe("Hub's sounds where they happen", () => {
  it("a focus task checked is heard, and the last one of the day is a small celebration", async () => {
    const user = userEvent.setup();
    const { played } = renderApp();
    const focus = await screen.findByRole("region", { name: "Fokus" });
    const open = () => within(focus).getAllByRole("button", { name: /^Tandai selesai: / });
    await waitFor(() => expect(open().length).toBe(2));
    await user.click(open()[0]!);
    await waitFor(() => expect(played).toContain("complete@today"));
    await waitFor(() => expect(open().length).toBe(1));
    await user.click(open()[0]!);
    await waitFor(() => expect(played.at(-1)).toBe("hero@today"));
  });

  it("switches, tabs, and choices sound by themselves, in the place they are in", async () => {
    const user = userEvent.setup();
    const { played } = renderApp({ path: "/pengaturan/sistem" });
    const autostart = await screen.findByRole("switch", { name: "Jalan saat Windows menyala" });
    await user.click(autostart);
    expect(played).toContain("off@settings");
    await user.click(screen.getByRole("link", { name: "Tampilan" }));
    await user.click(await screen.findByRole("radio", { name: "Gelap" }));
    expect(played).toContain("select@settings");
    // Typing makes no sound.
    const before = played.length;
    await user.click(screen.getByRole("link", { name: "Hari & kebiasaan" }));
    await user.type(await screen.findByLabelText("Nama di sapaan"), "x");
    expect(played.length).toBe(before);
  });

  it("Pengaturan has the switch, the volume (saved when let go), and a sample of each place", async () => {
    const user = userEvent.setup();
    const { played, backend } = renderApp({ path: "/pengaturan/tampilan" });
    const section = await screen.findByRole("region", { name: "Suara" });
    expect(within(section).getByRole("switch", { name: "Efek suara" })).toHaveAttribute("aria-checked", "true");
    const volume = within(section).getByLabelText("Volume");
    expect(volume).toHaveValue("30");
    // Dragged through 35 to 40: saved once, when the owner stops.
    fireEvent.change(volume, { target: { value: "35" } });
    fireEvent.change(volume, { target: { value: "40" } });
    expect(within(section).getByText("40%")).toBeInTheDocument();
    await waitFor(() => expect(backend.calls).toContain('update_settings {"patch":{"soundVolume":40}}'), { timeout: 2000 });
    expect(backend.calls.filter((call) => call.includes("soundVolume")).length).toBe(1);

    await user.click(within(section).getByRole("button", { name: "Studio" }));
    expect(played).toContain("complete@studio");
    await waitFor(() => expect(played).toContain("hero@studio"));
    await user.click(within(section).getByRole("switch", { name: "Efek suara" }));
    await waitFor(() => expect(within(section).getByRole("switch", { name: "Efek suara" })).toHaveAttribute("aria-checked", "false"));
    expect(within(section).getByLabelText("Volume")).toBeDisabled();
  });

  it("with sounds off nothing plays, but Coba still lets the owner hear one", async () => {
    const user = userEvent.setup();
    const { played } = renderApp({ path: "/pengaturan/tampilan", backend: createMockBackend({ settings: { soundEnabled: false } }) });
    await user.click(await screen.findByRole("radio", { name: "Gelap" }));
    expect(played).toEqual([]);
    await user.click(within(screen.getByRole("region", { name: "Suara" })).getByRole("button", { name: "Belajar" }));
    expect(played).toContain("complete@learning");
  });

  it("Belajar: a session's start is heard in the place's own voice", async () => {
    const user = userEvent.setup();
    const { played } = renderApp({ path: "/belajar" });
    const start = (await screen.findAllByRole("button", { name: /^Lanjutkan/ }))[0]!;
    await user.click(start);
    expect(played).toContain("sessionStart@learning");
  });

  it("Catat Cepat's line sounds like paper wherever it is", async () => {
    const user = userEvent.setup();
    const { played } = renderApp({ path: "/proyek?view=inbox" });
    await user.type(await screen.findByRole("textbox", { name: /Tugas/ }), "beli kertas HVS{Enter}");
    await waitFor(() => expect(played).toContain("saved@capture"));
  });
});
