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
      label: "美食",
    });
    expect(primaryCategoryOptions([])).not.toContainEqual({
      value: "photo_documentary",
      label: "纪实与工业",
    });
    expect(primaryCategoryOptions([])).not.toContainEqual({ value: "unknown", label: "未知" });
    expect(auxiliaryTagOptions([])).toContainEqual({ value: "night", label: "夜景" });
    expect(auxiliaryTagOptions([])).not.toContainEqual({ value: "mountain", label: "山" });
    expect(auxiliaryTagOptions([])).toContainEqual({ value: "single_person", label: "单人" });
    expect(auxiliaryTagOptions([])).toContainEqual({ value: "multiple_people", label: "多人" });
    expect(auxiliaryTagOptions([])).not.toContainEqual({ value: "person", label: "人物" });
    expect(auxiliaryTagOptions([])).not.toContainEqual({ value: "portrait", label: "人像" });
    expect(auxiliaryTagOptions([])).not.toContainEqual({ value: "pet", label: "宠物" });
    expect(TONE_OPTIONS).toContainEqual(["balanced", "均衡"]);
    expect(COLOR_OPTIONS).toContainEqual(["blue", "蓝色"]);
  });

  it("keeps historical labels readable without offering them as new choices", () => {
    expect(classificationValueLabel("mountain", "tag")).toBe("山");
    expect(classificationValueLabel("still_life", "tag")).toBe("静物");
    expect(classificationValueLabel("person", "tag")).toBe("单人");
    expect(classificationValueLabel("pet", "tag")).toBe("动物");
    expect(classificationValueLabel("unknown", "primary")).toBe("抽象艺术");
    expect(classificationValueLabel("photo_documentary", "primary")).toBe("抽象艺术");
  });
});
