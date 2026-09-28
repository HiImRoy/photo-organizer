import { describe, expect, it } from "vitest";

import {
  colorHueMatchThreshold,
  colorHueMatchThresholdPercent,
  colorHueFilterSummary,
  colorHueRequiresDominantMatch,
  colorHueStrictnessLabel,
} from "./colorFilter";

describe("color hue strictness", () => {
  it("maps strictness to 4%–32% of whole-image target-hue area", () => {
    expect(colorHueMatchThresholdPercent(0)).toBe(4);
    expect(colorHueMatchThresholdPercent(0.5)).toBe(18);
    expect(colorHueMatchThresholdPercent(0.8)).toBe(26);
    expect(colorHueMatchThresholdPercent(1)).toBe(32);
    expect(colorHueMatchThreshold(1)).toBeGreaterThan(colorHueMatchThreshold(0.5));
  });

  it("requires a matching dominant-color candidate at very high strictness", () => {
    expect(colorHueRequiresDominantMatch(0.79)).toBe(false);
    expect(colorHueRequiresDominantMatch(0.8)).toBe(true);
    expect(colorHueFilterSummary(0.75)).toBe("全图目标色面积 ≥ 25%");
    expect(colorHueFilterSummary(0.8)).toBe("全图目标色面积 ≥ 26%；且主色候选须与所选色相一致");
  });

  it("clamps invalid slider values and exposes readable labels", () => {
    expect(colorHueMatchThresholdPercent(-1)).toBe(4);
    expect(colorHueMatchThresholdPercent(2)).toBe(32);
    expect(colorHueStrictnessLabel(0)).toBe("宽松");
    expect(colorHueStrictnessLabel(0.5)).toBe("平衡");
    expect(colorHueStrictnessLabel(1)).toBe("极严");
  });
});
