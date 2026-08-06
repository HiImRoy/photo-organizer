import { useEffect, useMemo, useState } from "react";

import {
  cancelLibraryScan,
  cancelSemanticAnalysis,
  chooseLibraryFolder,
  fetchAssets,
  fetchLibraries,
  fetchLibraryFolders,
  fetchSemanticCatalog,
  fetchSemanticGroups,
  fetchSemanticProgress,
  fetchSemanticStatus,
  pauseSemanticAnalysis,
  prepareSemanticModel,
  reanalyzeAsset,
  resumeSemanticAnalysis,
  startLibraryScan,
  startSemanticAnalysis,
  subscribeScanProgress,
  subscribeSemanticProgress,
} from "./api";
import { AssetCard } from "./components/AssetCard";
import { DetailPanel } from "./components/DetailPanel";
import {
  FilterIcon,
  GridIcon,
  ImportIcon,
  LibraryIcon,
  PauseIcon,
  PlayIcon,
  SearchIcon,
  SingleImageIcon,
  SortIcon,
} from "./components/Icons";
import { ProgressPanel } from "./components/ProgressPanel";
import { Sidebar } from "./components/Sidebar";
import { Thumbnail } from "./components/Thumbnail";
import { formatDate } from "./format";
import {
  emptyAssetFilter,
  type AssetFilter,
  type AssetListItem,
  type FolderSummary,
  type LibrarySummary,
  type ScanProgress,
  type SemanticGroupSummary,
  type SemanticLabelDescriptor,
  type SemanticProgress,
  type SemanticRuntimeStatus,
  type SortDirection,
  type SortField,
  type ViewMode,
} from "./types";

const PAGE_SIZE = 120;

const sortLabels: Record<SortField, string> = {
  file_name: "文件名",
  capture_time: "拍摄时间",
  modified_time: "修改时间",
  brightness: "亮度",
  saturation: "饱和度",
};

