import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";

import { createMockBackend } from "../../data/mock";
import { renderApp } from "../../test/renderApp";
import { currentStudioStage, STUDIO_KEY } from "../../test/studioFlow";

type User = ReturnType<typeof userEvent.setup>;

async function atStack(user: User, settings = {}) {
  const backend = createMockBackend({ geminiKey: STUDIO_KEY, settings });
  const app = renderApp({ path: "/studio", backend });
  await user.type(await screen.findByLabelText("Ceritakan idenya"), "Absensi lab pakai kartu{Enter}");
  await waitFor(() => expect(screen.getByRole("button", { name: "Kirim" })).toBeInTheDocument(), { timeout: 3000 });
  await user.click(screen.getByRole("tab", { name: /Tech stack/ }));
  return { ...app, backend };
}

const layer = (name: string) => screen.getByRole("combobox", { name: `Pilihan untuk ${name}` });

// R3: stage 2, the tech stack.
describe("Studio: the tech stack", { timeout: 15_000 }, () => {
  it("picked by the owner: every layer chosen, then approved, and the PRD follows", async () => {
    const user = userEvent.setup();
    const { backend } = await atStack(user);
    await user.click(screen.getByRole("button", { name: /Pilih sendiri/ }));
    expect(await screen.findByText("7 lapisan belum dipilih.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Setujui tech stack" })).toBeDisabled();
    // Picking a layer makes the project stage 2.
    expect(currentStudioStage()).toHaveTextContent(/Tech stack/);
    const choices = ["Web", "React + Vite", "Laravel", "MySQL", "Tidak perlu", "Hosting kampus atau sekolah", "Tidak perlu"];
    const names = ["Platform", "Frontend", "Backend", "Database", "Autentikasi", "Hosting", "Layanan tambahan"];
    for (const [i, name] of names.entries()) {
      await user.type(layer(name), `${choices[i]}{Enter}`);
      await waitFor(() => expect(layer(name)).toHaveValue(choices[i]));
    }
    await waitFor(() => expect(screen.getByRole("button", { name: "Setujui tech stack" })).toBeEnabled());
    expect(backend.calls.filter((c) => c.startsWith("save_studio_stack")).pop()).toContain('"mode":"own"');
    await user.click(screen.getByRole("button", { name: "Setujui tech stack" }));
    const sheet = screen.getByRole("dialog", { name: "Setujui tech stack" });
    expect(within(sheet).getByText("Laravel")).toBeInTheDocument();
    await user.click(within(sheet).getByRole("button", { name: "Setujui dan susun PRD" }));
    expect(await screen.findByText("PRD tersusun.", undefined, { timeout: 3000 })).toBeInTheDocument();
    expect(currentStudioStage()).toHaveTextContent(/PRD dan bangun/);
    expect(backend.calls.some((c) => c.startsWith("approve_studio_stack"))).toBe(true);
  });

  it("recommended by Gemini: explained per layer, changed by the owner, and asked again", async () => {
    const user = userEvent.setup();
    const { backend } = await atStack(user);
    // The profile is empty: said once, with the way to fill it.
    expect(screen.getByText(/Profil developer belum diisi/)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: /Rekomendasikan dengan Lentera/ }));
    expect(await screen.findByText(/Rekomendasi tersusun/, undefined, { timeout: 3000 })).toBeInTheDocument();
    expect(layer("Backend")).toHaveValue("Supabase");
    const backend_ = layer("Backend").closest("li")!;
    expect(within(backend_).getByText("Lentera")).toBeInTheDocument();
    await user.click(within(backend_).getByText("Alasan dan biaya"));
    expect(within(backend_).getByText(/tanpa server sendiri/)).toBeVisible();

    // The owner takes an alternative: theirs now, and Gemini's choice kept as an alternative.
    await user.click(within(backend_).getByRole("button", { name: "Pakai Laravel" }));
    await waitFor(() => expect(layer("Backend")).toHaveValue("Laravel"));
    const changed = layer("Backend").closest("li")!;
    expect(within(changed).getByText("Kamu")).toBeInTheDocument();
    expect(within(changed).getByRole("button", { name: "Pakai Supabase" })).toHaveAttribute("title", "Pilihan sebelumnya");

    // Asked again with words: the current stack goes along.
    await user.type(screen.getByLabelText("Minta Lentera mengubah"), "lebih murah{Enter}");
    await waitFor(() => expect(layer("Hosting")).toHaveValue("VPS sekolah"), { timeout: 3000 });
    expect(backend.calls.some((c) => c.startsWith("recommend_studio_stack") && c.includes('"ask":"lebih murah"'))).toBe(true);
    // Not approved yet: stage 3 waits.
    await user.click(screen.getByRole("tab", { name: /PRD dan bangun/ }));
    expect(screen.getByText(/Setujui tech stack dulu di Tahap 2/)).toBeInTheDocument();
  });

  it("a failed recommendation says why and changes nothing", async () => {
    const user = userEvent.setup();
    await atStack(user, { devKnows: "PHP" });
    expect(screen.queryByText(/Profil developer belum diisi/)).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: /Rekomendasikan dengan Lentera/ }));
    await screen.findByText(/Rekomendasi tersusun/, undefined, { timeout: 3000 });
    await user.type(screen.getByLabelText("Minta Lentera mengubah"), "coba gagal{Enter}");
    expect(await screen.findByText("Rekomendasi tidak tersusun. Tidak ada koneksi ke Google.", undefined, { timeout: 3000 })).toBeInTheDocument();
    expect(layer("Backend")).toHaveValue("Supabase");
  });
});

describe("Pengaturan: the developer profile", () => {
  it("is saved line by line and with its cost", async () => {
    const user = userEvent.setup();
    const { backend } = renderApp({ path: "/pengaturan/ai", backend: createMockBackend({ geminiKey: STUDIO_KEY }) });
    const knows = await screen.findByLabelText("Teknologi yang kukuasai");
    await user.type(knows, "HTML, CSS, PHP");
    await user.tab();
    await waitFor(() => expect(backend.calls.some((c) => c.startsWith("update_settings") && c.includes('"devKnows":"HTML, CSS, PHP"'))).toBe(true));
    await user.click(screen.getByRole("radio", { name: "Murah" }));
    await waitFor(() => expect(screen.getByRole("radio", { name: "Murah" })).toBeChecked());
    expect(backend.calls.some((c) => c.includes('"devCost":"low"'))).toBe(true);
  });
});
