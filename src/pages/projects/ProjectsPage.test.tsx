import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";

import { renderApp } from "../../test/renderApp";

afterEach(() => {
  vi.useRealTimers();
});

describe("Proyek", () => {
  it("groups projects by Ruang, with progress and the next step", async () => {
    renderApp({ path: "/proyek" });
    const school = await screen.findByRole("region", { name: "Sekolah" });
    const lab = within(school)
      .getByRole("link", { name: /Laporan Lab Jaringan/ })
      .closest("li")!;
    expect(within(lab).getByRole("progressbar", { name: "Laporan Lab Jaringan" })).toHaveAttribute("aria-valuenow", "50");
    expect(within(lab).getByText("1 dari 2 tonggak")).toBeInTheDocument();
    expect(within(lab).getByText("Laporan Praktikum 5: VLAN dan Trunking")).toBeInTheDocument();
    expect(screen.getByRole("region", { name: "Belajar Cyber" })).toHaveTextContent("Jalur Jr Penetration Tester");
  });

  it("makes a new project and opens it", async () => {
    const user = userEvent.setup();
    renderApp({ path: "/proyek" });
    await user.click(await screen.findByRole("button", { name: "Proyek baru" }));
    const dialog = await screen.findByRole("dialog", { name: "Proyek baru" });
    await user.click(within(dialog).getByRole("button", { name: "Simpan" }));
    expect(within(dialog).getByText("Nama proyek belum diisi.")).toBeInTheDocument();
    await user.type(within(dialog).getByLabelText("Nama proyek"), "Homelab Proxmox");
    await user.selectOptions(within(dialog).getByLabelText("Ruang"), "Proyek Pribadi");
    await user.click(within(dialog).getByRole("button", { name: "Simpan" }));
    expect(await screen.findByRole("heading", { level: 1, name: "Homelab Proxmox" })).toBeInTheDocument();
    expect(screen.getByText("Proyek Pribadi")).toBeInTheDocument();
  });

  it("tracks milestones, status, and tasks on the project page", async () => {
    const user = userEvent.setup();
    const { backend } = renderApp({ path: "/proyek" });
    await user.click(await screen.findByRole("link", { name: /Laporan Lab Jaringan/ }));
    expect(await screen.findByRole("heading", { level: 1, name: "Laporan Lab Jaringan" })).toBeInTheDocument();

    await user.click(screen.getByRole("checkbox", { name: "Laporan ditulis" }));
    await waitFor(() =>
      expect(screen.getByRole("progressbar", { name: "Kemajuan Laporan Lab Jaringan" })).toHaveAttribute("aria-valuenow", "100"),
    );

    await user.type(screen.getByRole("textbox", { name: "Tambah tonggak" }), "Presentasi hasil{Enter}");
    expect(await screen.findByRole("checkbox", { name: "Presentasi hasil" })).not.toBeChecked();

    await user.click(screen.getByRole("radio", { name: "Jeda" }));
    await waitFor(() => expect(backend.calls.some((call) => call.startsWith("set_project_status") && call.includes("paused"))).toBe(true));

    await user.type(screen.getByRole("textbox", { name: "Tugas" }), "Cetak laporan{Enter}");
    expect(await screen.findByRole("button", { name: "Cetak laporan" })).toBeInTheDocument();
    expect(backend.calls.some((call) => call.startsWith("create_task") && call.includes("Cetak laporan"))).toBe(true);
  });

  it("asks once before deleting a project", async () => {
    const user = userEvent.setup();
    renderApp({ path: "/proyek" });
    await user.click(await screen.findByRole("link", { name: /Presentasi KIK/ }));
    await user.click(await screen.findByRole("button", { name: "Hapus proyek" }));
    expect(screen.getByText("Hapus proyek ini? Tugasnya tetap ada, tanpa proyek.")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Hapus proyek" }));
    expect(await screen.findByRole("heading", { level: 2, name: "Proyek" })).toBeInTheDocument();
    expect(screen.queryByRole("link", { name: /Presentasi KIK/ })).not.toBeInTheDocument();
  });
});
