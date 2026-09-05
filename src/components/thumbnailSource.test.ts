import { beforeEach, describe, expect, it, vi } from "vitest";

const api = vi.hoisted(() => ({
  fetchPreview: vi.fn(),
  fetchThumbnail: vi.fn(),
}));

vi.mock("../api", () => api);

import {
  cancelGridPreview,
  gridThumbnailProfileForColumns,
  requestGridPreview,
  requestThumbnail,
} from "./thumbnailSource";

describe("thumbnail request queue", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("limits concurrent thumbnail IPC requests", async () => {
    const release: Array<() => void> = [];
    let active = 0;
    let peak = 0;
    api.fetchThumbnail.mockImplementation((assetId: number) => {
      active += 1;
      peak = Math.max(peak, active);
      return new Promise<string>((resolve) => {
        release.push(() => {
          active -= 1;
          resolve(`thumbnail-${assetId}`);
        });
      });
    });

    const requests = Array.from({ length: 8 }, (_, index) => requestThumbnail(9001 + index, 1));
    expect(api.fetchThumbnail).toHaveBeenCalledTimes(6);

    release.splice(0, 6).forEach((resolve) => resolve());
    await vi.waitFor(() => expect(api.fetchThumbnail).toHaveBeenCalledTimes(8));
    release.splice(0).forEach((resolve) => resolve());

    await expect(Promise.all(requests)).resolves.toHaveLength(8);
    expect(peak).toBe(6);
  });

  it("uses bounded quality tiers only below eight columns", () => {
    expect(gridThumbnailProfileForColumns(2)).toMatchObject({
      key: "grid-1280",
      maxWidth: 1280,
      maxHeight: 960,
    });
    expect(gridThumbnailProfileForColumns(4)).toMatchObject({ key: "grid-960" });
    expect(gridThumbnailProfileForColumns(6)).toMatchObject({ key: "grid-768" });
    expect(gridThumbnailProfileForColumns(8)).toMatchObject({ key: "thumbnail" });
    expect(gridThumbnailProfileForColumns(12)).toMatchObject({ key: "thumbnail" });
  });

  it("cancels queued upgrades after a card leaves the viewport", async () => {
    const release: Array<() => void> = [];
    api.fetchPreview.mockImplementation(
      (assetId: number) =>
        new Promise<string>((resolve) => release.push(() => resolve(`preview-${assetId}`))),
    );
    const profile = gridThumbnailProfileForColumns(4);
    if (profile.key === "thumbnail") throw new Error("expected bounded preview profile");

    const activeRequests = [requestGridPreview(11001, profile), requestGridPreview(11002, profile)];
    const queuedRequest = requestGridPreview(11003, profile);
    expect(api.fetchPreview).toHaveBeenCalledTimes(2);
    expect(cancelGridPreview(11003, profile)).toBe(true);
    await expect(queuedRequest).rejects.toThrow("left the viewport");

    release.splice(0).forEach((resolve) => resolve());
    await expect(Promise.all(activeRequests)).resolves.toHaveLength(2);
    expect(api.fetchPreview).toHaveBeenCalledTimes(2);
  });
  it("limits the optional screen-preview upgrades to two concurrent requests", async () => {
    const release: Array<() => void> = [];
    let active = 0;
    let peak = 0;
    api.fetchPreview.mockImplementation((assetId: number) => {
      active += 1;
      peak = Math.max(peak, active);
      return new Promise<string>((resolve) => {
        release.push(() => {
          active -= 1;
          resolve(`preview-${assetId}`);
        });
      });
    });
    const profile = gridThumbnailProfileForColumns(2);
    if (profile.key === "thumbnail") throw new Error("expected bounded preview profile");

    const requests = Array.from({ length: 4 }, (_, index) =>
      requestGridPreview(10001 + index, profile),
    );
    expect(api.fetchPreview).toHaveBeenCalledTimes(2);
    expect(api.fetchPreview).toHaveBeenNthCalledWith(1, 10001, "screen", 1280, 960);

    release.splice(0, 2).forEach((resolve) => resolve());
    await vi.waitFor(() => expect(api.fetchPreview).toHaveBeenCalledTimes(4));
    release.splice(0).forEach((resolve) => resolve());

    await expect(Promise.all(requests)).resolves.toHaveLength(4);
    expect(peak).toBe(2);
  });
});