export default function App() {
  const [libraries, setLibraries] = useState<LibrarySummary[]>([]);
  const [selectedLibraryId, setSelectedLibraryId] = useState<number | null>(null);
  const [assets, setAssets] = useState<AssetListItem[]>([]);
  const [assetTotal, setAssetTotal] = useState(0);
  const [folders, setFolders] = useState<FolderSummary[]>([]);
  const [semanticGroups, setSemanticGroups] = useState<SemanticGroupSummary[]>([]);
  const [semanticCatalog, setSemanticCatalog] = useState<SemanticLabelDescriptor[]>([]);
  const [selectedAsset, setSelectedAsset] = useState<AssetListItem | null>(null);
  const [sort, setSort] = useState<SortField>("capture_time");
  const [direction, setDirection] = useState<SortDirection>("desc");
  const [filter, setFilterState] = useState<AssetFilter>(emptyAssetFilter);
  const [page, setPage] = useState(1);
  const [viewMode, setViewMode] = useState<ViewMode>("grid");
  const [groupBySemantic, setGroupBySemantic] = useState(false);
  const [leftCollapsed, setLeftCollapsed] = useState(false);
  const [rightCollapsed, setRightCollapsed] = useState(false);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [scanProgress, setScanProgress] = useState<ScanProgress | null>(null);
  const [semanticProgress, setSemanticProgress] = useState<SemanticProgress | null>(null);
  const [cancellingScan, setCancellingScan] = useState(false);
  const [semanticStatus, setSemanticStatus] = useState<SemanticRuntimeStatus | null>(null);
  const [refreshKey, setRefreshKey] = useState(0);

  const selectedLibrary = useMemo(
    () => libraries.find((library) => library.id === selectedLibraryId) ?? null,
    [libraries, selectedLibraryId],
  );
  const totalPages = Math.max(1, Math.ceil(assetTotal / PAGE_SIZE));
  const scanRunning =
    scanProgress !== null && ["running", "cancelling"].includes(scanProgress.status);
  const semanticRunning =
    semanticProgress !== null &&
    ["queued", "running", "paused", "cancelling"].includes(semanticProgress.status);
  const activeFilterCount = countActiveFilters(filter);
  const libraryName = selectedLibrary
    ? selectedLibrary.rootPath.split(/[\\/]/).at(-1) || selectedLibrary.rootPath
    : "PhotoOrganizer";

  useEffect(() => {
    let active = true;
    void Promise.allSettled([fetchLibraries(), fetchSemanticStatus(), fetchSemanticCatalog()]).then(
      ([libraryResult, statusResult, catalogResult]) => {
        if (!active) return;
        if (libraryResult.status === "fulfilled") {
          setLibraries(libraryResult.value);
          setSelectedLibraryId(libraryResult.value[0]?.id ?? null);
        } else {
          setError(messageFrom(libraryResult.reason));
        }
        if (statusResult.status === "fulfilled") setSemanticStatus(statusResult.value);
        if (catalogResult.status === "fulfilled") setSemanticCatalog(catalogResult.value);
        setLoading(false);
      },
    );
    return () => {
      active = false;
    };
  }, []);

  useEffect(() => {
    let active = true;
    if (selectedLibraryId === null) {
      return undefined;
    }
    void fetchAssets({
      libraryId: selectedLibraryId,
      sort,
      direction,
      page,
      pageSize: PAGE_SIZE,
      filter,
    })
      .then((result) => {
        if (!active) return;
        setAssets(result.items);
        setAssetTotal(result.total);
        setSelectedAsset((current) => {
          const restored = current && result.items.find((item) => item.id === current.id);
          return restored ?? (viewMode === "single" ? (result.items[0] ?? null) : null);
        });
      })
      .catch((reason: unknown) => {
        if (active) setError(messageFrom(reason));
      })
      .finally(() => {
        if (active) setLoading(false);
      });
    return () => {
      active = false;
    };
  }, [direction, filter, page, refreshKey, selectedLibraryId, sort, viewMode]);

  useEffect(() => {
    let active = true;
    if (selectedLibraryId === null) return undefined;
    void Promise.all([
      fetchLibraryFolders(selectedLibraryId),
      fetchSemanticGroups(selectedLibraryId),
      fetchSemanticProgress(selectedLibraryId),
    ])
      .then(([nextFolders, nextGroups, progress]) => {
        if (!active) return;
        setFolders(nextFolders);
        setSemanticGroups(nextGroups);
        setSemanticProgress(progress);
      })
      .catch((reason: unknown) => {
        if (active) setError(messageFrom(reason));
      });
    return () => {
      active = false;
    };
  }, [refreshKey, selectedLibraryId]);

  useEffect(() => {
    let disposed = false;
    let stopScan: (() => void) | undefined;
    let stopSemantic: (() => void) | undefined;
    void subscribeScanProgress((progress) => {
      if (disposed) return;
      setScanProgress(progress);
      if (progress.libraryId !== null) setSelectedLibraryId(progress.libraryId);
      if (
        progress.status !== "running" ||
        progress.processed === 1 ||
        progress.processed % 4 === 0
      ) {
        setRefreshKey((value) => value + 1);
      }
      if (["completed", "cancelled", "failed"].includes(progress.status)) setCancellingScan(false);
    }).then((stop) => {
      if (disposed) stop();
      else stopScan = stop;
    });
    void subscribeSemanticProgress((progress) => {
      if (disposed) return;
      setSemanticProgress(progress);
      if (
        progress.status !== "running" ||
        progress.processed === 1 ||
        progress.processed % 2 === 0
      ) {
        setRefreshKey((value) => value + 1);
      }
    }).then((stop) => {
      if (disposed) stop();
      else stopSemantic = stop;
    });
    return () => {
      disposed = true;
      stopScan?.();
      stopSemantic?.();
    };
  }, []);

  function updateFilter(next: AssetFilter) {
    setFilterState(next);
    setPage(1);
  }

  async function importFolder() {
    setError(null);
    try {
      const rootPath = await chooseLibraryFolder();
      if (!rootPath) return;
      const result = await startLibraryScan(rootPath);
      setScanProgress({
        taskId: result.taskId,
        libraryId: null,
        status: "running",
        stage: "preparing",
        discovered: 0,
        processed: 0,
        succeeded: 0,
        failed: 0,
        skipped: 0,
        missing: 0,
        currentPath: rootPath,
        error: null,
      });
    } catch (reason) {
      setError(messageFrom(reason));
    }
  }

  async function cancelScan() {
    if (!scanProgress) return;
    setCancellingScan(true);
    try {
      const result = await cancelLibraryScan(scanProgress.taskId);
      if (!result.accepted) setError("扫描任务已经结束，无法再次取消。");
    } catch (reason) {
      setError(messageFrom(reason));
      setCancellingScan(false);
    }
  }

  async function prepareOrAnalyze() {
    setError(null);
    try {
      if (semanticStatus?.status !== "ready") {
        const status = await prepareSemanticModel();
        setSemanticStatus(status);
        return;
      }
      if (selectedLibraryId === null) return;
      const { jobId } = await startSemanticAnalysis(selectedLibraryId);
      setSemanticProgress(pendingSemanticProgress(jobId, selectedLibraryId, semanticStatus));
    } catch (reason) {
      setError(messageFrom(reason));
    }
  }

  async function analyzeOne(asset: AssetListItem) {
    try {
      const { jobId } = await reanalyzeAsset(asset.libraryId, asset.id);
      setSemanticProgress(pendingSemanticProgress(jobId, asset.libraryId, semanticStatus));
    } catch (reason) {
      setError(messageFrom(reason));
    }
  }

  async function pauseOrResumeSemantic() {
    if (!semanticProgress) return;
    try {
      if (semanticProgress.status === "paused") {
        const result = await resumeSemanticAnalysis(semanticProgress.jobId);
        if (result.accepted) setSemanticProgress({ ...semanticProgress, status: "running" });
      } else {
        const result = await pauseSemanticAnalysis(semanticProgress.jobId);
        if (result.accepted) setSemanticProgress({ ...semanticProgress, status: "paused" });
      }
    } catch (reason) {
      setError(messageFrom(reason));
    }
  }

  async function cancelSemantic() {
    if (!semanticProgress) return;
    try {
      const result = await cancelSemanticAnalysis(semanticProgress.jobId);
      if (result.accepted) setSemanticProgress({ ...semanticProgress, status: "cancelling" });
    } catch (reason) {
      setError(messageFrom(reason));
    }
  }

  function selectLibrary(id: number) {
    setSelectedLibraryId(id);
    setSelectedAsset(null);
    setFilterState(emptyAssetFilter);
    setPage(1);
  }

  function changeView(next: ViewMode) {
    setViewMode(next);
    if (next === "single" && !selectedAsset) setSelectedAsset(assets[0] ?? null);
  }

  return (
    <div
      className={`photo-app left-${leftCollapsed ? "closed" : "open"} right-${rightCollapsed ? "closed" : "open"}`}
    >
      <header className="topbar">
        <div className="app-identity">
          <span className="brand-mark">
            <LibraryIcon width="17" height="17" />
          </span>
          <div>
            <strong title={selectedLibrary?.rootPath}>{libraryName}</strong>
            <small>PhotoOrganizer</small>
          </div>
        </div>
        <div className="library-stat">
          <strong>{assetTotal.toLocaleString()}</strong>
          <span>张图片</span>
          <i />
          <span>
            {scanRunning
              ? "正在扫描"
              : selectedLibrary
                ? `最近扫描 ${formatDate(selectedLibrary.lastScanAt)}`
                : "尚未建立图库"}
          </span>
        </div>
        <div className="topbar-actions">
          <div className="segmented" aria-label="视图模式">
            <button
              type="button"
              className={viewMode === "grid" ? "is-active" : ""}
              onClick={() => changeView("grid")}
              aria-label="网格视图"
            >
              <GridIcon width="16" height="16" />
            </button>
            <button
              type="button"
              className={viewMode === "single" ? "is-active" : ""}
              onClick={() => changeView("single")}
              aria-label="单图预览"
            >
              <SingleImageIcon width="16" height="16" />
            </button>
          </div>
          <label className="search-control">
            <SearchIcon width="15" height="15" />
            <input
              aria-label="搜索图片"
              placeholder="搜索文件名或路径"
              value={filter.search ?? ""}
              onChange={(event) => updateFilter({ ...filter, search: event.target.value || null })}
            />
          </label>
          <button
            className={activeFilterCount ? "tool-button is-active" : "tool-button"}
            type="button"
            onClick={() => setLeftCollapsed(false)}
          >
            <FilterIcon width="15" height="15" />
            筛选{activeFilterCount ? ` ${activeFilterCount}` : ""}
          </button>
          <label className="sort-select">
            <SortIcon width="15" height="15" />
            <span className="sr-only">排序</span>
            <select
              aria-label="排序"
              value={sort}
              onChange={(event) => {
                setSort(event.target.value as SortField);
                setPage(1);
              }}
            >
              {Object.entries(sortLabels).map(([value, label]) => (
                <option key={value} value={value}>
                  {label}
                </option>
              ))}
            </select>
          </label>
          <button
            className="direction-control"
            type="button"
            onClick={() => setDirection((value) => (value === "asc" ? "desc" : "asc"))}
            aria-label="切换排序方向"
          >
            {direction === "asc" ? "↑" : "↓"}
          </button>
          <button
            className="tool-button"
            type="button"
            onClick={() => void importFolder()}
            disabled={scanRunning}
          >
            <ImportIcon width="15" height="15" />
            导入
          </button>
          <button
            className="primary-action"
            type="button"
            onClick={() => void prepareOrAnalyze()}
            disabled={!selectedLibrary || semanticRunning}
          >
            <PlayIcon width="14" height="14" />
            {semanticStatus?.status === "ready" ? "语义分析" : "准备模型"}
          </button>
        </div>
      </header>

      <div className="workspace-shell">
        <Sidebar
          collapsed={leftCollapsed}
          libraries={libraries}
          selectedLibraryId={selectedLibraryId}
          folders={folders}
          groups={semanticGroups}
          catalog={semanticCatalog}
          filter={filter}
          semanticStatus={semanticStatus}
          onToggle={() => setLeftCollapsed((value) => !value)}
          onSelectLibrary={selectLibrary}
          onFilterChange={updateFilter}
        />

        <main className="center-workspace">
          {error ? (
            <div className="global-error" role="alert">
              <span>{error}</span>
              <button type="button" onClick={() => setError(null)}>
                关闭
              </button>
            </div>
          ) : null}
          {scanProgress ? (
            <ProgressPanel
              progress={scanProgress}
              cancelling={cancellingScan}
              onCancel={() => void cancelScan()}
              onDismiss={() => setScanProgress(null)}
            />
          ) : null}
          {semanticProgress && semanticRunning ? (
            <SemanticTaskBar
              progress={semanticProgress}
              onPauseResume={() => void pauseOrResumeSemantic()}
              onCancel={() => void cancelSemantic()}
            />
          ) : null}

          {selectedLibrary ? (
            <>
              <div className="content-toolbar">
                <div>
                  <strong>{assetTotal.toLocaleString()} 张</strong>
                  <span>
                    {activeFilterCount ? `已应用 ${activeFilterCount} 项筛选` : "全部图片"}
                  </span>
                </div>
                <div>
                  {activeFilterCount ? (
                    <button type="button" onClick={() => updateFilter(emptyAssetFilter)}>
                      清除筛选
                    </button>
                  ) : null}
                  <label className="group-toggle">
                    <input
                      type="checkbox"
                      checked={groupBySemantic}
                      onChange={(event) => setGroupBySemantic(event.target.checked)}
                    />
                    按主要语义标签分组
                  </label>
                </div>
              </div>
              {loading && assets.length === 0 ? (
                <GridLoading />
              ) : viewMode === "single" ? (
                <SinglePreview
                  assets={assets}
                  selected={selectedAsset}
                  onSelect={setSelectedAsset}
                />
              ) : assets.length ? (
                <GridWorkspace
                  assets={assets}
                  selected={selectedAsset}
                  grouped={groupBySemantic}
                  onSelect={setSelectedAsset}
                />
              ) : (
                <section className="library-empty">
                  <SingleImageIcon width="28" height="28" />
                  <h2>没有符合条件的图片</h2>
                  <p>
                    {activeFilterCount
                      ? "调整或清除组合筛选后重试。"
                      : "重新扫描目录，或导入包含 JPEG、PNG、WebP 的文件夹。"}
                  </p>
                </section>
              )}
              {totalPages > 1 ? (
                <nav className="pagination" aria-label="图库分页">
                  <button
                    type="button"
                    disabled={page <= 1}
                    onClick={() => setPage((value) => Math.max(1, value - 1))}
                  >
                    上一页
                  </button>
                  <span>
                    {page} / {totalPages}
                  </span>
                  <button
                    type="button"
                    disabled={page >= totalPages}
                    onClick={() => setPage((value) => Math.min(totalPages, value + 1))}
                  >
                    下一页
                  </button>
                </nav>
              ) : null}
            </>
          ) : !loading ? (
            <section className="welcome-state">
              <LibraryIcon width="30" height="30" />
              <div>
                <small>本地优先图片工作台</small>
                <h1>建立本地图片库</h1>
                <p>
                  递归索引、生成私有缩略图，并在本机完成影调、色彩和真实语义分析。浏览过程不会修改原始图片。
                </p>
                <button
                  className="primary-action"
                  type="button"
                  onClick={() => void importFolder()}
                >
                  <ImportIcon width="16" height="16" />
                  导入图片文件夹
                </button>
                <span>JPEG · PNG · WebP · 支持 Unicode 路径</span>
              </div>
            </section>
          ) : null}
        </main>

        <DetailPanel
          asset={selectedAsset}
          collapsed={rightCollapsed}
          semanticStatus={semanticStatus}
          onToggle={() => setRightCollapsed((value) => !value)}
          onReanalyze={(asset) => void analyzeOne(asset)}
        />
      </div>
    </div>
  );
}

