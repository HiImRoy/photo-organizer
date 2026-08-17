import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import {
  addAssetsToCollection,
  chooseEditedCopyTarget,
  createCollection,
  deleteCollection,
  executeEditExport,
  executeEditRollback,
  fetchCollection,
  fetchCollections,
  fetchDuplicateGroups,
  fetchFavoriteAssets,
  fetchPreview,
  fetchSimilarAssets,
  fetchSimilarityClusters,
  fetchThumbnail,
  previewEditExport,
  previewEditRollback,
  removeAssetsFromCollection,
  renderEditPreview,
  searchLocalImages,
  setAssetFavorite,
} from "../api";
import {
  emptyEditRecipe,
  type AssetListItem,
  type CollectionDetail,
  type CollectionSummary,
  type DuplicateGroup,
  type EditExportPlan,
  type EditExportResult,
  type EditRecipe,
  type EditRollbackPlan,
  type LocalSearchResponse,
  type AssetScopeDescription,
  type AssetScopeInputV1,
  type SimilarAsset,
  type SimilarityClusterResponse,
  type WorkflowAsset,
} from "../types";

export type WorkflowTool =
  "favorites" | "collections" | "search" | "duplicates" | "similar" | "compare" | "edit";

interface WorkflowWorkspaceProps {
  libraryId: number;
  selectedAssetIds: number[];
  activeAsset: AssetListItem | null;
  scope: AssetScopeInputV1;
  scopeDescription: AssetScopeDescription;
  onSelectAsset: (assetId: number) => void;
  onBack: () => void;
  onFavoriteChange: (assetId: number, favorite: boolean) => void;
  onCollectionSourceChange?: (collectionId: number) => void;
  onCollectionsChange?: () => void;
  initialTool?: WorkflowTool;
  embedded?: boolean;
}

const tabs: ReadonlyArray<{ id: WorkflowTool; label: string; note: string }> = [
  { id: "favorites", label: "收藏", note: "独立于星级" },
  { id: "collections", label: "集合", note: "虚拟分组" },
  { id: "search", label: "AI 搜索", note: "完全本地" },
  { id: "duplicates", label: "重复清理", note: "只生成审阅集" },
  { id: "similar", label: "相似聚类", note: "当前题材模型向量" },
  { id: "compare", label: "比较", note: "最多四张" },
  { id: "edit", label: "图像编辑", note: "非破坏性" },
];

