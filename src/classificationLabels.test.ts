import { describe, expect, it } from "vitest";

import {
  classificationSourceLabel,
  classificationValueLabel,
  COLOR_OPTIONS,
  primaryCategoryOptions,
  TONE_OPTIONS,
} from "./classificationLabels";

describe("classification display labels", () => {
  it("uses Chinese labels for stored classification ids", () => {
    expect(classificationValueLabel("landscape", "primary")).toBe("风景");
    expect(classificationValueLabel("mid_tone", "tone")).toBe("中调");
    expect(classificationValueLabel("neutral", "color")).toBe("中性色");
    expect(classificationValueLabel("medium", "saturation")).toBe("中饱和");
    expect(classificationSourceLabel("manual")).toBe("手动");
  });

  it("exposes selectable Chinese options instead of free text values", () => {
    expect(primaryCategoryOptions([])).toContainEqual({ value: "landscape", label: "风景" });
    expect(TONE_OPTIONS).toContainEqual(["balanced", "均衡"]);
    expect(COLOR_OPTIONS).toContainEqual(["blue", "蓝色"]);
  });
});
