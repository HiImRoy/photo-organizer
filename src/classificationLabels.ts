import type { SemanticLabelDescriptor } from "./types";

export type ClassificationValueKind = "primary" | "tag" | "tone" | "color" | "saturation";

export const PRIMARY_CATEGORY_OPTIONS = [
  ["photo_landscape", "风光自然"],
  ["photo_urban", "城市街拍"],
  ["photo_architecture", "建筑与空间"],
  ["photo_food", "美食餐饮"],
  ["photo_commercial", "商业与静物"],
  ["photo_indoor", "室内与生活"],
  ["photo_travel", "旅行人文"],
  ["photo_event", "活动与运动"],
  ["photo_transport", "交通与汽车"],
  ["photo_plant", "植物与园艺"],
  ["photo_documentary", "纪实与工业"],
  ["unknown", "未知"],
] as const;

export const AUXILIARY_TAG_OPTIONS = [
  ["indoor", "室内"],
  ["outdoor", "室外"],
  ["group", "多人"],
  ["person", "人物"],
  ["portrait", "人像"],
  ["vehicle", "车辆"],
  ["food", "食品"],
  ["animal", "动物"],
  ["pet", "宠物"],
  ["plant", "植物"],
  ["night", "夜景"],
  ["flower", "花卉"],
  ["abstract", "抽象"],
] as const;

export const TONE_OPTIONS = [
  ["low_key", "低调"],
  ["mid_tone", "中调"],
  ["balanced", "均衡"],
  ["high_key", "高调"],
] as const;

export const COLOR_OPTIONS = [
  ["red", "红色"],
  ["orange", "橙色"],
  ["yellow", "黄色"],
  ["green", "绿色"],
  ["cyan", "青色"],
  ["blue", "蓝色"],
  ["purple", "紫色"],
  ["neutral", "中性色"],
] as const;

export const SATURATION_OPTIONS = [
  ["low", "低饱和"],
  ["medium", "中饱和"],
  ["high", "高饱和"],
] as const;

const FALLBACK_LABELS = new Map<string, string>([
  ...PRIMARY_CATEGORY_OPTIONS,
  ...TONE_OPTIONS,
  ...COLOR_OPTIONS,
  ...SATURATION_OPTIONS,
  ...AUXILIARY_TAG_OPTIONS,
  // Compatibility labels for databases created before the photography-topic
  // taxonomy. They remain readable but are not offered as new choices.
  ["scene_nature", "自然风景与地貌"],
  ["scene_urban", "城市街道与社区"],
  ["scene_architecture", "建筑、地标与宗教"],
  ["scene_commerce", "餐饮与商业"],
  ["scene_residential", "居住与生活空间"],
  ["scene_public", "工作、教育、医疗与公共室内"],
  ["scene_transport", "交通、旅行与交通设施"],
  ["scene_sports", "运动、娱乐与活动"],
  ["scene_industrial", "工业、施工、能源与军事"],
  ["scene_agriculture", "农业、园林与户外休闲"],
  ["still_life", "静物"],
  ["screenshot", "截图"],
  ["mountain", "山"],
  ["water", "水体"],
  ["forest", "森林"],
  ["sunset", "日落"],
  ["other", "其他"],
  ["portrait", "人像"],
  ["landscape", "风景"],
  ["architecture", "建筑"],
  ["product", "产品"],
  ["animal", "动物"],
  ["document", "文档"],
]);

export const UNKNOWN_SEMANTIC_LABEL: SemanticLabelDescriptor = {
  id: "unknown",
  displayName: "未知",
  categoryGroup: "scene",
  threshold: 0,
  isPrimaryCategory: true,
  taxonomyVersion: "photo-organizer-photography-topics-v1",
};

export function classificationFieldLabel(field: string): string {
  switch (field) {
    case "primary_category":
      return "拍摄题材";
    case "auxiliary_tags":
      return "辅助标签";
    case "tone":
      return "影调";
    case "dominant_color_category":
      return "主色";
    case "saturation_level":
      return "饱和度级别";
    default:
      return "分类";
  }
}

export function classificationSourceLabel(source: string): string {
  switch (source) {
    case "auto":
      return "自动";
    case "manual":
      return "手动";
    case "mixed":
      return "混合";
    default:
      return "未设置";
  }
}

export function classificationValueLabel(
  value: string | null | undefined,
  kind: ClassificationValueKind,
  catalog: SemanticLabelDescriptor[] = [],
): string {
  if (!value) return "未设置";
  const catalogLabel = catalog.find((item) => item.id === value)?.displayName;
  if (catalogLabel) return catalogLabel;
  if (kind === "primary" && value === "unknown") return "未知";
  return FALLBACK_LABELS.get(value) ?? fallbackValueLabel(kind);
}

export function classificationValuesLabel(
  values: string[] | null | undefined,
  kind: ClassificationValueKind,
  catalog: SemanticLabelDescriptor[] = [],
): string {
  if (!values?.length) return "未设置";
  return values.map((value) => classificationValueLabel(value, kind, catalog)).join("、");
}

export function primaryCategoryOptions(
  catalog: SemanticLabelDescriptor[],
  selectedValue?: string | null,
) {
  const selectedCompatibilityOption =
    selectedValue &&
    !catalog.some((item) => item.id === selectedValue) &&
    selectedValue !== "unknown"
      ? [
          {
            value: selectedValue,
            label: classificationValueLabel(selectedValue, "primary", catalog),
          },
        ]
      : [];
  return mergeOptions(
    catalog
      .filter((item) => item.isPrimaryCategory)
      .map((item) => ({ value: item.id, label: item.displayName })),
    [
      ...PRIMARY_CATEGORY_OPTIONS.map(([value, label]) => ({ value, label })),
      ...selectedCompatibilityOption,
    ],
  );
}

export function auxiliaryTagOptions(
  catalog: SemanticLabelDescriptor[],
  selectedValues: string[] = [],
) {
  return mergeOptions(
    AUXILIARY_TAG_OPTIONS.map(([value, label]) => ({ value, label })),
    [
      ...catalog
        .filter((item) => !item.isPrimaryCategory)
        .map((item) => ({ value: item.id, label: item.displayName })),
      ...selectedValues.map((value) => ({
        value,
        label: classificationValueLabel(value, "tag", catalog),
      })),
    ],
  );
}

function mergeOptions(
  fallback: Array<{ value: string; label: string }>,
  catalogOptions: Array<{ id?: string; value?: string; displayName?: string; label?: string }>,
) {
  const options = [...fallback];
  for (const item of catalogOptions) {
    const value = item.value ?? item.id;
    const label = item.label ?? item.displayName;
    if (value && label && !options.some((option) => option.value === value)) {
      options.push({ value, label });
    }
  }
  return options;
}

function fallbackValueLabel(kind: ClassificationValueKind): string {
  switch (kind) {
    case "primary":
      return "未知";
    case "tag":
      return "其他标签";
    case "tone":
      return "未知影调";
    case "color":
      return "其他颜色";
    case "saturation":
      return "未知饱和度";
  }
}