export function WorkflowWorkspace({
  libraryId,
  selectedAssetIds,
  activeAsset,
  scope,
  scopeDescription,
  onSelectAsset,
  onBack,
  onFavoriteChange,
  onCollectionSourceChange,
  onCollectionsChange,
  initialTool = "search",
  embedded = false,
}: WorkflowWorkspaceProps) {
  const [tab, setTab] = useState<WorkflowTool>(initialTool);
  const [favorites, setFavorites] = useState<WorkflowAsset[]>([]);
  const [collections, setCollections] = useState<CollectionSummary[]>([]);
  const [collection, setCollection] = useState<CollectionDetail | null>(null);
  const [duplicates, setDuplicates] = useState<DuplicateGroup[]>([]);
  const [searchResult, setSearchResult] = useState<LocalSearchResponse | null>(null);
  const [similarAssets, setSimilarAssets] = useState<SimilarAsset[]>([]);
  const [clusters, setClusters] = useState<SimilarityClusterResponse | null>(null);
  const [busy, setBusy] = useState(true);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const activeTab = tabs.find((item) => item.id === tab) ?? tabs[0];

  useEffect(() => {
    let cancelled = false;
    void Promise.all([fetchFavoriteAssets(libraryId), fetchCollections()])
      .then(([favoriteItems, collectionItems]) => {
        if (cancelled) return;
        setFavorites(favoriteItems);
        setCollections(collectionItems);
      })
      .catch((reason) => {
        if (!cancelled) setError(messageFrom(reason));
      })
      .finally(() => {
        if (!cancelled) setBusy(false);
      });
    return () => {
      cancelled = true;
    };
  }, [libraryId]);

  useEffect(() => {
    if (tab !== "duplicates" || duplicates.length > 0) return;
    void fetchDuplicateGroups(libraryId)
      .then(setDuplicates)
      .catch((reason) => setError(messageFrom(reason)))
      .finally(() => setBusy(false));
  }, [duplicates.length, libraryId, tab]);

  const run = useCallback(async (operation: () => Promise<void>) => {
    setBusy(true);
    setError(null);
    setMessage(null);
    try {
      await operation();
    } catch (reason) {
      setError(messageFrom(reason));
    } finally {
      setBusy(false);
    }
  }, []);

  const selectCollection = useCallback(
    (collectionId: number) =>
      run(async () => {
        setCollection(await fetchCollection(collectionId));
        if (!embedded) onCollectionSourceChange?.(collectionId);
      }),
    [embedded, onCollectionSourceChange, run],
  );

  return (
    <section
      className={`workflow-workspace${embedded ? " workflow-workspace-embedded" : ""}`}
      aria-label="查找与审阅"
    >
      {!embedded ? (
        <aside className="workflow-nav">
          <div className="workflow-nav-heading">
            <small>QUERY / REVIEW CONTEXT</small>
            <h2>查找与审阅</h2>
            <p>原图只读，收藏与集合只写入本地数据库。</p>
          </div>
          <div className="workflow-scope-summary">
            <strong>{scopeDescription.label}</strong>
            <span>
              {scopeDescription.count.toLocaleString()} 张 ·
              {scope.kind === "selection" ? "当前选择范围" : "当前查询范围"}
            </span>
            <small>{scope.kind === "selection" ? "显式选择范围" : "动态查询范围"}</small>
          </div>
          <nav aria-label="工作流工具">
            {tabs.map((item) => (
              <button
                type="button"
                key={item.id}
                className={tab === item.id ? "is-active" : ""}
                onClick={() => {
                  setTab(item.id);
                  setError(null);
                  setMessage(null);
                }}
              >
                <span>{item.label}</span>
                <small>{item.note}</small>
              </button>
            ))}
          </nav>
          <div className="workflow-selection-note">
            <strong>{selectedAssetIds.length}</strong>
            <span>张已选择</span>
            <small>比较、集合和编辑会复用当前范围中的显式选择。</small>
          </div>
          <button type="button" className="workflow-back-button" onClick={onBack}>
            返回图库
          </button>
        </aside>
      ) : null}

      <div className="workflow-content">
        <header className="workflow-header">
          <div>
            <small>{embedded ? `当前范围 · ${scopeDescription.label}` : activeTab.note}</small>
            <h1>{activeTab.label}</h1>
            {embedded ? (
              <span className="workflow-context-scope">
                {scopeDescription.count.toLocaleString()} 张 ·
                {scope.kind === "selection" ? "显式选择范围" : "当前查询范围"}
              </span>
            ) : null}
          </div>
          <div className="workflow-header-actions">
            <span className="workflow-safety-pill">LOCAL · SOURCE READ-ONLY</span>
            {embedded ? (
              <button type="button" className="workflow-back-button" onClick={onBack}>
                返回图库
              </button>
            ) : null}
          </div>
        </header>
        {error ? <div className="workflow-banner is-error">{error}</div> : null}
        {message ? <div className="workflow-banner">{message}</div> : null}
        {busy ? <div className="workflow-progress">正在本机处理…</div> : null}

        {tab === "favorites" ? (
          <FavoritesView
            assets={favorites}
            onSelect={onSelectAsset}
            onRemove={(assetId) =>
              void run(async () => {
                await setAssetFavorite(assetId, false);
                setFavorites((items) => items.filter((asset) => asset.id !== assetId));
                onFavoriteChange(assetId, false);
                setMessage("已从收藏移除；星级与分类未改变。");
              })
            }
          />
        ) : null}

        {tab === "collections" ? (
          <CollectionsView
            collections={collections}
            collection={collection}
            selectedAssetIds={selectedAssetIds}
            onSelect={onSelectAsset}
            onSelectCollection={selectCollection}
            onCreate={(name) =>
              run(async () => {
                const created = await createCollection(name);
                setCollections(await fetchCollections());
                setCollection(await fetchCollection(created.id));
                if (!embedded) onCollectionSourceChange?.(created.id);
                onCollectionsChange?.();
                setMessage(`已创建虚拟集合“${created.name}”。`);
              })
            }
            onDelete={(collectionId) =>
              run(async () => {
                await deleteCollection(collectionId);
                setCollection(null);
                setCollections(await fetchCollections());
                onCollectionsChange?.();
                setMessage("集合已删除，原始图片未发生变化。");
              })
            }
            onAddSelected={(collectionId) =>
              run(async () => {
                await addAssetsToCollection(collectionId, selectedAssetIds);
                setCollection(await fetchCollection(collectionId));
                setCollections(await fetchCollections());
                onCollectionsChange?.();
                setMessage(`已加入 ${selectedAssetIds.length} 张图片。`);
              })
            }
            onRemoveAsset={(collectionId, assetId) =>
              run(async () => {
                await removeAssetsFromCollection(collectionId, [assetId]);
                setCollection(await fetchCollection(collectionId));
                setCollections(await fetchCollections());
                onCollectionsChange?.();
              })
            }
          />
        ) : null}

        {tab === "search" ? (
          <SearchView
            result={searchResult}
            onSelect={onSelectAsset}
            onSearch={(query) =>
              run(async () => {
                const result = await searchLocalImages(libraryId, query);
                setSearchResult(result);
                setMessage(`已在 ${result.embeddedAssetCount} 张已分析图片中完成本地检索。`);
              })
            }
          />
        ) : null}

        {tab === "duplicates" ? (
          <DuplicatesView
            groups={duplicates}
            onSelect={onSelectAsset}
            onRefresh={() =>
              run(async () => {
                setDuplicates(await fetchDuplicateGroups(libraryId));
              })
            }
            onCreateReview={() =>
              run(async () => {
                const reviewIds = duplicates.flatMap((group) =>
                  group.assets.slice(1).map((a) => a.id),
                );
                if (reviewIds.length === 0) return;
                const name = `重复待处理 ${new Date().toLocaleDateString("zh-CN")}`;
                const existing = (await fetchCollections()).find((item) => item.name === name);
                const target =
                  existing ??
                  (await createCollection(name, "精确重复文件的非保留项；仅供人工审阅。"));
                await addAssetsToCollection(target.id, reviewIds);
                setCollections(await fetchCollections());
                setMessage(
                  `已把 ${reviewIds.length} 张非保留项加入“${target.name}”；未删除或移动文件。`,
                );
              })
            }
          />
        ) : null}

        {tab === "similar" ? (
          <SimilarityView
            activeAssetId={activeAsset?.id ?? selectedAssetIds[0] ?? null}
            similarAssets={similarAssets}
            clusters={clusters}
            onSelect={onSelectAsset}
            onFind={(assetId) =>
              run(async () => {
                setSimilarAssets(await fetchSimilarAssets(libraryId, assetId));
                setClusters(null);
              })
            }
            onCluster={(threshold) =>
              run(async () => {
                setClusters(await fetchSimilarityClusters(libraryId, threshold));
                setSimilarAssets([]);
              })
            }
          />
        ) : null}

        {tab === "compare" ? (
          <CompareView
            assetIds={uniqueIds([
              ...selectedAssetIds,
              ...(activeAsset ? [activeAsset.id] : []),
            ]).slice(0, 4)}
          />
        ) : null}

        {tab === "edit" ? (
          <EditorView
            key={activeAsset?.id ?? "no-asset"}
            asset={activeAsset}
            onMessage={setMessage}
            onError={setError}
          />
        ) : null}
      </div>
    </section>
  );
}

