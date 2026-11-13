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
    expect(within(lab).getByText("1 dari 2 tugas selesai")).toBeInTheDocument();
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

  it("groups a project's tasks under its milestones and counts every task", async () => {
    const user = userEvent.setup();
    const { backend } = renderApp({ path: "/proyek" });
    await user.click(await screen.findByRole("link", { name: /Laporan Lab Jaringan/ }));
    expect(await screen.findByRole("heading", { level: 1, name: "Laporan Lab Jaringan" })).toBeInTheDocument();
    const milestone = (name: string) => screen.getByRole("region", { name });

    expect(within(milestone("Topologi dan konfigurasi")).getByText("1 dari 1 tugas")).toBeInTheDocument();
    expect(within(milestone("Laporan ditulis")).getByText("0 dari 1 tugas")).toBeInTheDocument();
    // A milestone with tasks is done when they are: there is nothing to tick by hand.
    expect(within(milestone("Laporan ditulis")).queryByRole("checkbox")).not.toBeInTheDocument();

    await user.click(within(milestone("Laporan ditulis")).getByRole("button", { name: /Tandai selesai: Laporan Praktikum 5/ }));
    await waitFor(() =>
      expect(screen.getByRole("progressbar", { name: "Kemajuan Laporan Lab Jaringan" })).toHaveAttribute("aria-valuenow", "100"),
    );
    expect(within(milestone("Laporan ditulis")).getByText("1 dari 1 tugas")).toBeInTheDocument();
    expect(screen.getByText(/^2 dari 2 tugas selesai/)).toBeInTheDocument();

    // A milestone without tasks is ticked on its diamond; a task added in its row joins it.
    await user.type(screen.getByRole("textbox", { name: "Tambah tonggak" }), "Presentasi hasil{Enter}");
    const tick = await screen.findByRole("checkbox", { name: "Presentasi hasil" });
    expect(tick).not.toBeChecked();
    await user.click(tick);
    await waitFor(() => expect(screen.getByRole("checkbox", { name: "Presentasi hasil" })).toBeChecked());
    await user.type(screen.getByRole("textbox", { name: "Tambah tugas ke Presentasi hasil" }), "Siapkan slide{Enter}");
    expect(await within(milestone("Presentasi hasil")).findByRole("button", { name: "Siapkan slide" })).toBeInTheDocument();
    expect(within(milestone("Presentasi hasil")).getByText("0 dari 1 tugas")).toBeInTheDocument();
    expect(within(milestone("Presentasi hasil")).queryByRole("checkbox")).not.toBeInTheDocument();
    expect(screen.getByRole("progressbar", { name: "Kemajuan Laporan Lab Jaringan" })).toHaveAttribute("aria-valuenow", "66");

    await user.click(screen.getByRole("radio", { name: "Jeda" }));
    await waitFor(() => expect(backend.calls.some((call) => call.startsWith("set_project_status") && call.includes("paused"))).toBe(true));

    await user.type(screen.getByRole("textbox", { name: "Tugas" }), "Cetak laporan{Enter}");
    expect(await screen.findByRole("button", { name: "Cetak laporan" })).toBeInTheDocument();
    expect(backend.calls.some((call) => call.startsWith("create_task") && call.includes("Cetak laporan"))).toBe(true);
    // Tasks outside any milestone sit together, above the milestones.
    expect(within(screen.getByRole("list", { name: "Tanpa tonggak" })).getByRole("button", { name: "Cetak laporan" })).toBeInTheDocument();
  });

  it("puts a task in a milestone from its panel", async () => {
    const user = userEvent.setup();
    const { backend } = renderApp({ path: "/proyek" });
    await user.click(await screen.findByRole("link", { name: /Laporan Lab Jaringan/ }));
    await user.type(await screen.findByRole("textbox", { name: "Tugas" }), "Cetak laporan{Enter}");
    await user.click(await screen.findByRole("button", { name: "Cetak laporan" }));
    const panel = await screen.findByRole("complementary", { name: "Detail tugas" });
    const select = await within(panel).findByLabelText("Tonggak");
    await user.selectOptions(select, "Laporan ditulis");
    await waitFor(() =>
      expect(backend.calls.some((call) => call.startsWith("update_task") && call.includes("Cetak laporan") && call.includes("milestoneId"))).toBe(true),
    );
    // Another project starts the task without a milestone.
    await user.selectOptions(within(panel).getByLabelText("Proyek"), "Hub");
    await waitFor(() => expect(within(panel).queryByLabelText("Tonggak")).not.toBeInTheDocument());
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
