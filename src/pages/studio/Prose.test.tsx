import { render } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { parseBlocks, Prose } from "./Prose";

describe("Prose", () => {
  it("reads paragraphs, lists, headings, and code blocks", () => {
    const blocks = parseBlocks("Halo\nlanjutan\n\n## Langkah\n- satu\n- dua\n  masih dua\n1. a\n2. b\n```\nkode <b>\n```");
    expect(blocks).toEqual([
      { kind: "paragraph", text: "Halo lanjutan" },
      { kind: "heading", text: "Langkah" },
      { kind: "list", ordered: false, items: ["satu", "dua masih dua"] },
      { kind: "list", ordered: true, items: ["a", "b"] },
      { kind: "code", text: "kode <b>" },
    ]);
  });

  it("never renders HTML from the text", () => {
    const { container } = render(<Prose text={'<img src=x onerror="alert(1)"> **tebal** `kode` <script>x</script>'} />);
    expect(container.querySelector("img")).toBeNull();
    expect(container.querySelector("script")).toBeNull();
    expect(container.querySelector("strong")?.textContent).toBe("tebal");
    expect(container.querySelector("code")?.textContent).toBe("kode");
    expect(container.textContent).toContain('<img src=x onerror="alert(1)">');
  });
});