function FavoritesView({
  assets,
  onSelect,
  onRemove,
}: {
  assets: WorkflowAsset[];
  onSelect: (assetId: number) => void;
  onRemove: (assetId: number) => void;
}) {
  return (
    <div className="workflow-section">
      <SectionIntro
        title="值得回看的图片"
        body="收藏是独立布尔标记，不会改变 0–5 星评分。点击图片可回到详情上下文。"
        metric={`${assets.length} 张`}
      />
      {assets.length ? (
        <AssetMosaic
          assets={assets}
          onSelect={onSelect}
          actionLabel="移除收藏"
          onAction={onRemove}
        />
      ) : (
        <EmptyWorkflow title="还没有收藏" body="在图库卡片右上角点击心形按钮即可收藏。" />
      )}
    </div>
  );
}

function CollectionsView({
  collections,
  collection,
  selectedAssetIds,
  onSelect,
  onSelectCollection,
  onCreate,
  onDelete,
  onAddSelected,
  onRemoveAsset,
}: {
  collections: CollectionSummary[];
  collection: CollectionDetail | null;
  selectedAssetIds: number[];
  onSelect: (assetId: number) => void;
  onSelectCollection: (collectionId: number) => void;
  onCreate: (name: string) => Promise<void>;
  onDelete: (collectionId: number) => Promise<void>;
  onAddSelected: (collectionId: number) => Promise<void>;
  onRemoveAsset: (collectionId: number, assetId: number) => Promise<void>;
}) {
  const [name, setName] = useState("");
  return (
    <div className="workflow-section collection-layout">
      <div className="collection-rail">
        <form
          className="workflow-inline-form"
          onSubmit={(event) => {
            event.preventDefault();
            const nextName = name.trim();
            if (!nextName) return;
            setName("");
            void onCreate(nextName);
          }}
        >
          <input
            value={name}
            onChange={(event) => setName(event.target.value)}
            placeholder="新集合名称"
            maxLength={100}
            aria-label="新集合名称"
          />
          <button type="submit">新建</button>
        </form>
        <div className="collection-list">
          {collections.map((item) => (
            <button
              type="button"
              key={item.id}
              className={collection?.id === item.id ? "is-active" : ""}
              onClick={() => onSelectCollection(item.id)}
            >
              <span>{item.name}</span>
              <small>{item.assetCount}</small>
            </button>
          ))}
        </div>
      </div>
      <div className="collection-detail">
        {collection ? (
          <>
            <SectionIntro
              title={collection.name}
              body={collection.description || "虚拟集合不会移动、复制或改名原始图片。"}
              metric={`${collection.assetCount} 张`}
            />
            <div className="workflow-actions">
              <button
                type="button"
                disabled={selectedAssetIds.length === 0}
                onClick={() => void onAddSelected(collection.id)}
              >
                加入已选 {selectedAssetIds.length} 张
              </button>
              <button
                type="button"
                className="is-danger"
                onClick={() => void onDelete(collection.id)}
              >
                删除集合
              </button>
            </div>
            <AssetMosaic
              assets={collection.assets}
              onSelect={onSelect}
              actionLabel="移出集合"
              onAction={(assetId) => void onRemoveAsset(collection.id, assetId)}
            />
          </>
        ) : (
          <EmptyWorkflow title="选择或新建一个集合" body="先在图库多选图片，再将它们加入集合。" />
        )}
      </div>
    </div>
  );
}

