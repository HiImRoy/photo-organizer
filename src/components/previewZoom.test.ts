import { describe, expect, it } from "vitest";

import { MAX_ZOOM, MIN_ZOOM, smoothZoomLevel } from "./previewZoom";

describe("smooth preview zoom", () => {
  it("changes zoom in small continuous steps", () => {
    const zoomIn = smoothZoomLevel(1, -100);
    const zoomOut = smoothZoomLevel(1, 100);

    expect(zoomIn).toBeGreaterThan(1);
    expect(zoomIn).toBeLessThan(1.2);
    expect(zoomOut).toBeLessThan(1);
    expect(zoomOut).toBeGreaterThan(0.8);
  });

  it("keeps the Lightroom zoom range bounded", () => {
    expect(smoothZoomLevel(MIN_ZOOM, 10_000)).toBe(MIN_ZOOM);
    expect(smoothZoomLevel(MAX_ZOOM, -10_000)).toBe(MAX_ZOOM);
  });
});
