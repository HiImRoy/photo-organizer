import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { CSSProperties } from "react";

import {
  addAssetsToCollections,
  cancelLibraryScan,
  cancelSemanticAnalysis,
  assignAssetToLibrary,
  batchUpdateClassification,
  chooseLibraryFolder,
  createCollection,
  deleteCollection,
  fetchAssets,
  fetchAssetDetail,
  fetchBrowseNodes,
  fetchClassificationRegistry,
  fetchCollections,
  fetchFavoriteAssetIds,
  fetchGpuCapabilities,
  fetchLibraries,
  fetchSemanticCatalog,
  fetchSemanticGroups,
  fetchSemanticProgress,
  fetchSemanticStatus,
  fetchSubjectStatus,
  moveAssetsBetweenCollections,
  moveCollection,
  openLibraryInExplorer,
  pauseSemanticAnalysis,
  prepareSemanticModel,
  prepareSubjectModel,
  reanalyzeAsset,
  removeAssetsFromCollection,
  removeLibrary,
  renameCollection,
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
  subscribeSemanticStatus,
  subscribeSemanticProgress,
  subscribeSubjectStatus,
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
import {
  CollectionActionDialog,
  type CollectionDialogRequest,
  type CollectionDialogSubmission,
} from "./components/CollectionActionDialog";
import { DetailPanel } from "./components/DetailPanel";
import { ManualMarkFilterBar } from "./components/ManualMarkFilterBar";
import { OrganizationWorkspace } from "./components/OrganizationWorkspace";
import {
  CheckIcon,
  ChevronIcon,
  BooksIcon,
  FilterIcon,
  GridIcon,
  HomeIcon,
  ImportIcon,
  LibraryIcon,
  PlayIcon,
  SearchIcon,
  SettingsIcon,
  SingleImageIcon,
  SortIcon,
} from "./components/Icons";
import { BackgroundTaskStatus } from "./components/BackgroundTaskStatus";
import { SettingsDialog } from "./components/SettingsDialog";
import { Sidebar } from "./components/Sidebar";
import { Thumbnail } from "./components/Thumbnail";
import { gridThumbnailProfileForColumns } from "./components/thumbnailSource";
import { WorkflowWorkspace, type WorkflowTool } from "./components/WorkflowWorkspace";
import { usePreviewController, type PreviewController } from "./components/usePreviewController";
import { colorHueFilterSummary } from "./colorFilter";
import { formatDate } from "./format";
import {
  DEFAULT_APP_SETTINGS,
  effectiveAnalysisBatchSize,
  preferredAnalysisBackend,
  normalizeAppSettings,
  persistAppSettings,
  readAppSettings,
  type AppSettings,
  type AppThemeMode,
} from "./settings";
import {
  createAssetQueryV1,
  describeAssetScopeV1,
  normalizeAssetQueryV1,
  stableAssetIds,
  updateAssetQueryBrowseRoot,
  updateAssetQueryFilter,
  updateAssetQueryLibrary,
  updateAssetQueryPage,
} from "./query";
import {
  emptyAssetFilter,
  MANUAL_COLOR_LABEL_OPTIONS,
  type AssetGroupBy,
  type AssetFilter,
  type AssetListItem,
  type AssetQueryV1,
  type AssetScopeInputV1,
  type BrowseNode,
  type ClassificationFieldDescriptor,
  type CollectionSummary,
  type GpuCapabilities,
  type LibrarySummary,
  type ManualColorLabel,
  type ScanProgress,
  type SemanticGroupSummary,
  type SemanticLabelDescriptor,
  type SemanticProgress,
  type SemanticRuntimeStatus,
  type SubjectRuntimeStatus,
  type SortDirection,
  type SortField,
  type ViewMode,
} from "./types";

const PAGE_SIZE = 120;
const LOAD_MORE_AHEAD_PX = 720;
const IMPORT_REFRESH_INTERVAL_MS = 750;
const DEFAULT_LEFT_PANEL_WIDTH = 270;
const DEFAULT_RIGHT_PANEL_WIDTH = 320;
const LEFT_PANEL_MIN_WIDTH = 218;
const LEFT_PANEL_MAX_WIDTH = 420;
const RIGHT_PANEL_MIN_WIDTH = 256;
const RIGHT_PANEL_MAX_WIDTH = 460;
const WORKFLOW_HEIGHT_DEFAULT = 360;
const WORKFLOW_HEIGHT_MIN = 250;
const WORKFLOW_HEIGHT_MAX = 640;
const WORKFLOW_HEIGHT_STEP = 16;
const TOPIC_MODEL_ID = "siglip2-base";
const GRID_COLUMNS_MIN = 2;
const GRID_COLUMNS_MAX = 12;
const GRID_COLUMNS_STEP = 2;

function isTerminalScanStatus(status: ScanProgress["status"]) {
  return ["completed", "cancelled", "failed"].includes(status);
}

function isTerminalSemanticStatus(status: SemanticProgress["status"]) {
  return ["completed", "cancelled", "failed"].includes(status);
}

const GROUP_BY_OPTIONS: Array<{ value: AssetGroupBy; label: string }> = [
  { value: "none", label: "不分组" },
  { value: "primary_category", label: "拍摄题材" },
  { value: "auxiliary_tag", label: "主体标签" },
  { value: "tone", label: "影调" },
  { value: "saturation_level", label: "饱和度级别" },
  { value: "dominant_color", label: "主色" },
  { value: "rating", label: "评分" },
];

const INTERACTIVE_KEYBOARD_TARGET_SELECTOR = [
  "button",
  "input",
  "textarea",
  "select",
  "option",
  "summary",
  "label",
  "a[href]",
  "[contenteditable]",
  '[tabindex]:not([tabindex="-1"])',
  '[role="button"]',
  '[role="link"]',
  '[role="tab"]',
  '[role="checkbox"]',
  '[role="radio"]',
  '[role="switch"]',
  '[role="slider"]',
  '[role="combobox"]',
  '[role="listbox"]',
  '[role="option"]',
  '[role="menuitem"]',
].join(", ");

function isInteractiveKeyboardTarget(target: EventTarget | null) {
  if (!(target instanceof Element)) return false;
  if (target instanceof HTMLElement && target.isContentEditable) return true;
  return (
    target.matches(INTERACTIVE_KEYBOARD_TARGET_SELECTOR) ||
    target.closest(INTERACTIVE_KEYBOARD_TARGET_SELECTOR) !== null
  );
}

function isGalleryKeyboardTarget(target: EventTarget | null) {
  return target instanceof Element && target.closest(".asset-card, .filmstrip button") !== null;
}

function topicModelIdFromStatus(status: SemanticRuntimeStatus | null): string | null {
  const name = status?.topicModel?.name ?? status?.model?.name;
  if (!name) return null;
  if (name === "SigLIP2-Base-Patch16-224") return "siglip2-base";
  return null;
}

function modelRuntimeNotice(
  label: string,
  status: SemanticRuntimeStatus | SubjectRuntimeStatus | null,
  statusReadError: string | null,
  preparationError: string | null,
) {
  if (preparationError) return `${label}准备失败：${preparationError}`;
  if (statusReadError) return `${label}状态读取失败：${statusReadError}`;
  if (!status) return `${label}状态检查中`;
  if (status.status === "ready" || (label === "主体模型" && status.status === "partial")) {
    return null;
  }
  if (status.status === "loading") return `${label}准备中`;
  if (status.status === "error") return `${label}准备失败：${status.message}`;
  if (status.status === "runtime_unavailable") {
    return `${label}运行时不可用：${status.message}`;
  }
  if (status.status === "model_unavailable") return `${label}尚未就绪：${status.message}`;
  return `${label}状态：${status.message || status.status}`;
}

function modelRuntimeCanRetry(
  status: SemanticRuntimeStatus | SubjectRuntimeStatus | null,
  statusReadError: string | null,
  preparationError: string | null,
) {
  return (
    statusReadError !== null ||
    preparationError !== null ||
    status?.status === "error" ||
    status?.status === "model_unavailable" ||
    status?.status === "runtime_unavailable"
  );
}

function readThemeMode(): AppThemeMode {
  if (typeof window === "undefined") return "dark";
  try {
    return window.localStorage.getItem("photo-organizer-theme") === "light" ? "light" : "dark";
  } catch {
    return "dark";
  }
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

function descendantLibraries(parentLibraryId: number, libraries: LibrarySummary[]) {
  const childrenByParent = new Map<number, LibrarySummary[]>();
  for (const library of libraries) {
    if (library.parentLibraryId === null) continue;
    const children = childrenByParent.get(library.parentLibraryId) ?? [];
    children.push(library);
    childrenByParent.set(library.parentLibraryId, children);
  }

  const descendants: LibrarySummary[] = [];
  const visited = new Set<number>();
  const visit = (libraryId: number) => {
    for (const child of childrenByParent.get(libraryId) ?? []) {
      if (visited.has(child.id)) continue;
      visited.add(child.id);
      descendants.push(child);
      visit(child.id);
    }
  };
  visit(parentLibraryId);
  return descendants;
}

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
  const { filter, sort, direction } = assetQuery;
  const browseRootActive =
    currentLibraryId !== null || filter.favoriteOnly || filter.collectionId !== null;
  const [assets, setAssets] = useState<AssetListItem[]>([]);
  const [assetTotal, setAssetTotal] = useState(0);
  const [hasMoreAssets, setHasMoreAssets] = useState(false);
  const [loadingMoreAssets, setLoadingMoreAssets] = useState(false);
  const [semanticGroups, setSemanticGroups] = useState<SemanticGroupSummary[]>([]);
  const [semanticCatalog, setSemanticCatalog] = useState<SemanticLabelDescriptor[]>([]);
  const [groupBy, setGroupBy] = useState<AssetGroupBy>("none");
  const [classificationRegistry, setClassificationRegistry] = useState<
    ClassificationFieldDescriptor[]
  >([]);
  const [activeAssetId, setActiveAssetId] = useState<number | null>(null);
  const [detailAsset, setDetailAsset] = useState<AssetListItem | null>(null);
  const [selectionAnchorId, setSelectionAnchorId] = useState<number | null>(null);
  const [filterPopoverOpen, setFilterPopoverOpen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [appSettings, setAppSettings] = useState<AppSettings>(readAppSettings);
  const [viewMode, setViewMode] = useState<ViewMode>(appSettings.startupView);
  const [themeMode, setThemeMode] = useState<AppThemeMode>(readThemeMode);
  const [leftPanelWidth, setLeftPanelWidth] = useState(DEFAULT_LEFT_PANEL_WIDTH);
  const [rightPanelWidth, setRightPanelWidth] = useState(DEFAULT_RIGHT_PANEL_WIDTH);
  const [sidebarLibraryRatio, setSidebarLibraryRatio] = useState<number | null>(null);
  const [gridColumns, setGridColumns] = useState(appSettings.defaultGridColumns);

  const [workflowPanelHeight, setWorkflowPanelHeight] = useState(WORKFLOW_HEIGHT_DEFAULT);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [scanProgress, setScanProgress] = useState<ScanProgress | null>(null);
  const [semanticProgress, setSemanticProgress] = useState<SemanticProgress | null>(null);
  const [cancellingScan, setCancellingScan] = useState(false);
  const [semanticControlBusy, setSemanticControlBusy] = useState(false);
  const [semanticStatus, setSemanticStatus] = useState<SemanticRuntimeStatus | null>(null);
  const [semanticStatusReadError, setSemanticStatusReadError] = useState<string | null>(null);
  const [semanticPreparationError, setSemanticPreparationError] = useState<string | null>(null);
  const [gpuCapabilities, setGpuCapabilities] = useState<GpuCapabilities | null>(null);
  const [subjectStatus, setSubjectStatus] = useState<SubjectRuntimeStatus | null>(null);
  const [subjectStatusReadError, setSubjectStatusReadError] = useState<string | null>(null);
  const [subjectPreparationError, setSubjectPreparationError] = useState<string | null>(null);
  const [analysisActionBusy, setAnalysisActionBusy] = useState(false);
  const [refreshKey, setRefreshKey] = useState(0);
  const [workspaceMode, setWorkspaceMode] = useState<"library" | "organization">(
    visualOrganizationMode ? "organization" : "library",
  );
  const [workflowTool, setWorkflowTool] = useState<WorkflowTool | null>(null);
  const [collections, setCollections] = useState<CollectionSummary[]>([]);
  const [collectionDialogRequest, setCollectionDialogRequest] =
    useState<CollectionDialogRequest | null>(null);
  const [collectionOperationBusy, setCollectionOperationBusy] = useState(false);
  const [browseNodes, setBrowseNodes] = useState<BrowseNode[]>([]);
  const [favoriteAssetIds, setFavoriteAssetIds] = useState<Set<number>>(new Set());
  const [selectedAssetIds, setSelectedAssetIds] = useState<number[]>([]);
  const [pendingImportPath, setPendingImportPath] = useState<string | null>(null);
  const [includeImportSubfolders, setIncludeImportSubfolders] = useState(false);
  const [includeImportSubfolderImages, setIncludeImportSubfolderImages] = useState(true);
  const [assetDropTargetLibraryId, setAssetDropTargetLibraryId] = useState<number | null>(null);
  const [batchEditorOpen, setBatchEditorOpen] = useState(false);
  const [batchField, setBatchField] = useState("primary_category");
  const [batchValue, setBatchValue] = useState<string[]>([]);
  const refreshTimerRef = useRef<number | null>(null);
  const filterPopoverRef = useRef<HTMLDivElement | null>(null);
  const gridResultsRef = useRef<HTMLDivElement | null>(null);
  const scanProgressRef = useRef<ScanProgress | null>(null);
  const scanTaskIdRef = useRef<string | null>(null);
  const scanTerminalTaskIdRef = useRef<string | null>(null);
  const retiredScanTaskIdsRef = useRef(new Set<string>());
  const scanControlTokenRef = useRef(0);
  const scanCancelRequestRef = useRef<{ taskId: string; token: number } | null>(null);
  const semanticProgressRef = useRef<SemanticProgress | null>(null);
  const semanticJobIdRef = useRef<string | null>(null);
  const semanticTerminalJobIdRef = useRef<string | null>(null);
  const retiredSemanticJobIdsRef = useRef(new Set<string>());
  const semanticControlTokenRef = useRef(0);
  const semanticControlRequestRef = useRef<{ jobId: string; token: number } | null>(null);
  const semanticProgressRevisionRef = useRef(0);
  const semanticStatusRef = useRef<SemanticRuntimeStatus | null>(null);
  const semanticStatusRevisionRef = useRef(0);
  const subjectStatusRef = useRef<SubjectRuntimeStatus | null>(null);
  const subjectStatusRevisionRef = useRef(0);
  const analysisActionBusyRef = useRef(false);
  const ensureAnalysisModelsRef = useRef<
    | (() => Promise<{
        nextSemanticStatus: SemanticRuntimeStatus | null;
        requestedBackend: string;
      }>)
    | null
  >(null);
  const lastAutoPrepareKeyRef = useRef<string | null>(null);
  const appSettingsRef = useRef(appSettings);
  const gpuCapabilitiesRef = useRef(gpuCapabilities);
  appSettingsRef.current = appSettings;
  gpuCapabilitiesRef.current = gpuCapabilities;

  function updateAppSettings(nextSettings: AppSettings) {
    appSettingsRef.current = nextSettings;
    setAppSettings(nextSettings);
  }

  const reportGpuFallback = useCallback((message: string) => {
    const current = gpuCapabilitiesRef.current;
    if (!current || current.directml.state !== "ready") return;
    const next = {
      ...current,
      directml: { ...current.directml, state: "error", message },
    };
    gpuCapabilitiesRef.current = next;
    setGpuCapabilities(next);
  }, []);

  const aiSearchTriggerRef = useRef<HTMLButtonElement | null>(null);
  const assetsRef = useRef<AssetListItem[]>([]);
  const assetQueryRef = useRef(assetQuery);
  const assetLoadGenerationRef = useRef(0);
  const nextAssetPageRef = useRef(1);
  const hasMoreAssetsRef = useRef(false);
  const loadingMoreAssetsRef = useRef(false);
  const manualMarkRequestVersionRef = useRef(new Map<number, number>());
  const assetPointerDragRef = useRef<AssetPointerDragState | null>(null);
  const librariesRef = useRef(libraries);
  const assetAssignmentRef = useRef<(assetIds: number[], targetLibraryId: number) => void>(
    () => {},
  );
  const beginScanProgress = useCallback((progress: ScanProgress) => {
    if (scanProgressRef.current?.taskId === progress.taskId) return;
    if (retiredScanTaskIdsRef.current.has(progress.taskId)) return;
    if (scanTaskIdRef.current !== null) {
      retiredScanTaskIdsRef.current.add(scanTaskIdRef.current);
    }
    scanControlTokenRef.current += 1;
    scanCancelRequestRef.current = null;
    scanTaskIdRef.current = progress.taskId;
    scanTerminalTaskIdRef.current = null;
    scanProgressRef.current = progress;
    setCancellingScan(false);
    setScanProgress(progress);
  }, []);
  const acceptScanProgress = useCallback((progress: ScanProgress) => {
    const current = scanProgressRef.current;
    if (retiredScanTaskIdsRef.current.has(progress.taskId)) return false;
    const switchingTask =
      scanTaskIdRef.current !== null && scanTaskIdRef.current !== progress.taskId;
    if (
      (switchingTask && current !== null && !isTerminalScanStatus(current.status)) ||
      scanTerminalTaskIdRef.current === progress.taskId ||
      (current?.taskId === progress.taskId && isTerminalScanStatus(current.status))
    ) {
      return false;
    }
    if (switchingTask) {
      retiredScanTaskIdsRef.current.add(scanTaskIdRef.current as string);
      scanTerminalTaskIdRef.current = null;
    }
    scanTaskIdRef.current = progress.taskId;
    scanProgressRef.current = progress;
    if (isTerminalScanStatus(progress.status)) {
      scanTerminalTaskIdRef.current = progress.taskId;
      retiredScanTaskIdsRef.current.add(progress.taskId);
    }
    setScanProgress(progress);
    return true;
  }, []);
  const dismissScanProgress = useCallback(() => {
    scanCancelRequestRef.current = null;
    setCancellingScan(false);
    setScanProgress(null);
  }, []);
  const beginSemanticProgress = useCallback((progress: SemanticProgress) => {
    if (semanticProgressRef.current?.jobId === progress.jobId) return;
    if (retiredSemanticJobIdsRef.current.has(progress.jobId)) return;
    if (semanticJobIdRef.current !== null) {
      retiredSemanticJobIdsRef.current.add(semanticJobIdRef.current);
    }
    semanticControlTokenRef.current += 1;
    semanticControlRequestRef.current = null;
    setSemanticControlBusy(false);
    semanticProgressRevisionRef.current += 1;
    semanticJobIdRef.current = progress.jobId;
    semanticTerminalJobIdRef.current = null;
    semanticProgressRef.current = progress;
    setSemanticProgress(progress);
  }, []);
  const acceptSemanticProgress = useCallback(
    (progress: SemanticProgress) => {
      const current = semanticProgressRef.current;
      if (retiredSemanticJobIdsRef.current.has(progress.jobId)) return false;
      const switchingJob =
        semanticJobIdRef.current !== null && semanticJobIdRef.current !== progress.jobId;
      if (
        (switchingJob && current !== null && !isTerminalSemanticStatus(current.status)) ||
        semanticTerminalJobIdRef.current === progress.jobId ||
        (current?.jobId === progress.jobId && isTerminalSemanticStatus(current.status))
      ) {
        return false;
      }
      if (switchingJob) {
        retiredSemanticJobIdsRef.current.add(semanticJobIdRef.current as string);
        semanticControlRequestRef.current = null;
        semanticControlTokenRef.current += 1;
        semanticTerminalJobIdRef.current = null;
        setSemanticControlBusy(false);
      }
      semanticJobIdRef.current = progress.jobId;
      semanticProgressRevisionRef.current += 1;
      semanticProgressRef.current = progress;
      if (isTerminalSemanticStatus(progress.status)) {
        semanticTerminalJobIdRef.current = progress.jobId;
        retiredSemanticJobIdsRef.current.add(progress.jobId);
      }
      setSemanticProgress(progress);
      if (
        progress.executionBackend === "cpu" &&
        semanticStatusRef.current?.selectedBackend === "direct_ml" &&
        appSettingsRef.current.gpuAccelerationEnabled
      ) {
        reportGpuFallback("DirectML 运行时不可用，当前分析已回退 CPU。");
      }
      return true;
    },
    [reportGpuFallback],
  );
  const closeSettingsDialog = useCallback(() => {
    setSettingsOpen(false);
  }, []);
  assetsRef.current = assets;
  assetQueryRef.current = assetQuery;

  const setCurrentLibraryId = useCallback((next: ValueUpdater<number | null>) => {
    setAssetQuery((current) => {
      const libraryId = typeof next === "function" ? next(current.libraryId) : next;
      if (
        libraryId === current.libraryId &&
        (libraryId === null ||
          (!current.filter.favoriteOnly && current.filter.collectionId === null))
      ) {
        return current;
      }
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

  const loadMoreAssets = useCallback(async (): Promise<AssetListItem[]> => {
    if (!browseRootActive || !hasMoreAssetsRef.current || loadingMoreAssetsRef.current) {
      return [];
    }

    const generation = assetLoadGenerationRef.current;
    const page = nextAssetPageRef.current;
    loadingMoreAssetsRef.current = true;
    setLoadingMoreAssets(true);
    try {
      const result = await fetchAssets({
        ...assetQueryRef.current,
        page,
        pageSize: PAGE_SIZE,
      });
      if (generation !== assetLoadGenerationRef.current) return [];

      setAssets((current) => {
        const existingIds = new Set(current.map((asset) => asset.id));
        return [...current, ...result.items.filter((asset) => !existingIds.has(asset.id))];
      });
      setAssetTotal(result.total);
      nextAssetPageRef.current = result.page + 1;
      const nextHasMore = result.items.length > 0 && result.page * result.pageSize < result.total;
      hasMoreAssetsRef.current = nextHasMore;
      setHasMoreAssets(nextHasMore);
      return result.items;
    } catch (reason: unknown) {
      if (generation === assetLoadGenerationRef.current) setError(messageFrom(reason));
      return [];
    } finally {
      if (generation === assetLoadGenerationRef.current) {
        loadingMoreAssetsRef.current = false;
        setLoadingMoreAssets(false);
      }
    }
  }, [browseRootActive]);

  const handleGridResultsScroll = useCallback(
    (event: React.UIEvent<HTMLDivElement>) => {
      const element = event.currentTarget;
      if (element.scrollHeight - element.scrollTop - element.clientHeight <= LOAD_MORE_AHEAD_PX) {
        void loadMoreAssets();
      }
    },
    [loadMoreAssets],
  );

  const handleGridZoomWheel = useCallback((event: React.WheelEvent<HTMLDivElement>) => {
    if (!event.ctrlKey || event.deltaY === 0) return;
    event.preventDefault();
    setGridColumns((current) =>
      Math.max(
        GRID_COLUMNS_MIN,
        Math.min(
          GRID_COLUMNS_MAX,
          current + (event.deltaY < 0 ? -GRID_COLUMNS_STEP : GRID_COLUMNS_STEP),
        ),
      ),
    );
  }, []);

  useEffect(() => {
    let active = true;
    let unlistenSemantic: (() => void) | null = null;
    let unlistenSubject: (() => void) | null = null;
    const semanticSubscription = subscribeSemanticStatus((status) => {
      if (!active) return;
      semanticStatusRevisionRef.current += 1;
      semanticStatusRef.current = status;
      setSemanticStatus(status);
      setSemanticStatusReadError(null);
      setSemanticPreparationError(null);
    }).then((nextUnlisten) => {
      if (active) unlistenSemantic = nextUnlisten;
      else nextUnlisten();
    });
    const subjectSubscription = subscribeSubjectStatus((status) => {
      if (!active) return;
      subjectStatusRevisionRef.current += 1;
      subjectStatusRef.current = status;
      setSubjectStatus(status);
      setSubjectStatusReadError(null);
      setSubjectPreparationError(null);
    }).then((nextUnlisten) => {
      if (active) unlistenSubject = nextUnlisten;
      else nextUnlisten();
    });
    void Promise.allSettled([semanticSubscription, subjectSubscription]).then(async () => {
      if (!active) return;
      const semanticRevision = semanticStatusRevisionRef.current;
      const subjectRevision = subjectStatusRevisionRef.current;
      const [semanticResult, subjectResult] = await Promise.allSettled([
        fetchSemanticStatus(),
        fetchSubjectStatus(),
      ]);
      if (!active) return;
      if (semanticResult.status === "fulfilled") {
        if (semanticRevision === semanticStatusRevisionRef.current) {
          semanticStatusRevisionRef.current += 1;
          semanticStatusRef.current = semanticResult.value;
          setSemanticStatus(semanticResult.value);
          setSemanticStatusReadError(null);
        }
      } else if (semanticRevision === semanticStatusRevisionRef.current) {
        setSemanticStatusReadError(messageFrom(semanticResult.reason));
      }
      if (subjectResult.status === "fulfilled") {
        if (subjectRevision === subjectStatusRevisionRef.current) {
          subjectStatusRevisionRef.current += 1;
          subjectStatusRef.current = subjectResult.value;
          setSubjectStatus(subjectResult.value);
          setSubjectStatusReadError(null);
        }
      } else if (subjectRevision === subjectStatusRevisionRef.current) {
        setSubjectStatusReadError(messageFrom(subjectResult.reason));
      }
    });
    return () => {
      active = false;
      unlistenSemantic?.();
      unlistenSubject?.();
    };
  }, []);

  useEffect(() => {
    let active = true;
    void fetchGpuCapabilities()
      .then((capabilities) => {
        if (active) {
          gpuCapabilitiesRef.current = capabilities;
          setGpuCapabilities(capabilities);
        }
      })
      .catch(() => {
        // Hardware probing is diagnostic and must not block the main workspace.
      });
    return () => {
      active = false;
    };
  }, []);

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
    let cancelled = false;
    if (currentLibraryId === null) {
      return undefined;
    }
    void fetchCollections()
      .then((items) => {
        if (!cancelled) setCollections(items);
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
      window.localStorage.setItem("photo-organizer-theme", themeMode);
    } catch {
      // Private browsing or a restricted webview may disable local storage.
    }
  }, [themeMode]);

  useEffect(() => {
    persistAppSettings(appSettings);
  }, [appSettings]);

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
    detailAsset !== null &&
    (detailBaseAsset === null ||
      detailAsset.id === detailBaseAsset.id ||
      detailAsset.id === activeAssetId)
      ? {
          ...detailAsset,
          ...(detailBaseAsset && detailAsset.id === detailBaseAsset.id
            ? { rating: detailBaseAsset.rating, colorLabel: detailBaseAsset.colorLabel }
            : {}),
        }
      : detailBaseAsset;
  const scanRunning =
    scanProgress !== null && ["running", "cancelling"].includes(scanProgress.status);
  const semanticRunning =
    semanticProgress !== null &&
    ["queued", "running", "paused", "cancelling", "interrupted"].includes(semanticProgress.status);
  const preferredBackendForSettings = preferredAnalysisBackend(
    gpuCapabilities,
    appSettings.gpuAccelerationEnabled,
  );
  const previousPreferredBackendRef = useRef(preferredBackendForSettings);
  useEffect(() => {
    const backendChanged = previousPreferredBackendRef.current !== preferredBackendForSettings;
    previousPreferredBackendRef.current = preferredBackendForSettings;
    const loadedBackendMismatch =
      semanticStatus?.status === "ready" &&
      semanticStatus.selectedBackend !== preferredBackendForSettings;
    if (!backendChanged && !loadedBackendMismatch) return;
    if (semanticRunning || analysisActionBusy) return;
    const autoPrepareKey = `${preferredBackendForSettings}:${semanticStatus?.status ?? "unknown"}:${semanticStatus?.selectedBackend ?? "none"}`;
    if (!backendChanged && lastAutoPrepareKeyRef.current === autoPrepareKey) return;
    lastAutoPrepareKeyRef.current = autoPrepareKey;

    analysisActionBusyRef.current = true;
    void Promise.resolve().then(async () => {
      setAnalysisActionBusy(true);
      try {
        await ensureAnalysisModelsRef.current?.();
      } catch (reason) {
        setSemanticPreparationError(messageFrom(reason));
      } finally {
        analysisActionBusyRef.current = false;
        setAnalysisActionBusy(false);
      }
    });
  }, [
    analysisActionBusy,
    appSettings.gpuAccelerationEnabled,
    gpuCapabilities,
    preferredBackendForSettings,
    semanticRunning,
    semanticStatus?.selectedBackend,
    semanticStatus?.status,
  ]);
  const semanticModelNotice = modelRuntimeNotice(
    "题材模型",
    semanticStatus,
    semanticStatusReadError,
    semanticPreparationError,
  );
  const subjectModelNotice = modelRuntimeNotice(
    "主体模型",
    subjectStatus,
    subjectStatusReadError,
    subjectPreparationError,
  );
  const modelCanRetry =
    modelRuntimeCanRetry(semanticStatus, semanticStatusReadError, semanticPreparationError) ||
    modelRuntimeCanRetry(subjectStatus, subjectStatusReadError, subjectPreparationError);
  const modelPreparationLoading = semanticStatus?.status === "loading";
  const activeFilterCount = countActiveFilters(filter);
  const activeCollection = filter.favoriteOnly
    ? (collections.find((collection) => collection.systemKey === "default_favorites") ?? null)
    : filter.collectionId !== null
      ? (collections.find((collection) => collection.id === filter.collectionId) ?? null)
      : null;
  const activeCollectionHasChildren =
    activeCollection !== null &&
    collections.some((collection) => collection.parentCollectionId === activeCollection.id);
  const collectionSubfilterCount = countActiveFilters({
    ...filter,
    favoriteOnly: false,
    collectionId: null,
  });
  const canRemoveFromCurrentCollection =
    activeCollection !== null && !activeCollectionHasChildren && collectionSubfilterCount === 0;
  const canMoveFromCurrentCollection =
    canRemoveFromCurrentCollection && activeCollection?.collectionKind === "manual";
  const browseRootLabel =
    activeCollection?.name ??
    (filter.favoriteOnly
      ? "默认收藏"
      : filter.collectionId !== null
        ? "收藏夹"
        : (selectedLibrary?.name ?? "未选择图库"));
  const effectiveGridColumns = gridColumns;
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
  const libraryName = browseRootLabel === "未选择图库" ? "PhotoOrganizer" : browseRootLabel;
  const taskLibraryName = (libraryId: number | null) =>
    libraryId === null
      ? null
      : (libraries.find((library) => library.id === libraryId)?.name ?? null);
  const scanTaskName =
    scanProgress === null
      ? null
      : (taskLibraryName(scanProgress.libraryId) ?? selectedLibrary?.name ?? "当前图库");
  const semanticTaskName =
    semanticProgress === null
      ? null
      : (activeCollection?.name ??
        taskLibraryName(semanticProgress.libraryId) ??
        selectedLibrary?.name ??
        "当前图库");

  useEffect(() => {
    if (!filterPopoverOpen) return undefined;

    const handlePointerDown = (event: PointerEvent) => {
      const target = event.target;
      if (target instanceof Node && !filterPopoverRef.current?.contains(target)) {
        setFilterPopoverOpen(false);
      }
    };
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape" || event.defaultPrevented || event.isComposing) return;
      event.preventDefault();
      event.stopPropagation();
      setFilterPopoverOpen(false);
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
      fetchBrowseNodes(),
      fetchCollections(),
      fetchSemanticCatalog(),
      fetchClassificationRegistry(),
    ]).then(([libraryResult, browseResult, collectionsResult, catalogResult, registryResult]) => {
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
      if (browseResult.status === "fulfilled") setBrowseNodes(browseResult.value);
      else setError(messageFrom(browseResult.reason));
      if (collectionsResult.status === "fulfilled") setCollections(collectionsResult.value);
      else setError(messageFrom(collectionsResult.reason));
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
    if (!browseRootActive) {
      assetLoadGenerationRef.current += 1;
      hasMoreAssetsRef.current = false;
      loadingMoreAssetsRef.current = false;
      return undefined;
    }
    const generation = ++assetLoadGenerationRef.current;
    nextAssetPageRef.current = 1;
    hasMoreAssetsRef.current = false;
    loadingMoreAssetsRef.current = false;
    queueMicrotask(() => {
      if (!active || generation !== assetLoadGenerationRef.current) return;
      setHasMoreAssets(false);
      setLoadingMoreAssets(false);
    });
    const request = fetchAssets({
      ...assetQuery,
      page: 1,
      pageSize: PAGE_SIZE,
    });
    void request
      .then((result) => {
        if (!active || generation !== assetLoadGenerationRef.current) return;
        setAssets(result.items);
        setAssetTotal(result.total);
        setActiveAssetId((current) => {
          if (current && result.items.some((item) => item.id === current)) return current;
          return null;
        });
        nextAssetPageRef.current = result.page + 1;
        const nextHasMore = result.items.length > 0 && result.page * result.pageSize < result.total;
        hasMoreAssetsRef.current = nextHasMore;
        setHasMoreAssets(nextHasMore);
      })
      .catch((reason: unknown) => {
        if (active && generation === assetLoadGenerationRef.current) {
          setError(messageFrom(reason));
        }
      })
      .finally(() => {
        if (active && generation === assetLoadGenerationRef.current) setLoading(false);
      });
    return () => {
      active = false;
    };
  }, [assetQuery, refreshKey, browseRootActive]);

  useEffect(() => {
    if (viewMode !== "grid" || !hasMoreAssets || loadingMoreAssets) return undefined;
    const element = gridResultsRef.current;
    if (!element || element.clientHeight <= 0 || element.scrollHeight <= 0) return undefined;
    if (element.scrollHeight - element.clientHeight <= LOAD_MORE_AHEAD_PX) {
      void loadMoreAssets();
    }
    return undefined;
  }, [assets.length, hasMoreAssets, loadingMoreAssets, loadMoreAssets, viewMode]);

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
    const fetchProgressRevision = semanticProgressRevisionRef.current;
    void Promise.all([
      fetchLibraries(),
      fetchBrowseNodes(),
      fetchSemanticGroups(currentLibraryId),
      fetchSemanticProgress(currentLibraryId),
    ])
      .then(([nextLibraries, nextBrowseNodes, nextGroups, progress]) => {
        if (!active) return;
        setLibraries(nextLibraries);
        setBrowseNodes(nextBrowseNodes);
        setSemanticGroups(nextGroups);
        if (fetchProgressRevision !== semanticProgressRevisionRef.current) return;
        if (progress) {
          acceptSemanticProgress(progress);
        } else if (semanticJobIdRef.current === null) {
          semanticProgressRevisionRef.current += 1;
          semanticProgressRef.current = null;
          setSemanticProgress(null);
        }
      })
      .catch((reason: unknown) => {
        if (active && fetchProgressRevision === semanticProgressRevisionRef.current) {
          setError(messageFrom(reason));
        }
      });
    return () => {
      active = false;
    };
  }, [acceptSemanticProgress, refreshKey, currentLibraryId]);

  useEffect(() => {
    let disposed = false;
    let stopScan: (() => void) | undefined;
    let stopSemantic: (() => void) | undefined;
    void subscribeScanProgress((progress) => {
      if (disposed) return;
      if (!acceptScanProgress(progress)) return;
      if (progress.libraryId !== null) setCurrentLibraryId(progress.libraryId);
      if (isTerminalScanStatus(progress.status)) {
        if (scanCancelRequestRef.current?.taskId === progress.taskId) {
          scanCancelRequestRef.current = null;
          scanControlTokenRef.current += 1;
        }
        setCancellingScan(false);
        requestDataRefresh(true);
      }
    }).then((stop) => {
      if (disposed) stop();
      else stopScan = stop;
    });
    void subscribeSemanticProgress((progress) => {
      if (disposed) return;
      if (!acceptSemanticProgress(progress)) return;
      if (isTerminalSemanticStatus(progress.status)) {
        if (semanticControlRequestRef.current?.jobId === progress.jobId) {
          semanticControlRequestRef.current = null;
          semanticControlTokenRef.current += 1;
          setSemanticControlBusy(false);
        }
        requestDataRefresh(true);
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
  }, [acceptScanProgress, acceptSemanticProgress, requestDataRefresh, setCurrentLibraryId]);

  function updateFilter(next: AssetFilter) {
    const normalized = {
      ...next,
      toneLabels: [],
      saturationLevels: [],
      ratings: next.ratings.length > 1 ? [Math.max(...next.ratings)] : next.ratings,
    };
    setFilterState(normalized);
    setPage(1);
  }

  async function importFolder() {
    setError(null);
    try {
      const rootPath = await chooseLibraryFolder();
      if (!rootPath) return;
      setIncludeImportSubfolders(false);
      setIncludeImportSubfolderImages(true);
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
        includeSubfolderImages: includeImportSubfolderImages,
        importWorkerCount: appSettings.importWorkerCount,
      });
      setPendingImportPath(null);
      beginScanProgress({
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
    const progress = scanProgressRef.current;
    if (!progress || isTerminalScanStatus(progress.status)) return;
    if (scanCancelRequestRef.current?.taskId === progress.taskId) return;
    const taskId = progress.taskId;
    const token = scanControlTokenRef.current + 1;
    scanControlTokenRef.current = token;
    scanCancelRequestRef.current = { taskId, token };
    setCancellingScan(true);
    try {
      const result = await cancelLibraryScan(taskId);
      const request = scanCancelRequestRef.current;
      if (!request || request.taskId !== taskId || request.token !== token) return;
      scanCancelRequestRef.current = null;
      if (!result.accepted) {
        setCancellingScan(false);
        setError("扫描任务已经结束，无法再次取消。");
      }
    } catch (reason) {
      if (
        scanCancelRequestRef.current?.taskId !== taskId ||
        scanCancelRequestRef.current.token !== token
      ) {
        return;
      }
      scanCancelRequestRef.current = null;
      setError(messageFrom(reason));
      setCancellingScan(false);
    }
  }

  async function ensureAnalysisModels() {
    const currentRequestedBackend = () =>
      preferredAnalysisBackend(
        gpuCapabilitiesRef.current,
        appSettingsRef.current.gpuAccelerationEnabled,
      );
    while (true) {
      const requestedBackend = currentRequestedBackend();
      let nextSemanticStatus = semanticStatusRef.current;
      if (nextSemanticStatus?.status === "loading") {
        return { nextSemanticStatus, requestedBackend };
      }
      if (
        nextSemanticStatus?.status !== "ready" ||
        topicModelIdFromStatus(nextSemanticStatus) !== TOPIC_MODEL_ID ||
        nextSemanticStatus.selectedBackend !== requestedBackend
      ) {
        const statusRevision = semanticStatusRevisionRef.current;
        const preparedStatus = await prepareSemanticModel(TOPIC_MODEL_ID, requestedBackend);
        if (currentRequestedBackend() !== requestedBackend) continue;
        if (statusRevision === semanticStatusRevisionRef.current) {
          semanticStatusRevisionRef.current += 1;
          semanticStatusRef.current = preparedStatus;
          setSemanticStatus(preparedStatus);
          setSemanticStatusReadError(null);
          setSemanticPreparationError(null);
        }
        nextSemanticStatus = semanticStatusRef.current;
        if (
          requestedBackend === "direct_ml" &&
          nextSemanticStatus?.selectedBackend !== requestedBackend
        ) {
          reportGpuFallback(
            nextSemanticStatus?.message || "DirectML 模型会话未通过，当前分析已回退 CPU。",
          );
        }
        if (currentRequestedBackend() !== requestedBackend) continue;
      }

      nextSemanticStatus = semanticStatusRef.current;
      if (nextSemanticStatus?.status === "loading" || nextSemanticStatus?.status !== "ready") {
        return { nextSemanticStatus, requestedBackend };
      }

      const nextSubjectStatus = subjectStatusRef.current;
      const subjectNeedsPreparation =
        nextSubjectStatus === null ||
        (nextSubjectStatus.status !== "ready" && nextSubjectStatus.status !== "partial") ||
        nextSubjectStatus.selectedBackend !== requestedBackend;
      if (nextSubjectStatus?.status !== "loading" && subjectNeedsPreparation) {
        const statusRevision = subjectStatusRevisionRef.current;
        try {
          const preparedStatus = await prepareSubjectModel(requestedBackend);
          if (currentRequestedBackend() !== requestedBackend) continue;
          if (statusRevision === subjectStatusRevisionRef.current) {
            subjectStatusRevisionRef.current += 1;
            subjectStatusRef.current = preparedStatus;
            setSubjectStatus(preparedStatus);
            setSubjectStatusReadError(null);
            setSubjectPreparationError(null);
          }
        } catch (reason) {
          if (currentRequestedBackend() !== requestedBackend) continue;
          if (statusRevision === subjectStatusRevisionRef.current) {
            setSubjectPreparationError(messageFrom(reason));
          }
        }
      }
      if (currentRequestedBackend() !== requestedBackend) continue;
      return { nextSemanticStatus, requestedBackend };
    }
  }

  ensureAnalysisModelsRef.current = ensureAnalysisModels;

  async function prepareOrAnalyze() {
    if (analysisActionBusyRef.current) return;
    analysisActionBusyRef.current = true;
    setAnalysisActionBusy(true);
    setError(null);
    let preparingModels = true;
    try {
      const { nextSemanticStatus } = await ensureAnalysisModels();
      preparingModels = false;
      if (nextSemanticStatus?.status !== "ready") {
        return;
      }
      if (currentLibraryId === null) return;
      const { jobId } = await startSemanticAnalysis(currentLibraryId, false, {
        batchSize: effectiveAnalysisBatchSize(
          appSettingsRef.current.analysisBatchSize,
          gpuCapabilitiesRef.current,
          nextSemanticStatus.selectedBackend === "direct_ml",
        ),
      });
      beginSemanticProgress(pendingSemanticProgress(jobId, currentLibraryId, nextSemanticStatus));
    } catch (reason) {
      const message = messageFrom(reason);
      if (preparingModels) setSemanticPreparationError(message);
      setError(message);
    } finally {
      analysisActionBusyRef.current = false;
      setAnalysisActionBusy(false);
    }
  }

  async function retryModelPreparation() {
    if (analysisActionBusyRef.current) return;
    analysisActionBusyRef.current = true;
    setAnalysisActionBusy(true);
    setSemanticPreparationError(null);
    setSubjectPreparationError(null);
    try {
      await ensureAnalysisModels();
    } catch (reason) {
      setSemanticPreparationError(messageFrom(reason));
    } finally {
      analysisActionBusyRef.current = false;
      setAnalysisActionBusy(false);
    }
  }

  async function analyzeOne(asset: AssetListItem) {
    if (analysisActionBusyRef.current) return;
    analysisActionBusyRef.current = true;
    setAnalysisActionBusy(true);
    setError(null);
    let preparingModels = true;
    try {
      const { nextSemanticStatus } = await ensureAnalysisModels();
      preparingModels = false;
      if (nextSemanticStatus?.status !== "ready") return;
      const { jobId } = await reanalyzeAsset(asset.libraryId, asset.id, {
        batchSize: effectiveAnalysisBatchSize(
          appSettingsRef.current.analysisBatchSize,
          gpuCapabilitiesRef.current,
          nextSemanticStatus.selectedBackend === "direct_ml",
        ),
      });
      beginSemanticProgress(pendingSemanticProgress(jobId, asset.libraryId, nextSemanticStatus));
    } catch (reason) {
      const message = messageFrom(reason);
      if (preparingModels) setSemanticPreparationError(message);
      setError(message);
    } finally {
      analysisActionBusyRef.current = false;
      setAnalysisActionBusy(false);
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
      return;
    }
    try {
      await refreshCollectionNavigation();
      if (filter.favoriteOnly) requestDataRefresh(true);
    } catch (reason) {
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

  async function requestSemanticControl(action: "pause" | "resume" | "cancel") {
    const progress = semanticProgressRef.current;
    if (
      !progress ||
      progress.status === "cancelling" ||
      isTerminalSemanticStatus(progress.status) ||
      semanticControlRequestRef.current !== null
    ) {
      return;
    }
    const jobId = progress.jobId;
    const token = semanticControlTokenRef.current + 1;
    semanticControlTokenRef.current = token;
    semanticControlRequestRef.current = { jobId, token };
    setSemanticControlBusy(true);
    try {
      const result =
        action === "pause"
          ? await pauseSemanticAnalysis(jobId)
          : action === "resume"
            ? await resumeSemanticAnalysis(jobId)
            : await cancelSemanticAnalysis(jobId);
      const request = semanticControlRequestRef.current;
      if (
        !request ||
        request.jobId !== jobId ||
        request.token !== token ||
        semanticJobIdRef.current !== jobId
      ) {
        return;
      }
      semanticProgressRevisionRef.current += 1;
      semanticControlRequestRef.current = null;
      setSemanticControlBusy(false);
      if (!result.accepted) {
        setError("语义任务已经结束，无法修改。");
        return;
      }
      const current = semanticProgressRef.current;
      if (!current || current.jobId !== jobId || isTerminalSemanticStatus(current.status)) {
        return;
      }
      const nextStatus =
        action === "pause" ? "paused" : action === "resume" ? "running" : "cancelling";
      const nextProgress = { ...current, status: nextStatus };
      semanticProgressRef.current = nextProgress;
      setSemanticProgress(nextProgress);
    } catch (reason) {
      if (
        semanticControlRequestRef.current?.jobId !== jobId ||
        semanticControlRequestRef.current.token !== token
      ) {
        return;
      }
      semanticControlRequestRef.current = null;
      setSemanticControlBusy(false);
      setError(messageFrom(reason));
    }
  }

  async function pauseOrResumeSemantic() {
    const progress = semanticProgressRef.current;
    if (!progress || isTerminalSemanticStatus(progress.status)) return;
    await requestSemanticControl(
      progress.status === "paused" || progress.status === "interrupted" ? "resume" : "pause",
    );
  }

  async function cancelSemantic() {
    await requestSemanticControl("cancel");
  }

  function selectLibrary(id: number) {
    setAssetQuery((current) =>
      updateAssetQueryBrowseRoot(
        { ...current, filter: emptyAssetFilter },
        { kind: "source", libraryId: id },
      ),
    );
    setActiveAssetId(null);
    setDetailAsset(null);
    setSelectionAnchorId(null);
    setSelectedAssetIds([]);
    setWorkspaceMode("library");
    setWorkflowTool(null);
  }

  function selectFavoriteSource() {
    setWorkflowTool(null);
    setActiveAssetId(null);
    setDetailAsset(null);
    setSelectionAnchorId(null);
    setSelectedAssetIds([]);
    setAssetQuery((current) => updateAssetQueryBrowseRoot(current, { kind: "favorites" }));
  }

  function selectCollectionSource(collectionId: number) {
    setWorkflowTool(null);
    setActiveAssetId(null);
    setDetailAsset(null);
    setSelectionAnchorId(null);
    setSelectedAssetIds([]);
    setAssetQuery((current) =>
      updateAssetQueryBrowseRoot(current, { kind: "collection", collectionId }),
    );
  }

  async function createSidebarCollection(name: string, parentCollectionId: number | null) {
    try {
      const created = await createCollection(name, "", parentCollectionId);
      const [nextCollections, nextBrowseNodes] = await Promise.all([
        fetchCollections(),
        fetchBrowseNodes(),
      ]);
      setCollections(nextCollections);
      setBrowseNodes(nextBrowseNodes);
      selectCollectionSource(created.id);
    } catch (reason) {
      setError(messageFrom(reason));
    }
  }

  async function refreshCollectionNavigation() {
    const [nextCollections, nextBrowseNodes, nextFavoriteIds] = await Promise.all([
      fetchCollections(),
      fetchBrowseNodes(),
      currentLibraryId !== null ? fetchFavoriteAssetIds(currentLibraryId) : Promise.resolve([]),
    ]);
    setCollections(nextCollections);
    setBrowseNodes(nextBrowseNodes);
    setFavoriteAssetIds(new Set(nextFavoriteIds));
  }

  async function submitCollectionDialog(submission: CollectionDialogSubmission) {
    setCollectionOperationBusy(true);
    try {
      if (submission.kind === "add-assets") {
        await addAssetsToCollections(submission.collectionIds, submission.assetIds);
        if (activeCollection && submission.collectionIds.includes(activeCollection.id)) {
          requestDataRefresh(true);
        }
      } else if (submission.kind === "move-assets") {
        await moveAssetsBetweenCollections(
          submission.sourceCollectionId,
          submission.targetCollectionId,
          submission.assetIds,
        );
        setSelectedAssetIds([]);
        setSelectionAnchorId(null);
        requestDataRefresh(true);
      } else if (submission.kind === "rename") {
        await renameCollection(submission.collectionId, submission.name);
      } else if (submission.kind === "move-collection") {
        await moveCollection(submission.collectionId, submission.parentCollectionId);
      } else {
        const leavesActiveCollection =
          filter.collectionId !== null &&
          (filter.collectionId === submission.collectionId ||
            (submission.mode === "deleteSubtree" &&
              isCollectionDescendant(filter.collectionId, submission.collectionId, collections)));
        await deleteCollection(submission.collectionId, submission.mode);
        if (leavesActiveCollection) {
          setAssetQuery((current) =>
            updateAssetQueryBrowseRoot(
              current,
              current.libraryId === null
                ? { kind: "all" }
                : { kind: "source", libraryId: current.libraryId },
            ),
          );
        }
      }
      await refreshCollectionNavigation();
      setCollectionDialogRequest(null);
    } catch (reason) {
      setError(messageFrom(reason));
    } finally {
      setCollectionOperationBusy(false);
    }
  }

  async function removeSelectionFromCurrentCollection() {
    if (!activeCollection || !canRemoveFromCurrentCollection || selectedAssetIds.length === 0) {
      return;
    }
    setCollectionOperationBusy(true);
    try {
      await removeAssetsFromCollection(activeCollection.id, selectedAssetIds);
      setSelectedAssetIds([]);
      setSelectionAnchorId(null);
      await refreshCollectionNavigation();
      requestDataRefresh(true);
    } catch (reason) {
      setError(messageFrom(reason));
    } finally {
      setCollectionOperationBusy(false);
    }
  }

  function openWorkflowTool(tool: WorkflowTool) {
    setWorkspaceMode("library");
    setWorkflowTool((current) => (current === tool ? null : tool));
  }

  const closeWorkflowTool = useCallback((restoreSearchFocus = false) => {
    setWorkflowTool(null);
    if (restoreSearchFocus) {
      window.requestAnimationFrame(() => aiSearchTriggerRef.current?.focus());
    }
  }, []);

  const focusAsset = useCallback((asset: AssetListItem) => {
    setActiveAssetId(asset.id);
    setDetailAsset(asset);
  }, []);

  const changeView = useCallback(
    (next: ViewMode) => {
      if (next === "single" && workflowTool !== null) closeWorkflowTool(false);
      setViewMode(next);
      if (next === "single") {
        const nextAsset = activeAsset ?? assets[0] ?? null;
        if (nextAsset) focusAsset(nextAsset);
        else {
          setActiveAssetId(null);
          setDetailAsset(null);
        }
      }
    },
    [activeAsset, assets, closeWorkflowTool, focusAsset, workflowTool],
  );

  function selectAsset(asset: AssetListItem, modifiers: SelectionModifiers = {}) {
    focusAsset(asset);
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
    // Workflow result clicks change the focused asset, not the user's explicit
    // selection. This keeps the current AssetScope intact while the DetailPanel
    // follows the review result.
    setActiveAssetId(assetId);
    const asset = assetsRef.current.find((item) => item.id === assetId);
    if (asset) setDetailAsset(asset);
    setSelectionAnchorId(assetId);
  }

  async function openWorkflowAsset(assetId: number) {
    setWorkspaceMode("library");
    setWorkflowTool(null);
    setViewMode("grid");
    setActiveAssetId(assetId);
    const knownAsset = assetsRef.current.find((asset) => asset.id === assetId);
    setDetailAsset(knownAsset ?? null);
    setSelectionAnchorId(assetId);

    let found = assetsRef.current.some((asset) => asset.id === assetId);
    let pagesLoaded = 0;
    while (!found && hasMoreAssetsRef.current && pagesLoaded < 64) {
      const pageItems = await loadMoreAssets();
      pagesLoaded += 1;
      found = pageItems.some((asset) => asset.id === assetId);
      if (pageItems.length === 0) break;
    }

    if (!found) {
      try {
        const detail = await fetchAssetDetail(assetId);
        if (detail) {
          setAssets((current) =>
            current.some((asset) => asset.id === detail.id) ? current : [...current, detail],
          );
        }
      } catch (reason) {
        setError(messageFrom(reason));
      }
    }

    const scrollToAsset = () => {
      const target = gridResultsRef.current?.querySelector<HTMLElement>(
        `[data-asset-id="${assetId}"]`,
      );
      target?.scrollIntoView?.({ block: "center", inline: "nearest", behavior: "auto" });
    };
    if (typeof window.requestAnimationFrame === "function") {
      window.requestAnimationFrame(() => window.requestAnimationFrame(scrollToAsset));
    } else {
      window.setTimeout(scrollToAsset, 0);
    }
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

  function toggleAssetSelectionById(assetId: number, modifiers: SelectionModifiers = {}) {
    setActiveAssetId(assetId);
    const asset = assetsRef.current.find((item) => item.id === assetId);
    if (asset) setDetailAsset(asset);
    if (modifiers.shiftKey) {
      const range = selectionRange(assetId);
      if (range) {
        setSelectedAssetIds(range);
        return;
      }
      setSelectedAssetIds([assetId]);
      setSelectionAnchorId(assetId);
      return;
    }
    setSelectionAnchorId(assetId);
    setSelectedAssetIds((current) =>
      current.includes(assetId) ? current.filter((id) => id !== assetId) : [...current, assetId],
    );
  }

  function toggleAssetSelection(asset: AssetListItem, modifiers: SelectionModifiers = {}) {
    toggleAssetSelectionById(asset.id, modifiers);
  }

  function toggleAssetSelectionOnly(asset: AssetListItem, modifiers: SelectionModifiers = {}) {
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

  const openSinglePreview = useCallback(
    (asset: AssetListItem) => {
      focusAsset(asset);
      setViewMode("single");
    },
    [focusAsset],
  );

  const selectPreview = useCallback(
    (asset: AssetListItem) => {
      focusAsset(asset);
    },
    [focusAsset],
  );

  const navigatePreview = useCallback(
    (delta: -1 | 1) => {
      if (!activeAsset) return;
      const index = assets.findIndex((asset) => asset.id === activeAsset.id);
      const target = assets[index + delta];
      if (target) {
        selectPreview(target);
      } else if (delta === 1 && hasMoreAssetsRef.current) {
        void loadMoreAssets().then((nextAssets) => {
          const nextAsset = nextAssets[0];
          if (nextAsset) selectPreview(nextAsset);
        });
      }
    },
    [activeAsset, assets, loadMoreAssets, selectPreview],
  );

  async function removeLibraryById(library: LibrarySummary) {
    const confirmed = window.confirm(
      "从 PhotoOrganizer 中移除此图库？\n\n这只会移除 PhotoOrganizer 中的索引、缩略图和分析结果，不会删除或修改磁盘中的任何原始图片。",
    );
    if (!confirmed) return;

    const descendants = descendantLibraries(library.id, libraries);
    const removeDescendants =
      descendants.length > 0 &&
      window.confirm(
        `${library.name || library.sourcePath} 包含 ${descendants.length} 个子图库（包括嵌套子图库）。\n\n` +
          "是否同时移除这些子图库？\n\n确定：移除当前图库和全部子图库。\n取消：仅移除当前图库，保留子图库。",
      );
    const targets = removeDescendants ? [...descendants].reverse().concat(library) : [library];
    const removedIds: number[] = [];
    try {
      for (const target of targets) {
        if (await removeLibrary(target.id)) removedIds.push(target.id);
      }
      const remaining = libraries.filter((item) => !removedIds.includes(item.id));
      setLibraries(remaining);
      if (currentLibraryId !== null && removedIds.includes(currentLibraryId)) {
        setCurrentLibraryId(remaining[0]?.id ?? null);
        setActiveAssetId(null);
        setSelectedAssetIds([]);
        setSelectionAnchorId(null);
        setPage(1);
      }
      requestDataRefresh(true);
    } catch (reason) {
      setError(messageFrom(reason));
      requestDataRefresh(true);
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
      const result = await requestLibraryRescan(library.id, {
        importWorkerCount: appSettings.importWorkerCount,
      });
      beginScanProgress({
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
      if (event.defaultPrevented || event.isComposing) return;

      const batchEditorVisible = batchEditorOpen && selectedAssetIds.length > 0;
      const blockingOverlayOpen =
        settingsOpen ||
        filterPopoverOpen ||
        collectionDialogRequest !== null ||
        pendingImportPath !== null ||
        batchEditorVisible;
      const interactiveTarget = isInteractiveKeyboardTarget(event.target);
      const galleryTarget = isGalleryKeyboardTarget(event.target);

      if (event.key === "Escape") {
        if (settingsOpen) {
          event.preventDefault();
          closeSettingsDialog();
          return;
        }
        if (pendingImportPath !== null) {
          event.preventDefault();
          setPendingImportPath(null);
          return;
        }
        if (collectionDialogRequest !== null) {
          event.preventDefault();
          if (!collectionOperationBusy) setCollectionDialogRequest(null);
          return;
        }
        if (batchEditorVisible) {
          event.preventDefault();
          setBatchEditorOpen(false);
          return;
        }
        if (filterPopoverOpen) {
          event.preventDefault();
          setFilterPopoverOpen(false);
          return;
        }
        if (workflowTool !== null) {
          event.preventDefault();
          closeWorkflowTool(true);
          return;
        }
        if (isInteractiveKeyboardTarget(event.target) && !isGalleryKeyboardTarget(event.target)) {
          return;
        }
        event.preventDefault();
        if (viewMode === "single") setViewMode("grid");
        else clearSelection();
        return;
      }

      if (!blockingOverlayOpen && (event.ctrlKey || event.metaKey) && event.key === ",") {
        event.preventDefault();
        setSettingsOpen(true);
        setFilterPopoverOpen(false);
        return;
      }

      if (blockingOverlayOpen || (interactiveTarget && !galleryTarget)) return;

      const hasShortcutModifier = event.ctrlKey || event.metaKey || event.altKey;

      const markedAssetIds =
        activeAsset && selectedAssetIds.includes(activeAsset.id)
          ? selectedAssetIds
          : activeAsset
            ? [activeAsset.id]
            : [];
      const ratingShortcut = Object.entries(appSettings.shortcuts.ratings).find(
        ([, shortcut]) => shortcut.toLowerCase() === event.key.toLowerCase(),
      );
      const ratingKey = ratingShortcut ? Number(ratingShortcut[0]) : null;
      const colorShortcut = Object.entries(appSettings.shortcuts.colors).find(
        ([, shortcut]) => shortcut.toLowerCase() === event.key.toLowerCase(),
      );
      if (!hasShortcutModifier && markedAssetIds.length > 0 && ratingKey !== null) {
        event.preventDefault();
        void Promise.all(markedAssetIds.map((assetId) => editAssetRating(assetId, ratingKey)));
        return;
      }
      if (
        !hasShortcutModifier &&
        markedAssetIds.length > 0 &&
        (event.key === appSettings.shortcuts.ratingDown ||
          event.key === appSettings.shortcuts.ratingUp)
      ) {
        event.preventDefault();
        const delta = event.key === appSettings.shortcuts.ratingUp ? 1 : -1;
        void Promise.all(
          markedAssetIds.map((assetId) => {
            const current = assets.find((item) => item.id === assetId)?.rating ?? 0;
            return editAssetRating(assetId, Math.max(0, Math.min(5, current + delta)));
          }),
        );
        return;
      }
      if (!hasShortcutModifier && markedAssetIds.length > 0 && colorShortcut) {
        event.preventDefault();
        const colorLabel = colorShortcut[0] as ManualColorLabel;
        void toggleAssetColorLabelForSelection(markedAssetIds[0], colorLabel);
        return;
      }
      const viewShortcut = Object.entries(appSettings.shortcuts.view).find(
        ([, shortcut]) => shortcut.toLowerCase() === event.key.toLowerCase(),
      );
      if (viewShortcut && !hasShortcutModifier) {
        event.preventDefault();
        if (viewShortcut[0] === "grid") changeView("grid");
        else changeView("single");
        return;
      }
      if (
        interactiveTarget &&
        !galleryTarget &&
        event.key !== "ArrowLeft" &&
        event.key !== "ArrowRight"
      ) {
        return;
      }
      if (event.key === "ArrowLeft" && viewMode === "single") {
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
    appSettings,
    assets,
    batchEditorOpen,
    changeView,
    closeSettingsDialog,
    closeWorkflowTool,
    collectionDialogRequest,
    collectionOperationBusy,
    editAssetColorLabel,
    editAssetRating,
    filterPopoverOpen,
    navigatePreview,
    openSinglePreview,
    pendingImportPath,
    selectedAssetIds,
    settingsOpen,
    toggleAssetColorLabelForSelection,
    viewMode,
    workflowTool,
  ]);

  const showBatchEditor = batchEditorOpen && selectedAssetIds.length > 0;
  const navigationView = workspaceMode === "organization" ? "organization" : "library";

  function openSettingsDialog() {
    setSettingsOpen(true);
    setFilterPopoverOpen(false);
  }

  return (
    <div
      className={`photo-app${showBatchEditor ? " has-batch-classification" : ""}${
        themeMode === "light" ? " theme-light" : ""
      }`}
      onContextMenu={(event) => {
        if (
          event.target instanceof Element &&
          event.target.closest("input, textarea, [contenteditable='true']")
        ) {
          return;
        }
        event.preventDefault();
      }}
    >
      <header className="topbar">
        <div className="app-identity">
          <span className="brand-mark">
            <LibraryIcon width="17" height="17" />
          </span>
          <div>
            <strong title={selectedLibrary?.sourcePath ?? browseRootLabel}>{libraryName}</strong>
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
                : activeCollection
                  ? "当前收藏夹"
                  : "尚未建立图库"}
          </span>
          <BackgroundTaskStatus
            key={scanProgress?.taskId ?? semanticProgress?.jobId ?? "background-tasks"}
            scanProgress={scanProgress}
            semanticProgress={semanticProgress}
            scanRunning={scanRunning}
            semanticRunning={semanticRunning}
            scanTaskName={scanTaskName}
            semanticTaskName={semanticTaskName}
            cancellingScan={cancellingScan}
            semanticControlBusy={semanticControlBusy}
            onCancelScan={() => void cancelScan()}
            onDismissScan={dismissScanProgress}
            onPauseResumeSemantic={() => void pauseOrResumeSemantic()}
            onCancelSemantic={() => void cancelSemantic()}
          />
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
              <button
                className="tool-button"
                type="button"
                onClick={() =>
                  setCollectionDialogRequest({
                    kind: "add-assets",
                    assetIds: [...selectedAssetIds],
                  })
                }
              >
                加入收藏
              </button>
              {canMoveFromCurrentCollection && activeCollection ? (
                <button
                  className="tool-button"
                  type="button"
                  onClick={() =>
                    setCollectionDialogRequest({
                      kind: "move-assets",
                      sourceCollectionId: activeCollection.id,
                      assetIds: [...selectedAssetIds],
                    })
                  }
                >
                  移动到收藏夹
                </button>
              ) : null}
              {canRemoveFromCurrentCollection ? (
                <button
                  className="tool-button"
                  type="button"
                  disabled={collectionOperationBusy}
                  onClick={() => void removeSelectionFromCurrentCollection()}
                >
                  移出收藏
                </button>
              ) : null}
              <button
                className="tool-button"
                type="button"
                disabled={selectedAssetIds.length < 2}
                onClick={() => openWorkflowTool("compare")}
              >
                比较
              </button>
              <button
                className="tool-button"
                type="button"
                onClick={() => openWorkflowTool("similar")}
              >
                找相似
              </button>
              <button
                className="tool-button"
                type="button"
                onClick={() => openWorkflowTool("duplicates")}
              >
                重复审阅
              </button>
              {selectedAssetIds.length === 1 ? (
                <button
                  className="tool-button"
                  type="button"
                  onClick={() => openWorkflowTool("edit")}
                >
                  编辑副本
                </button>
              ) : null}
            </div>
          ) : null}
          <div className="topbar-browse-controls" role="group" aria-label="浏览控制">
            <div className="segmented" aria-label="视图模式">
              <button
                type="button"
                className={viewMode === "grid" ? "is-active" : ""}
                onClick={() => changeView("grid")}
                aria-label="网格视图"
                aria-keyshortcuts={appSettings.shortcuts.view.grid}
                title={`网格视图（${appSettings.shortcuts.view.grid.toUpperCase()}）`}
              >
                <GridIcon width="16" height="16" />
              </button>
              <button
                type="button"
                className={viewMode === "single" ? "is-active" : ""}
                onClick={() => changeView("single")}
                aria-label="单图预览"
                aria-keyshortcuts={appSettings.shortcuts.view.single}
                title={`单图预览（${appSettings.shortcuts.view.single.toUpperCase()}）`}
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
                onChange={(event) =>
                  updateFilter({ ...filter, search: event.target.value || null })
                }
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
          </div>
          <div className="topbar-action-controls" role="group" aria-label="图库操作">
            {semanticModelNotice || subjectModelNotice ? (
              <span
                className="topbar-model-status"
                role="status"
                aria-live={modelCanRetry ? "assertive" : "polite"}
                title={[semanticModelNotice, subjectModelNotice].filter(Boolean).join("；")}
              >
                {[semanticModelNotice, subjectModelNotice].filter(Boolean).join("；")}
              </span>
            ) : null}
            {modelCanRetry ? (
              <button
                className="tool-button topbar-model-control topbar-model-retry-action"
                type="button"
                onClick={() => void retryModelPreparation()}
                disabled={analysisActionBusy || modelPreparationLoading}
              >
                重试模型准备
              </button>
            ) : null}
            <button
              className="primary-action topbar-analysis-action topbar-model-control"
              type="button"
              onClick={() => void prepareOrAnalyze()}
              disabled={
                !selectedLibrary || semanticRunning || analysisActionBusy || modelPreparationLoading
              }
            >
              <PlayIcon width="14" height="14" />
              分析
            </button>
          </div>
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
            <option value="primary_category">拍摄题材</option>
            <option value="tone">影调</option>
            <option value="dominant_color_category">主色</option>
            <option value="saturation_level">饱和度级别</option>
          </select>
          {batchField === "primary_category" ? (
            <select
              value={batchValue[0] ?? ""}
              onChange={(event) => setBatchValue(event.target.value ? [event.target.value] : [])}
            >
              <option value="">请选择拍摄题材</option>
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
        <AppNavigationRail
          activeView={navigationView}
          settingsActive={settingsOpen}
          hasLibrary={selectedLibrary !== null}
          onOpenLibrary={() => {
            setWorkspaceMode("library");
            setWorkflowTool(null);
          }}
          onOpenOrganization={() => {
            setWorkspaceMode("organization");
            setWorkflowTool(null);
          }}
          onOpenSettings={openSettingsDialog}
        />
        {workspaceMode === "organization" && selectedLibrary ? (
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
              browseNodes={browseNodes}
              selectedLibraryId={currentLibraryId}
              groups={semanticGroups}
              catalog={semanticCatalog}
              filter={filter}
              collections={collections}
              favoriteSourceActive={filter.favoriteOnly}
              activeCollectionId={filter.collectionId}
              browseRootActive={browseRootActive}
              libraryPanelRatio={sidebarLibraryRatio}
              onLibraryPanelRatioChange={setSidebarLibraryRatio}
              assetDropTargetLibraryId={assetDropTargetLibraryId}
              onImportLibrary={() => void importFolder()}
              onCreateCollection={(name, parentCollectionId) =>
                void createSidebarCollection(name, parentCollectionId)
              }
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
              onSelectFavorites={selectFavoriteSource}
              onSelectCollection={selectCollectionSource}
              onRenameCollection={(collection) =>
                setCollectionDialogRequest({ kind: "rename", collection })
              }
              onMoveCollection={(collection) =>
                setCollectionDialogRequest({ kind: "move-collection", collection })
              }
              onDeleteCollection={(collection, hasChildren) =>
                setCollectionDialogRequest({ kind: "delete", collection, hasChildren })
              }
            />

            <PanelResizeHandle
              side="left"
              value={leftPanelWidth}
              min={LEFT_PANEL_MIN_WIDTH}
              max={LEFT_PANEL_MAX_WIDTH}
              onChange={setLeftPanelWidth}
            />

            <div className="center-column">
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
                {browseRootActive ? (
                  <>
                    <div className="content-toolbar">
                      <div className="content-toolbar-summary">
                        <strong>{assetTotal.toLocaleString()} 张</strong>
                        <span>
                          {activeFilterCount
                            ? browseRootLabel + " · " + activeFilterCount + " 项筛选"
                            : browseRootLabel}
                        </span>
                        {selectedAssetIds.length > 0 ? (
                          <em className="selection-count">已选择 {selectedAssetIds.length} 张</em>
                        ) : null}
                      </div>
                      <div className="content-toolbar-manual">
                        <ManualMarkFilterBar filter={filter} onFilterChange={updateFilter} />
                      </div>
                      <div className="content-toolbar-controls">
                        <button
                          ref={aiSearchTriggerRef}
                          type="button"
                          className={
                            workflowTool === "search"
                              ? "tool-button content-toolbar-query is-active"
                              : "tool-button content-toolbar-query"
                          }
                          aria-label="AI 搜索"
                          aria-expanded={workflowTool === "search"}
                          title="AI 搜索"
                          onClick={() => openWorkflowTool("search")}
                        >
                          <SearchIcon width="15" height="15" />
                          <span>AI 搜索</span>
                        </button>
                        <label className="group-select-control">
                          <span>分组</span>
                          <select
                            aria-label="分组"
                            value={groupBy}
                            onChange={(event) => setGroupBy(event.target.value as AssetGroupBy)}
                          >
                            {GROUP_BY_OPTIONS.map((option) => (
                              <option value={option.value} key={option.value}>
                                {option.label}
                              </option>
                            ))}
                          </select>
                        </label>
                        <AnalysisStatusFilterBar
                          filter={filter}
                          visible={(selectedLibrary?.semanticPendingCount ?? 0) > 0}
                          onFilterChange={updateFilter}
                        />
                        {viewMode === "single" && activeAsset ? (
                          <button
                            type="button"
                            className={`single-selection-toggle single-selection-toolbar-toggle${
                              selectedAssetIds.includes(activeAsset.id) ? " is-selected" : ""
                            }`}
                            onClick={() => toggleAssetSelection(activeAsset)}
                            aria-label={
                              selectedAssetIds.includes(activeAsset.id)
                                ? `取消选择 ${activeAsset.fileName}`
                                : `选择 ${activeAsset.fileName}`
                            }
                            aria-pressed={selectedAssetIds.includes(activeAsset.id)}
                            title={selectedAssetIds.includes(activeAsset.id) ? "取消选择" : "选择"}
                          >
                            <span className="single-selection-mark" aria-hidden="true">
                              {selectedAssetIds.includes(activeAsset.id) ? (
                                <CheckIcon width="14" height="14" />
                              ) : null}
                            </span>
                          </button>
                        ) : null}
                      </div>
                    </div>
                    {loading && assets.length === 0 ? (
                      <GridLoading />
                    ) : viewMode === "single" ? (
                      <SinglePreview
                        assets={assets}
                        selected={activeAsset}
                        controller={previewController}
                        selectedAssetIds={selectedAssetIds}
                        onSelect={selectPreview}
                        onToggleSelection={toggleAssetSelectionOnly}
                      />
                    ) : (
                      <div className="grid-workspace-shell">
                        <div
                          ref={gridResultsRef}
                          className="grid-workspace-results"
                          onScroll={handleGridResultsScroll}
                          onWheel={handleGridZoomWheel}
                          style={
                            {
                              "--grid-column-count": effectiveGridColumns,
                            } as CSSProperties
                          }
                        >
                          {assets.length ? (
                            <GridWorkspace
                              key={groupBy}
                              assets={assets}
                              gridColumns={effectiveGridColumns}
                              active={activeAsset}
                              selectedAssetIds={selectedAssetIds}
                              groupBy={groupBy}
                              semanticCatalog={semanticCatalog}
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
                          {loadingMoreAssets ? (
                            <div className="asset-load-status" role="status">
                              正在加载更多图片…
                            </div>
                          ) : null}
                        </div>
                      </div>
                    )}
                  </>
                ) : !loading ? (
                  <section className="welcome-state" aria-labelledby="welcome-state-title">
                    <div className="welcome-state-rule" aria-hidden="true" />
                    <div className="welcome-state-copy">
                      <LibraryIcon className="welcome-state-icon" width="20" height="20" />
                      <h1 id="welcome-state-title">建立本地图片库</h1>
                      <div className="welcome-state-actions">
                        <button
                          className="primary-action"
                          type="button"
                          onClick={() => void importFolder()}
                        >
                          <ImportIcon width="16" height="16" />
                          选择照片文件夹
                        </button>
                        <span>JPEG · PNG · WebP</span>
                      </div>
                      <small className="welcome-state-local-note">本地处理 · 不修改原图</small>
                    </div>
                  </section>
                ) : null}
              </main>
              {workflowTool && selectedLibrary ? (
                <>
                  {workflowTool === "search" ? null : (
                    <WorkflowHeightResizeHandle
                      value={workflowPanelHeight}
                      min={WORKFLOW_HEIGHT_MIN}
                      max={WORKFLOW_HEIGHT_MAX}
                      step={WORKFLOW_HEIGHT_STEP}
                      onChange={setWorkflowPanelHeight}
                    />
                  )}
                  <div
                    className={
                      workflowTool === "search"
                        ? "workflow-embedded-shell is-floating-search"
                        : "workflow-embedded-shell"
                    }
                    style={{ "--workflow-height": `${workflowPanelHeight}px` } as CSSProperties}
                  >
                    <WorkflowWorkspace
                      key={workflowTool}
                      embedded
                      floatingSearch={workflowTool === "search"}
                      initialTool={workflowTool}
                      libraryId={selectedLibrary.id}
                      selectedAssetIds={selectedAssetIds}
                      activeAsset={detailPanelAsset}
                      scope={currentScope}
                      scopeDescription={currentScopeDescription}
                      onSelectAsset={selectWorkflowAsset}
                      onToggleSelection={(assetId, modifiers) =>
                        toggleAssetSelectionById(assetId, modifiers)
                      }
                      onUpdateRating={(assetId, rating) =>
                        editAssetRatingForSelection(assetId, rating)
                      }
                      onUpdateColorLabel={(assetId, colorLabel) =>
                        editAssetColorLabelForSelection(assetId, colorLabel)
                      }
                      onOpenAsset={openWorkflowAsset}
                      onBack={() => closeWorkflowTool(true)}
                      onFavoriteChange={(assetId, favorite) => {
                        setFavoriteAssetIds((current) => {
                          const next = new Set(current);
                          if (favorite) next.add(assetId);
                          else next.delete(assetId);
                          return next;
                        });
                        if (filter.favoriteOnly && !favorite) requestDataRefresh(true);
                      }}
                      onCollectionsChange={() => {
                        void Promise.all([fetchCollections(), fetchBrowseNodes()])
                          .then(([nextCollections, nextBrowseNodes]) => {
                            setCollections(nextCollections);
                            setBrowseNodes(nextBrowseNodes);
                          })
                          .catch((reason) => setError(messageFrom(reason)));
                      }}
                    />
                  </div>
                </>
              ) : null}
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
              subjectStatus={subjectStatus}
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
      {collectionDialogRequest ? (
        <CollectionActionDialog
          request={collectionDialogRequest}
          collections={collections}
          busy={collectionOperationBusy}
          onClose={() => {
            if (!collectionOperationBusy) setCollectionDialogRequest(null);
          }}
          onSubmit={(submission) => void submitCollectionDialog(submission)}
        />
      ) : null}
      {pendingImportPath ? (
        <ImportLibraryDialog
          path={pendingImportPath}
          includeSubfolders={includeImportSubfolders}
          includeSubfolderImages={includeImportSubfolderImages}
          onIncludeSubfoldersChange={(value) => {
            if (includeImportSubfolderImages) setIncludeImportSubfolders(value);
          }}
          onIncludeSubfolderImagesChange={(value) => {
            setIncludeImportSubfolderImages(value);
            if (!value) setIncludeImportSubfolders(false);
          }}
          onCancel={() => setPendingImportPath(null)}
          onConfirm={() => void confirmImport()}
        />
      ) : null}
      {settingsOpen ? (
        <SettingsDialog
          settings={appSettings}
          gpuCapabilities={gpuCapabilities}
          themeMode={themeMode}
          onChange={updateAppSettings}
          onThemeChange={setThemeMode}
          onReset={() => {
            updateAppSettings(normalizeAppSettings(DEFAULT_APP_SETTINGS));
            setThemeMode("dark");
          }}
          onClose={closeSettingsDialog}
        />
      ) : null}
    </div>
  );
}

function ImportLibraryDialog({
  path,
  includeSubfolders,
  includeSubfolderImages,
  onIncludeSubfoldersChange,
  onIncludeSubfolderImagesChange,
  onCancel,
  onConfirm,
}: {
  path: string;
  includeSubfolders: boolean;
  includeSubfolderImages: boolean;
  onIncludeSubfoldersChange: (value: boolean) => void;
  onIncludeSubfolderImagesChange: (value: boolean) => void;
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
          <h2 id="import-library-dialog-title">确认导入方式</h2>
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
            aria-label="导入子文件夹中的图片"
            checked={includeSubfolderImages}
            onChange={(event) => onIncludeSubfolderImagesChange(event.target.checked)}
          />
          <span>
            <strong>导入子文件夹中的图片</strong>
          </span>
        </label>
        <label className={`import-option${includeSubfolderImages ? "" : " is-disabled"}`}>
          <input
            type="checkbox"
            aria-label="按子文件夹建立图库结构"
            checked={includeSubfolders}
            disabled={!includeSubfolderImages}
            onChange={(event) => onIncludeSubfoldersChange(event.target.checked)}
          />
          <span>
            <strong>按子文件夹建立图库结构</strong>
          </span>
        </label>
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

type AppNavigationView = "library" | "organization";

function AppNavigationRail({
  activeView,
  settingsActive,
  hasLibrary,
  onOpenLibrary,
  onOpenOrganization,
  onOpenSettings,
}: {
  activeView: AppNavigationView;
  settingsActive: boolean;
  hasLibrary: boolean;
  onOpenLibrary: () => void;
  onOpenOrganization: () => void;
  onOpenSettings: () => void;
}) {
  return (
    <nav className="app-navigation-rail" aria-label="工作区导航">
      <div className="app-navigation-rail-main">
        <button
          className={
            activeView === "library"
              ? "app-navigation-rail-button is-active"
              : "app-navigation-rail-button"
          }
          type="button"
          aria-label="图库"
          aria-pressed={activeView === "library"}
          title="图库"
          onClick={onOpenLibrary}
        >
          <HomeIcon width="20" height="20" />
          <span className="sr-only">图库</span>
        </button>
        <button
          className={
            activeView === "organization"
              ? "app-navigation-rail-button is-active"
              : "app-navigation-rail-button"
          }
          type="button"
          aria-label="整理预览"
          aria-pressed={activeView === "organization"}
          title="整理预览"
          disabled={!hasLibrary}
          onClick={onOpenOrganization}
        >
          <BooksIcon width="20" height="20" />
          <span className="sr-only">整理预览</span>
        </button>
      </div>
      <div className="app-navigation-rail-footer">
        <button
          className={
            settingsActive ? "app-navigation-rail-button is-active" : "app-navigation-rail-button"
          }
          type="button"
          aria-label="打开设置"
          aria-pressed={settingsActive}
          title="设置（Ctrl+,）"
          onClick={onOpenSettings}
        >
          <SettingsIcon width="20" height="20" />
          <span className="sr-only">设置</span>
        </button>
      </div>
    </nav>
  );
}

function WorkflowHeightResizeHandle({
  value,
  min,
  max,
  step,
  onChange,
}: {
  value: number;
  min: number;
  max: number;
  step: number;
  onChange: (value: number) => void;
}) {
  const dragRef = useRef<{ pointerId: number; startY: number; startHeight: number } | null>(null);

  useEffect(() => {
    const handlePointerMove = (event: PointerEvent) => {
      const drag = dragRef.current;
      if (!drag || drag.pointerId !== (event.pointerId || 1)) return;
      event.preventDefault();
      const clientY = Number.isFinite(event.clientY) ? event.clientY : drag.startY;
      const nextHeight = drag.startHeight - (clientY - drag.startY);
      onChange(Math.max(min, Math.min(max, nextHeight)));
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
  }, [max, min, onChange]);

  const changeByKeyboard = (event: React.KeyboardEvent<HTMLDivElement>) => {
    if (event.key !== "ArrowUp" && event.key !== "ArrowDown") return;
    event.preventDefault();
    const delta = event.key === "ArrowUp" ? step : -step;
    onChange(Math.max(min, Math.min(max, value + delta)));
  };

  return (
    <div
      className="workflow-height-resize-handle"
      role="separator"
      aria-label="调整查找与审阅高度"
      aria-orientation="horizontal"
      aria-valuemin={min}
      aria-valuemax={max}
      aria-valuenow={Math.round(value)}
      aria-valuetext={`查找与审阅高度 ${Math.round(value)} 像素`}
      tabIndex={0}
      onKeyDown={changeByKeyboard}
      onPointerDown={(event) => {
        event.preventDefault();
        dragRef.current = {
          pointerId: event.pointerId || 1,
          startY: Number.isFinite(event.clientY) ? event.clientY : 0,
          startHeight: value,
        };
        event.currentTarget.setPointerCapture?.(event.pointerId);
      }}
    />
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
      title={`拖动调整${side === "left" ? "左侧图库与筛选" : "右侧信息"}宽度`}
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
  gridColumns,
  active,
  selectedAssetIds,
  groupBy,
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
}: {
  assets: AssetListItem[];
  gridColumns: number;
  active: AssetListItem | null;
  selectedAssetIds: number[];
  groupBy: AssetGroupBy;
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
}) {
  const [collapsedGroups, setCollapsedGroups] = useState<Set<string>>(new Set());
  const thumbnailProfile = gridThumbnailProfileForColumns(gridColumns);

  if (groupBy === "none")
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
            thumbnailProfile={thumbnailProfile}
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
    const key = groupValueForAsset(asset, groupBy, semanticCatalog);
    const section = sections.get(key) ?? [];
    section.push(asset);
    sections.set(key, section);
  }
  return (
    <div className="semantic-groups">
      {[...sections].map(([id, items], index) => {
        const groupKey = `${groupBy}:${id}`;
        const collapsed = collapsedGroups.has(groupKey);
        const groupPanelId = `semantic-group-${index}`;
        return (
          <section className={collapsed ? "is-collapsed" : ""} key={id}>
            <button
              type="button"
              className="group-heading"
              aria-expanded={!collapsed}
              aria-controls={groupPanelId}
              aria-label={`${collapsed ? "展开" : "折叠"}分组：${id}（${items.length} 张）`}
              onClick={() =>
                setCollapsedGroups((current) => {
                  const next = new Set(current);
                  if (next.has(groupKey)) next.delete(groupKey);
                  else next.add(groupKey);
                  return next;
                })
              }
            >
              <ChevronIcon
                className={
                  collapsed ? "group-heading-chevron is-collapsed" : "group-heading-chevron"
                }
                width="13"
                height="13"
              />
              <strong>{id}</strong>
              <span className="group-heading-count">{items.length} 张</span>
            </button>
            <div id={groupPanelId} className="semantic-group-items" hidden={collapsed}>
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
            </div>
          </section>
        );
      })}
    </div>
  );
}

function groupValueForAsset(
  asset: AssetListItem,
  groupBy: Exclude<AssetGroupBy, "none">,
  semanticCatalog: SemanticLabelDescriptor[],
): string {
  switch (groupBy) {
    case "primary_category": {
      const value =
        asset.classification.primaryCategory.effective ??
        asset.semanticLabels.find((label) => label.isPrimary && label.categoryGroup === "scene")
          ?.labelId;
      return value ? classificationValueLabel(value, "primary", semanticCatalog) : "未分类";
    }
    case "auxiliary_tag": {
      const value =
        asset.classification.auxiliaryTags.effective[0] ??
        asset.semanticLabels.find((label) => label.categoryGroup === "subject")?.labelId;
      return value ? classificationValueLabel(value, "tag", semanticCatalog) : "未设置";
    }
    case "tone": {
      const value = asset.classification.tone.effective ?? asset.toneLabel;
      return value ? classificationValueLabel(value, "tone", semanticCatalog) : "未设置";
    }
    case "saturation_level": {
      const value = asset.classification.saturationLevel.effective ?? asset.saturationLabel;
      return value ? classificationValueLabel(value, "saturation", semanticCatalog) : "未设置";
    }
    case "dominant_color": {
      const value =
        asset.classification.dominantColorCategories.effective?.[0] ?? asset.dominantColorCategory;
      return value ? classificationValueLabel(value, "color", semanticCatalog) : "未设置";
    }
    case "rating":
      return asset.rating > 0 ? `${asset.rating} 星` : "未评分";
  }
}

function SinglePreview({
  assets,
  selected,
  controller,
  selectedAssetIds,
  onSelect,
  onToggleSelection,
}: {
  assets: AssetListItem[];
  selected: AssetListItem | null;
  controller: PreviewController;
  selectedAssetIds: number[];
  onSelect: (asset: AssetListItem) => void;
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
        <ZoomablePreview key={selected.id} asset={selected} controller={controller} />
      ) : (
        <div className="single-empty">没有可预览的图片</div>
      )}
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
            onClick={(event) => {
              if (event.ctrlKey || event.metaKey || event.shiftKey) {
                onToggleSelection(asset, {
                  ctrlKey: event.ctrlKey,
                  metaKey: event.metaKey,
                  shiftKey: event.shiftKey,
                });
              } else {
                onSelect(asset);
              }
            }}
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
}: {
  asset: AssetListItem;
  controller: PreviewController;
}) {
  const {
    stageRef,
    thumbnailSource,
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
        {thumbnailSource && displaySource && displaySource !== thumbnailSource ? (
          <img
            className="preview-image is-thumbnail"
            src={thumbnailSource}
            alt=""
            draggable={false}
            style={{
              width: `${naturalSize.width}px`,
              height: `${naturalSize.height}px`,
              left: "50%",
              top: "50%",
              transform: `translate3d(calc(-50% + ${controller.offset.x}px), calc(-50% + ${controller.offset.y}px), 0) scale(${displayScale})`,
            }}
          />
        ) : null}
        {displaySource ? (
          <img
            className={`preview-image${displaySource === thumbnailSource ? " is-thumbnail" : ""}`}
            src={displaySource}
            alt={asset.fileName}
            draggable={false}
            style={{
              width: `${naturalSize.width}px`,
              height: `${naturalSize.height}px`,
              left: "50%",
              top: "50%",
              transform: `translate3d(calc(-50% + ${controller.offset.x}px), calc(-50% + ${controller.offset.y}px), 0) scale(${displayScale})`,
            }}
            onLoad={
              displaySource === thumbnailSource
                ? undefined
                : (event) =>
                    onImageLoad({
                      width: event.currentTarget.naturalWidth,
                      height: event.currentTarget.naturalHeight,
                    })
            }
          />
        ) : (
          <div className="preview-thumbnail-fallback">
            <Thumbnail asset={asset} />
          </div>
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
    "拍摄题材",
    "primary",
    catalog,
  );
  appendStringFilterConditions(conditions, filter, "auxiliaryTags", "辅助标签", "tag", catalog);
  appendStringFilterConditions(conditions, filter, "colorCategories", "主色", "color", catalog);

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

  if (filter.colorHueCenter !== null && filter.colorHueWidth !== null) {
    const start = normalizeHue(filter.colorHueCenter - filter.colorHueWidth / 2);
    const end = normalizeHue(filter.colorHueCenter + filter.colorHueWidth / 2);
    conditions.push({
      id: "color-hue-range",
      label: "颜色范围",
      value: `${formatHue(start)}° — ${formatHue(end)}°（宽度 ${formatHue(filter.colorHueWidth)}°；${colorHueFilterSummary(filter.colorHueStrictness)}）`,
      remove: (current) => ({ ...current, colorHueCenter: null, colorHueWidth: null }),
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

function normalizeHue(value: number) {
  return ((value % 360) + 360) % 360;
}

function formatHue(value: number) {
  return String(Math.round(normalizeHue(value)));
}

function isCollectionDescendant(
  candidateId: number,
  ancestorId: number,
  collections: CollectionSummary[],
) {
  const byId = new Map(collections.map((collection) => [collection.id, collection]));
  let current = byId.get(candidateId)?.parentCollectionId ?? null;
  while (current !== null) {
    if (current === ancestorId) return true;
    current = byId.get(current)?.parentCollectionId ?? null;
  }
  return false;
}
function countActiveFilters(filter: AssetFilter) {
  return (
    Number(Boolean(filter.search)) +
    filter.primaryCategories.length +
    filter.auxiliaryTags.length +
    filter.colorCategories.length +
    filter.ratings.length +
    filter.colorLabels.length +
    Number(filter.colorHueCenter !== null && filter.colorHueWidth !== null) +
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