function GridWorkspace({
  assets,
  selected,
  grouped,
  onSelect,
}: {
  assets: AssetListItem[];
  selected: AssetListItem | null;
  grouped: boolean;
  onSelect: (asset: AssetListItem) => void;
}) {
  if (!grouped)
    return (
      <section className="asset-grid" aria-label="图片网格">
        {assets.map((asset) => (
          <AssetCard
            key={asset.id}
            asset={asset}
            selected={selected?.id === asset.id}
            onSelect={onSelect}
          />
        ))}
      </section>
    );
  const sections = new Map<string, AssetListItem[]>();
  for (const asset of assets) {
    const primary = asset.semanticLabels.find((label) => label.isPrimary);
    const key = primary?.labelId ?? "not_analyzed";
    const section = sections.get(key) ?? [];
    section.push(asset);
    sections.set(key, section);
  }
  return (
    <div className="semantic-groups">
      {[...sections].map(([id, items]) => {
        const label =
          items[0]?.semanticLabels.find((item) => item.isPrimary)?.displayName ?? "未分析";
        return (
          <section key={id}>
            <div className="group-heading">
              <strong>{label}</strong>
              <span>{items.length} 张</span>
            </div>
            <div className="asset-grid">
              {items.map((asset) => (
                <AssetCard
                  key={asset.id}
                  asset={asset}
                  selected={selected?.id === asset.id}
                  onSelect={onSelect}
                />
              ))}
            </div>
          </section>
        );
      })}
    </div>
  );
}

