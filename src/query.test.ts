import { describe, expect, it } from "vitest";

import {
  assetQueryFromV1,
  createAssetQueryV1,
  describeAssetScopeV1,
  normalizeAssetQueryV1,
  stableAssetIds,
} from "./query";

describe("AssetQueryV1", () => {
  it("normalizes paging without mutating the original query", () => {
    const query = createAssetQueryV1(7, 120);
    const normalized = normalizeAssetQueryV1({ ...query, page: 0, pageSize: 999 });

    expect(normalized).toMatchObject({ version: 1, libraryId: 7, page: 1, pageSize: 500 });
    expect(query.page).toBe(1);
    expect(query.pageSize).toBe(120);
  });

  it("keeps explicit selection ids unique and stable", () => {
    expect(stableAssetIds([8, 2, 8, 0, -1, 2])).toEqual([8, 2]);
  });

  it("describes query and selection scopes distinctly", () => {
    const query = createAssetQueryV1(3);
    expect(describeAssetScopeV1({ kind: "query", query }, 42)).toEqual({
      kind: "query",
      label: "当前查询",
      count: 42,
      isExplicitSelection: false,
    });
    expect(describeAssetScopeV1({ kind: "selection", query, assetIds: [9, 4] }, 42)).toMatchObject({
      kind: "selection",
      label: "已选择 2 张",
      count: 2,
      isExplicitSelection: true,
    });
  });

  it("converts legacy source and virtual roots into one query contract", () => {
    const source = assetQueryFromV1(createAssetQueryV1(7));
    expect(source).toMatchObject({
      version: 2,
      root: { kind: "source", libraryId: 7 },
      includeDescendants: true,
      filter: { favoriteOnly: false, collectionId: null },
    });

    const favorite = assetQueryFromV1({
      ...createAssetQueryV1(7),
      filter: { ...createAssetQueryV1(7).filter, favoriteOnly: true },
    });
    expect(favorite.root).toEqual({ kind: "favorites" });

    const collection = assetQueryFromV1({
      ...createAssetQueryV1(7),
      filter: { ...createAssetQueryV1(7).filter, collectionId: 12 },
    });
    expect(collection.root).toEqual({ kind: "collection", collectionId: 12 });
  });
});
