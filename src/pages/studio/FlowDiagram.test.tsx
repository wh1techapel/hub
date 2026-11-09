import { render, screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import type { FlowChart } from "../../data/generated/FlowChart";
import { samplePrd } from "../../data/mockPrd";
import { FlowDiagram } from "./FlowDiagram";
import { layoutFlow, wrap } from "./flowLayout";
import { sectionOf, SECTIONS } from "./prdFields";

const flow = (): FlowChart => samplePrd("Absensi").flow;

// U5b: the user flow drawn by Hub.
describe("Studio: the user flow diagram", () => {
  it("places steps in rows from the start, a decision's ways side by side, and a way back on the right", () => {
    const layout = layoutFlow(flow());
    const at = (id: string) => layout.nodes.find((node) => node.id === id)!;
    // Top to bottom: the start, the screen, the decision, then its two ways in one row.
    expect(at("N1").y).toBeLessThan(at("N2").y);
    expect(at("N2").y).toBeLessThan(at("N3").y);
    expect(at("N4").y).toBe(at("N5").y);
    expect(at("N4").x).not.toBe(at("N5").x);
    // "coba lagi" goes back up: drawn as a return, out to the right of everything.
    const back = layout.edges.filter((edge) => edge.back);
    expect(back.map((edge) => `${edge.from}-${edge.to}`)).toEqual(["N5-N2"]);
    const rightmost = Math.max(...layout.nodes.map((node) => node.x + node.width));
    expect(back[0]!.points[1]![0]).toBeGreaterThan(rightmost);
    expect(layout.width).toBeGreaterThan(rightmost);
    // A decision's arrows carry their answers.
    expect(layout.edges.filter((edge) => edge.from === "N3").map((edge) => edge.label).sort()).toEqual(["Tidak", "Ya"]);
    expect(layout.incomplete).toBe(false);
  });

  it("leaves out an arrow to a step that is not there, and says so", () => {
    const broken = flow();
    broken.edges.push({ from: "N2", to: "N99", label: "" });
    const layout = layoutFlow(broken);
    expect(layout.incomplete).toBe(true);
    expect(layout.edges.some((edge) => edge.to === "N99")).toBe(false);
    render(<FlowDiagram flow={broken} />);
    expect(screen.getByText(/belum bisa digambar utuh/)).toBeInTheDocument();
  });

  it("wraps a long label into at most three lines", () => {
    expect(wrap("Layar absen")).toEqual(["Layar absen"]);
    const long = wrap("Sistem memeriksa kartu pelajar siswa terhadap daftar kelas hari ini dan mencatat jam masuknya ke rekap guru");
    expect(long).toHaveLength(3);
    expect(long[2]!.endsWith("…")).toBe(true);
  });

  it("is a labelled figure, readable as text too", () => {
    render(<FlowDiagram flow={flow()} />);
    const figure = screen.getByRole("figure", { name: "Diagram alur pengguna: Absen masuk praktikum" });
    expect(within(figure).getByRole("img", { name: "Diagram alur pengguna: Absen masuk praktikum" })).toBeInTheDocument();
    // The steps and their ways out, as an ordered list (each step's ways in a list of its own).
    const steps = figure.querySelectorAll("ol > li");
    expect(steps).toHaveLength(7);
    expect(steps[2]).toHaveTextContent("N3 Keputusan: Kartu terdaftar?Ya: ke N4 Sistem mencatat hadirTidak: ke N5 Pesan kartu tidak dikenal");
    expect(within(figure.querySelector("ol") as HTMLElement).getByText(/ditangani FEAT-002/)).toBeInTheDocument();
    // Labels are text, never markup.
    const tricky = flow();
    tricky.nodes[1]!.label = "<b>Layar</b> absen";
    render(<FlowDiagram flow={tricky} />);
    expect(document.querySelector("svg b")).toBeNull();
  });

  it("an older PRD without a diagram says how to get one; problems in it belong to its section", () => {
    render(<FlowDiagram flow={{ title: "", nodes: [], edges: [] }} />);
    expect(screen.getByText(/belum punya diagram alur/)).toBeInTheDocument();
    expect(SECTIONS.indexOf("flow")).toBe(SECTIONS.indexOf("experience") + 1);
    expect(sectionOf("flow.N3")).toBe("flow");
  });
});
