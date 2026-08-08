import type { SemanticLabelDescriptor } from "./types";

export type ClassificationValueKind = "primary" | "tag" | "tone" | "color" | "saturation";

export const PRIMARY_CATEGORY_OPTIONS = [
  ["portrait", "人像"],
  ["landscape", "风景"],
  ["architecture", "建筑"],
  ["product", "产品"],
  ["animal", "动物"],
  ["document", "文档"],
  ["unknown", "未知"],
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
  ["indoor", "室内"],
  ["street", "街道"],
  ["vehicle", "车辆"],
  ["still_life", "静物"],
  ["food", "食品"],
  ["screenshot", "截图"],
  ["night", "夜景"],
  ["flower", "花卉"],
  ["mountain", "山"],
  ["water", "水体"],
  ["forest", "森林"],
  ["sunset", "日落"],
  ["abstract", "抽象"],
]);

export function classificationFieldLabel(field: string): string {
  switch (field) {
    case "primary_category":
      return "主类别";
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

export function primaryCategoryOptions(catalog: SemanticLabelDescriptor[]) {
  return mergeOptions(
    PRIMARY_CATEGORY_OPTIONS.map(([value, label]) => ({ value, label })),
    catalog.filter((item) => item.isPrimaryCategory),
  );
}

export function auxiliaryTagOptions(
  catalog: SemanticLabelDescriptor[],
  selectedValues: string[] = [],
) {
  return mergeOptions(
    catalog
      .filter((item) => !item.isPrimaryCategory)
      .map((item) => ({ value: item.id, label: item.displayName })),
    selectedValues.map((value) => ({
      value,
      label: classificationValueLabel(value, "tag", catalog),
    })),
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