function SinglePreview({
  assets,
  selected,
  onSelect,
}: {
  assets: AssetListItem[];
  selected: AssetListItem | null;
  onSelect: (asset: AssetListItem) => void;
}) {
  return (
    <section className="single-workspace">
      {selected ? (
        <div className="single-canvas">
          <Thumbnail asset={selected} />
          <div className="single-caption">
            <strong>{selected.fileName}</strong>
            <span>
              {selected.width && selected.height
                ? `${selected.width} × ${selected.height}`
                : "尺寸未知"}
            </span>
          </div>
        </div>
      ) : (
        <div className="single-empty">没有可预览的图片</div>
      )}
      <div className="filmstrip" aria-label="胶片栏">
        {assets.map((asset) => (
          <button
            type="button"
            key={asset.id}
            className={selected?.id === asset.id ? "is-active" : ""}
            onClick={() => onSelect(asset)}
            aria-label={asset.fileName}
          >
            <Thumbnail asset={asset} />
          </button>
        ))}
      </div>
    </section>
  );
}

function SemanticTaskBar({
  progress,
  onPauseResume,
  onCancel,
}: {
  progress: SemanticProgress;
  onPauseResume: () => void;
  onCancel: () => void;
}) {
  const percent = progress.total > 0 ? Math.round((progress.processed / progress.total) * 100) : 0;
  return (
    <section className="semantic-taskbar">
      <div>
        <PlayIcon width="15" height="15" />
        <span>
          <strong>
            {progress.status === "paused" ? "语义分析已暂停" : "正在进行真实语义分析"}
          </strong>
          <small>
            {progress.processed} / {progress.total} ·{" "}
            {progress.executionBackend?.toUpperCase() ?? "CPU"} · 失败 {progress.failed}
          </small>
        </span>
      </div>
      <div className="task-progress">
        <i style={{ width: `${percent}%` }} />
      </div>
      <div>
        <button type="button" onClick={onPauseResume}>
          {progress.status === "paused" ? (
            <PlayIcon width="13" height="13" />
          ) : (
            <PauseIcon width="13" height="13" />
          )}
          {progress.status === "paused" ? "继续" : "暂停"}
        </button>
        <button type="button" onClick={onCancel}>
          取消
        </button>
      </div>
    </section>
  );
}

