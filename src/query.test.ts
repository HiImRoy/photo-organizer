import { describe, expect, it } from "vitest";

import {
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
});
