import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

const api = vi.hoisted(() => ({
  fetchAssetGroups: vi.fn(),
  fetchAssetsInGroup: vi.fn(),
}));

vi.mock("../api", () => api);

import { GroupedAssetGrid } from "./GroupedAssetGrid";
import { assetQueryFromV1, createAssetQueryV1 } from "../query";
import type { AssetGroupPage, AssetListItem } from "../types";

const query = assetQueryFromV1(createAssetQueryV1(null));
const emptyItemsPage = { items: [], total: 0, page: 1, pageSize: 120 };

function groupsPage(labels: Array<[string, number]>): AssetGroupPage {
  return {
    groups: labels.map(([label, uniqueCount]) => ({ key: label, label, uniqueCount })),
    groupCount: labels.length,
    globalUniqueTotal: labels.reduce((sum, [, count]) => sum + count, 0),
    page: 1,
    pageSize: 100,
  };
}

function renderGrid(refreshKey = 0) {
  return render(
    <GroupedAssetGrid
      query={query}
      refreshKey={refreshKey}
      groupBy="primary_category"
      gridColumns={4}
      activeAssetId={null}
      selectedAssetIds={[]}
      semanticCatalog={[]}
      onStartAssetDrag={vi.fn()}
      onSelect={vi.fn()}
      onToggleSelection={vi.fn()}
      onOpen={vi.fn()}
      onClearSelection={vi.fn()}
      onUpdateRating={vi.fn()}
      onUpdateColorLabel={vi.fn()}
      favoriteAssetIds={new Set()}
      onToggleFavorite={vi.fn()}
    />,
  );
}

beforeEach(() => {
  vi.clearAllMocks();
  api.fetchAssetGroups.mockResolvedValue(
    groupsPage([
      ["landscape", 2],
      ["portrait", 1],
    ]),
  );
  api.fetchAssetsInGroup.mockResolvedValue(emptyItemsPage);
});

describe("GroupedAssetGrid", () => {
  it("starts collapsed and only requests members for the expanded group", async () => {
    const user = userEvent.setup();
    renderGrid();

    const landscape = await screen.findByRole("button", { name: /展开分组：landscape/ });
    const portrait = screen.getByRole("button", { name: /展开分组：portrait/ });
    expect(landscape).toHaveAttribute("aria-expanded", "false");
    expect(portrait).toHaveAttribute("aria-expanded", "false");
    expect(api.fetchAssetsInGroup).not.toHaveBeenCalled();

    await user.click(landscape);
    await waitFor(() => expect(api.fetchAssetsInGroup).toHaveBeenCalledTimes(1));
    expect(api.fetchAssetsInGroup).toHaveBeenCalledWith(
      query,
      "primary_category",
      "landscape",
      1,
      120,
    );
    expect(api.fetchAssetsInGroup).not.toHaveBeenCalledWith(
      query,
      "primary_category",
      "portrait",
      1,
      120,
    );
  });

  it("clears the group-summary error after retry succeeds", async () => {
    const user = userEvent.setup();
    api.fetchAssetGroups
      .mockRejectedValueOnce(new Error("temporary summary failure"))
      .mockResolvedValueOnce(groupsPage([["landscape", 2]]));
    renderGrid();

    expect(await screen.findByRole("alert")).toHaveTextContent("temporary summary failure");
    await user.click(screen.getByRole("button", { name: "重试分组" }));
    expect(
      await screen.findByText("landscape", { selector: ".group-heading strong" }),
    ).toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("clears an initial member error after retry succeeds", async () => {
    const user = userEvent.setup();
    api.fetchAssetsInGroup
      .mockRejectedValueOnce(new Error("temporary member failure"))
      .mockResolvedValueOnce(emptyItemsPage);
    renderGrid();

    await user.click(await screen.findByRole("button", { name: /展开分组：landscape/ }));
    expect(await screen.findByRole("alert")).toHaveTextContent("temporary member failure");
    await user.click(screen.getByRole("button", { name: "重试" }));
    await waitFor(() => expect(api.fetchAssetsInGroup).toHaveBeenCalledTimes(2));
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("retries the same append page after a later member page fails", async () => {
    const user = userEvent.setup();
    api.fetchAssetsInGroup
      .mockResolvedValueOnce({
        items: [] satisfies AssetListItem[],
        total: 240,
        page: 1,
        pageSize: 120,
      })
      .mockRejectedValueOnce(new Error("temporary page failure"))
      .mockResolvedValueOnce({ items: [], total: 240, page: 2, pageSize: 120 });
    renderGrid();

    await user.click(await screen.findByRole("button", { name: /展开分组：landscape/ }));
    const group = screen.getByRole("button", { name: /折叠分组：landscape/ }).closest("section");
    const loadMore = await within(group as HTMLElement).findByRole("button", {
      name: "加载更多图片（0/240）",
    });
    await user.click(loadMore);
    expect(await screen.findByRole("alert")).toHaveTextContent("temporary page failure");
    await user.click(screen.getByRole("button", { name: "重试本页" }));

    await waitFor(() => expect(api.fetchAssetsInGroup).toHaveBeenCalledTimes(3));
    expect(api.fetchAssetsInGroup.mock.calls.map((call) => call[3])).toEqual([1, 2, 2]);
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("refreshes group aggregates after the query data changes", async () => {
    const view = renderGrid(1);
    expect(
      await screen.findByText("1 张", { selector: ".group-heading-count" }),
    ).toBeInTheDocument();

    api.fetchAssetGroups.mockResolvedValueOnce(groupsPage([["landscape", 5]]));
    view.rerender(
      <GroupedAssetGrid
        query={query}
        refreshKey={2}
        groupBy="primary_category"
        gridColumns={4}
        activeAssetId={null}
        selectedAssetIds={[]}
        semanticCatalog={[]}
        onStartAssetDrag={vi.fn()}
        onSelect={vi.fn()}
        onToggleSelection={vi.fn()}
        onOpen={vi.fn()}
        onClearSelection={vi.fn()}
        onUpdateRating={vi.fn()}
        onUpdateColorLabel={vi.fn()}
        favoriteAssetIds={new Set()}
        onToggleFavorite={vi.fn()}
      />,
    );

    expect(
      await screen.findByText("5 张", { selector: ".group-heading-count" }),
    ).toBeInTheDocument();
    expect(api.fetchAssetGroups).toHaveBeenCalledTimes(2);
    expect(api.fetchAssetsInGroup).not.toHaveBeenCalled();
  });
});
