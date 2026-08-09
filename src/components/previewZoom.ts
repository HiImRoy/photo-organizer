export const ZOOM_LEVELS = [0.0625, 0.125, 0.25, 0.5, 1, 2, 4, 8, 11] as const;
export const MIN_ZOOM = ZOOM_LEVELS[0];
export const MAX_ZOOM = ZOOM_LEVELS[ZOOM_LEVELS.length - 1];
const WHEEL_ZOOM_SENSITIVITY = 0.0008;

export function adjacentZoomLevel(current: number, direction: -1 | 1) {
  if (direction > 0) {
    return ZOOM_LEVELS.find((level) => level > current + 0.0001) ?? MAX_ZOOM;
  }
  return [...ZOOM_LEVELS].reverse().find((level) => level < current - 0.0001) ?? MIN_ZOOM;
}

export function smoothZoomLevel(current: number, deltaY: number) {
  const boundedDelta = Math.max(-200, Math.min(200, deltaY));
  const next = current * Math.exp(-boundedDelta * WHEEL_ZOOM_SENSITIVITY);
  return Math.max(MIN_ZOOM, Math.min(MAX_ZOOM, next));
}

export function formatZoomPercent(scale: number) {
  return `${(scale * 100).toFixed(2).replace(/\.?(0+)$/, "")}%`;
}
