import { describe, expect, it } from "vitest";

import type { ModelInfo } from "../../data/generated/ModelInfo";
import { chatModels, groupModels, isTextModel, modelLabel } from "./models";

const info = (id: string): ModelInfo => ({ id, displayName: id, inputTokenLimit: null, outputTokenLimit: null, generates: true });

describe("Lentera's models (L2)", () => {
  it("names a model without Gemini, from its id alone", () => {
    expect(modelLabel("gemini-3.5-flash")).toBe("3.5 Flash");
    expect(modelLabel("gemini-3-pro-preview")).toBe("3 Pro Preview");
    expect(modelLabel("gemini-2.5-flash-lite")).toBe("2.5 Flash-Lite");
    expect(modelLabel("gemini-2.5-flash-preview-05-20")).toBe("2.5 Flash Preview 05-20");
    // An id Hub does not recognise is shown as it is.
    expect(modelLabel("model-kustom")).toBe("model-kustom");
  });

  it("offers text models only: speech, image, live, and embedding models are left out", () => {
    for (const id of ["gemini-3.5-flash", "gemini-3-pro-preview", "gemini-2.0-flash-001", "gemini-2.5-flash-lite"]) expect(isTextModel(id), id).toBe(true);
    for (const id of [
      "gemini-3.5-flash-preview-tts",
      "gemini-3.5-flash-image",
      "gemini-2.5-flash-live-001",
      "gemini-2.5-flash-native-audio-dialog",
      "gemini-embedding-001",
      "gemma-3-27b-it",
      "gemini-flash-latest",
    ])
      expect(isTextModel(id), id).toBe(false);
  });

  it("lists the newest first, Flash, Pro, then Flash-Lite, a stable model before its preview", () => {
    const list = chatModels(
      [
        "gemini-2.5-pro",
        "gemini-3.8-flash-image",
        "gemini-3.5-flash-lite",
        "gemini-3.8-pro-preview",
        "gemini-3.5-flash-preview-09-25",
        "gemini-3.5-flash",
        "gemini-3.8-flash",
      ].map(info),
      "gemini-3.8-flash",
    );
    expect(list.map((m) => m.id)).toEqual([
      "gemini-3.8-flash",
      "gemini-3.8-pro-preview",
      "gemini-3.5-flash",
      "gemini-3.5-flash-preview-09-25",
      "gemini-3.5-flash-lite",
      "gemini-2.5-pro",
    ]);
    expect(list[1]).toEqual({ id: "gemini-3.8-pro-preview", label: "3.8 Pro Preview", family: "pro" });
  });

  it("groups the menu by family: Flash, Pro, Flash-Lite, then what Hub does not recognise", () => {
    const list = chatModels(["gemini-3.5-flash-lite", "gemini-2.5-pro", "gemini-3.8-flash", "gemini-3.7-flash"].map(info), "model-kustom");
    expect(groupModels(list).map((group) => [group.family, group.items.map((m) => m.id)])).toEqual([
      ["flash", ["gemini-3.8-flash", "gemini-3.7-flash"]],
      ["pro", ["gemini-2.5-pro"]],
      ["flash-lite", ["gemini-3.5-flash-lite"]],
      [null, ["model-kustom"]],
    ]);
    // An empty family is left out.
    expect(groupModels(chatModels([info("gemini-3.8-flash")], "gemini-3.8-flash")).map((group) => group.family)).toEqual(["flash"]);
  });

  it("keeps the saved model when Google's list lacks it or was not read", () => {
    expect(chatModels(undefined, "gemini-3.5-flash")).toEqual([{ id: "gemini-3.5-flash", label: "3.5 Flash", family: "flash" }]);
    expect(chatModels([info("gemini-3.8-flash")], "model-kustom").map((m) => m.id)).toEqual(["model-kustom", "gemini-3.8-flash"]);
  });
});
