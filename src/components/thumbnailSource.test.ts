import { beforeEach, describe, expect, it, vi } from "vitest";

const api = vi.hoisted(() => ({
  fetchThumbnail: vi.fn(),
}));

vi.mock("../api", () => api);

import { requestThumbnail } from "./thumbnailSource";

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
});