function SearchView({
  result,
  onSearch,
  onSelect,
}: {
  result: LocalSearchResponse | null;
  onSearch: (query: string) => Promise<void>;
  onSelect: (assetId: number) => void;
}) {
  const [query, setQuery] = useState("");
  return (
    <div className="workflow-section">
      <SectionIntro
        title="用自然语言找图"
        body="查询和图片向量均在本机当前题材模型中计算，默认使用 SigLIP 2 Base。支持常见中文主题词映射，不发送到网络。"
        metric={result ? `${result.items.length} 个结果` : "LOCAL AI"}
      />
      <form
        className="ai-search-form"
        onSubmit={(event) => {
          event.preventDefault();
          if (query.trim()) void onSearch(query.trim());
        }}
      >
        <input
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder="例如：夜晚的建筑、花、portrait in warm light"
          aria-label="本地 AI 搜索"
        />
        <button type="submit">本地搜索</button>
      </form>
      {result ? (
        <>
          <p className="workflow-meta">
            模型查询：{result.normalizedQuery} · 已分析 {result.embeddedAssetCount} 张
          </p>
          <AssetMosaic assets={result.items} onSelect={onSelect} showSimilarity />
        </>
      ) : (
        <EmptyWorkflow
          title="输入画面描述"
          body="没有语义向量的图片不会参与搜索，可先在图库运行语义分析。"
        />
      )}
    </div>
  );
}

