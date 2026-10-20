import { useEffect, useMemo, useState } from "react";

import { fetchAssetGroups, fetchAssetsInGroup } from "../api";
import { classificationValueLabel } from "../classificationLabels";
import type {
  AssetGroupField,
  AssetGroupPage,
  AssetGroupSummary,
  AssetListItem,
  AssetQuery,
  ManualColorLabel,
  SemanticLabelDescriptor,
} from "../types";
import { AssetCard } from "./AssetCard";
import { ChevronIcon } from "./Icons";
import { gridThumbnailProfileForColumns } from "./thumbnailSource";

type SelectionModifiers = {
  ctrlKey?: boolean;
  metaKey?: boolean;
  shiftKey?: boolean;
};

type Props = {
  query: AssetQuery;
  refreshKey: number;
  groupBy: AssetGroupField;
  gridColumns: number;
  activeAssetId: number | null;
  selectedAssetIds: number[];
  semanticCatalog: SemanticLabelDescriptor[];
  onStartAssetDrag: (asset: AssetListItem, event: React.PointerEvent<HTMLButtonElement>) => void;
  onSelect: (asset: AssetListItem, modifiers?: SelectionModifiers) => void;
  onToggleSelection: (asset: AssetListItem, modifiers?: SelectionModifiers) => void;
  onOpen: (asset: AssetListItem) => void;
  onClearSelection: () => void;
  onUpdateRating: (assetId: number, rating: number) => void;
  onUpdateColorLabel: (assetId: number, colorLabel: ManualColorLabel | null) => void;
  favoriteAssetIds: Set<number>;
  onToggleFavorite: (assetId: number) => void;
};

const GROUP_PAGE_SIZE = 100;
const ASSET_PAGE_SIZE = 120;

type GroupListState = {
  requestKey: string;
  page: AssetGroupPage;
};

export function GroupedAssetGrid({
  query,
  refreshKey,
  groupBy,
  gridColumns,
  activeAssetId,
  selectedAssetIds,
  semanticCatalog,
  onStartAssetDrag,
  onSelect,
  onToggleSelection,
  onOpen,
  onClearSelection,
  onUpdateRating,
  onUpdateColorLabel,
  favoriteAssetIds,
  onToggleFavorite,
}: Props) {
  const requestKey = `${JSON.stringify(query)}:${groupBy}:${refreshKey}`;
  const [state, setState] = useState<GroupListState | null>(null);
  const [error, setError] = useState<{ requestKey: string; message: string } | null>(null);
  const [retry, setRetry] = useState(0);
  const [loadingMore, setLoadingMore] = useState(false);
  const current = state?.requestKey === requestKey ? state.page : null;
  const currentError = error?.requestKey === requestKey ? error.message : null;

  useEffect(() => {
    let active = true;
    void fetchAssetGroups(query, groupBy, 1, GROUP_PAGE_SIZE)
      .then((page) => {
        if (active) {
          setState({ requestKey, page });
          setError((currentError) =>
            currentError?.requestKey === requestKey ? null : currentError,
          );
        }
      })
      .catch((reason: unknown) => {
        if (active) {
          setError({
            requestKey,
            message: reason instanceof Error ? reason.message : String(reason),
          });
        }
      });
    return () => {
      active = false;
    };
  }, [groupBy, query, requestKey, retry]);

  async function loadMoreGroups() {
    if (!current || loadingMore || current.page * current.pageSize >= current.groupCount) return;
    setLoadingMore(true);
    try {
      const page = await fetchAssetGroups(query, groupBy, current.page + 1, GROUP_PAGE_SIZE);
      setState((previous) => {
        if (!previous || previous.requestKey !== requestKey) return previous;
        const seen = new Set(previous.page.groups.map((group) => group.key));
        return {
          requestKey,
          page: {
            ...page,
            groups: [
              ...previous.page.groups,
              ...page.groups.filter((group) => !seen.has(group.key)),
            ],
          },
        };
      });
      setError((currentError) => (currentError?.requestKey === requestKey ? null : currentError));
    } catch (reason) {
      setError({
        requestKey,
        message: reason instanceof Error ? reason.message : String(reason),
      });
    } finally {
      setLoadingMore(false);
    }
  }

  if (currentError) {
    return (
      <section className="semantic-groups" aria-label="分组图片">
        <div role="alert">无法加载完整分组：{currentError}</div>
        <button
          type="button"
          className="tool-button"
          onClick={() => setRetry((value) => value + 1)}
        >
          重试分组
        </button>
      </section>
    );
  }
  if (!current) {
    return (
      <div className="asset-load-status" role="status">
        正在统计全部匹配图片的分组…
      </div>
    );
  }

  return (
    <div className="semantic-groups">
      {current.groups.map((group, index) => (
        <GroupedAssetSection
          key={`${requestKey}:${group.key}`}
          query={query}
          refreshKey={refreshKey}
          groupBy={groupBy}
          group={group}
          groupIndex={index}
          gridColumns={gridColumns}
          activeAssetId={activeAssetId}
          selectedAssetIds={selectedAssetIds}
          semanticCatalog={semanticCatalog}
          onStartAssetDrag={onStartAssetDrag}
          onSelect={onSelect}
          onToggleSelection={onToggleSelection}
          onOpen={onOpen}
          onClearSelection={onClearSelection}
          onUpdateRating={onUpdateRating}
          onUpdateColorLabel={onUpdateColorLabel}
          favoriteAssetIds={favoriteAssetIds}
          onToggleFavorite={onToggleFavorite}
        />
      ))}
      {current.groups.length < current.groupCount ? (
        <button
          type="button"
          className="tool-button"
          disabled={loadingMore}
          onClick={() => void loadMoreGroups()}
        >
          {loadingMore ? "正在加载分组…" : "加载更多分组"}
        </button>
      ) : null}
    </div>
  );
}

