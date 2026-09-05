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
    expect(classificationValueLabel("landscape", "primary")).toBe("风光");
    expect(classificationValueLabel("mid_tone", "tone")).toBe("中调");
    expect(classificationValueLabel("neutral", "color")).toBe("中性色");
    expect(classificationValueLabel("medium", "saturation")).toBe("中饱和");
    expect(classificationSourceLabel("manual")).toBe("手动");
  });

  it("exposes selectable Chinese options instead of free text values", () => {
    expect(primaryCategoryOptions([])).toContainEqual({
      value: "photo_landscape",
      label: "风光",
    });
    expect(primaryCategoryOptions([])).toContainEqual({
      value: "photo_still_life",
      label: "静物特写",
    });
    expect(primaryCategoryOptions([])).not.toContainEqual({
      value: "photo_food",
      label: "美食",
    });
    expect(primaryCategoryOptions([])).not.toContainEqual({
      value: "photo_activity",
      label: "运动",
    });
    expect(primaryCategoryOptions([])).not.toContainEqual({
      value: "photo_document",
      label: "文档截图",
    });
    expect(primaryCategoryOptions([])).not.toContainEqual({
      value: "photo_documentary",
      label: "纪实与工业",
    });
    expect(primaryCategoryOptions([])).not.toContainEqual({ value: "unknown", label: "未知" });
    expect(auxiliaryTagOptions([])).not.toContainEqual({ value: "night", label: "夜景" });
    expect(auxiliaryTagOptions([])).not.toContainEqual({ value: "mountain", label: "山" });
    expect(auxiliaryTagOptions([])).toContainEqual({ value: "single_person", label: "单人" });
    expect(auxiliaryTagOptions([])).toContainEqual({ value: "multiple_people", label: "多人" });
    expect(auxiliaryTagOptions([])).toContainEqual({ value: "food", label: "食物" });
    expect(auxiliaryTagOptions([])).toContainEqual({ value: "scenery", label: "风景" });
    expect(auxiliaryTagOptions([])).not.toContainEqual({ value: "vehicle", label: "车辆" });
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
    expect(classificationValueLabel("food", "tag")).toBe("食物");
    expect(classificationValueLabel("unknown", "primary")).toBe("抽象艺术");
    expect(classificationValueLabel("photo_documentary", "primary")).toBe("抽象艺术");
  });

  it("keeps active option sets closed even when catalog contains legacy values", () => {
    const catalog = [
      {
        id: "photo_food",
        displayName: "美食",
        categoryGroup: "scene",
        threshold: 0.2,
        isPrimaryCategory: true,
        taxonomyVersion: "legacy",
      },
      {
        id: "photo_street",
        displayName: "街拍",
        categoryGroup: "scene",
        threshold: 0.2,
        isPrimaryCategory: true,
        taxonomyVersion: "current",
      },
      {
        id: "vehicle",
        displayName: "车辆",
        categoryGroup: "subject",
        threshold: 0.4,
        isPrimaryCategory: false,
        taxonomyVersion: "legacy",
      },
      {
        id: "night",
        displayName: "夜景",
        categoryGroup: "context",
        threshold: 0.4,
        isPrimaryCategory: false,
        taxonomyVersion: "legacy",
      },
    ];

    expect(primaryCategoryOptions(catalog, "photo_food")).toEqual([
      { value: "photo_portrait", label: "人像" },
      { value: "photo_landscape", label: "风光" },
      { value: "photo_street", label: "街拍" },
      { value: "photo_architecture", label: "建筑" },
      { value: "photo_still_life", label: "静物特写" },
      { value: "photo_wildlife", label: "动物" },
      { value: "photo_macro", label: "植物" },
      { value: "photo_vehicle", label: "交通工具" },
      { value: "photo_abstract", label: "抽象艺术" },
    ]);
    expect(auxiliaryTagOptions(catalog, ["vehicle", "pet", "scenery"])).toEqual([
      { value: "single_person", label: "单人" },
      { value: "multiple_people", label: "多人" },
      { value: "animal", label: "动物" },
      { value: "plant", label: "植物" },
      { value: "food", label: "食物" },
      { value: "scenery", label: "风景" },
    ]);
  });
});
