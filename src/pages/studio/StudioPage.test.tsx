import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";

import type { StudioProject } from "../../data/generated/StudioProject";
import { createMockBackend } from "../../data/mock";
import { renderApp } from "../../test/renderApp";

const KEY = "AIzaTestKeyForHubStudio0123456789abcd";

function project(
  name: string,
  stage: StudioProject["stage"],
  waiting = 0,
  projectName: string | null = null,
  stackStatus: StudioProject["stackStatus"] = null,
): StudioProject {
  const at = new Date().toISOString();
  return {
    id: crypto.randomUUID(),
    name,
    idea: `Ide ${name}`,
    stage,
    projectId: projectName ? crypto.randomUUID() : null,
    projectName,
    repoPath: null,
    claudeAccess: "none",
    waiting,
    stackStatus,
    createdAt: at,
    updatedAt: at,
  };
}

describe("Studio", () => {
  it("is in the sidebar and greets with the question", async () => {
    const user = userEvent.setup();
    renderApp({ path: "/" });
    await user.click(await screen.findByRole("link", { name: "Studio" }));
    expect(await screen.findByRole("heading", { level: 1, name: "Apa yang ingin kamu bangun hari ini?" })).toBeInTheDocument();
  });

  it("without a Gemini key it points to Pengaturan", async () => {
    const user = userEvent.setup();
    renderApp({ path: "/studio" });
    expect(await screen.findByText(/Lentera butuh API key dari Google AI Studio/)).toBeInTheDocument();
    await user.click(screen.getByRole("link", { name: "Buka Pengaturan" }));
    // Straight to the part with the key (P1), not to Pengaturan's first part.
    expect(await screen.findByRole("heading", { level: 1, name: "Integrasi AI" })).toBeInTheDocument();
    expect(screen.getByLabelText("API key Gemini")).toBeInTheDocument();
  });

  it("greets by the light of the day, with one field in the middle (U3)", async () => {
    const user = userEvent.setup();
    renderApp({ path: "/studio", backend: createMockBackend({ geminiKey: KEY }) });
    // renderApp pins 12.40: midday.
    expect(await screen.findByText("Selamat siang,", { exact: false })).toHaveTextContent("Selamat siang, Glenvio.");
    const field = await screen.findByLabelText("Ceritakan idenya");
    // The one thing on the page: the cursor starts there.
    await waitFor(() => expect(field).toHaveFocus());
    const send = screen.getByRole("button", { name: "Mulai" });
    expect(send).toBeDisabled();
    // L2: the model, where Gemini puts its own, is a menu named by the model.
    expect(screen.getByRole("button", { name: "Model: 3.8 Flash. Ganti model" })).toHaveAttribute("aria-haspopup", "menu");
    expect(screen.getByText("Percakapan Studio diproses Google. Data Hub lainnya tidak.")).toBeInTheDocument();
    // L3: the glow by the light of the day is decoration only, hidden from assistive technology.
    expect(screen.getByTestId("studio-glow")).toHaveAttribute("aria-hidden", "true");
    expect(screen.queryByText(/Simpan API key-nya dulu/)).not.toBeInTheDocument();

    // Shift+Enter is a new line; the button wakes once there is text.
    await user.type(field, "Kasir kantin{Shift>}{Enter}{/Shift}pakai kartu");
    expect(field).toHaveValue("Kasir kantin\npakai kartu");
    expect(send).toBeEnabled();
  });

  // L2: the chip opens the text models the key can use; the choice is Hub's one setting.
  it("chooses Lentera's model from the chip, by keyboard, for every project (L2)", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend({ geminiKey: KEY });
    renderApp({ path: "/studio", backend });
    const chip = await screen.findByRole("button", { name: "Model: 3.8 Flash. Ganti model" });
    // Google's list is read when the menu first opens, not on every visit.
    expect(backend.calls.some((call) => call.startsWith("gemini_models"))).toBe(false);
    await user.click(chip);
    const menu = screen.getByRole("menu", { name: "Model" });
    expect(chip).toHaveAttribute("aria-expanded", "true");
    // Text models only, newest first, one group per family with its hint said once; the speech
    // and image models are not offered.
    await waitFor(() => expect(within(menu).getAllByRole("menuitemradio")).toHaveLength(5));
    const groups = within(menu).getAllByRole("group");
    expect(groups).toHaveLength(3);
    expect(groups.map((group) => within(group).getAllByRole("menuitemradio").map((option) => option.textContent))).toEqual([
      ["3.8 Flash", "3.7 Flash"],
      ["3.8 Pro Preview", "2.5 Pro"],
      ["3.5 Flash-Lite"],
    ]);
    expect(screen.getByRole("group", { name: "Flash" })).toHaveAccessibleDescription("Seimbang antara cepat dan teliti.");
    expect(screen.getByRole("group", { name: "Pro" })).toHaveAccessibleDescription("Paling teliti. Lebih lambat, kuota gratisnya lebih kecil.");
    expect(screen.getByRole("group", { name: "Flash-Lite" })).toHaveAccessibleDescription("Paling cepat dan hemat kuota.");
    // Each family's hint is said once, not on every row.
    expect(within(menu).getAllByText("Seimbang antara cepat dan teliti.")).toHaveLength(1);
    const options = within(menu).getAllByRole("menuitemradio");
    expect(options[0]).toHaveAttribute("aria-checked", "true");
    // The focus starts on the chosen model; the arrows move, Escape closes back to the chip.
    expect(options[0]).toHaveFocus();
    await user.keyboard("{ArrowDown}");
    expect(options[1]).toHaveFocus();
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
    expect(chip).toHaveFocus();
    // Opened by the keyboard, chosen with Enter.
    await user.keyboard("{ArrowDown}");
    await waitFor(() => expect(within(screen.getByRole("menu", { name: "Model" })).getAllByRole("menuitemradio")[0]).toHaveFocus());
    // Down through the groups to the last model, 3.5 Flash-Lite.
    await user.keyboard("{ArrowDown}{ArrowDown}{ArrowDown}{ArrowDown}{Enter}");
    await waitFor(() => expect(backend.calls).toContain('update_settings {"patch":{"geminiModel":"gemini-3.5-flash-lite"}}'));
    expect(await screen.findByRole("button", { name: "Model: 3.5 Flash-Lite. Ganti model" })).toHaveFocus();
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
    expect(backend.calls.filter((call) => call.startsWith("gemini_models"))).toHaveLength(1);
  });

  it("keeps the projects in the sidebar, not on the page (U3)", async () => {
    const backend = createMockBackend({
      geminiKey: KEY,
      studioProjects: [project("Absensi lab", "build", 2, "Absensi"), project("Bank soal", "prd"), project("Kantin", "explore")],
    });
    renderApp({ path: "/studio", backend });
    const history = await screen.findByRole("navigation", { name: "Riwayat Studio" });
    expect(await within(history).findAllByRole("link")).toHaveLength(3);
    const main = screen.getByRole("main");
    expect(within(main).queryByRole("link", { name: /Absensi lab|Bank soal|Kantin/ })).not.toBeInTheDocument();
    expect(within(main).queryByText(/Proyek Studio/)).not.toBeInTheDocument();
  });
});
