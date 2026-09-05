import { describe, expect, it } from "vitest";

import {
  DEFAULT_APP_SETTINGS,
  effectiveAnalysisBatchSize,
  normalizeAppSettings,
  preferredAnalysisBackend,
} from "./settings";

describe("app settings", () => {
  it("migrates older stored settings to the current display defaults", () => {
    const settings = normalizeAppSettings({ importWorkerCount: 1, analysisBatchSize: 8 });

    expect(settings.startupView).toBe("grid");
    expect(settings.defaultGridColumns).toBe(6);
    expect(settings.importWorkerCount).toBe(1);
    expect(settings.analysisBatchSize).toBe(8);
  });

  it("normalizes startup display values to supported choices", () => {
    expect(normalizeAppSettings({ startupView: "single", defaultGridColumns: 9 })).toMatchObject({
      startupView: "single",
      defaultGridColumns: 10,
    });
    expect(normalizeAppSettings({ defaultGridColumns: 99 }).defaultGridColumns).toBe(12);
    expect(normalizeAppSettings(DEFAULT_APP_SETTINGS)).toEqual(DEFAULT_APP_SETTINGS);
  });
  it("allows a wider configured batch size without raising the CPU execution cap", () => {
    expect(normalizeAppSettings({ analysisBatchSize: 32 }).analysisBatchSize).toBe(32);
    expect(normalizeAppSettings({ analysisBatchSize: 64 }).analysisBatchSize).toBe(32);
    expect(effectiveAnalysisBatchSize(32, { directml: { state: "not_configured" } }, false)).toBe(
      8,
    );
  });

  it("only unlocks the wider batch size after an enabled DirectML provider is ready", () => {
    const capabilities = { directml: { state: "ready" } };

    expect(effectiveAnalysisBatchSize(32, capabilities, false)).toBe(8);
    expect(effectiveAnalysisBatchSize(32, capabilities, true)).toBe(32);
    expect(effectiveAnalysisBatchSize(99, capabilities, true)).toBe(32);
  });

  it("caps DirectML to the detected adapter capacity tier", () => {
    const capabilities = {
      directml: { state: "ready" },
      recommendedAnalysisBatchSize: 16,
    };

    expect(effectiveAnalysisBatchSize(32, capabilities, true)).toBe(16);
    expect(effectiveAnalysisBatchSize(99, capabilities, true)).toBe(16);
  });

  it("selects DirectML only after the provider is ready and enabled", () => {
    expect(preferredAnalysisBackend({ directml: { state: "error" } }, true)).toBe("cpu");
    expect(preferredAnalysisBackend({ directml: { state: "ready" } }, false)).toBe("cpu");
    expect(preferredAnalysisBackend({ directml: { state: "ready" } }, true)).toBe("direct_ml");
  });
});
