import { describe, expect, it } from "vitest";

import {
  colorHueMatchThreshold,
  colorHueMatchThresholdPercent,
  colorHueStrictnessLabel,
} from "./colorFilter";

describe("color hue strictness", () => {
  it("maps the slider to an increasing minimum hue share", () => {
    expect(colorHueMatchThresholdPercent(0)).toBe(8);
    expect(colorHueMatchThresholdPercent(0.5)).toBe(42);
    expect(colorHueMatchThresholdPercent(1)).toBe(75);
    expect(colorHueMatchThreshold(1)).toBeGreaterThan(colorHueMatchThreshold(0.5));
  });

  it("clamps invalid slider values and exposes readable labels", () => {
    expect(colorHueMatchThresholdPercent(-1)).toBe(8);
    expect(colorHueMatchThresholdPercent(2)).toBe(75);
    expect(colorHueStrictnessLabel(0)).toBe("宽松");
    expect(colorHueStrictnessLabel(0.5)).toBe("平衡");
    expect(colorHueStrictnessLabel(1)).toBe("极严");
  });
});
