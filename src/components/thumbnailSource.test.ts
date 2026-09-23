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

  it("bounds unique thumbnail sources and evicts the least recently used entry", async () => {
    api.fetchThumbnail.mockImplementation(async (assetId: number) => `thumbnail-${assetId}`);
    const assetIds = Array.from({ length: 300 }, (_, index) => 12000 + index);

    for (const assetId of assetIds) {
      await requestThumbnail(assetId);
    }
    expect(api.fetchThumbnail).toHaveBeenCalledTimes(assetIds.length);

    // The cache keeps the newest 256 entries. Touching the oldest retained
    // entry makes it newer than the next entry for the upcoming eviction.
    await expect(requestThumbnail(assetIds[44])).resolves.toBe(`thumbnail-${assetIds[44]}`);
    await expect(requestThumbnail(12300)).resolves.toBe("thumbnail-12300");
    await expect(requestThumbnail(assetIds[44])).resolves.toBe(`thumbnail-${assetIds[44]}`);
    expect(api.fetchThumbnail).toHaveBeenCalledTimes(assetIds.length + 1);

    await expect(requestThumbnail(assetIds[45])).resolves.toBe(`thumbnail-${assetIds[45]}`);
    await expect(requestThumbnail(assetIds[0])).resolves.toBe(`thumbnail-${assetIds[0]}`);
    expect(api.fetchThumbnail).toHaveBeenCalledTimes(assetIds.length + 3);
  });

  it("does not cache a failed thumbnail request", async () => {
    api.fetchThumbnail
      .mockRejectedValueOnce(new Error("thumbnail fetch failed"))
      .mockResolvedValueOnce("thumbnail-retry-succeeded");

    await expect(requestThumbnail(13001)).rejects.toThrow("thumbnail fetch failed");
    await expect(requestThumbnail(13001)).resolves.toBe("thumbnail-retry-succeeded");
    expect(api.fetchThumbnail).toHaveBeenCalledTimes(2);
  });

  it("evicts sources when their combined size exceeds the character budget", async () => {
    const assetIds = [14001, 14002, 14003, 14004];
    api.fetchThumbnail.mockImplementation(async (assetId: number) =>
      String.fromCharCode(65 + assetId - assetIds[0]).repeat(9 * 1024 * 1024),
    );

    for (const assetId of assetIds) {
      await requestThumbnail(assetId);
    }
    await requestThumbnail(assetIds[0]);

    expect(api.fetchThumbnail).toHaveBeenCalledTimes(assetIds.length + 1);
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