function DuplicatesView({
  groups,
  onSelect,
  onRefresh,
  onCreateReview,
}: {
  groups: DuplicateGroup[];
  onSelect: (assetId: number) => void;
  onRefresh: () => Promise<void>;
  onCreateReview: () => Promise<void>;
}) {
  const savings = groups.reduce((total, group) => total + group.reclaimableBytes, 0);
  return (
    <div className="workflow-section">
      <SectionIntro
        title="精确重复审阅"
        body="使用扫描阶段的全文件 BLAKE3 指纹。每组第一张按收藏、星级优先作为建议保留项。"
        metric={`${groups.length} 组 · ${formatBytes(savings)}`}
      />
      <div className="workflow-actions">
        <button type="button" onClick={() => void onRefresh()}>
          重新检查
        </button>
        <button type="button" disabled={!groups.length} onClick={() => void onCreateReview()}>
          生成“重复待处理”集合
        </button>
        <span>不会删除、移动或重命名任何原图。</span>
      </div>
      {groups.map((group, index) => (
        <article className="duplicate-group" key={group.fingerprint}>
          <header>
            <div>
              <strong>重复组 {index + 1}</strong>
              <small>{group.fingerprint.slice(0, 16)}…</small>
            </div>
            <span>
              {group.assets.length} 份 · 可审阅 {formatBytes(group.reclaimableBytes)}
            </span>
          </header>
          <AssetMosaic assets={group.assets} onSelect={onSelect} keeperFirst />
        </article>
      ))}
      {!groups.length ? (
        <EmptyWorkflow title="没有精确重复项" body="相似但不完全一致的图片请使用“相似聚类”。" />
      ) : null}
    </div>
  );
}

function SimilarityView({
  activeAssetId,
  similarAssets,
  clusters,
  onSelect,
  onFind,
  onCluster,
}: {
  activeAssetId: number | null;
  similarAssets: SimilarAsset[];
  clusters: SimilarityClusterResponse | null;
  onSelect: (assetId: number) => void;
  onFind: (assetId: number) => Promise<void>;
  onCluster: (threshold: number) => Promise<void>;
}) {
  const [threshold, setThreshold] = useState(0.92);
  return (
    <div className="workflow-section">
      <SectionIntro
        title="视觉相似图片"
        body="复用当前题材模型的图像向量；单图搜索使用精确余弦，相似组先按向量主维度召回再精排。"
        metric={clusters ? `${clusters.clusters.length} 组` : `${similarAssets.length} 张`}
      />
      <div className="workflow-actions similarity-actions">
        <button
          type="button"
          disabled={activeAssetId === null}
          onClick={() => activeAssetId !== null && void onFind(activeAssetId)}
        >
          查找当前图片的相似项
        </button>
        <label>
          聚类阈值 {threshold.toFixed(2)}
          <input
            type="range"
            min="0.82"
            max="0.98"
            step="0.01"
            value={threshold}
            onChange={(event) => setThreshold(Number(event.target.value))}
          />
        </label>
        <button type="button" onClick={() => void onCluster(threshold)}>
          构建相似组
        </button>
      </div>
      {similarAssets.length ? (
        <AssetMosaic assets={similarAssets} onSelect={onSelect} showSimilarity />
      ) : null}
      {clusters ? (
        <div className="cluster-list">
          <p className="workflow-meta">
            已分析 {clusters.embeddedAssetCount} 张 · 候选对 {clusters.candidatePairCount}
            {clusters.truncated ? " · 大图库已按 5000 张上限截断" : ""}
          </p>
          {clusters.clusters.map((cluster, index) => (
            <article className="duplicate-group" key={cluster.id}>
              <header>
                <strong>相似组 {index + 1}</strong>
                <span>{cluster.assets.length} 张</span>
              </header>
              <AssetMosaic assets={cluster.assets} onSelect={onSelect} showSimilarity />
            </article>
          ))}
        </div>
      ) : null}
      {!similarAssets.length && !clusters ? (
        <EmptyWorkflow title="选择参考图或构建相似组" body="图片需要先完成语义分析并保存向量。" />
      ) : null}
    </div>
  );
}

