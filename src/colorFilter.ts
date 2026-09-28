export const DEFAULT_COLOR_HUE_STRICTNESS = 0.5;

const MIN_COLOR_HUE_MATCH_RATIO = 0.04;
const MAX_COLOR_HUE_MATCH_RATIO = 0.32;
const DOMINANT_COLOR_MATCH_STRICTNESS = 0.8;

export function normalizeColorHueStrictness(value: number) {
  return Number.isFinite(value) ? Math.max(0, Math.min(1, value)) : DEFAULT_COLOR_HUE_STRICTNESS;
}

/**
 * Convert strictness into the minimum share of the whole image covered by the
 * selected hue range. The backend uses this same 4%–32% contract.
 */
export function colorHueMatchThreshold(strictness: number) {
  const normalized = normalizeColorHueStrictness(strictness);
  return (
    MIN_COLOR_HUE_MATCH_RATIO + (MAX_COLOR_HUE_MATCH_RATIO - MIN_COLOR_HUE_MATCH_RATIO) * normalized
  );
}

export function colorHueMatchThresholdPercent(strictness: number) {
  return Math.round(colorHueMatchThreshold(strictness) * 100);
}

export function colorHueRequiresDominantMatch(strictness: number) {
  return normalizeColorHueStrictness(strictness) >= DOMINANT_COLOR_MATCH_STRICTNESS;
}

export function colorHueFilterSummary(strictness: number) {
  const areaRequirement = `全图目标色面积 ≥ ${colorHueMatchThresholdPercent(strictness)}%`;
  return colorHueRequiresDominantMatch(strictness)
    ? `${areaRequirement}；且主色候选须与所选色相一致`
    : areaRequirement;
}

export function colorHueStrictnessLabel(strictness: number) {
  const normalized = normalizeColorHueStrictness(strictness);
  if (normalized < 0.25) return "宽松";
  if (normalized < 0.55) return "平衡";
  if (normalized < 0.8) return "严格";
  return "极严";
}
