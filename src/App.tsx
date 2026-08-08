import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import {
  cancelLibraryScan,
  cancelSemanticAnalysis,
  assignAssetToLibrary,
  batchUpdateClassification,
  chooseLibraryFolder,
  fetchAssets,
  fetchAssetDetail,
  fetchClassificationRegistry,
  fetchLibraries,
  fetchSemanticCatalog,
  fetchSemanticGroups,
  fetchSemanticProgress,
  fetchSemanticStatus,
  openLibraryInExplorer,
  pauseSemanticAnalysis,
  prepareSemanticModel,
  reanalyzeAsset,
  removeLibrary,
  rescanLibrary as requestLibraryRescan,
  resumeSemanticAnalysis,
  setLibraryParent,
  startLibraryScan,
  startSemanticAnalysis,
  startSemanticAnalysisForAssets,
  updateClassificationOverride,
  updateTagOverride,
  restoreAutoClassification,
  subscribeScanProgress,
  subscribeSemanticProgress,
} from "./api";
import { primaryCategoryOptions, SATURATION_OPTIONS, TONE_OPTIONS } from "./classificationLabels";
import { AssetCard } from "./components/AssetCard";
import { ColorSwatches } from "./components/ColorSwatches";
import { DetailPanel } from "./components/DetailPanel";
import { OrganizationWorkspace } from "./components/OrganizationWorkspace";
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
import { usePreviewController, type PreviewController } from "./components/usePreviewController";
import { formatDate } from "./format";
import {
  emptyAssetFilter,
  type AssetFilter,
  type AssetPage,
  type AssetListItem,
  type ClassificationFieldDescriptor,
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
const PREVIEW_PAGE_SIZE = 500;
const IMPORT_REFRESH_INTERVAL_MS = 750;

type AssetQueryOptions = {
  libraryId: number;
  sort: SortField;
  direction: SortDirection;
  filter: AssetFilter;
};

async function fetchAllPreviewAssets(options: AssetQueryOptions): Promise<AssetPage> {
  const firstPage = await fetchAssets({
    ...options,
    page: 1,
    pageSize: PREVIEW_PAGE_SIZE,
  });
  const pageSize = Math.max(firstPage.pageSize, 1);
  const totalPages = Math.ceil(firstPage.total / pageSize);
  if (totalPages <= 1 || firstPage.items.length >= firstPage.total) {
    return { ...firstPage, page: 1 };
  }

  const remainingPages = await Promise.all(
    Array.from({ length: totalPages - 1 }, (_, index) =>
      fetchAssets({
        ...options,
        page: index + 2,
        pageSize: PREVIEW_PAGE_SIZE,
      }),
    ),
  );
  return {
    ...firstPage,
    page: 1,
    pageSize,
    items: [firstPage, ...remainingPages].flatMap((result) => result.items),
  };
}

type SelectionModifiers = {
  ctrlKey?: boolean;
  metaKey?: boolean;
  shiftKey?: boolean;
};

type AssetPointerDragState = {
  assetIds: number[];
  pointerId: number;
  startX: number;
  startY: number;
  active: boolean;
};

const sortLabels: Record<SortField, string> = {
  file_name: "文件名",
  capture_time: "拍摄时间",
  modified_time: "修改时间",
  brightness: "亮度",
  saturation: "饱和度",
};

const visualOrganizationMode =
  import.meta.env.DEV &&
  (new URLSearchParams(window.location.search).get("visual-fixture") === "organization" ||
    new URLSearchParams(window.location.search).get("organization") === "1");

export default function App() {
  const [libraries, setLibraries] = useState<LibrarySummary[]>([]);
  const [currentLibraryId, setCurrentLibraryId] = useState<number | null>(null);
  const [assets, setAssets] = useState<AssetListItem[]>([]);
  const [assetTotal, setAssetTotal] = useState(0);
  const [semanticGroups, setSemanticGroups] = useState<SemanticGroupSummary[]>([]);
  const [semanticCatalog, setSemanticCatalog] = useState<SemanticLabelDescriptor[]>([]);
  const [classificationRegistry, setClassificationRegistry] = useState<
    ClassificationFieldDescriptor[]
  >([]);
  const [activeAssetId, setActiveAssetId] = useState<number | null>(null);
  const [detailAsset, setDetailAsset] = useState<AssetListItem | null>(null);
  const [previewAssetId, setPreviewAssetId] = useState<number | null>(null);
  const [selectionAnchorId, setSelectionAnchorId] = useState<number | null>(null);
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
  const [workspaceMode, setWorkspaceMode] = useState<"library" | "organization">(
    visualOrganizationMode ? "organization" : "library",
  );
  const [selectedAssetIds, setSelectedAssetIds] = useState<number[]>([]);
  const [pendingImportPath, setPendingImportPath] = useState<string | null>(null);
  const [includeImportSubfolders, setIncludeImportSubfolders] = useState(false);
  const [assetDropTargetLibraryId, setAssetDropTargetLibraryId] = useState<number | null>(null);
  const [batchEditorOpen, setBatchEditorOpen] = useState(false);
  const [batchField, setBatchField] = useState("primary_category");
  const [batchValue, setBatchValue] = useState<string[]>([]);
  const refreshTimerRef = useRef<number | null>(null);
  const assetPointerDragRef = useRef<AssetPointerDragState | null>(null);
  const librariesRef = useRef(libraries);
  const assetAssignmentRef = useRef<(assetIds: number[], targetLibraryId: number) => void>(
    () => {},
  );

  const requestDataRefresh = useCallback((immediate = false) => {
    if (immediate) {
      if (refreshTimerRef.current !== null) {
        window.clearTimeout(refreshTimerRef.current);
        refreshTimerRef.current = null;
      }
      setRefreshKey((value) => value + 1);
      return;
    }
    if (refreshTimerRef.current !== null) return;
    refreshTimerRef.current = window.setTimeout(() => {
      refreshTimerRef.current = null;
      setRefreshKey((value) => value + 1);
    }, IMPORT_REFRESH_INTERVAL_MS);
  }, []);

  const moveAssetsToLibrary = useCallback(
    async (assetIds: number[], targetLibraryId: number) => {
      const results = await Promise.allSettled(
        assetIds.map((assetId) => assignAssetToLibrary(assetId, targetLibraryId)),
      );
      if (
        results.some((result) => result.status === "fulfilled" && result.value) ||
        results.some((result) => result.status === "rejected")
      ) {
        requestDataRefresh(true);
      }
      const failure = results.find((result) => result.status === "rejected");
      if (failure?.status === "rejected") {
        setError(messageFrom(failure.reason));
      }
    },
    [requestDataRefresh],
  );

  useEffect(() => {
    librariesRef.current = libraries;
    assetAssignmentRef.current = (assetId, targetLibraryId) => {
      void moveAssetsToLibrary(assetId, targetLibraryId);
    };
  }, [libraries, moveAssetsToLibrary]);

  useEffect(() => {
    const findLibraryTarget = (event: PointerEvent): number | null => {
      const pointElement =
        typeof document.elementFromPoint === "function"
          ? document.elementFromPoint(event.clientX, event.clientY)
          : null;
      const element = pointElement ?? event.target;
      if (!(element instanceof Element)) return null;
      const row = element.closest<HTMLElement>("[data-library-drop-id]");
      if (!row) return null;
      const libraryId = Number(row.dataset.libraryDropId);
      return Number.isInteger(libraryId) && libraryId > 0 ? libraryId : null;
    };

    const handlePointerMove = (event: PointerEvent) => {
      const drag = assetPointerDragRef.current;
      const pointerId = event.pointerId || 1;
      if (!drag || drag.pointerId !== pointerId) return;
      const distance = Math.hypot(event.clientX - drag.startX, event.clientY - drag.startY);
      if (!drag.active && distance < 6) return;
      if (!drag.active) {
        drag.active = true;
      }
      event.preventDefault();
      const target = findLibraryTarget(event);
      setAssetDropTargetLibraryId(
        target !== null && librariesRef.current.some((library) => library.id === target)
          ? target
          : null,
      );
    };

    const finishPointerDrag = (event: PointerEvent, cancelled: boolean) => {
      const drag = assetPointerDragRef.current;
      const pointerId = event.pointerId || 1;
      if (!drag || drag.pointerId !== pointerId) return;
      const target = drag.active && !cancelled ? findLibraryTarget(event) : null;
      if (target !== null && librariesRef.current.some((library) => library.id === target)) {
        assetAssignmentRef.current(drag.assetIds, target);
      }
      assetPointerDragRef.current = null;
      setAssetDropTargetLibraryId(null);
    };

    const handlePointerUp = (event: PointerEvent) => finishPointerDrag(event, false);
    const handlePointerCancel = (event: PointerEvent) => finishPointerDrag(event, true);
    document.addEventListener("pointermove", handlePointerMove, { passive: false });
    document.addEventListener("pointerup", handlePointerUp);
    document.addEventListener("pointercancel", handlePointerCancel);
    return () => {
      document.removeEventListener("pointermove", handlePointerMove);
      document.removeEventListener("pointerup", handlePointerUp);
      document.removeEventListener("pointercancel", handlePointerCancel);
    };
  }, []);

  const beginAssetPointerDrag = (
    asset: AssetListItem,
    event: React.PointerEvent<HTMLButtonElement>,
  ) => {
    if (event.button !== undefined && event.button !== 0 && event.button !== -1) return;
    assetPointerDragRef.current = {
      assetIds: selectedAssetIds.includes(asset.id) ? [...selectedAssetIds] : [asset.id],
      pointerId: event.pointerId || 1,
      startX: event.clientX,
      startY: event.clientY,
      active: false,
    };
  };

  const selectedLibrary = useMemo(
    () => libraries.find((library) => library.id === currentLibraryId) ?? null,
    [libraries, currentLibraryId],
  );
  const activeAsset = useMemo(
    () => assets.find((asset) => asset.id === activeAssetId) ?? null,
    [activeAssetId, assets],
  );
  const previewAsset = useMemo(
    () => assets.find((asset) => asset.id === previewAssetId) ?? null,
    [assets, previewAssetId],
  );
  const previewController = usePreviewController(
    viewMode === "single" ? previewAsset : null,
    viewMode === "single",
  );
  const totalPages = Math.max(1, Math.ceil(assetTotal / PAGE_SIZE));
  const scanRunning =
    scanProgress !== null && ["running", "cancelling"].includes(scanProgress.status);
  const semanticRunning =
    semanticProgress !== null &&
    ["queued", "running", "paused", "cancelling"].includes(semanticProgress.status);
  const activeFilterCount = countActiveFilters(filter);
  const libraryName = selectedLibrary?.name || "PhotoOrganizer";

  useEffect(() => {
    let active = true;
    void Promise.allSettled([
      fetchLibraries(),
      fetchSemanticStatus(),
      fetchSemanticCatalog(),
      fetchClassificationRegistry(),
    ]).then(([libraryResult, statusResult, catalogResult, registryResult]) => {
      if (!active) return;
      if (libraryResult.status === "fulfilled") {
        setLibraries(libraryResult.value);
        setCurrentLibraryId((current) =>
          current !== null && libraryResult.value.some((library) => library.id === current)
            ? current
            : (libraryResult.value[0]?.id ?? null),
        );
      } else {
        setError(messageFrom(libraryResult.reason));
      }
      if (statusResult.status === "fulfilled") setSemanticStatus(statusResult.value);
      if (catalogResult.status === "fulfilled") setSemanticCatalog(catalogResult.value);
      if (registryResult.status === "fulfilled") setClassificationRegistry(registryResult.value);
      setLoading(false);
    });
    return () => {
      active = false;
    };
  }, []);

  useEffect(() => {
    let active = true;
    if (currentLibraryId === null) {
      return undefined;
    }
    const request =
      viewMode === "single"
        ? fetchAllPreviewAssets({ libraryId: currentLibraryId, sort, direction, filter })
        : fetchAssets({
            libraryId: currentLibraryId,
            sort,
            direction,
            page,
            pageSize: PAGE_SIZE,
            filter,
          });
    void request
      .then((result) => {
        if (!active) return;
        setAssets(result.items);
        setAssetTotal(result.total);
        const fallbackId = viewMode === "single" ? (result.items[0]?.id ?? null) : null;
        setActiveAssetId((current) => {
          if (current && result.items.some((item) => item.id === current)) return current;
          return fallbackId;
        });
        setPreviewAssetId((current) => {
          if (current && result.items.some((item) => item.id === current)) return current;
          return fallbackId;
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
  }, [direction, filter, page, refreshKey, currentLibraryId, sort, viewMode]);

  useEffect(() => {
    let active = true;
    if (activeAssetId === null) {
      return undefined;
    }
    void fetchAssetDetail(activeAssetId)
      .then((detail) => {
        if (active && detail) {
          setDetailAsset(detail);
          setAssets((current) =>
            current.map((item) => (item.id === detail.id ? { ...item, ...detail } : item)),
          );
        }
      })
      .catch((reason: unknown) => {
        if (active) setError(messageFrom(reason));
      });
    return () => {
      active = false;
    };
  }, [activeAssetId, refreshKey]);

  useEffect(() => {
    return () => {
      if (refreshTimerRef.current !== null) {
        window.clearTimeout(refreshTimerRef.current);
        refreshTimerRef.current = null;
      }
    };
  }, []);

  useEffect(() => {
    let active = true;
    if (currentLibraryId === null) return undefined;
    void Promise.all([
      fetchLibraries(),
      fetchSemanticGroups(currentLibraryId),
      fetchSemanticProgress(currentLibraryId),
    ])
      .then(([nextLibraries, nextGroups, progress]) => {
        if (!active) return;
        setLibraries(nextLibraries);
        setSemanticGroups(nextGroups);
        setSemanticProgress(progress);
      })
      .catch((reason: unknown) => {
        if (active) setError(messageFrom(reason));
      });
    return () => {
      active = false;
    };
  }, [refreshKey, currentLibraryId]);

  useEffect(() => {
    let disposed = false;
    let stopScan: (() => void) | undefined;
    let stopSemantic: (() => void) | undefined;
    void subscribeScanProgress((progress) => {
      if (disposed) return;
      setScanProgress(progress);
      if (progress.libraryId !== null) setCurrentLibraryId(progress.libraryId);
      requestDataRefresh(progress.status !== "running");
      if (["completed", "cancelled", "failed"].includes(progress.status)) setCancellingScan(false);
    }).then((stop) => {
      if (disposed) stop();
      else stopScan = stop;
    });
    void subscribeSemanticProgress((progress) => {
      if (disposed) return;
      setSemanticProgress(progress);
      requestDataRefresh(progress.status !== "running");
    }).then((stop) => {
      if (disposed) stop();
      else stopSemantic = stop;
    });
    return () => {
      disposed = true;
      stopScan?.();
      stopSemantic?.();
    };
  }, [requestDataRefresh]);

  useEffect(() => {
    if (
      !scanProgress ||
      scanProgress.status !== "completed" ||
      scanProgress.failed > 0 ||
      scanProgress.missing > 0
    ) {
      return undefined;
    }

    const taskId = scanProgress.taskId;
    const dismissTimer = window.setTimeout(() => {
      setScanProgress((current) => (current?.taskId === taskId ? null : current));
    }, 700);
    return () => window.clearTimeout(dismissTimer);
  }, [scanProgress]);

  function updateFilter(next: AssetFilter) {
    setFilterState(next);
    setPage(1);
  }

  async function importFolder() {
    setError(null);
    try {
      const rootPath = await chooseLibraryFolder();
      if (!rootPath) return;
      setIncludeImportSubfolders(false);
      setPendingImportPath(rootPath);
    } catch (reason) {
      setError(messageFrom(reason));
    }
  }

  async function confirmImport() {
    if (!pendingImportPath) return;
    const rootPath = pendingImportPath;
    try {
      const result = await startLibraryScan(rootPath, {
        includeSubfolders: includeImportSubfolders,
      });
      setPendingImportPath(null);
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
      if (currentLibraryId === null) return;
      const { jobId } = await startSemanticAnalysis(currentLibraryId);
      setSemanticProgress(pendingSemanticProgress(jobId, currentLibraryId, semanticStatus));
    } catch (reason) {
      setError(messageFrom(reason));
    }
  }

  async function analyzeSelected() {
    setError(null);
    if (!selectedLibrary || selectedAssetIds.length === 0) return;
    try {
      if (semanticStatus?.status !== "ready") {
        const status = await prepareSemanticModel();
        setSemanticStatus(status);
        return;
      }
      const { jobId } = await startSemanticAnalysisForAssets(selectedLibrary.id, selectedAssetIds);
      setSemanticProgress(pendingSemanticProgress(jobId, selectedLibrary.id, semanticStatus));
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

  function applyDetailUpdate(detail: AssetListItem | null) {
    if (!detail) return;
    setDetailAsset(detail);
    setAssets((current) =>
      current.map((item) => (item.id === detail.id ? { ...item, ...detail } : item)),
    );
  }

  async function editClassification(assetId: number, field: string, value: string | string[]) {
    try {
      applyDetailUpdate(await updateClassificationOverride(assetId, field, value));
    } catch (reason) {
      setError(messageFrom(reason));
    }
  }

  async function editTagOverride(assetId: number, tagId: string, state: "add" | "remove") {
    try {
      applyDetailUpdate(await updateTagOverride(assetId, tagId, state));
    } catch (reason) {
      setError(messageFrom(reason));
    }
  }

  async function restoreClassification(assetId: number, field?: string) {
    try {
      applyDetailUpdate(await restoreAutoClassification(assetId, field));
    } catch (reason) {
      setError(messageFrom(reason));
    }
  }

  async function applyBatchClassification() {
    if (!batchValue.length || !selectedAssetIds.length) return;
    try {
      await batchUpdateClassification(
        selectedAssetIds,
        batchField,
        batchField === "dominant_color_category" ? batchValue : batchValue[0],
      );
      setBatchEditorOpen(false);
      setBatchValue([]);
      requestDataRefresh(true);
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
    setCurrentLibraryId(id);
    setActiveAssetId(null);
    setPreviewAssetId(null);
    setSelectionAnchorId(null);
    setSelectedAssetIds([]);
    setWorkspaceMode("library");
    setFilterState(emptyAssetFilter);
    setPage(1);
  }

  function changeView(next: ViewMode) {
    setViewMode(next);
    if (next === "single") {
      const nextAsset = activeAsset ?? assets[0] ?? null;
      setActiveAssetId(nextAsset?.id ?? null);
      setPreviewAssetId(nextAsset?.id ?? null);
    }
  }

  function selectAsset(asset: AssetListItem, modifiers: SelectionModifiers = {}) {
    setActiveAssetId(asset.id);
    if (modifiers.shiftKey) {
      const range = selectionRange(asset.id);
      if (range) {
        setSelectedAssetIds(range);
        return;
      }
      setSelectionAnchorId(asset.id);
      setSelectedAssetIds([asset.id]);
      return;
    }
    if (modifiers.ctrlKey || modifiers.metaKey) {
      setSelectionAnchorId(asset.id);
      setSelectedAssetIds((current) =>
        current.includes(asset.id)
          ? current.filter((id) => id !== asset.id)
          : [...current, asset.id],
      );
      return;
    }
    setSelectionAnchorId(asset.id);
  }

  function selectionRange(targetId: number): number[] | null {
    if (selectionAnchorId === null) return null;
    const anchorIndex = assets.findIndex((item) => item.id === selectionAnchorId);
    const targetIndex = assets.findIndex((item) => item.id === targetId);
    if (anchorIndex < 0 || targetIndex < 0) return null;
    const start = Math.min(anchorIndex, targetIndex);
    const end = Math.max(anchorIndex, targetIndex);
    return assets.slice(start, end + 1).map((item) => item.id);
  }

  function toggleAssetSelection(asset: AssetListItem, modifiers: SelectionModifiers = {}) {
    setActiveAssetId(asset.id);
    if (modifiers.shiftKey) {
      const range = selectionRange(asset.id);
      if (range) {
        setSelectedAssetIds(range);
        return;
      }
      setSelectedAssetIds([asset.id]);
      setSelectionAnchorId(asset.id);
      return;
    }
    setSelectionAnchorId(asset.id);
    setSelectedAssetIds((current) =>
      current.includes(asset.id) ? current.filter((id) => id !== asset.id) : [...current, asset.id],
    );
  }

  function clearSelection() {
    setSelectedAssetIds([]);
    setSelectionAnchorId(null);
  }

  function openSinglePreview(asset: AssetListItem) {
    setActiveAssetId(asset.id);
    setPreviewAssetId(asset.id);
    setViewMode("single");
  }

  function selectPreview(asset: AssetListItem) {
    setActiveAssetId(asset.id);
    setPreviewAssetId(asset.id);
  }

  const navigatePreview = useCallback(
    (delta: -1 | 1) => {
      if (!previewAsset) return;
      const index = assets.findIndex((asset) => asset.id === previewAsset.id);
      const target = assets[index + delta];
      if (target) {
        selectPreview(target);
      }
    },
    [assets, previewAsset],
  );

  async function removeLibraryById(library: LibrarySummary) {
    const confirmed = window.confirm(
      "从 PhotoOrganizer 中移除此图库？\n\n这只会移除 PhotoOrganizer 中的索引、缩略图和分析结果，不会删除或修改磁盘中的任何原始图片。",
    );
    if (!confirmed) return;
    try {
      await removeLibrary(library.id);
      const remaining = libraries.filter((item) => item.id !== library.id);
      setLibraries(remaining);
      if (currentLibraryId === library.id) {
        setCurrentLibraryId(remaining[0]?.id ?? null);
        setActiveAssetId(null);
        setPreviewAssetId(null);
        setSelectedAssetIds([]);
        setSelectionAnchorId(null);
        setPage(1);
      }
    } catch (reason) {
      setError(messageFrom(reason));
    }
  }

  async function changeLibraryParent(library: LibrarySummary, parentLibraryId: number | null) {
    try {
      await setLibraryParent(library.id, parentLibraryId);
      requestDataRefresh(true);
    } catch (reason) {
      setError(messageFrom(reason));
    }
  }

  async function rescanLibrary(library: LibrarySummary) {
    try {
      const result = await requestLibraryRescan(library.id);
      setScanProgress({
        taskId: result.taskId,
        libraryId: library.id,
        status: "running",
        stage: "preparing",
        discovered: 0,
        processed: 0,
        succeeded: 0,
        failed: 0,
        skipped: 0,
        missing: 0,
        currentPath: library.sourcePath,
        error: null,
      });
    } catch (reason) {
      setError(messageFrom(reason));
    }
  }

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        if (viewMode === "single") setViewMode("grid");
        else clearSelection();
      } else if (event.key === "ArrowLeft" && viewMode === "single") {
        event.preventDefault();
        navigatePreview(-1);
      } else if (event.key === "ArrowRight" && viewMode === "single") {
        event.preventDefault();
        navigatePreview(1);
      } else if (event.key === "Enter" && activeAsset && viewMode === "grid") {
        event.preventDefault();
        openSinglePreview(activeAsset);
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [activeAsset, navigatePreview, previewAsset, viewMode]);

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
            <strong title={selectedLibrary?.sourcePath}>{libraryName}</strong>
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
            {semanticStatus?.status === "ready" ? "分析" : "准备模型"}
          </button>
          {selectedAssetIds.length > 0 ? (
            <>
              <button
                className="tool-button is-active"
                type="button"
                onClick={() => void analyzeSelected()}
                disabled={semanticRunning}
              >
                <PlayIcon width="14" height="14" />
                分析选中
              </button>
              <button className="tool-button" type="button" onClick={clearSelection}>
                清除选择
              </button>
              <button
                className={batchEditorOpen ? "tool-button is-active" : "tool-button"}
                type="button"
                onClick={() => setBatchEditorOpen((value) => !value)}
              >
                批量修正
              </button>
            </>
          ) : null}
          {selectedLibrary ? (
            <button
              className={workspaceMode === "organization" ? "tool-button is-active" : "tool-button"}
              type="button"
              onClick={() =>
                setWorkspaceMode((value) => (value === "library" ? "organization" : "library"))
              }
            >
              整理预览
            </button>
          ) : null}
        </div>
      </header>

      {batchEditorOpen && selectedAssetIds.length > 0 ? (
        <div className="batch-classification-bar">
          <strong>批量修正 {selectedAssetIds.length} 张图片</strong>
          <select
            value={batchField}
            onChange={(event) => {
              setBatchField(event.target.value);
              setBatchValue([]);
            }}
          >
            <option value="primary_category">主类别</option>
            <option value="tone">影调</option>
            <option value="dominant_color_category">主色</option>
            <option value="saturation_level">饱和度级别</option>
          </select>
          {batchField === "primary_category" ? (
            <select
              value={batchValue[0] ?? ""}
              onChange={(event) => setBatchValue(event.target.value ? [event.target.value] : [])}
            >
              <option value="">请选择主类别</option>
              {primaryCategoryOptions(semanticCatalog).map((option) => (
                <option value={option.value} key={option.value}>
                  {option.label}
                </option>
              ))}
            </select>
          ) : null}
          {batchField === "tone" ? (
            <select
              value={batchValue[0] ?? ""}
              onChange={(event) => setBatchValue(event.target.value ? [event.target.value] : [])}
            >
              <option value="">请选择影调</option>
              {TONE_OPTIONS.map(([value, label]) => (
                <option value={value} key={value}>
                  {label}
                </option>
              ))}
            </select>
          ) : null}
          {batchField === "dominant_color_category" ? (
            <ColorSwatches value={batchValue} onChange={setBatchValue} ariaLabel="选择主色" />
          ) : null}
          {batchField === "saturation_level" ? (
            <select
              value={batchValue[0] ?? ""}
              onChange={(event) => setBatchValue(event.target.value ? [event.target.value] : [])}
            >
              <option value="">请选择饱和度</option>
              {SATURATION_OPTIONS.map(([value, label]) => (
                <option value={value} key={value}>
                  {label}
                </option>
              ))}
            </select>
          ) : null}
          <button
            className="primary-action"
            type="button"
            onClick={() => void applyBatchClassification()}
          >
            保存
          </button>
          <button className="tool-button" type="button" onClick={() => setBatchEditorOpen(false)}>
            取消
          </button>
        </div>
      ) : null}

      <div className="workspace-shell">
        {workspaceMode === "organization" && selectedLibrary ? (
          <main className="center-workspace organization-mode-shell">
            <OrganizationWorkspace
              library={selectedLibrary}
              filter={filter}
              selectedAssetIds={selectedAssetIds}
              filteredCount={assetTotal}
              onClose={() => setWorkspaceMode("library")}
            />
          </main>
        ) : (
          <>
            <Sidebar
              collapsed={leftCollapsed}
              libraries={libraries}
              selectedLibraryId={currentLibraryId}
              groups={semanticGroups}
              catalog={semanticCatalog}
              filter={filter}
              semanticStatus={semanticStatus}
              assetDropTargetLibraryId={assetDropTargetLibraryId}
              onToggle={() => setLeftCollapsed((value) => !value)}
              onImportLibrary={() => void importFolder()}
              onSelectLibrary={selectLibrary}
              onRescanLibrary={(library) => void rescanLibrary(library)}
              onOpenLibrary={(library) =>
                void openLibraryInExplorer(library.id).catch((reason) =>
                  setError(messageFrom(reason)),
                )
              }
              onShowLibraryInfo={(library) =>
                window.alert(
                  `${library.name}\n${library.sourcePath}\n\n图片：${library.presentCount}\n缺失：${library.missingCount}`,
                )
              }
              onRemoveLibrary={(library) => void removeLibraryById(library)}
              onChangeLibraryParent={(library, parentLibraryId) =>
                void changeLibraryParent(library, parentLibraryId)
              }
              onFilterChange={updateFilter}
            />

            <main
              className={`center-workspace${viewMode === "single" ? " is-single-preview" : ""}`}
              onClick={(event) => {
                if (event.target === event.currentTarget) clearSelection();
              }}
            >
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
                      {selectedAssetIds.length > 0 ? (
                        <em className="selection-count">已选择 {selectedAssetIds.length} 张</em>
                      ) : null}
                    </div>
                    <div>
                      {selectedLibrary.semanticPendingCount > 0 ? (
                        <span className="analysis-hint">
                          已导入 {selectedLibrary.presentCount} 张图片，
                          {selectedLibrary.semanticPendingCount} 张尚未完成语义分析
                        </span>
                      ) : null}
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
                      selected={previewAsset}
                      controller={previewController}
                      onSelect={selectPreview}
                    />
                  ) : assets.length ? (
                    <GridWorkspace
                      assets={assets}
                      active={activeAsset}
                      selectedAssetIds={selectedAssetIds}
                      grouped={groupBySemantic}
                      onStartAssetDrag={beginAssetPointerDrag}
                      onSelect={selectAsset}
                      onToggleSelection={toggleAssetSelection}
                      onOpen={openSinglePreview}
                      onClearSelection={clearSelection}
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
                  {viewMode !== "single" && totalPages > 1 ? (
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
              asset={detailAsset?.id === activeAsset?.id ? detailAsset : activeAsset}
              collapsed={rightCollapsed}
              semanticStatus={semanticStatus}
              previewNavigator={previewController.navigator}
              onToggle={() => setRightCollapsed((value) => !value)}
              onReanalyze={(asset) => void analyzeOne(asset)}
              classificationRegistry={classificationRegistry}
              catalog={semanticCatalog}
              onUpdateClassification={(assetId, field, value) =>
                void editClassification(assetId, field, value)
              }
              onUpdateTagOverride={(assetId, tagId, state) =>
                void editTagOverride(assetId, tagId, state)
              }
              onRestoreAuto={(assetId, field) => void restoreClassification(assetId, field)}
            />
          </>
        )}
      </div>
      {pendingImportPath ? (
        <ImportLibraryDialog
          path={pendingImportPath}
          includeSubfolders={includeImportSubfolders}
          onIncludeSubfoldersChange={setIncludeImportSubfolders}
          onCancel={() => setPendingImportPath(null)}
          onConfirm={() => void confirmImport()}
        />
      ) : null}
    </div>
  );
}

function ImportLibraryDialog({
  path,
  includeSubfolders,
  onIncludeSubfoldersChange,
  onCancel,
  onConfirm,
}: {
  path: string;
  includeSubfolders: boolean;
  onIncludeSubfoldersChange: (value: boolean) => void;
  onCancel: () => void;
  onConfirm: () => void;
}) {
  return (
    <div className="modal-backdrop" role="presentation">
      <section
        className="import-library-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="import-library-dialog-title"
      >
        <div className="dialog-heading">
          <div>
            <small>导入图库</small>
            <h2 id="import-library-dialog-title">确认导入方式</h2>
          </div>
          <button type="button" className="dialog-close" onClick={onCancel} aria-label="关闭">
            ×
          </button>
        </div>
        <p className="dialog-path" title={path}>
          {path}
        </p>
        <label className="import-option">
          <input
            type="checkbox"
            checked={includeSubfolders}
            onChange={(event) => onIncludeSubfoldersChange(event.target.checked)}
          />
          <span>
            <strong>导入子文件夹结构</strong>
            <small>
              每个直接包含图片的子文件夹建立独立图库，并按磁盘嵌套关系初始化层级；空文件夹不建立图库。
            </small>
          </span>
        </label>
        <p className="dialog-hint">
          不勾选时仍会递归扫描所有图片，但只建立一个图库，子文件夹不会显示在图库树中。
        </p>
        <div className="dialog-actions">
          <button type="button" className="tool-button" onClick={onCancel}>
            取消
          </button>
          <button type="button" className="primary-action" onClick={onConfirm}>
            开始导入
          </button>
        </div>
      </section>
    </div>
  );
}

function GridWorkspace({
  assets,
  active,
  selectedAssetIds,
  grouped,
  onStartAssetDrag,
  onSelect,
  onToggleSelection,
  onOpen,
  onClearSelection,
}: {
  assets: AssetListItem[];
  active: AssetListItem | null;
  selectedAssetIds: number[];
  grouped: boolean;
  onStartAssetDrag: (asset: AssetListItem, event: React.PointerEvent<HTMLButtonElement>) => void;
  onSelect: (asset: AssetListItem, modifiers?: SelectionModifiers) => void;
  onToggleSelection: (asset: AssetListItem, modifiers?: SelectionModifiers) => void;
  onOpen: (asset: AssetListItem) => void;
  onClearSelection: () => void;
}) {
  if (!grouped)
    return (
      <section
        className="asset-grid"
        aria-label="图片网格"
        onClick={(event) => {
          if (event.target === event.currentTarget) onClearSelection();
        }}
      >
        {assets.map((asset) => (
          <AssetCard
            key={asset.id}
            asset={asset}
            active={active?.id === asset.id}
            selected={selectedAssetIds.includes(asset.id)}
            onStartDrag={onStartAssetDrag}
            onSelect={onSelect}
            onToggleSelection={onToggleSelection}
            onOpen={onOpen}
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
                  active={active?.id === asset.id}
                  selected={selectedAssetIds.includes(asset.id)}
                  onStartDrag={onStartAssetDrag}
                  onSelect={onSelect}
                  onToggleSelection={onToggleSelection}
                  onOpen={onOpen}
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
  controller,
  onSelect,
}: {
  assets: AssetListItem[];
  selected: AssetListItem | null;
  controller: PreviewController;
  onSelect: (asset: AssetListItem) => void;
}) {
  return (
    <section className="single-workspace">
      {selected ? (
        <ZoomablePreview key={selected.id} asset={selected} controller={controller} />
      ) : (
        <div className="single-empty">没有可预览的图片</div>
      )}
      <div
        className="filmstrip"
        aria-label="胶片栏"
        onWheel={(event) => {
          const delta =
            Math.abs(event.deltaY) >= Math.abs(event.deltaX) ? event.deltaY : event.deltaX;
          if (delta === 0) return;
          event.preventDefault();
          event.currentTarget.scrollLeft += delta;
        }}
      >
        {assets.map((asset) => (
          <button
            type="button"
            key={asset.id}
            className={selected?.id === asset.id ? "is-active" : ""}
            ref={(element) => {
              if (
                element &&
                selected?.id === asset.id &&
                typeof element.scrollIntoView === "function"
              ) {
                element.scrollIntoView({ block: "nearest", inline: "center" });
              }
            }}
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

function ZoomablePreview({
  asset,
  controller,
}: {
  asset: AssetListItem;
  controller: PreviewController;
}) {
  const {
    stageRef,
    screenSource,
    displaySource,
    loadState,
    originalState,
    naturalSize,
    displayScale,
    dragging,
    onWheel,
    onPointerDown,
    onPointerMove,
    onPointerUp,
    onPointerCancel,
    onDoubleClick,
    onImageLoad,
  } = controller;

  return (
    <div className="single-canvas">
      <div
        ref={stageRef}
        className={`zoom-stage${dragging ? " is-dragging" : ""}`}
        onWheel={onWheel}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
        onPointerCancel={onPointerCancel}
        onLostPointerCapture={onPointerCancel}
        onDoubleClick={onDoubleClick}
      >
        {screenSource ? (
          <img
            className="preview-image"
            src={displaySource ?? screenSource}
            alt={asset.fileName}
            draggable={false}
            style={{
              width: `${naturalSize.width}px`,
              height: `${naturalSize.height}px`,
              left: "50%",
              top: "50%",
              transform: `translate3d(calc(-50% + ${controller.offset.x}px), calc(-50% + ${controller.offset.y}px), 0) scale(${displayScale})`,
            }}
            onLoad={(event) =>
              onImageLoad({
                width: event.currentTarget.naturalWidth,
                height: event.currentTarget.naturalHeight,
              })
            }
          />
        ) : (
          <Thumbnail asset={asset} />
        )}
        {loadState === "loading" ? <span className="preview-loading">正在加载原图…</span> : null}
        {loadState === "error" ? (
          <span className="preview-error">无法加载高清预览，请重试</span>
        ) : null}
        {originalState === "loading" ? (
          <span className="preview-loading">正在加载原图…</span>
        ) : null}
        {originalState === "error" ? (
          <span className="preview-error">原图加载失败，仍可使用屏幕预览</span>
        ) : null}
      </div>
      <div className="single-caption">
        <strong>{asset.fileName}</strong>
        <span>{asset.width && asset.height ? `${asset.width} × ${asset.height}` : "尺寸未知"}</span>
      </div>
    </div>
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
            {progress.processed} / {progress.total} · 本地计算 · 失败 {progress.failed}
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
    filter.primaryCategories.length +
    filter.auxiliaryTags.length +
    filter.toneLabels.length +
    filter.colorCategories.length +
    filter.saturationLevels.length +
    Number(filter.brightnessMin !== null || filter.brightnessMax !== null) +
    Number(filter.saturationMin !== null || filter.saturationMax !== null) +
    Number(Boolean(filter.capturedFrom || filter.capturedTo)) +
    Number(Boolean(filter.analysisStatus))
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
    modelName: status?.model.name ?? "语义模型",
    modelVersion: status?.model.version ?? "",
    error: null,
  };
}

function messageFrom(reason: unknown): string {
  if (reason instanceof Error) return reason.message;
  if (typeof reason === "string") return reason;
  return "发生未知错误，请查看应用日志。";
}
