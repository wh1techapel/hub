import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";

import { renderApp } from "../../test/renderApp";

afterEach(() => {
  vi.useRealTimers();
});

const places = () => screen.getByRole("navigation", { name: "Tempat file" });

describe("File", () => {
  it("shows the Library by kind, Koleksi, and missing files", async () => {
    const user = userEvent.setup();
    renderApp({ path: "/file" });
    const grid = await screen.findByRole("list", { name: "Semua file" });
    expect(await within(grid).findAllByRole("listitem")).toHaveLength(5);
    expect(within(grid).getByRole("button", { name: "Ubuntu 24.04 lab.ova, Hilang" })).toBeInTheDocument();

    await user.click(within(places()).getByRole("button", { name: /Dokumen/ }));
    expect(await screen.findByRole("heading", { level: 2, name: "Dokumen" })).toBeInTheDocument();
    expect(within(screen.getByRole("list", { name: "Dokumen" })).getAllByRole("listitem")).toHaveLength(3);

    await user.click(within(places()).getByRole("button", { name: /Write-up/ }));
    const writeups = await screen.findByRole("list", { name: "Write-up" });
    expect(within(writeups).getByRole("button", { name: "writeup-lame.md" })).toBeInTheDocument();

    await user.click(within(places()).getByRole("button", { name: /Hilang/ }));
    expect(await screen.findByRole("button", { name: "Ubuntu 24.04 lab.ova, Hilang" })).toBeInTheDocument();
  });

  it("a file opens beside the grid: note, Koleksi, attach, Vault", async () => {
    const user = userEvent.setup();
    const { backend } = renderApp({ path: "/file" });
    await user.click(await screen.findByRole("button", { name: "nmap-cheatsheet.pdf" }));
    const panel = await screen.findByRole("complementary", { name: "Detail file" });
    expect(within(panel).getByText("Materi: Nmap Live Host Discovery")).toBeInTheDocument();

    const note = within(panel).getByLabelText("Catatan");
    await user.type(note, "Flag -sn untuk host discovery");
    await user.tab();
    await waitFor(() => expect(backend.calls.some((call) => call.startsWith("update_file") && call.includes("-sn"))).toBe(true));

    await user.click(within(panel).getByRole("button", { name: "Write-up" }));
    await waitFor(() => expect(within(panel).getByRole("button", { name: "Write-up" })).toHaveAttribute("aria-pressed", "true"));

    await user.selectOptions(within(panel).getByLabelText("Lampirkan ke proyek"), "Hub");
    expect(await within(panel).findByText("Proyek: Hub")).toBeInTheDocument();

    await user.click(within(panel).getByRole("button", { name: "Simpan salinan" }));
    expect(await within(panel).findByRole("button", { name: "Hapus salinan" })).toBeInTheDocument();
    expect(within(screen.getByRole("status")).getByText("Salinan tersimpan di Vault.")).toBeInTheDocument();
  });

  it("forgets a file without touching the disk, and Urungkan brings it back", async () => {
    const user = userEvent.setup();
    renderApp({ path: "/file" });
    await user.click(await screen.findByRole("button", { name: "topologi-vlan.png" }));
    const panel = await screen.findByRole("complementary", { name: "Detail file" });
    expect(within(panel).getByText("Hanya dihapus dari Hub. File di disk tidak disentuh.")).toBeInTheDocument();
    await user.click(within(panel).getByRole("button", { name: "Lupakan file" }));
    await waitFor(() => expect(screen.queryByRole("button", { name: "topologi-vlan.png" })).not.toBeInTheDocument());
    await user.click(within(screen.getByRole("status")).getByRole("button", { name: "Urungkan" }));
    expect(await screen.findByRole("button", { name: "topologi-vlan.png" })).toBeInTheDocument();
  });

  it("a missing file asks where it is now", async () => {
    const user = userEvent.setup();
    const { backend } = renderApp({ path: "/file?missing=1" });
    await user.click(await screen.findByRole("button", { name: "Ubuntu 24.04 lab.ova, Hilang" }));
    const panel = await screen.findByRole("complementary", { name: "Detail file" });
    expect(within(panel).getByText("Tidak ditemukan di tempatnya.")).toBeInTheDocument();
    expect(within(panel).getByRole("button", { name: "Buka" })).toBeDisabled();
    await user.click(within(panel).getByRole("button", { name: "Cari lokasinya" }));
    expect(await within(screen.getByRole("status")).findByText("Letak baru tersimpan.")).toBeInTheDocument();
    expect(backend.calls.some((call) => call.startsWith("relocate_file"))).toBe(true);
  });

  it("makes a Koleksi from the side list", async () => {
    const user = userEvent.setup();
    renderApp({ path: "/file" });
    await screen.findByRole("navigation", { name: "Tempat file" });
    await user.click(await within(places()).findByRole("button", { name: "Koleksi baru" }));
    await user.type(within(places()).getByRole("textbox", { name: "Nama koleksi" }), "Soal latihan{Enter}");
    expect(await screen.findByRole("heading", { level: 2, name: "Soal latihan" })).toBeInTheDocument();
    expect(screen.getByText("Tidak ada file di sini.")).toBeInTheDocument();
  });
});
