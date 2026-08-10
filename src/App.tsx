import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { CSSProperties } from "react";

import {
  cancelLibraryScan,
  cancelSemanticAnalysis,
  assignAssetToLibrary,
  batchUpdateClassification,
  chooseLibraryFolder,
  fetchAssets,
  fetchAssetDetail,
  fetchClassificationRegistry,
  fetchFavoriteAssetIds,
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
  setAssetFavorite,
  updateClassificationOverride,
  updateAssetColorLabel as updateAssetColorLabelApi,
  updateAssetRating as updateAssetRatingApi,
  updateTagOverride,
  restoreAutoClassification,
  subscribeScanProgress,
  subscribeSemanticProgress,
} from "./api";
import {
  classificationValueLabel,
  primaryCategoryOptions,
  SATURATION_OPTIONS,
  TONE_OPTIONS,
  type ClassificationValueKind,
} from "./classificationLabels";
import { AssetCard } from "./components/AssetCard";
import { AnalysisStatusFilterBar } from "./components/AnalysisStatusFilterBar";
import { ColorSwatches } from "./components/ColorSwatches";
import { DetailPanel } from "./components/DetailPanel";
import { ManualMarkFilterBar } from "./components/ManualMarkFilterBar";
import { OrganizationWorkspace } from "./components/OrganizationWorkspace";
import {
  CheckIcon,
  FilterIcon,
  GridIcon,
  ImportIcon,
  LibraryIcon,
  MoonIcon,
  PauseIcon,
  PlayIcon,
  SearchIcon,
  SingleImageIcon,
  SortIcon,
  SunIcon,
} from "./components/Icons";
import { ProgressPanel } from "./components/ProgressPanel";
import { Sidebar } from "./components/Sidebar";
import { Thumbnail } from "./components/Thumbnail";
import { WorkflowWorkspace } from "./components/WorkflowWorkspace";
import { usePreviewController, type PreviewController } from "./components/usePreviewController";
import { formatDate } from "./format";
import {
  createAssetQueryV1,
  describeAssetScopeV1,
  normalizeAssetQueryV1,
  stableAssetIds,
  updateAssetQueryFilter,
  updateAssetQueryLibrary,
  updateAssetQueryPage,
} from "./query";
import {
  emptyAssetFilter,
  MANUAL_COLOR_LABEL_OPTIONS,
  type AssetFilter,
  type AssetPage,
  type AssetListItem,
  type AssetQueryV1,
  type AssetScopeInputV1,
  type ClassificationFieldDescriptor,
  type LibrarySummary,
  type ManualColorLabel,
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
const DEFAULT_LEFT_PANEL_WIDTH = 270;
const DEFAULT_RIGHT_PANEL_WIDTH = 320;
const LEFT_PANEL_MIN_WIDTH = 218;
const LEFT_PANEL_MAX_WIDTH = 420;
const RIGHT_PANEL_MIN_WIDTH = 256;
const RIGHT_PANEL_MAX_WIDTH = 460;
const THEME_STORAGE_KEY = "photo-organizer-theme";

type ThemeMode = "dark" | "light";

function readThemeMode(): ThemeMode {
  if (typeof window === "undefined") return "dark";
  try {
    return window.localStorage.getItem(THEME_STORAGE_KEY) === "light" ? "light" : "dark";
  } catch {
    return "dark";
  }
}

async function fetchAllAssets(options: AssetQueryV1): Promise<AssetPage> {
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

type ValueUpdater<T> = T | ((current: T) => T);

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
  const [assetQuery, setAssetQuery] = useState<AssetQueryV1>(() =>
    createAssetQueryV1(null, PAGE_SIZE),
  );
  const currentLibraryId = assetQuery.libraryId;
  const { filter, sort, direction, groupBySemantic } = assetQuery;
  const [assets, setAssets] = useState<AssetListItem[]>([]);
  const [assetTotal, setAssetTotal] = useState(0);
  const [semanticGroups, setSemanticGroups] = useState<SemanticGroupSummary[]>([]);
  const [semanticCatalog, setSemanticCatalog] = useState<SemanticLabelDescriptor[]>([]);
  const [classificationRegistry, setClassificationRegistry] = useState<
    ClassificationFieldDescriptor[]
  >([]);
  const [activeAssetId, setActiveAssetId] = useState<number | null>(null);
  const [detailAsset, setDetailAsset] = useState<AssetListItem | null>(null);
  const [selectionAnchorId, setSelectionAnchorId] = useState<number | null>(null);
  const [filterPopoverOpen, setFilterPopoverOpen] = useState(false);
  const [viewMode, setViewMode] = useState<ViewMode>("grid");
  const [themeMode, setThemeMode] = useState<ThemeMode>(readThemeMode);
  const [leftPanelWidth, setLeftPanelWidth] = useState(DEFAULT_LEFT_PANEL_WIDTH);
  const [rightPanelWidth, setRightPanelWidth] = useState(DEFAULT_RIGHT_PANEL_WIDTH);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [scanProgress, setScanProgress] = useState<ScanProgress | null>(null);
  const [semanticProgress, setSemanticProgress] = useState<SemanticProgress | null>(null);
  const [cancellingScan, setCancellingScan] = useState(false);
  const [semanticStatus, setSemanticStatus] = useState<SemanticRuntimeStatus | null>(null);
  const [refreshKey, setRefreshKey] = useState(0);
  const [workspaceMode, setWorkspaceMode] = useState<"library" | "organization" | "workflows">(
    visualOrganizationMode ? "organization" : "library",
  );
  const [favoriteAssetIds, setFavoriteAssetIds] = useState<Set<number>>(new Set());
  const [selectedAssetIds, setSelectedAssetIds] = useState<number[]>([]);
  const [pendingImportPath, setPendingImportPath] = useState<string | null>(null);
  const [includeImportSubfolders, setIncludeImportSubfolders] = useState(false);
  const [assetDropTargetLibraryId, setAssetDropTargetLibraryId] = useState<number | null>(null);
  const [batchEditorOpen, setBatchEditorOpen] = useState(false);
  const [batchField, setBatchField] = useState("primary_category");
  const [batchValue, setBatchValue] = useState<string[]>([]);
  const refreshTimerRef = useRef<number | null>(null);
  const filterPopoverRef = useRef<HTMLDivElement | null>(null);
  const assetsRef = useRef<AssetListItem[]>([]);
  const manualMarkRequestVersionRef = useRef(new Map<number, number>());
  const assetPointerDragRef = useRef<AssetPointerDragState | null>(null);
  const librariesRef = useRef(libraries);
  const assetAssignmentRef = useRef<(assetIds: number[], targetLibraryId: number) => void>(
    () => {},
  );
  assetsRef.current = assets;

  const setCurrentLibraryId = useCallback((next: ValueUpdater<number | null>) => {
    setAssetQuery((current) => {
      const libraryId = typeof next === "function" ? next(current.libraryId) : next;
      return updateAssetQueryLibrary(current, libraryId);
    });
  }, []);

  function setFilterState(next: AssetFilter) {
    setAssetQuery((current) => updateAssetQueryFilter(current, next));
  }

  function setPage(next: ValueUpdater<number>) {
    setAssetQuery((current) => {
      const page = typeof next === "function" ? next(current.page) : next;
      return updateAssetQueryPage(current, page);
    });
  }

  function setSort(next: SortField) {
    setAssetQuery((current) => normalizeAssetQueryV1({ ...current, sort: next, page: 1 }));
  }

  function setDirection(next: ValueUpdater<SortDirection>) {
    setAssetQuery((current) => {
      const direction = typeof next === "function" ? next(current.direction) : next;
      return normalizeAssetQueryV1({ ...current, direction });
    });
  }

  function setGroupBySemantic(next: ValueUpdater<boolean>) {
    setAssetQuery((current) => {
      const groupBySemantic = typeof next === "function" ? next(current.groupBySemantic) : next;
      return normalizeAssetQueryV1({ ...current, groupBySemantic });
    });
  }

  useEffect(() => {
    let cancelled = false;
    if (currentLibraryId === null) {
      return;
    }
    void fetchFavoriteAssetIds(currentLibraryId)
      .then((ids) => {
        if (!cancelled) setFavoriteAssetIds(new Set(ids));
      })
      .catch((reason) => {
        if (!cancelled) setError(messageFrom(reason));
      });
    return () => {
      cancelled = true;
    };
  }, [currentLibraryId, refreshKey]);

  useEffect(() => {
    try {
      window.localStorage.setItem(THEME_STORAGE_KEY, themeMode);
    } catch {
      // Private browsing or a restricted webview may disable local storage.
    }
  }, [themeMode]);

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
  const previewNeighbors = useMemo(() => {
    if (viewMode !== "single" || activeAssetId === null) return [];
    const index = assets.findIndex((asset) => asset.id === activeAssetId);
    if (index < 0) return [];
    return [assets[index - 1], assets[index + 1]].filter(
      (asset): asset is AssetListItem => asset !== undefined,
    );
  }, [activeAssetId, assets, viewMode]);
  const previewPrefetchAssets = useMemo(
    () => (activeAsset ? [activeAsset, ...previewNeighbors] : []),
    [activeAsset, previewNeighbors],
  );
  const previewController = usePreviewController(
    viewMode === "single" ? activeAsset : null,
    viewMode === "single",
    previewPrefetchAssets,
  );
  const detailBaseAsset = activeAsset ?? (viewMode === "grid" ? (assets[0] ?? null) : null);
  const detailPanelAsset =
    detailAsset !== null && detailBaseAsset !== null && detailAsset.id === detailBaseAsset.id
      ? {
          ...detailAsset,
          rating: detailBaseAsset.rating,
          colorLabel: detailBaseAsset.colorLabel,
        }
      : detailBaseAsset;
  const scanRunning =
    scanProgress !== null && ["running", "cancelling"].includes(scanProgress.status);
  const semanticRunning =
    semanticProgress !== null &&
    ["queued", "running", "paused", "cancelling"].includes(semanticProgress.status);
  const activeFilterCount = countActiveFilters(filter);
  const currentScope = useMemo<AssetScopeInputV1>(() => {
    const ids = stableAssetIds(selectedAssetIds);
    return ids.length > 0
      ? { kind: "selection", query: assetQuery, assetIds: ids }
      : { kind: "query", query: assetQuery };
  }, [assetQuery, selectedAssetIds]);
  const currentScopeDescription = useMemo(
    () => describeAssetScopeV1(currentScope, assetTotal),
    [assetTotal, currentScope],
  );
  const activeFilterConditions = useMemo(
    () => buildFilterConditions(filter, semanticCatalog),
    [filter, semanticCatalog],
  );
  const libraryName = selectedLibrary?.name || "PhotoOrganizer";

  useEffect(() => {
    if (!filterPopoverOpen) return undefined;

    const handlePointerDown = (event: PointerEvent) => {
      const target = event.target;
      if (target instanceof Node && !filterPopoverRef.current?.contains(target)) {
        setFilterPopoverOpen(false);
      }
    };
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") setFilterPopoverOpen(false);
    };

    document.addEventListener("pointerdown", handlePointerDown);
    document.addEventListener("keydown", handleKeyDown);
    return () => {
      document.removeEventListener("pointerdown", handlePointerDown);
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [filterPopoverOpen]);

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
  }, [setCurrentLibraryId]);

  useEffect(() => {
    let active = true;
    if (currentLibraryId === null) {
      return undefined;
    }
    const request = fetchAllAssets(assetQuery);
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
  }, [assetQuery, refreshKey, currentLibraryId, viewMode]);

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
  }, [requestDataRefresh, setCurrentLibraryId]);

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
    const normalized =
      next.ratings.length > 1 ? { ...next, ratings: [Math.max(...next.ratings)] } : next;
    setFilterState(normalized);
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

  async function analyzeOne(asset: AssetListItem) {
    try {
      const { jobId } = await reanalyzeAsset(asset.libraryId, asset.id);
      setSemanticProgress(pendingSemanticProgress(jobId, asset.libraryId, semanticStatus));
    } catch (reason) {
      setError(messageFrom(reason));
    }
  }

  const applyDetailUpdate = useCallback((detail: AssetListItem | null) => {
    if (!detail) return;
    setDetailAsset(detail);
    assetsRef.current = assetsRef.current.map((item) =>
      item.id === detail.id ? { ...item, ...detail } : item,
    );
    setAssets((current) =>
      current.map((item) => (item.id === detail.id ? { ...item, ...detail } : item)),
    );
  }, []);

  const applyAssetMarkUpdate = useCallback(
    (assetId: number, update: Partial<Pick<AssetListItem, "rating" | "colorLabel">>) => {
      assetsRef.current = assetsRef.current.map((item) =>
        item.id === assetId ? { ...item, ...update } : item,
      );
      setAssets((current) =>
        current.map((item) => (item.id === assetId ? { ...item, ...update } : item)),
      );
      setDetailAsset((current) => (current?.id === assetId ? { ...current, ...update } : current));
    },
    [],
  );

  const editAssetRating = useCallback(
    async (assetId: number, rating: number) => {
      const previous = assetsRef.current.find((item) => item.id === assetId)?.rating ?? 0;
      const requestVersion = (manualMarkRequestVersionRef.current.get(assetId) ?? 0) + 1;
      manualMarkRequestVersionRef.current.set(assetId, requestVersion);
      applyAssetMarkUpdate(assetId, { rating });
      try {
        const detail = await updateAssetRatingApi(assetId, rating);
        if (manualMarkRequestVersionRef.current.get(assetId) === requestVersion) {
          applyDetailUpdate(detail);
        }
      } catch (reason) {
        if (manualMarkRequestVersionRef.current.get(assetId) === requestVersion) {
          applyAssetMarkUpdate(assetId, { rating: previous });
        }
        setError(messageFrom(reason));
      }
    },
    [applyAssetMarkUpdate, applyDetailUpdate],
  );

  const editAssetColorLabel = useCallback(
    async (assetId: number, colorLabel: ManualColorLabel | null) => {
      const previous = assetsRef.current.find((item) => item.id === assetId)?.colorLabel ?? null;
      const requestVersion = (manualMarkRequestVersionRef.current.get(assetId) ?? 0) + 1;
      manualMarkRequestVersionRef.current.set(assetId, requestVersion);
      applyAssetMarkUpdate(assetId, { colorLabel });
      try {
        const detail = await updateAssetColorLabelApi(assetId, colorLabel);
        if (manualMarkRequestVersionRef.current.get(assetId) === requestVersion) {
          applyDetailUpdate(detail);
        }
      } catch (reason) {
        if (manualMarkRequestVersionRef.current.get(assetId) === requestVersion) {
          applyAssetMarkUpdate(assetId, { colorLabel: previous });
        }
        setError(messageFrom(reason));
      }
    },
    [applyAssetMarkUpdate, applyDetailUpdate],
  );

  async function editAssetRatingForSelection(assetId: number, rating: number) {
    const targetIds = selectedAssetIds.includes(assetId) ? selectedAssetIds : [assetId];
    await Promise.all(targetIds.map((targetId) => editAssetRating(targetId, rating)));
  }

  async function editAssetColorLabelForSelection(
    assetId: number,
    colorLabel: ManualColorLabel | null,
  ) {
    const targetIds = selectedAssetIds.includes(assetId) ? selectedAssetIds : [assetId];
    await Promise.all(targetIds.map((targetId) => editAssetColorLabel(targetId, colorLabel)));
  }

  async function toggleFavorite(assetId: number) {
    const wasFavorite = favoriteAssetIds.has(assetId);
    setFavoriteAssetIds((current) => {
      const next = new Set(current);
      if (wasFavorite) next.delete(assetId);
      else next.add(assetId);
      return next;
    });
    try {
      await setAssetFavorite(assetId, !wasFavorite);
    } catch (reason) {
      setFavoriteAssetIds((current) => {
        const next = new Set(current);
        if (wasFavorite) next.add(assetId);
        else next.delete(assetId);
        return next;
      });
      setError(messageFrom(reason));
    }
  }

  const toggleAssetColorLabelForSelection = useCallback(
    async (assetId: number, colorLabel: ManualColorLabel) => {
      const targetIds = selectedAssetIds.includes(assetId) ? selectedAssetIds : [assetId];
      const shouldClear = targetIds.every(
        (targetId) =>
          assetsRef.current.find((item) => item.id === targetId)?.colorLabel === colorLabel,
      );
      const nextColorLabel = shouldClear ? null : colorLabel;
      await Promise.all(targetIds.map((targetId) => editAssetColorLabel(targetId, nextColorLabel)));
    },
    [editAssetColorLabel, selectedAssetIds],
  );

  async function editClassification(assetId: number, field: string, value: string | string[]) {
    try {
      applyDetailUpdate(await updateClassificationOverride(assetId, field, value));
      requestDataRefresh(true);
    } catch (reason) {
      setError(messageFrom(reason));
    }
  }

  async function editTagOverride(assetId: number, tagId: string, state: "add" | "remove") {
    try {
      applyDetailUpdate(await updateTagOverride(assetId, tagId, state));
      requestDataRefresh(true);
    } catch (reason) {
      setError(messageFrom(reason));
    }
  }

  async function restoreClassification(assetId: number, field?: string) {
    try {
      applyDetailUpdate(await restoreAutoClassification(assetId, field));
      requestDataRefresh(true);
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

  function selectWorkflowAsset(assetId: number) {
    setWorkspaceMode("library");
    setActiveAssetId(assetId);
    setSelectionAnchorId(assetId);
    setSelectedAssetIds([assetId]);
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
    setViewMode("single");
  }

  function selectPreview(asset: AssetListItem) {
    setActiveAssetId(asset.id);
  }

  const navigatePreview = useCallback(
    (delta: -1 | 1) => {
      if (!activeAsset) return;
      const index = assets.findIndex((asset) => asset.id === activeAsset.id);
      const target = assets[index + delta];
      if (target) {
        selectPreview(target);
      }
    },
    [activeAsset, assets],
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
      const target = event.target;
      const isFormControl =
        target instanceof HTMLInputElement ||
        target instanceof HTMLTextAreaElement ||
        target instanceof HTMLSelectElement ||
        (target instanceof HTMLElement && target.isContentEditable);
      if (!isFormControl) {
        const markedAssetIds =
          activeAsset && selectedAssetIds.includes(activeAsset.id)
            ? selectedAssetIds
            : activeAsset
              ? [activeAsset.id]
              : [];
        const ratingKey = /^[0-5]$/.test(event.key) ? Number(event.key) : null;
        const colorLabelByKey: Record<string, ManualColorLabel> = {
          "6": "red",
          "7": "yellow",
          "8": "green",
          "9": "blue",
        };
        if (markedAssetIds.length > 0 && ratingKey !== null) {
          event.preventDefault();
          void Promise.all(markedAssetIds.map((assetId) => editAssetRating(assetId, ratingKey)));
          return;
        }
        if (markedAssetIds.length > 0 && (event.key === "[" || event.key === "]")) {
          event.preventDefault();
          const delta = event.key === "]" ? 1 : -1;
          void Promise.all(
            markedAssetIds.map((assetId) => {
              const current = assets.find((item) => item.id === assetId)?.rating ?? 0;
              return editAssetRating(assetId, Math.max(0, Math.min(5, current + delta)));
            }),
          );
          return;
        }
        if (markedAssetIds.length > 0 && colorLabelByKey[event.key]) {
          event.preventDefault();
          const colorLabel = colorLabelByKey[event.key];
          void toggleAssetColorLabelForSelection(markedAssetIds[0], colorLabel);
          return;
        }
      }
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
  }, [
    activeAsset,
    assets,
    editAssetColorLabel,
    editAssetRating,
    navigatePreview,
    selectedAssetIds,
    toggleAssetColorLabelForSelection,
    viewMode,
  ]);

  const showBatchEditor = batchEditorOpen && selectedAssetIds.length > 0;

  return (
    <div
      className={`photo-app${showBatchEditor ? " has-batch-classification" : ""}${
        themeMode === "light" ? " theme-light" : ""
      }`}
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
          {selectedAssetIds.length > 0 ? (
            <div className="topbar-selection-actions" role="group" aria-label="选择操作">
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
            </div>
          ) : null}
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
          <div className="filter-popover-anchor" ref={filterPopoverRef}>
            <button
              className={activeFilterCount ? "tool-button is-active" : "tool-button"}
              type="button"
              aria-expanded={filterPopoverOpen}
              aria-haspopup="dialog"
              aria-controls="filter-conditions-popover"
              onClick={() => setFilterPopoverOpen((value) => !value)}
            >
              <FilterIcon width="15" height="15" />
              筛选{activeFilterCount ? ` ${activeFilterCount}` : ""}
            </button>
            {filterPopoverOpen ? (
              <FilterConditionsPopover
                conditions={activeFilterConditions}
                onClear={() => updateFilter(emptyAssetFilter)}
                onRemove={(condition) => updateFilter(condition.remove(filter))}
              />
            ) : null}
          </div>
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
          {selectedLibrary ? (
            <>
              <button
                className={workspaceMode === "workflows" ? "tool-button is-active" : "tool-button"}
                type="button"
                onClick={() =>
                  setWorkspaceMode((value) => (value === "workflows" ? "library" : "workflows"))
                }
              >
                查找与审阅
              </button>
              <button
                className="primary-action"
                type="button"
                onClick={() =>
                  setWorkspaceMode((value) =>
                    value === "organization" ? "library" : "organization",
                  )
                }
              >
                整理预览
              </button>
            </>
          ) : null}
          <button
            className="tool-button theme-toggle"
            type="button"
            onClick={() => setThemeMode((value) => (value === "dark" ? "light" : "dark"))}
            aria-label={themeMode === "dark" ? "切换到白天模式" : "切换到深色模式"}
            aria-pressed={themeMode === "light"}
            title={themeMode === "dark" ? "切换到白天模式" : "切换到深色模式"}
          >
            {themeMode === "dark" ? (
              <SunIcon width="15" height="15" />
            ) : (
              <MoonIcon width="15" height="15" />
            )}
            <span>{themeMode === "dark" ? "白天" : "深色"}</span>
          </button>
        </div>
      </header>

      {showBatchEditor ? (
        <div className="batch-classification-bar">
          <strong>批量修正 {selectedAssetIds.length} 张图片</strong>
          <select
            value={batchField}
            onChange={(event) => {
              setBatchField(event.target.value);
              setBatchValue([]);
            }}
          >
            <option value="primary_category">场景分类</option>
            <option value="tone">影调</option>
            <option value="dominant_color_category">主色</option>
            <option value="saturation_level">饱和度级别</option>
          </select>
          {batchField === "primary_category" ? (
            <select
              value={batchValue[0] ?? ""}
              onChange={(event) => setBatchValue(event.target.value ? [event.target.value] : [])}
            >
              <option value="">请选择场景分类</option>
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

      <div
        className="workspace-shell"
        style={
          {
            "--left-panel-width": `${leftPanelWidth}px`,
            "--right-panel-width": `${rightPanelWidth}px`,
          } as CSSProperties
        }
      >
        {workspaceMode === "workflows" && selectedLibrary ? (
          <main className="center-workspace workflow-mode-shell">
            <WorkflowWorkspace
              libraryId={selectedLibrary.id}
              selectedAssetIds={selectedAssetIds}
              activeAsset={detailPanelAsset}
              scope={currentScope}
              scopeDescription={currentScopeDescription}
              onSelectAsset={selectWorkflowAsset}
              onBack={() => setWorkspaceMode("library")}
              onFavoriteChange={(assetId, favorite) =>
                setFavoriteAssetIds((current) => {
                  const next = new Set(current);
                  if (favorite) next.add(assetId);
                  else next.delete(assetId);
                  return next;
                })
              }
            />
          </main>
        ) : workspaceMode === "organization" && selectedLibrary ? (
          <main className="center-workspace organization-mode-shell">
            <OrganizationWorkspace
              library={selectedLibrary}
              selectedAssetIds={selectedAssetIds}
              filteredCount={assetTotal}
              scopeInput={currentScope}
              scopeDescription={currentScopeDescription}
              onClose={() => setWorkspaceMode("library")}
            />
          </main>
        ) : (
          <>
            <Sidebar
              libraries={libraries}
              selectedLibraryId={currentLibraryId}
              groups={semanticGroups}
              catalog={semanticCatalog}
              filter={filter}
              semanticStatus={semanticStatus}
              assetDropTargetLibraryId={assetDropTargetLibraryId}
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

            <PanelResizeHandle
              side="left"
              value={leftPanelWidth}
              min={LEFT_PANEL_MIN_WIDTH}
              max={LEFT_PANEL_MAX_WIDTH}
              onChange={setLeftPanelWidth}
            />

            <div className="center-column">
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
                        <label className="group-toggle">
                          <input
                            type="checkbox"
                            checked={groupBySemantic}
                            onChange={(event) => setGroupBySemantic(event.target.checked)}
                          />
                          按场景分类分组
                        </label>
                      </div>
                    </div>
                    <AnalysisStatusFilterBar
                      filter={filter}
                      visible={selectedLibrary.semanticPendingCount > 0}
                      onFilterChange={updateFilter}
                    />
                    {loading && assets.length === 0 ? (
                      <GridLoading />
                    ) : viewMode === "single" ? (
                      <SinglePreview
                        assets={assets}
                        selected={activeAsset}
                        controller={previewController}
                        selectedAssetIds={selectedAssetIds}
                        filter={filter}
                        onSelect={selectPreview}
                        onFilterChange={updateFilter}
                        onToggleSelection={toggleAssetSelection}
                      />
                    ) : (
                      <div className="grid-workspace-shell">
                        <div className="grid-workspace-results">
                          {assets.length ? (
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
                              onUpdateRating={(assetId, rating) =>
                                void editAssetRatingForSelection(assetId, rating)
                              }
                              onUpdateColorLabel={(assetId, colorLabel) =>
                                void editAssetColorLabelForSelection(assetId, colorLabel)
                              }
                              favoriteAssetIds={favoriteAssetIds}
                              onToggleFavorite={(assetId) => void toggleFavorite(assetId)}
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
                        </div>
                        <ManualMarkFilterBar filter={filter} onFilterChange={updateFilter} />
                      </div>
                    )}
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
            </div>

            <PanelResizeHandle
              side="right"
              value={rightPanelWidth}
              min={RIGHT_PANEL_MIN_WIDTH}
              max={RIGHT_PANEL_MAX_WIDTH}
              onChange={setRightPanelWidth}
            />

            <DetailPanel
              asset={detailPanelAsset}
              semanticStatus={semanticStatus}
              previewNavigator={previewController.navigator}
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
              onUpdateRating={(assetId, rating) =>
                void editAssetRatingForSelection(assetId, rating)
              }
              onUpdateColorLabel={(assetId, colorLabel) =>
                void editAssetColorLabelForSelection(assetId, colorLabel)
              }
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

function PanelResizeHandle({
  side,
  value,
  min,
  max,
  onChange,
}: {
  side: "left" | "right";
  value: number;
  min: number;
  max: number;
  onChange: (value: number) => void;
}) {
  const dragRef = useRef<{ pointerId: number; startX: number; startWidth: number } | null>(null);

  useEffect(() => {
    const handlePointerMove = (event: PointerEvent) => {
      const drag = dragRef.current;
      if (!drag || drag.pointerId !== (event.pointerId || 1)) return;
      event.preventDefault();
      const direction = side === "left" ? 1 : -1;
      const clientX = Number.isFinite(event.clientX) ? event.clientX : drag.startX;
      const delta = (clientX - drag.startX) * direction;
      onChange(Math.max(min, Math.min(max, drag.startWidth + delta)));
    };
    const finishPointerDrag = (event: PointerEvent) => {
      if (dragRef.current?.pointerId === (event.pointerId || 1)) dragRef.current = null;
    };

    document.addEventListener("pointermove", handlePointerMove, { passive: false });
    document.addEventListener("pointerup", finishPointerDrag);
    document.addEventListener("pointercancel", finishPointerDrag);
    return () => {
      document.removeEventListener("pointermove", handlePointerMove);
      document.removeEventListener("pointerup", finishPointerDrag);
      document.removeEventListener("pointercancel", finishPointerDrag);
    };
  }, [max, min, onChange, side]);

  const changeByKeyboard = (event: React.KeyboardEvent<HTMLDivElement>) => {
    if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
    event.preventDefault();
    const increasesWidth = side === "left" ? event.key === "ArrowRight" : event.key === "ArrowLeft";
    onChange(Math.max(min, Math.min(max, value + (increasesWidth ? 16 : -16))));
  };

  return (
    <div
      className={`panel-resize-handle panel-resize-handle-${side}`}
      role="separator"
      aria-label={`调整${side === "left" ? "左侧" : "右侧"}面板宽度`}
      aria-orientation="vertical"
      aria-valuemin={min}
      aria-valuemax={max}
      aria-valuenow={Math.round(value)}
      tabIndex={0}
      onKeyDown={changeByKeyboard}
      onPointerDown={(event) => {
        event.preventDefault();
        dragRef.current = {
          pointerId: event.pointerId || 1,
          startX: Number.isFinite(event.clientX) ? event.clientX : 0,
          startWidth: value,
        };
        event.currentTarget.setPointerCapture?.(event.pointerId);
      }}
    />
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
  onUpdateRating,
  onUpdateColorLabel,
  favoriteAssetIds,
  onToggleFavorite,
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
  onUpdateRating: (assetId: number, rating: number) => void;
  onUpdateColorLabel: (assetId: number, colorLabel: ManualColorLabel | null) => void;
  favoriteAssetIds: Set<number>;
  onToggleFavorite: (assetId: number) => void;
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
            onUpdateRating={onUpdateRating}
            onUpdateColorLabel={onUpdateColorLabel}
            favorite={favoriteAssetIds.has(asset.id)}
            onToggleFavorite={onToggleFavorite}
          />
        ))}
      </section>
    );
  const sections = new Map<string, AssetListItem[]>();
  for (const asset of assets) {
    const primary = asset.semanticLabels.find((label) => label.isPrimary);
    const effectivePrimary = asset.classification.primaryCategory.effective;
    const key = primary?.labelId ?? effectivePrimary ?? "not_analyzed";
    const section = sections.get(key) ?? [];
    section.push(asset);
    sections.set(key, section);
  }
  return (
    <div className="semantic-groups">
      {[...sections].map(([id, items]) => {
        const label =
          items[0]?.semanticLabels.find((item) => item.isPrimary)?.displayName ??
          (items[0]?.classification.primaryCategory.effective === "unknown" ? "未知" : "未分析");
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
                  onUpdateRating={onUpdateRating}
                  onUpdateColorLabel={onUpdateColorLabel}
                  favorite={favoriteAssetIds.has(asset.id)}
                  onToggleFavorite={onToggleFavorite}
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
  selectedAssetIds,
  filter,
  onSelect,
  onFilterChange,
  onToggleSelection,
}: {
  assets: AssetListItem[];
  selected: AssetListItem | null;
  controller: PreviewController;
  selectedAssetIds: number[];
  filter: AssetFilter;
  onSelect: (asset: AssetListItem) => void;
  onFilterChange: (filter: AssetFilter) => void;
  onToggleSelection: (asset: AssetListItem, modifiers?: SelectionModifiers) => void;
}) {
  const filmstripRef = useRef<HTMLDivElement | null>(null);
  const activeAssetId = selected?.id ?? null;

  useEffect(() => {
    if (activeAssetId === null) return;
    const activeButton = filmstripRef.current?.querySelector<HTMLButtonElement>(
      'button[aria-current="true"]',
    );
    if (activeButton && typeof activeButton.scrollIntoView === "function") {
      activeButton.scrollIntoView({ block: "nearest", inline: "nearest" });
    }
  }, [activeAssetId, assets]);

  return (
    <section className="single-workspace">
      {selected ? (
        <ZoomablePreview
          key={selected.id}
          asset={selected}
          controller={controller}
          selected={selectedAssetIds.includes(selected.id)}
          onToggleSelection={onToggleSelection}
        />
      ) : (
        <div className="single-empty">没有可预览的图片</div>
      )}
      <ManualMarkFilterBar filter={filter} onFilterChange={onFilterChange} />
      <div
        ref={filmstripRef}
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
            className={[
              selected?.id === asset.id ? "is-active" : "",
              selectedAssetIds.includes(asset.id) ? "is-selected" : "",
            ]
              .filter(Boolean)
              .join(" ")}
            onClick={() => onSelect(asset)}
            aria-label={asset.fileName}
            aria-current={selected?.id === asset.id ? "true" : undefined}
            aria-pressed={selectedAssetIds.includes(asset.id)}
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
  selected,
  onToggleSelection,
}: {
  asset: AssetListItem;
  controller: PreviewController;
  selected: boolean;
  onToggleSelection: (asset: AssetListItem, modifiers?: SelectionModifiers) => void;
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
      <button
        type="button"
        className={`single-selection-toggle${selected ? " is-selected" : ""}`}
        onClick={() => onToggleSelection(asset)}
        aria-label={selected ? `取消选择 ${asset.fileName}` : `选择 ${asset.fileName}`}
        aria-pressed={selected}
      >
        <span className="single-selection-mark" aria-hidden="true">
          {selected ? <CheckIcon width="13" height="13" /> : null}
        </span>
        <span>{selected ? "已选择" : "选择"}</span>
      </button>
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

type FilterCondition = {
  id: string;
  label: string;
  value: string;
  remove: (filter: AssetFilter) => AssetFilter;
};

type StringFilterField =
  "primaryCategories" | "auxiliaryTags" | "toneLabels" | "colorCategories" | "saturationLevels";

function FilterConditionsPopover({
  conditions,
  onClear,
  onRemove,
}: {
  conditions: FilterCondition[];
  onClear: () => void;
  onRemove: (condition: FilterCondition) => void;
}) {
  return (
    <div
      id="filter-conditions-popover"
      className="filter-conditions-popover"
      role="dialog"
      aria-label="当前条件"
    >
      <div className="filter-conditions-header">
        <strong>当前条件</strong>
        <button
          type="button"
          className="filter-conditions-clear"
          onClick={onClear}
          disabled={conditions.length === 0}
        >
          清除筛选
        </button>
      </div>
      {conditions.length > 0 ? (
        <div className="filter-condition-list">
          {conditions.map((condition) => (
            <div className="filter-condition" key={condition.id}>
              <span className="filter-condition-value">
                <strong>{condition.label}</strong>
                <span>{condition.value}</span>
              </span>
              <button
                type="button"
                className="filter-condition-remove"
                aria-label={`移除筛选条件：${condition.label} ${condition.value}`}
                onClick={() => onRemove(condition)}
              >
                ×
              </button>
            </div>
          ))}
        </div>
      ) : (
        <p className="filter-conditions-empty">当前没有筛选条件</p>
      )}
    </div>
  );
}

function buildFilterConditions(
  filter: AssetFilter,
  catalog: SemanticLabelDescriptor[],
): FilterCondition[] {
  const conditions: FilterCondition[] = [];

  if (filter.search) {
    conditions.push({
      id: "search",
      label: "搜索",
      value: filter.search,
      remove: (current) => ({ ...current, search: null }),
    });
  }

  appendStringFilterConditions(
    conditions,
    filter,
    "primaryCategories",
    "场景分类",
    "primary",
    catalog,
  );
  appendStringFilterConditions(conditions, filter, "auxiliaryTags", "辅助标签", "tag", catalog);
  appendStringFilterConditions(conditions, filter, "toneLabels", "影调", "tone", catalog);
  appendStringFilterConditions(conditions, filter, "colorCategories", "主色", "color", catalog);
  appendStringFilterConditions(
    conditions,
    filter,
    "saturationLevels",
    "饱和度级别",
    "saturation",
    catalog,
  );

  const ratingThreshold = filter.ratings.length > 0 ? Math.max(...filter.ratings) : null;
  if (ratingThreshold !== null) {
    conditions.push({
      id: "rating",
      label: "星级",
      value: `${ratingThreshold} 星及以上`,
      remove: (current) => ({
        ...current,
        ratings: [],
      }),
    });
  }

  for (const colorLabel of filter.colorLabels) {
    const label =
      MANUAL_COLOR_LABEL_OPTIONS.find((option) => option.id === colorLabel)?.label ?? colorLabel;
    conditions.push({
      id: `color-label:${colorLabel}`,
      label: "色标",
      value: label,
      remove: (current) => ({
        ...current,
        colorLabels: current.colorLabels.filter((value) => value !== colorLabel),
      }),
    });
  }

  appendRangeCondition(conditions, filter, "brightnessMin", "brightnessMax", "亮度");
  appendRangeCondition(conditions, filter, "saturationMin", "saturationMax", "饱和度");

  if (filter.capturedFrom || filter.capturedTo) {
    conditions.push({
      id: "capture-date",
      label: "拍摄日期",
      value: formatDateCondition(filter.capturedFrom, filter.capturedTo),
      remove: (current) => ({ ...current, capturedFrom: null, capturedTo: null }),
    });
  }

  if (filter.analysisStatus) {
    const statusLabel = {
      not_analyzed: "尚未语义分析",
      failed: "分析失败",
      completed: "分析完成",
    }[filter.analysisStatus];
    conditions.push({
      id: "analysis-status",
      label: "分析状态",
      value: statusLabel,
      remove: (current) => ({ ...current, analysisStatus: null }),
    });
  }

  return conditions;
}

function appendStringFilterConditions(
  conditions: FilterCondition[],
  filter: AssetFilter,
  field: StringFilterField,
  label: string,
  kind: ClassificationValueKind,
  catalog: SemanticLabelDescriptor[],
) {
  for (const value of filter[field]) {
    conditions.push({
      id: `${field}:${value}`,
      label,
      value: classificationValueLabel(value, kind, catalog),
      remove: (current) => ({
        ...current,
        [field]: current[field].filter((item) => item !== value),
      }),
    });
  }
}

function appendRangeCondition(
  conditions: FilterCondition[],
  filter: AssetFilter,
  minField: "brightnessMin" | "saturationMin",
  maxField: "brightnessMax" | "saturationMax",
  label: string,
) {
  const min = filter[minField];
  const max = filter[maxField];
  if (min === null && max === null) return;

  conditions.push({
    id: `${minField}:${maxField}`,
    label,
    value: formatFilterRange(min, max),
    remove: (current) => ({ ...current, [minField]: null, [maxField]: null }),
  });
}

function formatFilterRange(min: number | null, max: number | null) {
  const format = (value: number) => String(Number(value.toFixed(2)));
  if (min !== null && max !== null) return `${format(min)} — ${format(max)}`;
  if (min !== null) return `≥ ${format(min)}`;
  return `≤ ${format(max ?? 0)}`;
}

function formatDateCondition(from: string | null, to: string | null) {
  const format = (value: string | null, fallback: string) =>
    value ? value.slice(0, 10).replace(/-/g, "/") : fallback;
  return `${format(from, "最早")} — ${format(to, "最近")}`;
}

function countActiveFilters(filter: AssetFilter) {
  return (
    Number(Boolean(filter.search)) +
    filter.primaryCategories.length +
    filter.auxiliaryTags.length +
    filter.toneLabels.length +
    filter.colorCategories.length +
    filter.saturationLevels.length +
    filter.ratings.length +
    filter.colorLabels.length +
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
