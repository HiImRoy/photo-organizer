export const DEFAULT_COLOR_HUE_STRICTNESS = 0.5;

const MIN_COLOR_HUE_MATCH_RATIO = 0.08;
const MAX_COLOR_HUE_MATCH_RATIO = 0.75;

export function normalizeColorHueStrictness(value: number) {
  return Number.isFinite(value) ? Math.max(0, Math.min(1, value)) : DEFAULT_COLOR_HUE_STRICTNESS;
}

/**
 * Convert the user-facing strictness into the minimum share of chromatic hue
 * samples that must fall inside the selected hue range.
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

export function colorHueStrictnessLabel(strictness: number) {
  const normalized = normalizeColorHueStrictness(strictness);
  if (normalized < 0.25) return "宽松";
  if (normalized < 0.55) return "平衡";
  if (normalized < 0.8) return "严格";
  return "极严";
}
