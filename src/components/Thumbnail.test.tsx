import { render, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { AssetListItem } from "../types";

const api = vi.hoisted(() => ({
  fetchPreview: vi.fn(),
  fetchThumbnail: vi.fn(),
}));

vi.mock("../api", () => api);

import { Thumbnail } from "./Thumbnail";

function asset(id: number): AssetListItem {
  return {
    id,
    thumbnailAvailable: true,
    analysisStatus: "completed",
    errorMessage: null,
  } as AssetListItem;
}

describe("Thumbnail", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    api.fetchThumbnail.mockImplementation((assetId: number) =>
      Promise.resolve(`data:image/jpeg;base64,asset-${assetId}`),
    );
  });

  it("drops the previous image source when the asset changes", async () => {
    const { container, rerender } = render(<Thumbnail asset={asset(31_001)} />);

    await waitFor(() =>
      expect(container.querySelector("img")?.getAttribute("src")).toContain("asset-31001"),
    );

    rerender(<Thumbnail asset={asset(31_002)} />);

    expect(container.querySelector("img")?.getAttribute("src") ?? "").not.toContain("asset-31001");
    await waitFor(() =>
      expect(container.querySelector("img")?.getAttribute("src")).toContain("asset-31002"),
    );
    expect(api.fetchThumbnail).toHaveBeenCalledWith(31_002);
  });
});