function CompareView({ assetIds }: { assetIds: number[] }) {
  const [previews, setPreviews] = useState<Record<number, string>>({});
  const [fit, setFit] = useState<"contain" | "cover">("contain");
  useEffect(() => {
    let cancelled = false;
    void Promise.all(
      assetIds.map(
        async (assetId) => [assetId, await fetchPreview(assetId, "screen", 1600, 1200)] as const,
      ),
    ).then((items) => {
      if (!cancelled) setPreviews(Object.fromEntries(items));
    });
    return () => {
      cancelled = true;
    };
  }, [assetIds]);
  return (
    <div className="workflow-section compare-section">
      <SectionIntro
        title="双图 / 四图比较"
        body="从图库多选 2–4 张图片。画布共享适应/填充模式，便于构图与细节审阅。"
        metric={`${assetIds.length} / 4`}
      />
      <div className="workflow-actions">
        <button
          type="button"
          className={fit === "contain" ? "is-active" : ""}
          onClick={() => setFit("contain")}
        >
          适应画布
        </button>
        <button
          type="button"
          className={fit === "cover" ? "is-active" : ""}
          onClick={() => setFit("cover")}
        >
          填充画布
        </button>
      </div>
      {assetIds.length >= 2 ? (
        <div className={`compare-grid compare-count-${assetIds.length}`}>
          {assetIds.map((assetId) => (
            <figure key={assetId}>
              {previews[assetId] ? (
                <img
                  src={previews[assetId]}
                  alt={`比较图片 ${assetId}`}
                  style={{ objectFit: fit }}
                />
              ) : (
                <span>加载预览…</span>
              )}
              <figcaption>#{assetId}</figcaption>
            </figure>
          ))}
        </div>
      ) : (
        <EmptyWorkflow
          title="请先选择至少两张图片"
          body="返回图库，使用 Ctrl/⌘ 或 Shift 多选后再打开比较工具。"
        />
      )}
    </div>
  );
}