function GridLoading() {
  return (
    <div className="grid-loading" aria-label="正在加载图库">
      {Array.from({ length: 12 }, (_, index) => (
        <span key={index} />
      ))}
    </div>
  );
}

function countActiveFilters(filter: AssetFilter) {
  return (
    Number(Boolean(filter.search)) +
    filter.semanticLabels.length +
    filter.toneLabels.length +
    filter.colorCategories.length +
    Number(filter.brightnessMin !== null || filter.brightnessMax !== null) +
    Number(filter.saturationMin !== null || filter.saturationMax !== null) +
    Number(Boolean(filter.capturedFrom || filter.capturedTo)) +
    Number(Boolean(filter.folderPrefix)) +
    Number(Boolean(filter.semanticState))
  );
}

function pendingSemanticProgress(
  jobId: string,
  libraryId: number,
  status: SemanticRuntimeStatus | null,
): SemanticProgress {
  return {
    jobId,
    libraryId,
    status: "queued",
    total: 0,
    processed: 0,
    completed: 0,
    failed: 0,
    skipped: 0,
    currentAssetId: null,
    currentPath: null,
    executionBackend: status?.selectedBackend ?? "cpu",
    modelName: status?.model.name ?? "TinyCLIP",
    modelVersion: status?.model.version ?? "",
    error: null,
  };
}

function messageFrom(reason: unknown): string {
  if (reason instanceof Error) return reason.message;
  if (typeof reason === "string") return reason;
  return "发生未知错误，请查看应用日志。";
}
