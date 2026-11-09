import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";

import { createMockBackend } from "../../data/mock";
import { renderApp } from "../../test/renderApp";

const KEY = "AIzaTestKeyForHubStudio0123456789abcd";

describe("Integrasi AI: Google Gemini", () => {
  it("a key is typed hidden, can be shown, and is saved without ever coming back to the page", async () => {
    const user = userEvent.setup();
    const { backend } = renderApp({ path: "/pengaturan/ai" });
    const field = await screen.findByLabelText("API key Gemini");
    expect(field).toHaveAttribute("type", "password");
    await user.type(field, KEY);
    await user.click(screen.getByRole("button", { name: "Tampilkan key" }));
    expect(field).toHaveAttribute("type", "text");
    expect(screen.getByRole("button", { name: "Sembunyikan key" })).toHaveAttribute("aria-pressed", "true");
    await user.click(screen.getByRole("button", { name: "Simpan" }));
    expect(await screen.findByText("Tersimpan di Windows Credential Manager.")).toBeInTheDocument();
    expect(screen.queryByLabelText("API key Gemini")).not.toBeInTheDocument();
    expect(backend.calls).toContain(`gemini_save_key {"key":"${KEY}"}`);
    // Nothing the page reads afterwards carries the key.
    const answers = backend.calls.filter((call) => !call.startsWith("gemini_save_key"));
    expect(answers.join("\n")).not.toContain(KEY);
    expect(document.body.textContent).not.toContain(KEY);
  });

  it("a key of the wrong shape is refused next to the field", async () => {
    const user = userEvent.setup();
    renderApp({ path: "/pengaturan/ai" });
    const field = await screen.findByLabelText("API key Gemini");
    await user.type(field, "pendek");
    await user.click(screen.getByRole("button", { name: "Simpan" }));
    expect(await screen.findByText("Key terlalu pendek atau terlalu panjang. Salin ulang dari Google AI Studio.")).toBeInTheDocument();
    expect(field).toHaveAttribute("aria-invalid", "true");
  });

  it("removing the key asks once", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend({ geminiKey: KEY });
    renderApp({ path: "/pengaturan/ai", backend });
    await user.click(await screen.findByRole("button", { name: "Hapus" }));
    expect(screen.getByText(/Lentera tidak bisa bekerja sampai key baru disimpan/)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Hapus key" }));
    expect(await screen.findByLabelText("API key Gemini")).toBeInTheDocument();
    expect(backend.calls).toContain("gemini_delete_key");
  });

  it("Uji koneksi names the outcome in words", async () => {
    const user = userEvent.setup();
    renderApp({ path: "/pengaturan/ai", backend: createMockBackend({ geminiKey: KEY }) });
    await user.click(await screen.findByRole("button", { name: "Uji koneksi" }));
    expect(await screen.findByText("Terhubung. Gemini 3.8 Flash siap dipakai.")).toBeInTheDocument();
  });

  it("a refused key and an empty quota are told apart, with Google's words and wait", async () => {
    const user = userEvent.setup();
    renderApp({ path: "/pengaturan/ai", backend: createMockBackend({ geminiKey: `${KEY}invalid` }) });
    await user.click(await screen.findByRole("button", { name: "Uji koneksi" }));
    expect(await screen.findByText(/API key salah atau tidak berlaku/)).toBeInTheDocument();
    expect(screen.getByText("Kata Google: API key not valid.")).toBeInTheDocument();
  });

  it("quota says how long to wait", async () => {
    const user = userEvent.setup();
    renderApp({ path: "/pengaturan/ai", backend: createMockBackend({ geminiKey: `${KEY}quota` }) });
    await user.click(await screen.findByRole("button", { name: "Uji koneksi" }));
    expect(await screen.findByText(/Kuota atau batas pemakaian habis/)).toBeInTheDocument();
    expect(screen.getByText("Google meminta menunggu 30 detik.")).toBeInTheDocument();
  });

  it("without a key the test waits and Google is not asked for models", async () => {
    const { backend } = renderApp({ path: "/pengaturan/ai" });
    expect(await screen.findByRole("button", { name: "Uji koneksi" })).toBeDisabled();
    expect(backend.calls.some((call) => call.startsWith("gemini_models"))).toBe(false);
  });

  it("with a key the model is picked from Google's list and saved", async () => {
    const user = userEvent.setup();
    const backend = createMockBackend({ geminiKey: KEY });
    renderApp({ path: "/pengaturan/ai", backend });
    const select = await screen.findByLabelText("Model");
    // Text models only (L2): the mock's speech and image models are not offered.
    await waitFor(() => expect(select.querySelectorAll("option")).toHaveLength(5));
    expect(Array.from(select.querySelectorAll("option"), (option) => option.value)).toEqual([
      "gemini-3.8-flash",
      "gemini-3.5-flash-lite",
      "gemini-3.8-pro-preview",
      "gemini-3.7-flash",
      "gemini-2.5-pro",
    ]);
    await user.selectOptions(select, "gemini-3.5-flash-lite");
    await waitFor(() => expect(backend.calls).toContain('update_settings {"patch":{"geminiModel":"gemini-3.5-flash-lite"}}'));
    expect(backend.calls.filter((call) => call.startsWith("gemini_models"))).toHaveLength(1);
  });

  it("says what is sent to Google", async () => {
    renderApp({ path: "/pengaturan/ai" });
    expect(await screen.findByText(/Yang dikirim ke Google hanya percakapan dan dokumen Studio/)).toBeInTheDocument();
  });
});