function EditorView({
  asset,
  onMessage,
  onError,
}: {
  asset: AssetListItem | null;
  onMessage: (message: string | null) => void;
  onError: (message: string | null) => void;
}) {
  const [recipe, setRecipe] = useState<EditRecipe>(emptyEditRecipe);
  const [preview, setPreview] = useState<string | null>(null);
  const [plan, setPlan] = useState<EditExportPlan | null>(null);
  const [completedExport, setCompletedExport] = useState<EditExportResult | null>(null);
  const [rollbackPlan, setRollbackPlan] = useState<EditRollbackPlan | null>(null);
  const [busy, setBusy] = useState(false);
  const requestVersion = useRef(0);

  useEffect(() => {
    if (!asset) return;
    const version = ++requestVersion.current;
    const timer = window.setTimeout(() => {
      void renderEditPreview(asset.id, recipe)
        .then((url) => {
          if (requestVersion.current === version) setPreview(url);
        })
        .catch((reason) => {
          if (requestVersion.current === version) onError(messageFrom(reason));
        });
    }, 180);
    return () => window.clearTimeout(timer);
  }, [asset, onError, recipe]);

  const update = <K extends keyof EditRecipe>(key: K, value: EditRecipe[K]) => {
    setPlan(null);
    setCompletedExport(null);
    setRollbackPlan(null);
    setRecipe((current) => ({ ...current, [key]: value }));
  };

  const squareCrop = useMemo(() => {
    if (!asset?.width || !asset.height) return null;
    if (asset.width > asset.height) {
      const width = asset.height / asset.width;
      return { x: (1 - width) / 2, y: 0, width, height: 1 };
    }
    const height = asset.width / asset.height;
    return { x: 0, y: (1 - height) / 2, width: 1, height };
  }, [asset]);

  if (!asset) {
    return (
      <EmptyWorkflow
        title="选择一张图片开始编辑"
        body="编辑配方只用于预览和另存副本，绝不写回原图。"
      />
    );
  }

  return (
    <div className="workflow-section editor-layout">
      <div className="editor-canvas">
        {preview ? (
          <img src={preview} alt={`${asset.fileName} 编辑预览`} />
        ) : (
          <span>生成本地预览…</span>
        )}
        <div className="editor-canvas-meta">
          <strong>{asset.fileName}</strong>
          <span>源文件保持只读</span>
        </div>
      </div>
      <aside className="editor-controls">
        <SectionIntro
          title="非破坏性配方"
          body="旋转、翻转、裁剪与基础调色会在另存副本时应用。"
          metric="RECIPE"
        />
        <div className="editor-button-grid">
          <button
            type="button"
            onClick={() =>
              update(
                "rotateDegrees",
                ((recipe.rotateDegrees + 90) % 360) as EditRecipe["rotateDegrees"],
              )
            }
          >
            旋转 90°
          </button>
          <button
            type="button"
            className={recipe.flipHorizontal ? "is-active" : ""}
            onClick={() => update("flipHorizontal", !recipe.flipHorizontal)}
          >
            水平翻转
          </button>
          <button
            type="button"
            className={recipe.flipVertical ? "is-active" : ""}
            onClick={() => update("flipVertical", !recipe.flipVertical)}
          >
            垂直翻转
          </button>
          <button
            type="button"
            className={!recipe.crop ? "is-active" : ""}
            onClick={() => update("crop", null)}
          >
            原始比例
          </button>
          <button type="button" disabled={!squareCrop} onClick={() => update("crop", squareCrop)}>
            中心方形
          </button>
        </div>
        <Adjustment
          label="曝光"
          value={recipe.exposure}
          min={-2}
          max={2}
          step={0.1}
          onChange={(value) => update("exposure", value)}
        />
        <Adjustment
          label="对比度"
          value={recipe.contrast}
          min={-1}
          max={1}
          step={0.05}
          onChange={(value) => update("contrast", value)}
        />
        <Adjustment
          label="饱和度"
          value={recipe.saturation}
          min={-1}
          max={1}
          step={0.05}
          onChange={(value) => update("saturation", value)}
        />
        <div className="editor-export">
          <button
            type="button"
            disabled={busy}
            onClick={() => {
              setBusy(true);
              setPlan(null);
              setCompletedExport(null);
              setRollbackPlan(null);
              onError(null);
              void chooseEditedCopyTarget(asset.fileName)
                .then((target) => (target ? previewEditExport(asset.id, target, recipe) : null))
                .then((nextPlan) => setPlan(nextPlan))
                .catch((reason) => onError(messageFrom(reason)))
                .finally(() => setBusy(false));
            }}
          >
            {busy ? "准备中…" : "预览另存计划"}
          </button>
          <button type="button" onClick={() => setRecipe(emptyEditRecipe)}>
            重置配方
          </button>
        </div>
        {plan ? (
          <div className="export-confirmation">
            <strong>确认创建新文件</strong>
            <span title={plan.targetPath}>{plan.targetPath}</span>
            <small>目标已校验：不存在、位于图库根目录之外。执行前会再次校验源指纹。</small>
            <button
              type="button"
              onClick={() => {
                setBusy(true);
                void executeEditExport(plan.planId)
                  .then((result) => {
                    setPlan(null);
                    setCompletedExport(result);
                    onMessage(`编辑副本已创建：${result.targetPath}`);
                  })
                  .catch((reason) => onError(messageFrom(reason)))
                  .finally(() => setBusy(false));
              }}
            >
              确认另存副本
            </button>
          </div>
        ) : null}
        {completedExport ? (
          <div className="export-confirmation">
            <strong>副本已创建，可安全回滚</strong>
            <span title={completedExport.targetPath}>{completedExport.targetPath}</span>
            <small>回滚前会重新校验目标哈希；如果文件已被修改，将拒绝删除。</small>
            {rollbackPlan ? (
              <button
                type="button"
                onClick={() => {
                  setBusy(true);
                  void executeEditRollback(rollbackPlan.planId)
                    .then(() => {
                      setCompletedExport(null);
                      setRollbackPlan(null);
                      onMessage("编辑副本已回滚；原图未改变。");
                    })
                    .catch((reason) => onError(messageFrom(reason)))
                    .finally(() => setBusy(false));
                }}
              >
                确认删除该生成副本
              </button>
            ) : (
              <button
                type="button"
                onClick={() => {
                  setBusy(true);
                  void previewEditRollback(completedExport.planId)
                    .then(setRollbackPlan)
                    .catch((reason) => onError(messageFrom(reason)))
                    .finally(() => setBusy(false));
                }}
              >
                预览撤销
              </button>
            )}
          </div>
        ) : null}
      </aside>
    </div>
  );
}

