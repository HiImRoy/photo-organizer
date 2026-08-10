import { describe, expect, it } from "vitest";

import {
  auxiliaryTagOptions,
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
    expect(primaryCategoryOptions([])).toContainEqual({
      value: "photo_landscape",
      label: "风光自然",
    });
    expect(primaryCategoryOptions([])).toContainEqual({
      value: "photo_food",
      label: "美食餐饮",
    });
    expect(auxiliaryTagOptions([])).toContainEqual({ value: "night", label: "夜景" });
    expect(auxiliaryTagOptions([])).not.toContainEqual({ value: "mountain", label: "山" });
    expect(TONE_OPTIONS).toContainEqual(["balanced", "均衡"]);
    expect(COLOR_OPTIONS).toContainEqual(["blue", "蓝色"]);
  });

  it("keeps historical labels readable without offering them as new choices", () => {
    expect(classificationValueLabel("mountain", "tag")).toBe("山");
    expect(classificationValueLabel("still_life", "tag")).toBe("静物");
  });
});