function GroupedAssetSection({
  query,
  groupBy,
  group,
  groupIndex,
  gridColumns,
  activeAssetId,
  selectedAssetIds,
  semanticCatalog,
  onStartAssetDrag,
  onSelect,
  onToggleSelection,
  onOpen,
  onClearSelection,
  onUpdateRating,
  onUpdateColorLabel,
  favoriteAssetIds,
  onToggleFavorite,
}: Props & { group: AssetGroupSummary; groupIndex: number }) {
  const [collapsed, setCollapsed] = useState(true);
  const [items, setItems] = useState<AssetListItem[] | null>(null);
  const [total, setTotal] = useState(0);
  const [page, setPage] = useState(0);
  const [loadingMore, setLoadingMore] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [retry, setRetry] = useState(0);
  const selectedAssetIdSet = useMemo(() => new Set(selectedAssetIds), [selectedAssetIds]);
  const thumbnailProfile = gridThumbnailProfileForColumns(gridColumns);
  const label = groupDisplayLabel(group, groupBy, semanticCatalog);
  const groupPanelId = `asset-group-${groupIndex}`;

  useEffect(() => {
    if (collapsed || items !== null) return undefined;
    let active = true;
    void fetchAssetsInGroup(query, groupBy, group.key, 1, ASSET_PAGE_SIZE)
      .then((result) => {
        if (!active) return;
        setItems(result.items);
        setTotal(result.total);
        setPage(result.page);
        setError(null);
      })
      .catch((reason: unknown) => {
        if (active) setError(reason instanceof Error ? reason.message : String(reason));
      });
    return () => {
      active = false;
    };
  }, [collapsed, group.key, groupBy, items, query, retry]);

  async function loadMoreItems() {
    if (items === null || loadingMore || items.length >= total) return;
    setLoadingMore(true);
    setError(null);
    try {
      const result = await fetchAssetsInGroup(query, groupBy, group.key, page + 1, ASSET_PAGE_SIZE);
      setItems((current) => {
        if (current === null) return result.items;
        const seen = new Set(current.map((asset) => asset.id));
        return [...current, ...result.items.filter((asset) => !seen.has(asset.id))];
      });
      setTotal(result.total);
      setPage(result.page);
      setError(null);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setLoadingMore(false);
    }
  }

  const collapsedCount = group.uniqueCount;
  const visibleCount = groupBy === "auxiliary_tag" ? group.uniqueCount : total || group.uniqueCount;
  return (
    <section className={collapsed ? "is-collapsed" : ""}>
      <button
        type="button"
        className="group-heading"
        aria-expanded={!collapsed}
        aria-controls={groupPanelId}
        aria-label={`${collapsed ? "展开" : "折叠"}分组：${label}（${collapsedCount} 张）`}
        onClick={() => setCollapsed((value) => !value)}
      >
        <ChevronIcon
          className={collapsed ? "group-heading-chevron is-collapsed" : "group-heading-chevron"}
          width="13"
          height="13"
        />
        <strong>{label}</strong>
        <span className="group-heading-count">{visibleCount.toLocaleString()} 张</span>
      </button>
      <div id={groupPanelId} className="semantic-group-items" hidden={collapsed}>
        {items === null && error ? (
          <div role="alert">
            <span>此分组加载失败：{error}</span>
            <button
              type="button"
              className="tool-button"
              onClick={() => {
                setError(null);
                setRetry((value) => value + 1);
              }}
            >
              重试
            </button>
          </div>
        ) : items === null ? (
          <div className="asset-load-status" role="status">
            正在加载此分组…
          </div>
        ) : (
          <>
            {error ? (
              <div role="alert">
                <span>此分组后续图片加载失败：{error}</span>
                <button type="button" className="tool-button" onClick={() => void loadMoreItems()}>
                  重试本页
                </button>
              </div>
            ) : null}
            <div
              className="asset-grid"
              onClick={(event) => {
                if (event.target === event.currentTarget) onClearSelection();
              }}
            >
              {items.map((asset) => (
                <AssetCard
                  key={asset.id}
                  asset={asset}
                  thumbnailProfile={thumbnailProfile}
                  active={activeAssetId === asset.id}
                  selected={selectedAssetIdSet.has(asset.id)}
                  onStartDrag={onStartAssetDrag}
                  onSelect={onSelect}
                  onToggleSelection={onToggleSelection}
                  onOpen={onOpen}
                  onUpdateRating={onUpdateRating}
                  onUpdateColorLabel={onUpdateColorLabel}
                  favorite={favoriteAssetIds.has(asset.id)}
                  onToggleFavorite={onToggleFavorite}
                />
              ))}
            </div>
            {items.length < total ? (
              <button
                type="button"
                className="tool-button"
                disabled={loadingMore}
                onClick={() => void loadMoreItems()}
              >
                {loadingMore ? "正在加载图片…" : `加载更多图片（${items.length}/${total}）`}
              </button>
            ) : null}
          </>
        )}
      </div>
    </section>
  );
}

function groupDisplayLabel(
  group: AssetGroupSummary,
  groupBy: AssetGroupField,
  semanticCatalog: SemanticLabelDescriptor[],
): string {
  if (group.label) return group.label;
  if (group.key === "__missing__") {
    if (groupBy === "primary_category") return "未分类";
    if (groupBy === "rating") return "未评分";
    return "未设置";
  }
  if (groupBy === "rating") {
    const rating = Number.parseInt(group.key, 10);
    return Number.isFinite(rating) && rating > 0 ? `${rating} 星` : "未评分";
  }
  const kindByGroup: Record<
    AssetGroupField,
    "primary" | "tag" | "tone" | "saturation" | "color" | null
  > = {
    primary_category: "primary",
    auxiliary_tag: "tag",
    tone: "tone",
    saturation_level: "saturation",
    dominant_color: "color",
    rating: null,
  };
  const kind = kindByGroup[groupBy];
  return kind ? classificationValueLabel(group.key, kind, semanticCatalog) : group.key;
}