function AssetMosaic({
  assets,
  onSelect,
  actionLabel,
  onAction,
  showSimilarity = false,
  keeperFirst = false,
}: {
  assets: WorkflowAsset[] | SimilarAsset[];
  onSelect: (assetId: number) => void;
  actionLabel?: string;
  onAction?: (assetId: number) => void;
  showSimilarity?: boolean;
  keeperFirst?: boolean;
}) {
  return (
    <div className="workflow-mosaic">
      {assets.map((asset, index) => (
        <article className="workflow-asset" key={asset.id}>
          <button
            type="button"
            className="workflow-asset-preview"
            onClick={() => onSelect(asset.id)}
          >
            <WorkflowThumbnail assetId={asset.id} fileName={asset.fileName} />
            {keeperFirst && index === 0 ? <span className="keeper-badge">建议保留</span> : null}
            {showSimilarity && "similarity" in asset ? (
              <span className="similarity-badge">{Math.round(asset.similarity * 100)}%</span>
            ) : null}
          </button>
          <div className="workflow-asset-meta">
            <strong title={asset.fileName}>{asset.fileName}</strong>
            <span>
              {asset.width && asset.height
                ? `${asset.width} × ${asset.height}`
                : formatBytes(asset.fileSize)}
            </span>
          </div>
          {actionLabel && onAction ? (
            <button
              type="button"
              className="workflow-asset-action"
              onClick={() => onAction(asset.id)}
            >
              {actionLabel}
            </button>
          ) : null}
        </article>
      ))}
    </div>
  );
}

function WorkflowThumbnail({ assetId, fileName }: { assetId: number; fileName: string }) {
  const [source, setSource] = useState<string | null>(null);
  useEffect(() => {
    let cancelled = false;
    void fetchThumbnail(assetId)
      .then((url) => {
        if (!cancelled) setSource(url);
      })
      .catch(() => {
        if (!cancelled) setSource(null);
      });
    return () => {
      cancelled = true;
    };
  }, [assetId]);
  return source ? (
    <img src={source} alt={fileName} loading="lazy" />
  ) : (
    <span className="workflow-thumb-placeholder">IMG</span>
  );
}

function SectionIntro({ title, body, metric }: { title: string; body: string; metric: string }) {
  return (
    <header className="workflow-section-intro">
      <div>
        <h2>{title}</h2>
        <p>{body}</p>
      </div>
      <strong>{metric}</strong>
    </header>
  );
}

function EmptyWorkflow({ title, body }: { title: string; body: string }) {
  return (
    <div className="workflow-empty">
      <span>◎</span>
      <strong>{title}</strong>
      <p>{body}</p>
    </div>
  );
}

function Adjustment({
  label,
  value,
  min,
  max,
  step,
  onChange,
}: {
  label: string;
  value: number;
  min: number;
  max: number;
  step: number;
  onChange: (value: number) => void;
}) {
  return (
    <label className="editor-adjustment">
      <span>
        {label}
        <strong>
          {value > 0 ? "+" : ""}
          {value.toFixed(2)}
        </strong>
      </span>
      <input
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        onChange={(event) => onChange(Number(event.target.value))}
      />
    </label>
  );
}

function uniqueIds(ids: number[]): number[] {
  return [...new Set(ids)];
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 ** 2) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 ** 3) return `${(bytes / 1024 ** 2).toFixed(1)} MB`;
  return `${(bytes / 1024 ** 3).toFixed(1)} GB`;
}

function messageFrom(value: unknown): string {
  return value instanceof Error ? value.message : String(value);
}
