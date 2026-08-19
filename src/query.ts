import {
  emptyAssetFilter,
  type AssetFilter,
  type AssetQuery,
  type AssetQueryV1,
  type AssetScopeDescription,
  type AssetScopeInputV1,
  type SortDirection,
  type SortField,
} from "./types";

export const DEFAULT_ASSET_PAGE_SIZE = 120;

export function assetQueryFromV1(query: AssetQueryV1): AssetQuery {
  const normalized = normalizeAssetQueryV1(query);
  const { favoriteOnly, collectionId } = normalized.filter;
  const root =
    collectionId !== null
      ? { kind: "collection" as const, collectionId }
      : favoriteOnly
        ? { kind: "favorites" as const }
        : normalized.libraryId === null
          ? { kind: "all" as const }
          : { kind: "source" as const, libraryId: normalized.libraryId };
  return {
    version: 2,
    root,
    includeDescendants: true,
    filter: {
      ...normalized.filter,
      favoriteOnly: false,
      collectionId: null,
    },
    sort: normalized.sort,
    direction: normalized.direction,
    page: normalized.page,
    pageSize: normalized.pageSize,
  };
}

export function createAssetQueryV1(
  libraryId: number | null = null,
  pageSize = DEFAULT_ASSET_PAGE_SIZE,
): AssetQueryV1 {
  return {
    version: 1,
    libraryId,
    filter: { ...emptyAssetFilter },
    sort: "capture_time",
    direction: "desc",
    page: 1,
    pageSize,
  };
}

export function normalizeAssetQueryV1(query: AssetQueryV1): AssetQueryV1 {
  return {
    ...query,
    version: 1,
    page: Math.max(1, Math.floor(query.page) || 1),
    pageSize: Math.min(500, Math.max(1, Math.floor(query.pageSize) || DEFAULT_ASSET_PAGE_SIZE)),
    filter: { ...query.filter },
  };
}

export function updateAssetQueryFilter(query: AssetQueryV1, filter: AssetFilter): AssetQueryV1 {
  return normalizeAssetQueryV1({ ...query, filter, page: 1 });
}

export function updateAssetQueryLibrary(
  query: AssetQueryV1,
  libraryId: number | null,
): AssetQueryV1 {
  return normalizeAssetQueryV1({ ...query, libraryId, page: 1 });
}

export function updateAssetQuerySort(
  query: AssetQueryV1,
  sort: SortField,
  direction?: SortDirection,
): AssetQueryV1 {
  return normalizeAssetQueryV1({
    ...query,
    sort,
    direction: direction ?? query.direction,
    page: 1,
  });
}

export function updateAssetQueryPage(query: AssetQueryV1, page: number): AssetQueryV1 {
  return normalizeAssetQueryV1({ ...query, page });
}

export function describeAssetScopeV1(
  scope: AssetScopeInputV1,
  queryCount: number,
): AssetScopeDescription {
  if (scope.kind === "selection") {
    return {
      kind: scope.kind,
      label: `已选择 ${scope.assetIds.length} 张`,
      count: scope.assetIds.length,
      isExplicitSelection: true,
    };
  }
  return {
    kind: scope.kind,
    label: "当前查询",
    count: queryCount,
    isExplicitSelection: false,
  };
}

export function stableAssetIds(assetIds: number[]): number[] {
  return [...new Set(assetIds)].filter((id) => Number.isSafeInteger(id) && id > 0);
}
