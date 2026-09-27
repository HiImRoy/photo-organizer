import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import App from "./App";
import { emptyEffectiveClassification } from "./types";
import type {
  AssetListItem,
  LibrarySummary,
  ScanProgress,
  SemanticProgress,
  SemanticRuntimeStatus,
  SubjectRuntimeStatus,
} from "./types";

const api = vi.hoisted(() => ({
  chooseLibraryFolder: vi.fn(),
  fetchLibraries: vi.fn(),
  fetchAssets: vi.fn(),
  fetchAssetDetail: vi.fn(),
  fetchFavoriteAssetIds: vi.fn(),
  fetchFavoriteAssets: vi.fn(),
  fetchCollections: vi.fn(),
  fetchBrowseNodes: vi.fn(),
  createCollection: vi.fn(),
  fetchCollection: vi.fn(),
  addAssetsToCollection: vi.fn(),
  addAssetsToCollections: vi.fn(),
  renameCollection: vi.fn(),
  moveCollection: vi.fn(),
  deleteCollection: vi.fn(),
  removeAssetsFromCollection: vi.fn(),
  moveAssetsBetweenCollections: vi.fn(),
  searchLocalImages: vi.fn(),
  fetchDuplicateGroups: vi.fn(),
  fetchSimilarAssets: vi.fn(),
  renderEditPreview: vi.fn(),
  fetchClassificationRegistry: vi.fn(),
  startLibraryScan: vi.fn(),
  rescanLibrary: vi.fn(),
  cancelLibraryScan: vi.fn(),
  fetchThumbnail: vi.fn(),
  fetchPreview: vi.fn(),
  removeLibrary: vi.fn(),
  setLibraryParent: vi.fn(),
  assignAssetToLibrary: vi.fn(),
  openLibraryInExplorer: vi.fn(),
  fetchGpuCapabilities: vi.fn(),
  fetchSemanticStatus: vi.fn(),
  fetchSubjectStatus: vi.fn(),
  prepareSemanticModel: vi.fn(),
  prepareSubjectModel: vi.fn(),
  fetchSemanticCatalog: vi.fn(),
  fetchLibraryFolders: vi.fn(),
  fetchSemanticGroups: vi.fn(),
  fetchSemanticProgress: vi.fn(),
  startSemanticAnalysis: vi.fn(),
  startSemanticAnalysisForAssets: vi.fn(),
  reanalyzeAsset: vi.fn(),
  updateClassificationOverride: vi.fn(),
  updateAssetRating: vi.fn(),
  updateAssetColorLabel: vi.fn(),
  setAssetFavorite: vi.fn(),
  updateTagOverride: vi.fn(),
  restoreAutoClassification: vi.fn(),
  pauseSemanticAnalysis: vi.fn(),
  resumeSemanticAnalysis: vi.fn(),
  cancelSemanticAnalysis: vi.fn(),
  subscribeScanProgress: vi.fn(),
  subscribeSemanticProgress: vi.fn(),
  subscribeSemanticStatus: vi.fn(),
  subscribeSubjectStatus: vi.fn(),
}));

vi.mock("./api", () => api);

const library: LibrarySummary = {
  id: 7,
  rootPath: "C:\\fixtures\\中文 图库",
  name: "中文 图库",
  sourcePath: "C:\\fixtures\\中文 图库",
  sourceIdentityKey: "c:/fixtures/中文 图库",
  parentLibraryId: null,
  displayOrder: 0,
  createdAt: "2026-08-06T10:00:00Z",
  lastScanAt: "2026-08-06T10:10:00Z",
  status: "ready",
  assetCount: 1,
  presentCount: 1,
  missingCount: 0,
  semanticPendingCount: 0,
};

const asset: AssetListItem = {
  id: 12,
  libraryId: 7,
  absolutePath: "C:\\fixtures\\中文 图库\\晚霞.png",
  relativePath: "晚霞.png",
  fileName: "晚霞.png",
  extension: "png",
  fileSize: 2048,
  modifiedAt: Date.parse("2026-08-06T09:00:00Z"),
  width: 1200,
  height: 800,
  orientation: 1,
  captureTime: "2026-08-05T18:30:00",
  cameraMake: "FUJIFILM",
  cameraModel: "X-T5",
  lensModel: "XF16-55mmF2.8",
  exposureTime: "1/250",
  aperture: 5.6,
  iso: 200,
  focalLength: 35,
  fileStatus: "present",
  scanStatus: "indexed",
  analysisStatus: "completed",
  errorMessage: null,
  thumbnailAvailable: true,
  brightness: 0.64,
  contrast: 0.5,
  toneLabel: "balanced",
  saturation: 0.72,
  chroma: 0.68,
  saturationLabel: "high",
  dominantColor: "#D76A52",
  dominantColorCategory: "orange",
  colorPalette: {
    algorithmVersion: "accent-oklab-v3",
    coveragePalette: [
      {
        rank: 1,
        color: "#D76A52",
        category: "orange",
        areaCoverage: 0.62,
        saliencyCoverage: 0.58,
        localContrast: 0.3,
        chroma: 0.14,
        spatialCoherence: 0.8,
      },
    ],
    prominentPalette: [
      {
        rank: 1,
        color: "#D76A52",
        category: "orange",
        areaCoverage: 0.62,
        saliencyCoverage: 0.58,
        localContrast: 0.3,
        chroma: 0.14,
        spatialCoherence: 0.8,
      },
      {
        rank: 2,
        color: "#294B70",
        category: "blue",
        areaCoverage: 0.2,
        saliencyCoverage: 0.24,
        localContrast: 0.42,
        chroma: 0.12,
        spatialCoherence: 0.56,
      },
    ],
  },
  neutralRatio: 0.12,
  dominantColorCoverage: 0.72,
  semanticStatus: "completed",
  semanticError: null,
  semanticAnalyzedAt: "2026-08-06T10:00:00Z",
  rating: 0,
  colorLabel: null,
  semanticLabels: [
    {
      labelId: "sunset",
      displayName: "日落",
      categoryGroup: "context",
      similarity: 0.31,
      threshold: 0.16,
      modelName: "SigLIP2-Base-Patch16-224",
      modelVersion: "test",
      analysisVersion: "test",
      taxonomyVersion: "photo-organizer-taxonomy-v2",
      analyzedAt: "2026-08-06T10:00:00Z",
      isManual: false,
      isPrimary: true,
    },
  ],
  classification: emptyEffectiveClassification(),
};

const secondAsset: AssetListItem = {
  ...asset,
  id: 13,
  absolutePath: "C:\\fixtures\\中文 图库\\海边.png",
  relativePath: "海边.png",
  fileName: "海边.png",
  dominantColor: "#3D78A2",
  dominantColorCategory: "blue",
};

const thirdAsset: AssetListItem = {
  ...asset,
  id: 14,
  absolutePath: "C:\\fixtures\\中文 图库\\山谷.png",
  relativePath: "山谷.png",
  fileName: "山谷.png",
  dominantColor: "#6E8A63",
  dominantColorCategory: "green",
};

function sidebarButtonWithTitle(title: string): HTMLButtonElement | undefined {
  return Array.from(document.querySelectorAll<HTMLButtonElement>("button.nav-row[title]")).find(
    (button) => button.title === title,
  );
}

let progressListener: ((progress: ScanProgress) => void) | undefined;
let semanticProgressListener: ((progress: SemanticProgress) => void) | undefined;
let semanticStatusListener: ((status: SemanticRuntimeStatus) => void) | undefined;
let subjectStatusListener: ((status: SubjectRuntimeStatus) => void) | undefined;

function semanticRuntimeStatus(
  status: string,
  selectedBackend: string | null,
  message = status,
): SemanticRuntimeStatus {
  return {
    status,
    message,
    model: {
      name: "SigLIP2-Base-Patch16-224",
      version: "test",
      analysisVersion: "test",
      license: "Apache-2.0",
      installed: true,
      modelSizeBytes: 1,
      modelSha256: "test",
      supportedBackends: ["cpu", "direct_ml"],
    },
    selectedBackend,
  };
}

beforeEach(() => {
  vi.clearAllMocks();
  window.localStorage.removeItem("photo-organizer-theme");
  window.localStorage.removeItem("photo-organizer-settings");
  progressListener = undefined;
  semanticProgressListener = undefined;
  semanticStatusListener = undefined;
  subjectStatusListener = undefined;
  api.chooseLibraryFolder.mockResolvedValue(null);
  api.fetchLibraries.mockResolvedValue([]);
  api.fetchAssets.mockResolvedValue({ items: [], total: 0, page: 1, pageSize: 200 });
  api.fetchAssetDetail.mockResolvedValue(null);
  api.fetchFavoriteAssetIds.mockResolvedValue([]);
  api.fetchFavoriteAssets.mockResolvedValue([]);
  api.fetchCollections.mockResolvedValue([]);
  api.fetchBrowseNodes.mockResolvedValue([]);
  api.createCollection.mockResolvedValue(null);
  api.fetchCollection.mockResolvedValue(null);
  api.addAssetsToCollection.mockResolvedValue(null);
  api.addAssetsToCollections.mockResolvedValue([]);
  api.renameCollection.mockResolvedValue(null);
  api.moveCollection.mockResolvedValue(null);
  api.deleteCollection.mockResolvedValue(null);
  api.removeAssetsFromCollection.mockResolvedValue(null);
  api.moveAssetsBetweenCollections.mockResolvedValue({
    affectedAssetCount: 0,
    skippedAssetCount: 0,
  });
  api.searchLocalImages.mockResolvedValue({
    query: "",
    normalizedQuery: "",
    embeddedAssetCount: 0,
    items: [],
  });
  api.fetchDuplicateGroups.mockResolvedValue([]);
  api.fetchSimilarAssets.mockResolvedValue([]);
  api.renderEditPreview.mockResolvedValue("data:image/jpeg;base64,ZmFrZQ==");
  api.updateAssetRating.mockResolvedValue(null);
  api.updateAssetColorLabel.mockResolvedValue(null);
  api.setAssetFavorite.mockResolvedValue(true);
  api.fetchClassificationRegistry.mockResolvedValue([]);
  api.startLibraryScan.mockResolvedValue({ taskId: "task-1" });
  api.rescanLibrary.mockResolvedValue({ taskId: "task-2" });
  api.cancelLibraryScan.mockResolvedValue({ taskId: "task-1", accepted: true });
  api.fetchThumbnail.mockResolvedValue("data:image/jpeg;base64,ZmFrZQ==");
  api.fetchPreview.mockResolvedValue("data:image/jpeg;base64,ZmFrZQ==");
  api.removeLibrary.mockResolvedValue(true);
  api.setLibraryParent.mockResolvedValue(true);
  api.assignAssetToLibrary.mockResolvedValue(true);
  api.openLibraryInExplorer.mockResolvedValue(undefined);
  api.fetchGpuCapabilities.mockResolvedValue({
    status: "integrated_only",
    message: "仅检测到集成或共享显存适配器；按当前策略不启用独立 GPU 加速。",
    adapters: [],
    selectedAdapterIndex: null,
    dedicatedGpuAvailable: false,
    recommendedAnalysisBatchSize: 8,
    directml: {
      id: "directml",
      state: "not_configured",
      message: "DirectML Provider 尚未接入，当前分析使用 CPU。",
    },
  });
  api.fetchSemanticStatus.mockResolvedValue({
    status: "model_unavailable",
    message: "not installed",
    model: {
      name: "none",
      version: "0",
      analysisVersion: "semantic-interface-v1",
      license: null,
      installed: false,
      modelSizeBytes: null,
      modelSha256: null,
      supportedBackends: ["cpu"],
    },
    selectedBackend: null,
  });
  api.prepareSemanticModel.mockResolvedValue({
    status: "ready",
    message: "ready",
    model: {
      name: "SigLIP2-Base-Patch16-224",
      version: "test",
      analysisVersion: "test",
      license: "Apache-2.0",
      installed: true,
      modelSizeBytes: 378_000_135,
      modelSha256: "test",
      supportedBackends: ["cpu"],
    },
    selectedBackend: "cpu",
  });
  api.fetchSubjectStatus.mockResolvedValue({
    status: "ready",
    message: "ready",
    model: {
      name: "PicoDet-S-COCO",
      version: "test",
      analysisVersion: "test",
      license: "Apache-2.0",
      installed: true,
      modelSizeBytes: 1,
      modelSha256: "test",
      supportedBackends: ["cpu"],
    },
    faceModel: {
      name: "YuNet-FaceDetector",
      version: "test",
      analysisVersion: "test",
      license: "MIT",
      installed: true,
      modelSizeBytes: 1,
      modelSha256: "test",
      supportedBackends: ["cpu"],
    },
    selectedBackend: "cpu",
  });
  api.subscribeSemanticStatus.mockImplementation(
    async (listener: (status: SemanticRuntimeStatus) => void) => {
      semanticStatusListener = listener;
      return vi.fn();
    },
  );
  api.subscribeSubjectStatus.mockImplementation(
    async (listener: (status: SubjectRuntimeStatus) => void) => {
      subjectStatusListener = listener;
      return vi.fn();
    },
  );
  api.prepareSubjectModel.mockResolvedValue({
    status: "ready",
    message: "ready",
    model: {
      name: "PicoDet-S-COCO",
      version: "test",
      analysisVersion: "test",
      license: "Apache-2.0",
      installed: true,
      modelSizeBytes: 1,
      modelSha256: "test",
      supportedBackends: ["cpu"],
    },
    faceModel: {
      name: "YuNet-FaceDetector",
      version: "test",
      analysisVersion: "test",
      license: "MIT",
      installed: true,
      modelSizeBytes: 1,
      modelSha256: "test",
      supportedBackends: ["cpu"],
    },
    selectedBackend: "cpu",
  });
  api.fetchSemanticCatalog.mockResolvedValue([]);
  api.fetchLibraryFolders.mockResolvedValue([]);
  api.fetchSemanticGroups.mockResolvedValue([]);
  api.fetchSemanticProgress.mockResolvedValue(null);
  api.startSemanticAnalysis.mockResolvedValue({ jobId: "semantic-1" });
  api.reanalyzeAsset.mockResolvedValue({ jobId: "semantic-2" });
  api.pauseSemanticAnalysis.mockResolvedValue({ jobId: "semantic-1", accepted: true });
  api.resumeSemanticAnalysis.mockResolvedValue({ jobId: "semantic-1", accepted: true });
  api.cancelSemanticAnalysis.mockResolvedValue({ jobId: "semantic-1", accepted: true });
  api.subscribeScanProgress.mockImplementation(
    async (listener: (progress: ScanProgress) => void) => {
      progressListener = listener;
      return vi.fn();
    },
  );
  api.subscribeSemanticProgress.mockImplementation(
    async (listener: (progress: SemanticProgress) => void) => {
      semanticProgressListener = listener;
      return vi.fn();
    },
  );
});

afterEach(() => {
  vi.useRealTimers();
});

it("suppresses the browser context menu outside editable controls", () => {
  render(<App />);

  const appRoot = document.querySelector<HTMLElement>(".photo-app");
  expect(appRoot).not.toBeNull();

  const canvasMenuEvent = new MouseEvent("contextmenu", {
    bubbles: true,
    cancelable: true,
  });
  appRoot?.dispatchEvent(canvasMenuEvent);
  expect(canvasMenuEvent.defaultPrevented).toBe(true);

  const searchInput = screen.getByRole("textbox", { name: "搜索图片" });
  const inputMenuEvent = new MouseEvent("contextmenu", {
    bubbles: true,
    cancelable: true,
  });
  searchInput.dispatchEvent(inputMenuEvent);
  expect(inputMenuEvent.defaultPrevented).toBe(false);
});

describe("PhotoOrganizer application shell", () => {
  it("switches to the light theme and persists the day-mode choice", async () => {
    const user = userEvent.setup();
    render(<App />);

    const app = document.querySelector<HTMLElement>(".photo-app");
    const settingsButton = await screen.findByRole("button", { name: "打开设置" });
    expect(app).not.toHaveClass("theme-light");

    await user.click(settingsButton);
    const settings = screen.getByRole("dialog", { name: "设置" });
    await user.click(within(settings).getByRole("radio", { name: "白天" }));

    expect(app).toHaveClass("theme-light");
    expect(window.localStorage.getItem("photo-organizer-theme")).toBe("light");
    expect(within(settings).getByRole("radio", { name: "白天" })).toBeChecked();
    expect(
      screen.queryByRole("button", { name: /切换到白天模式|切换到深色模式/ }),
    ).not.toBeInTheDocument();
  });

  it("keeps import and theme controls out of the top action group", async () => {
    api.fetchLibraries.mockResolvedValue([library]);
    render(<App />);

    const actionGroup = await screen.findByRole("group", { name: "图库操作" });
    expect(within(actionGroup).queryByRole("button", { name: "导入" })).not.toBeInTheDocument();
    expect(
      within(actionGroup).queryByRole("button", { name: /白天|深色/ }),
    ).not.toBeInTheDocument();
    expect(within(actionGroup).queryByRole("button", { name: "打开设置" })).not.toBeInTheDocument();
    expect(screen.getByRole("navigation", { name: "工作区导航" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "打开设置" })).toBeInTheDocument();
    expect(within(actionGroup).getAllByRole("button").at(-1)).toHaveTextContent("分析");
  });

  it("shows the first-run empty state and import action", async () => {
    render(<App />);
    expect(await screen.findByRole("heading", { name: "建立本地图片库" })).toBeInTheDocument();
    expect(screen.getAllByRole("button", { name: "选择照片文件夹" }).length).toBeGreaterThan(0);
  });

  it("reserves the semantic filter count badge before a category is selected", async () => {
    const user = userEvent.setup();
    api.fetchLibraries.mockResolvedValue([library]);
    api.fetchSemanticCatalog.mockResolvedValue([
      {
        id: "portrait",
        displayName: "人像",
        categoryGroup: "scene",
        threshold: 0.2,
        isPrimaryCategory: true,
        taxonomyVersion: "photo-organizer-taxonomy-v2",
      },
    ]);
    render(<App />);

    const categoryButton = await screen.findByRole("button", { name: "人像" });
    const sidebar = screen.getByRole("complementary", { name: "图库与筛选" });
    const categorySection = within(sidebar).getByText("拍摄题材").closest(".panel-section");
    const countBadge = categorySection?.querySelector(".panel-section-heading small");

    expect(countBadge).toHaveClass("is-placeholder");
    await user.click(categoryButton);

    expect(countBadge).not.toHaveClass("is-placeholder");
    expect(countBadge).toHaveTextContent("1");
  });

  it("groups the current results through the browse grouping selector", async () => {
    const user = userEvent.setup();
    const portraitAsset = {
      ...asset,
      id: 1201,
      classification: {
        ...asset.classification,
        primaryCategory: {
          auto: "photo_portrait",
          manual: null,
          effective: "photo_portrait",
          source: "auto" as const,
        },
      },
    };
    const landscapeAsset = {
      ...secondAsset,
      id: 1301,
      classification: {
        ...secondAsset.classification,
        primaryCategory: {
          auto: "photo_landscape",
          manual: null,
          effective: "photo_landscape",
          source: "auto" as const,
        },
      },
    };
    api.fetchLibraries.mockResolvedValue([library]);
    api.fetchAssets.mockResolvedValue({
      items: [portraitAsset, landscapeAsset],
      total: 2,
      page: 1,
      pageSize: 200,
    });
    render(<App />);

    await screen.findByRole("button", { name: "晚霞.png" });
    const groupSelect = screen.getByRole("combobox", { name: "分组" });
    const fetchCount = api.fetchAssets.mock.calls.length;
    await user.selectOptions(groupSelect, "primary_category");

    expect(screen.getByText("人像", { selector: ".group-heading strong" })).toBeInTheDocument();
    expect(screen.getByText("风光", { selector: ".group-heading strong" })).toBeInTheDocument();
    const portraitGroupToggle = screen.getByRole("button", { name: /折叠分组：人像/ });
    const portraitGroup = portraitGroupToggle.closest("section");
    expect(portraitGroup?.querySelector(".semantic-group-items")).not.toHaveAttribute("hidden");
    await user.click(portraitGroupToggle);
    expect(portraitGroupToggle).toHaveAttribute("aria-expanded", "false");
    expect(portraitGroup?.querySelector(".semantic-group-items")).toHaveAttribute("hidden");
    expect(api.fetchAssets).toHaveBeenCalledTimes(fetchCount);
    await user.click(portraitGroupToggle);
    expect(portraitGroupToggle).toHaveAttribute("aria-expanded", "true");
    expect(api.fetchAssets.mock.calls.length).toBe(fetchCount);
  });

  it("opens the folder chooser and starts a scan", async () => {
    const user = userEvent.setup();
    api.chooseLibraryFolder.mockResolvedValue("C:\\fixtures\\emoji 😀");
    render(<App />);
    await screen.findByRole("heading", { name: "建立本地图片库" });

    await user.click(screen.getAllByRole("button", { name: "选择照片文件夹" })[0]);

    expect(api.chooseLibraryFolder).toHaveBeenCalledOnce();
    const importDialog = screen.getByRole("dialog", { name: "确认导入方式" });
    expect(importDialog).toBeInTheDocument();
    expect(importDialog.querySelectorAll("small")).toHaveLength(0);
    expect(within(importDialog).queryByText(/关闭后只导入/)).not.toBeInTheDocument();
    expect(within(importDialog).queryByText(/将递归导入/)).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "关闭" })).toHaveClass("dialog-close");
    expect(screen.getByRole("checkbox", { name: "导入子文件夹中的图片" })).toBeChecked();
    expect(screen.getByRole("checkbox", { name: "按子文件夹建立图库结构" })).not.toBeChecked();
    await user.click(screen.getByRole("button", { name: "开始导入" }));
    expect(api.startLibraryScan).toHaveBeenCalledWith("C:\\fixtures\\emoji 😀", {
      includeSubfolders: false,
      includeSubfolderImages: true,
      importWorkerCount: 2,
    });
    await user.click(await screen.findByRole("button", { name: "查看后台任务" }));
    expect(await screen.findByText("准备图库")).toBeInTheDocument();
  });

  it("does not reset a scan terminal event received before start resolves", async () => {
    const user = userEvent.setup();
    api.chooseLibraryFolder.mockResolvedValue("C:\\fixtures\\scan");
    let resolveStart: ((value: { taskId: string }) => void) | undefined;
    api.startLibraryScan.mockImplementationOnce(
      () =>
        new Promise<{ taskId: string }>((resolve) => {
          resolveStart = resolve;
        }),
    );
    render(<App />);

    await screen.findByRole("heading", { name: "建立本地图片库" });
    await user.click(screen.getAllByRole("button", { name: "选择照片文件夹" })[0]);
    await user.click(screen.getByRole("button", { name: "开始导入" }));
    await waitFor(() => expect(api.startLibraryScan).toHaveBeenCalledOnce());

    act(() => {
      progressListener?.({
        taskId: "task-1",
        libraryId: 7,
        status: "failed",
        stage: "processing",
        discovered: 2,
        processed: 1,
        succeeded: 0,
        failed: 1,
        skipped: 0,
        missing: 0,
        currentPath: "C:\\fixtures\\scan\\one.png",
        error: "fixture failure",
      });
    });
    await user.click(screen.getByRole("button", { name: "查看后台任务" }));
    expect(await screen.findByText("当前图库 · 导入失败")).toBeInTheDocument();

    expect(resolveStart).toBeDefined();
    await act(async () => {
      resolveStart?.({ taskId: "task-1" });
      await Promise.resolve();
    });
    expect(screen.getByText("当前图库 · 导入失败")).toBeInTheDocument();
    expect(screen.queryByText("当前图库 · 导入中")).not.toBeInTheDocument();

    act(() => {
      progressListener?.({
        taskId: "task-2",
        libraryId: 7,
        status: "failed",
        stage: "processing",
        discovered: 3,
        processed: 2,
        succeeded: 1,
        failed: 1,
        skipped: 0,
        missing: 0,
        currentPath: "C:\\fixtures\\scan\\two.png",
        error: "new fixture failure",
      });
    });
    expect(screen.getByText("当前图库 · 导入失败")).toBeInTheDocument();
    act(() => {
      progressListener?.({
        taskId: "task-1",
        libraryId: 7,
        status: "running",
        stage: "processing",
        discovered: 4,
        processed: 3,
        succeeded: 3,
        failed: 0,
        skipped: 0,
        missing: 0,
        currentPath: "C:\\fixtures\\scan\\late.png",
        error: null,
      });
    });
    expect(screen.getByText("当前图库 · 导入失败")).toBeInTheDocument();
    expect(screen.queryByText("当前图库 · 导入中")).not.toBeInTheDocument();
  });

  it("does not reset a semantic terminal event received before start resolves", async () => {
    const user = userEvent.setup();
    api.fetchLibraries.mockResolvedValue([library]);
    api.fetchSemanticStatus.mockResolvedValue({
      status: "ready",
      message: "ready",
      model: { name: "SigLIP2-Base-Patch16-224", version: "test" },
      selectedBackend: "cpu",
    });
    let resolveStart: ((value: { jobId: string }) => void) | undefined;
    api.startSemanticAnalysis.mockImplementationOnce(
      () =>
        new Promise<{ jobId: string }>((resolve) => {
          resolveStart = resolve;
        }),
    );
    render(<App />);

    const analyzeButton = await screen.findByRole("button", { name: "分析" });
    await user.click(analyzeButton);
    await waitFor(() => expect(api.startSemanticAnalysis).toHaveBeenCalledOnce());

    act(() => {
      semanticProgressListener?.({
        jobId: "semantic-1",
        libraryId: 7,
        status: "completed",
        total: 2,
        processed: 2,
        completed: 2,
        failed: 0,
        skipped: 0,
        currentAssetId: null,
        currentPath: null,
        executionBackend: "cpu",
        modelName: "SigLIP2",
        modelVersion: "test",
        error: null,
      });
    });
    await waitFor(() =>
      expect(screen.queryByRole("button", { name: "查看后台任务" })).not.toBeInTheDocument(),
    );

    expect(resolveStart).toBeDefined();
    await act(async () => {
      resolveStart?.({ jobId: "semantic-1" });
      await Promise.resolve();
    });
    expect(screen.queryByRole("button", { name: "查看后台任务" })).not.toBeInTheDocument();

    act(() => {
      semanticProgressListener?.({
        jobId: "semantic-2",
        libraryId: 7,
        status: "running",
        total: 2,
        processed: 0,
        completed: 0,
        failed: 0,
        skipped: 0,
        currentAssetId: null,
        currentPath: null,
        executionBackend: "cpu",
        modelName: "SigLIP2",
        modelVersion: "test",
        error: null,
      });
    });
    expect(await screen.findByRole("button", { name: "查看后台任务" })).toBeInTheDocument();

    act(() => {
      semanticProgressListener?.({
        jobId: "semantic-2",
        libraryId: 7,
        status: "completed",
        total: 2,
        processed: 2,
        completed: 2,
        failed: 0,
        skipped: 0,
        currentAssetId: null,
        currentPath: null,
        executionBackend: "cpu",
        modelName: "SigLIP2",
        modelVersion: "test",
        error: null,
      });
    });
    await waitFor(() =>
      expect(screen.queryByRole("button", { name: "查看后台任务" })).not.toBeInTheDocument(),
    );
    act(() => {
      semanticProgressListener?.({
        jobId: "semantic-1",
        libraryId: 7,
        status: "running",
        total: 2,
        processed: 1,
        completed: 1,
        failed: 0,
        skipped: 0,
        currentAssetId: 12,
        currentPath: "C:\\fixtures\\late.png",
        executionBackend: "cpu",
        modelName: "SigLIP2",
        modelVersion: "test",
        error: null,
      });
    });
    expect(screen.queryByRole("button", { name: "查看后台任务" })).not.toBeInTheDocument();
  });

  it("can exclude child-folder images while importing a folder", async () => {
    const user = userEvent.setup();
    api.chooseLibraryFolder.mockResolvedValue("C:\\fixtures\\root-only");
    render(<App />);

    await screen.findByRole("heading", { name: "建立本地图片库" });
    await user.click(screen.getAllByRole("button", { name: "选择照片文件夹" })[0]);

    const imageToggle = screen.getByRole("checkbox", { name: "导入子文件夹中的图片" });
    const structureToggle = screen.getByRole("checkbox", { name: "按子文件夹建立图库结构" });
    await user.click(imageToggle);

    expect(imageToggle).not.toBeChecked();
    expect(structureToggle).not.toBeChecked();
    expect(structureToggle).toBeDisabled();
    await user.click(screen.getByRole("button", { name: "开始导入" }));

    expect(api.startLibraryScan).toHaveBeenCalledWith("C:\\fixtures\\root-only", {
      includeSubfolders: false,
      includeSubfolderImages: false,
      importWorkerCount: 2,
    });
  });

  it("persists display, processing, and shortcut settings by function", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(screen.getByRole("button", { name: "打开设置" }));
    const dialog = screen.getByRole("dialog", { name: "设置" });
    expect(dialog).toBeInTheDocument();

    await user.click(within(dialog).getByRole("radio", { name: "单图" }));
    await user.selectOptions(
      within(dialog).getByRole("combobox", { name: "默认每行图片数" }),
      "10",
    );
    await user.click(within(dialog).getByRole("tab", { name: /处理/ }));
    expect(within(dialog).getByText("独立 GPU 加速")).toBeInTheDocument();
    expect(within(dialog).getByText("不可用")).toBeInTheDocument();
    const gpuToggle = within(dialog).getByRole("checkbox", { name: "启用 GPU 加速" });
    expect(gpuToggle).toBeDisabled();
    expect(within(dialog).getByRole("option", { name: "16（当前上限）" })).toBeDisabled();
    await user.selectOptions(within(dialog).getByRole("combobox", { name: "导入并行数" }), "1");
    await user.selectOptions(within(dialog).getByRole("combobox", { name: "分析批大小" }), "8");
    await user.click(within(dialog).getByRole("tab", { name: /快捷键/ }));
    const resetButton = within(dialog).getByRole("button", { name: "恢复默认" });
    const doneButton = within(dialog).getByRole("button", { name: "完成" });
    expect(resetButton).toHaveClass("settings-footer-action");
    expect(doneButton).toHaveClass("settings-footer-action");
    expect(resetButton).toHaveClass("settings-footer-action-secondary");
    expect(doneButton).toHaveClass("settings-footer-action-primary");
    expect(within(dialog).getByRole("textbox", { name: "多图预览快捷键" })).toHaveValue("g");
    expect(within(dialog).getByRole("textbox", { name: "单图预览快捷键" })).toHaveValue("f");
    const ratingShortcut = within(dialog).getByRole("textbox", { name: "3 星快捷键" });
    await user.clear(ratingShortcut);
    await user.type(ratingShortcut, "q");
    await user.click(within(dialog).getByRole("button", { name: "完成" }));

    await waitFor(() => {
      const stored = JSON.parse(window.localStorage.getItem("photo-organizer-settings") ?? "{}");
      expect(stored.startupView).toBe("single");
      expect(stored.defaultGridColumns).toBe(10);
      expect(stored.importWorkerCount).toBe(1);
      expect(stored.analysisBatchSize).toBe(8);
      expect(stored.shortcuts.ratings["3"]).toBe("q");
    });
  });

  it("uses the saved startup view on the next mount", async () => {
    window.localStorage.setItem(
      "photo-organizer-settings",
      JSON.stringify({ startupView: "single", defaultGridColumns: 10 }),
    );
    api.fetchLibraries.mockResolvedValue([library]);
    api.fetchAssets.mockResolvedValue({
      items: [{ ...asset, id: 9701 }],
      total: 1,
      page: 1,
      pageSize: 200,
    });

    render(<App />);

    const singleButton = await screen.findByRole("button", { name: "单图预览" });
    expect(singleButton).toHaveClass("is-active");
    expect(document.querySelector(".single-workspace")).not.toBeNull();
  });

  it("uses the saved grid density on the next mount", async () => {
    window.localStorage.setItem(
      "photo-organizer-settings",
      JSON.stringify({ startupView: "grid", defaultGridColumns: 10 }),
    );
    api.fetchLibraries.mockResolvedValue([library]);
    api.fetchAssets.mockResolvedValue({
      items: [{ ...asset, id: 9702 }],
      total: 1,
      page: 1,
      pageSize: 200,
    });

    render(<App />);

    await waitFor(() => expect(document.querySelector(".grid-workspace-results")).not.toBeNull());
    expect(document.querySelector(".grid-workspace-results")).toHaveStyle(
      "--grid-column-count: 10",
    );
  });

  it("switches between grid and single preview with the view shortcuts", async () => {
    const user = userEvent.setup();
    const viewAsset = { ...asset, id: 712 };
    const viewSecondAsset = { ...secondAsset, id: 713 };
    api.fetchLibraries.mockResolvedValue([library]);
    api.fetchAssets.mockResolvedValue({
      items: [viewAsset, viewSecondAsset],
      total: 2,
      page: 1,
      pageSize: 200,
    });
    render(<App />);

    const assetCard = await screen.findByRole("button", { name: viewAsset.fileName });
    const gridButton = screen.getByRole("button", { name: "网格视图" });
    const singleButton = screen.getByRole("button", { name: "单图预览" });
    expect(gridButton).toHaveClass("is-active");
    expect(singleButton).not.toHaveClass("is-active");

    await user.click(assetCard);
    fireEvent.keyDown(assetCard, { key: "3" });
    await waitFor(() => expect(api.updateAssetRating).toHaveBeenCalledWith(viewAsset.id, 3));

    fireEvent.keyDown(assetCard, { key: "f" });
    await waitFor(() => expect(document.querySelector(".single-workspace")).not.toBeNull());
    expect(singleButton).toHaveClass("is-active");
    expect(gridButton).not.toHaveClass("is-active");

    fireEvent.keyDown(window, { key: "g" });
    await waitFor(() => expect(screen.getByLabelText("图片网格")).toBeInTheDocument());
    expect(gridButton).toHaveClass("is-active");
    expect(singleButton).not.toHaveClass("is-active");
  });

  it("opens single preview when a focused gallery card receives Enter", async () => {
    const user = userEvent.setup();
    const enterAsset = { ...asset, id: 912 };
    const enterSecondAsset = { ...secondAsset, id: 913 };
    api.fetchLibraries.mockResolvedValue([library]);
    api.fetchAssets.mockResolvedValue({
      items: [enterAsset, enterSecondAsset],
      total: 2,
      page: 1,
      pageSize: 200,
    });
    render(<App />);

    const assetCard = await screen.findByRole("button", { name: enterAsset.fileName });
    await user.click(assetCard);
    const enterEvent = new KeyboardEvent("keydown", {
      bubbles: true,
      cancelable: true,
      key: "Enter",
    });
    assetCard.dispatchEvent(enterEvent);

    expect(enterEvent.defaultPrevented).toBe(true);
    await waitFor(() => expect(document.querySelector(".single-workspace")).not.toBeNull());
  });

  it("keeps text editing and modified/composing keydowns out of global shortcuts", async () => {
    const user = userEvent.setup();
    const textAsset = { ...asset, id: 812 };
    const textSecondAsset = { ...secondAsset, id: 813 };
    api.fetchLibraries.mockResolvedValue([library]);
    api.fetchAssets.mockResolvedValue({
      items: [textAsset, textSecondAsset],
      total: 2,
      page: 1,
      pageSize: 200,
    });
    render(<App />);

    const assetCard = await screen.findByRole("button", { name: textAsset.fileName });
    await user.click(assetCard);
    const searchInput = screen.getByRole("textbox", { name: "搜索图片" });

    fireEvent.keyDown(searchInput, { key: "Enter" });
    expect(document.querySelector(".single-workspace")).toBeNull();

    api.updateAssetRating.mockClear();
    for (const modifier of ["ctrlKey", "metaKey", "altKey"] as const) {
      fireEvent.keyDown(assetCard, { key: "3", [modifier]: true });
    }
    const preventedEvent = new KeyboardEvent("keydown", {
      bubbles: true,
      cancelable: true,
      key: "3",
    });
    preventedEvent.preventDefault();
    window.dispatchEvent(preventedEvent);
    window.dispatchEvent(
      new KeyboardEvent("keydown", {
        bubbles: true,
        cancelable: true,
        isComposing: true,
        key: "3",
      }),
    );
    expect(api.updateAssetRating).not.toHaveBeenCalled();

    fireEvent.keyDown(window, { key: "f" });
    await waitFor(() =>
      expect(screen.getByRole("heading", { name: textAsset.fileName })).toBeInTheDocument(),
    );
    searchInput.focus();
    fireEvent.keyDown(searchInput, { key: "ArrowRight" });
    expect(screen.getByRole("heading", { name: textAsset.fileName })).toBeInTheDocument();
  });

  it("keeps the settings layer above gallery shortcuts and restores its trigger focus", async () => {
    const user = userEvent.setup();
    const settingsAsset = { ...asset, id: 822 };
    api.fetchLibraries.mockResolvedValue([library]);
    api.fetchAssets.mockResolvedValue({
      items: [settingsAsset],
      total: 1,
      page: 1,
      pageSize: 200,
    });
    render(<App />);

    const trigger = await screen.findByRole("button", { name: "打开设置" });
    await user.click(trigger);
    const dialog = screen.getByRole("dialog", { name: "设置" });
    await waitFor(() => expect(dialog.contains(document.activeElement)).toBe(true));

    api.updateAssetRating.mockClear();
    document.body.focus();
    fireEvent.keyDown(window, { key: "3" });
    expect(api.updateAssetRating).not.toHaveBeenCalled();

    fireEvent.keyDown(window, { key: "Escape" });
    await waitFor(() =>
      expect(screen.queryByRole("dialog", { name: "设置" })).not.toBeInTheDocument(),
    );
    expect(trigger).toHaveFocus();
  });

  it("consumes filter Escape without clearing selection or leaving the gallery", async () => {
    const user = userEvent.setup();
    const filterAsset = { ...asset, id: 832 };
    const filterSecondAsset = { ...secondAsset, id: 833 };
    api.fetchLibraries.mockResolvedValue([library]);
    api.fetchAssets.mockResolvedValue({
      items: [filterAsset, filterSecondAsset],
      total: 2,
      page: 1,
      pageSize: 200,
    });
    render(<App />);

    const assetCard = await screen.findByRole("button", { name: filterAsset.fileName });
    await user.click(assetCard);
    await user.click(screen.getByRole("button", { name: "选择 " + filterAsset.fileName }));
    await user.click(screen.getByRole("button", { name: "筛选" }));
    const filterDialog = screen.getByRole("dialog", { name: "当前条件" });
    const escape = new KeyboardEvent("keydown", {
      bubbles: true,
      cancelable: true,
      key: "Escape",
    });
    act(() => filterDialog.dispatchEvent(escape));

    expect(escape.defaultPrevented).toBe(true);
    expect(screen.queryByRole("dialog", { name: "当前条件" })).not.toBeInTheDocument();
    expect(screen.getByText("已选择 1 张")).toBeInTheDocument();
    expect(screen.getByLabelText("图片网格")).toBeInTheDocument();
  });

  it("restores a library, renders the grid, and opens details", async () => {
    const user = userEvent.setup();
    api.fetchSemanticStatus.mockResolvedValue({
      status: "ready",
      message: "ready",
      model: {
        name: "SigLIP2-Base-Patch16-224",
        version: "test",
        analysisVersion: "test",
        license: "Apache-2.0",
        installed: true,
        modelSizeBytes: 378_000_135,
        modelSha256: "test",
        supportedBackends: ["cpu"],
      },
      selectedBackend: "cpu",
    });
    api.fetchLibraries.mockResolvedValue([library]);
    api.fetchAssets.mockResolvedValue({ items: [asset], total: 1, page: 1, pageSize: 200 });
    render(<App />);

    expect(screen.queryByLabelText("题材模型")).not.toBeInTheDocument();
    const assetButton = await screen.findByRole("button", { name: "晚霞.png" });
    const galleryActions = screen.getByRole("group", { name: "图库操作" });
    const galleryActionButtons = within(galleryActions).getAllByRole("button");
    expect(galleryActionButtons.at(-1)).toHaveTextContent("分析");
    expect(galleryActionButtons.at(-1)).toHaveClass("topbar-analysis-action");
    expect(await screen.findByText("1200 × 800")).toBeInTheDocument();
    expect(assetButton).not.toHaveAttribute("aria-current");
    await user.click(assetButton);

    expect(screen.getByRole("complementary", { name: "图片详情" })).toBeInTheDocument();
    expect(screen.getByText("直方图")).toBeInTheDocument();
    expect(screen.getByText("强调色")).toBeInTheDocument();
    expect(screen.getByText("面积色")).toBeInTheDocument();
    const histogramChannels = screen.getByRole("group", { name: "直方图通道" });
    expect(histogramChannels).toBeInTheDocument();
    expect(within(histogramChannels).getByRole("button", { name: "显示全部通道" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    expect(within(histogramChannels).getByRole("button", { name: "显示L通道" })).toHaveAttribute(
      "aria-pressed",
      "false",
    );
    await user.click(within(histogramChannels).getByRole("button", { name: "显示R通道" }));
    expect(within(histogramChannels).getByRole("button", { name: "显示L通道" })).toHaveAttribute(
      "aria-pressed",
      "false",
    );
    expect(within(histogramChannels).getByRole("button", { name: "显示R通道" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    await user.click(within(histogramChannels).getByRole("button", { name: "显示全部通道" }));
    expect(within(histogramChannels).getByRole("button", { name: "显示全部通道" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    expect(screen.getByRole("button", { name: "重新分析此图片" })).toHaveClass(
      "primary-action",
      "detail-reanalyze-action",
    );
    expect(await screen.findByRole("button", { name: "分析" })).toBeInTheDocument();
    await waitFor(() => expect(assetButton).toHaveAttribute("aria-current", "true"));
    expect(screen.getByText("1200 × 800")).toBeInTheDocument();
    expect(screen.getByText("#D76A52")).toBeInTheDocument();
    await waitFor(() => expect(api.fetchThumbnail).toHaveBeenCalledWith(12));
  });

  it("subscribes to model status before fetching snapshots and keeps newer events", async () => {
    let resolveSemanticSnapshot: ((status: SemanticRuntimeStatus) => void) | undefined;
    api.fetchSemanticStatus.mockImplementation(() => {
      expect(semanticStatusListener).toBeDefined();
      expect(subjectStatusListener).toBeDefined();
      return new Promise<SemanticRuntimeStatus>((resolve) => {
        resolveSemanticSnapshot = resolve;
      });
    });
    render(<App />);

    await waitFor(() => expect(api.fetchSemanticStatus).toHaveBeenCalledOnce());
    expect(semanticStatusListener).toBeDefined();
    expect(subjectStatusListener).toBeDefined();
    act(() => {
      semanticStatusListener?.(semanticRuntimeStatus("loading", "cpu", "正在准备题材模型"));
    });
    expect(screen.getByText(/题材模型准备中/)).toBeInTheDocument();

    await act(async () => {
      resolveSemanticSnapshot?.(semanticRuntimeStatus("ready", "cpu"));
      await Promise.resolve();
    });
    expect(screen.getByText(/题材模型准备中/)).toBeInTheDocument();
  });

  it("shows model preparation errors and retries preparation from the status action", async () => {
    const user = userEvent.setup();
    api.fetchLibraries.mockResolvedValue([library]);
    api.fetchSemanticStatus.mockResolvedValue(
      semanticRuntimeStatus("error", null, "weights unavailable"),
    );
    render(<App />);

    const actionGroup = await screen.findByRole("group", { name: "图库操作" });
    await waitFor(() =>
      expect(within(actionGroup).getByRole("status")).toHaveTextContent(
        "题材模型准备失败：weights unavailable",
      ),
    );
    await user.click(await screen.findByRole("button", { name: "重试模型准备" }));
    await waitFor(() =>
      expect(api.prepareSemanticModel).toHaveBeenCalledWith("siglip2-base", "cpu"),
    );
    await waitFor(() =>
      expect(screen.queryByRole("button", { name: "重试模型准备" })).not.toBeInTheDocument(),
    );
    expect(screen.getByRole("button", { name: "分析" })).toBeInTheDocument();
  });

  it("offers model retry when runtime recovery reports runtime_unavailable", async () => {
    api.fetchSemanticStatus.mockResolvedValue(
      semanticRuntimeStatus("runtime_unavailable", "cpu", "CPU fallback failed"),
    );
    render(<App />);

    const actions = await screen.findByRole("group", { name: "图库操作" });
    await waitFor(() =>
      expect(within(actions).getByRole("status")).toHaveTextContent(
        "题材模型运行时不可用：CPU fallback failed",
      ),
    );
    expect(within(actions).getByRole("button", { name: "重试模型准备" })).toBeEnabled();
  });

  it("distinguishes status lookup failures from model preparation failures", async () => {
    const user = userEvent.setup();
    api.fetchSemanticStatus.mockRejectedValue(new Error("status endpoint offline"));
    render(<App />);

    const actions = await screen.findByRole("group", { name: "图库操作" });
    await waitFor(() =>
      expect(within(actions).getByRole("status")).toHaveTextContent(
        "题材模型状态读取失败：status endpoint offline",
      ),
    );
    await user.click(within(actions).getByRole("button", { name: "重试模型准备" }));
    await waitFor(() =>
      expect(api.prepareSemanticModel).toHaveBeenCalledWith("siglip2-base", "cpu"),
    );
    await waitFor(() => expect(within(actions).queryByRole("status")).not.toBeInTheDocument());
  });

  it("ignores repeated analysis clicks while model preparation is pending", async () => {
    api.fetchLibraries.mockResolvedValue([library]);
    let resolvePreparation: ((status: SemanticRuntimeStatus) => void) | undefined;
    api.prepareSemanticModel.mockImplementationOnce(
      () =>
        new Promise<SemanticRuntimeStatus>((resolve) => {
          resolvePreparation = resolve;
        }),
    );
    render(<App />);
    await waitFor(() => expect(api.fetchSemanticStatus).toHaveBeenCalledOnce());
    const analyzeButton = await screen.findByRole("button", { name: "分析" });

    act(() => {
      fireEvent.click(analyzeButton);
      fireEvent.click(analyzeButton);
    });
    await waitFor(() => expect(api.prepareSemanticModel).toHaveBeenCalledOnce());
    expect(analyzeButton).toBeDisabled();

    await act(async () => {
      resolvePreparation?.(semanticRuntimeStatus("ready", "cpu"));
      await Promise.resolve();
    });
    await waitFor(() => expect(api.startSemanticAnalysis).toHaveBeenCalledOnce());
  });

  it("re-prepares for a GPU setting changed while CPU preparation is in flight", async () => {
    const user = userEvent.setup();
    api.fetchLibraries.mockResolvedValue([library]);
    api.fetchGpuCapabilities.mockResolvedValue({
      status: "ready",
      message: "独立 GPU 可用",
      adapters: [],
      selectedAdapterIndex: 0,
      dedicatedGpuAvailable: true,
      recommendedAnalysisBatchSize: 8,
      directml: { id: "directml", state: "ready", message: "ready" },
    });
    let resolveCpuPreparation: ((status: SemanticRuntimeStatus) => void) | undefined;
    api.prepareSemanticModel
      .mockImplementationOnce(
        () =>
          new Promise<SemanticRuntimeStatus>((resolve) => {
            resolveCpuPreparation = resolve;
          }),
      )
      .mockResolvedValueOnce(semanticRuntimeStatus("ready", "direct_ml"));
    render(<App />);
    await waitFor(() => expect(api.fetchSemanticStatus).toHaveBeenCalledOnce());
    await user.click(await screen.findByRole("button", { name: "分析" }));
    await waitFor(() =>
      expect(api.prepareSemanticModel).toHaveBeenNthCalledWith(1, "siglip2-base", "cpu"),
    );

    await user.click(screen.getByRole("button", { name: "打开设置" }));
    const settings = screen.getByRole("dialog", { name: "设置" });
    await user.click(within(settings).getByRole("tab", { name: /处理/ }));
    await user.click(within(settings).getByRole("checkbox", { name: "启用 GPU 加速" }));
    await act(async () => {
      resolveCpuPreparation?.(semanticRuntimeStatus("ready", "cpu"));
      await Promise.resolve();
    });

    await waitFor(() =>
      expect(api.prepareSemanticModel).toHaveBeenNthCalledWith(2, "siglip2-base", "direct_ml"),
    );
    await waitFor(() => expect(api.startSemanticAnalysis).toHaveBeenCalledOnce());
    expect(api.startSemanticAnalysis).toHaveBeenCalledWith(
      library.id,
      false,
      expect.objectContaining({ batchSize: 4 }),
    );
  });

  it("prepares the selected backend after changing GPU settings without starting analysis", async () => {
    const user = userEvent.setup();
    api.fetchLibraries.mockResolvedValue([library]);
    api.fetchSemanticStatus.mockResolvedValue(semanticRuntimeStatus("ready", "cpu"));
    api.fetchGpuCapabilities.mockResolvedValue({
      status: "ready",
      message: "独立 GPU 可用",
      adapters: [],
      selectedAdapterIndex: 0,
      dedicatedGpuAvailable: true,
      recommendedAnalysisBatchSize: 8,
      directml: { id: "directml", state: "ready", message: "ready" },
    });
    render(<App />);
    await waitFor(() => expect(api.fetchSemanticStatus).toHaveBeenCalledOnce());
    await waitFor(() => expect(api.fetchGpuCapabilities).toHaveBeenCalledOnce());
    expect(api.prepareSemanticModel).not.toHaveBeenCalled();

    await user.click(screen.getByRole("button", { name: "打开设置" }));
    const settings = screen.getByRole("dialog", { name: "设置" });
    await user.click(within(settings).getByRole("tab", { name: /处理/ }));
    await user.click(within(settings).getByRole("checkbox", { name: "启用 GPU 加速" }));

    await waitFor(() =>
      expect(api.prepareSemanticModel).toHaveBeenCalledWith("siglip2-base", "direct_ml"),
    );
    expect(api.startSemanticAnalysis).not.toHaveBeenCalled();
    expect(within(settings).getByRole("checkbox", { name: "启用 GPU 加速" })).toBeChecked();
  });

  it("remembers a runtime CPU fallback and does not retry DirectML on the next analysis", async () => {
    const user = userEvent.setup();
    window.localStorage.setItem(
      "photo-organizer-settings",
      JSON.stringify({ gpuAccelerationEnabled: true }),
    );
    api.fetchLibraries.mockResolvedValue([library]);
    api.fetchGpuCapabilities.mockResolvedValue({
      status: "ready",
      message: "独立 GPU 可用",
      adapters: [],
      selectedAdapterIndex: 0,
      dedicatedGpuAvailable: true,
      recommendedAnalysisBatchSize: 8,
      directml: { id: "directml", state: "ready", message: "ready" },
    });
    api.prepareSemanticModel.mockImplementation((_modelId: string, backend: string) =>
      Promise.resolve(semanticRuntimeStatus("ready", backend)),
    );
    api.startSemanticAnalysis
      .mockResolvedValueOnce({ jobId: "semantic-gpu" })
      .mockResolvedValueOnce({ jobId: "semantic-cpu" });
    render(<App />);
    await waitFor(() => expect(api.fetchSemanticStatus).toHaveBeenCalledOnce());
    const analyzeButton = await screen.findByRole("button", { name: "分析" });
    await waitFor(() => expect(analyzeButton).toBeEnabled());
    await user.click(analyzeButton);
    await waitFor(() => expect(api.startSemanticAnalysis).toHaveBeenCalledOnce());
    await waitFor(() => expect(semanticProgressListener).toBeDefined());

    act(() => {
      semanticProgressListener?.({
        jobId: "semantic-gpu",
        libraryId: library.id,
        status: "running",
        total: 2,
        processed: 0,
        completed: 0,
        failed: 0,
        skipped: 0,
        currentAssetId: null,
        currentPath: null,
        executionBackend: "cpu",
        modelName: "SigLIP2",
        modelVersion: "test",
        error: null,
      });
    });
    act(() => {
      semanticProgressListener?.({
        jobId: "semantic-gpu",
        libraryId: library.id,
        status: "completed",
        total: 2,
        processed: 2,
        completed: 2,
        failed: 0,
        skipped: 0,
        currentAssetId: null,
        currentPath: null,
        executionBackend: "cpu",
        modelName: "SigLIP2",
        modelVersion: "test",
        error: null,
      });
    });

    await waitFor(() =>
      expect(api.prepareSemanticModel).toHaveBeenCalledWith("siglip2-base", "cpu"),
    );
    await waitFor(() => expect(screen.getByRole("button", { name: "分析" })).toBeEnabled());
    await user.click(screen.getByRole("button", { name: "分析" }));
    await waitFor(() => expect(api.startSemanticAnalysis).toHaveBeenCalledTimes(2));
    expect(api.prepareSemanticModel.mock.calls.map(([, backend]) => backend)).toEqual([
      "direct_ml",
      "cpu",
    ]);
  });

  it("uses shared responsive control geometry for model retry and analysis", async () => {
    api.fetchLibraries.mockResolvedValue([library]);
    api.fetchSemanticStatus.mockResolvedValue(
      semanticRuntimeStatus("error", null, "weights unavailable"),
    );
    render(<App />);

    const actions = await screen.findByRole("group", { name: "图库操作" });
    const retryButton = await within(actions).findByRole("button", { name: "重试模型准备" });
    const analyzeButton = within(actions).getByRole("button", { name: "分析" });
    const status = within(actions).getByRole("status");
    expect(retryButton).toHaveClass("topbar-model-control", "topbar-model-retry-action");
    expect(analyzeButton).toHaveClass("topbar-model-control", "topbar-analysis-action");
    expect(retryButton.parentElement).toBe(analyzeButton.parentElement);
    expect(status).toHaveClass("topbar-model-status");
    expect(status).toHaveAttribute("title", expect.stringContaining("weights unavailable"));
  });

  it("loads SigLIP 2 when analysis is requested before the model is ready", async () => {
    const user = userEvent.setup();
    api.fetchLibraries.mockResolvedValue([library]);
    api.fetchAssets.mockResolvedValue({ items: [asset], total: 1, page: 1, pageSize: 200 });
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "分析" }));

    await waitFor(() =>
      expect(api.prepareSemanticModel).toHaveBeenCalledWith("siglip2-base", "cpu"),
    );
  });

  it("refreshes the grid and sidebar category counts after a manual classification change", async () => {
    const user = userEvent.setup();
    const classifiedAsset = {
      ...asset,
      classification: {
        revision: 1,
        primaryCategory: {
          auto: "portrait",
          manual: null,
          effective: "portrait",
          source: "auto" as const,
        },
        auxiliaryTags: {
          auto: [],
          manualAdditions: [],
          manualRemovals: [],
          effective: [],
          source: "none" as const,
        },
        tone: {
          auto: "balanced",
          manual: null,
          effective: "balanced",
          source: "auto" as const,
        },
        dominantColorCategories: {
          auto: ["orange"],
          manual: null,
          effective: ["orange"],
          source: "auto" as const,
        },
        saturationLevel: {
          auto: "high",
          manual: null,
          effective: "high",
          source: "auto" as const,
        },
      },
    };
    const updatedAsset = {
      ...classifiedAsset,
      classification: {
        ...classifiedAsset.classification,
        revision: 2,
        primaryCategory: {
          auto: "portrait",
          manual: "photo_landscape",
          effective: "photo_landscape",
          source: "manual" as const,
        },
      },
    };
    api.fetchLibraries.mockResolvedValue([library]);
    api.fetchAssets.mockResolvedValue({
      items: [classifiedAsset],
      total: 1,
      page: 1,
      pageSize: 200,
    });
    api.fetchAssetDetail.mockResolvedValue(classifiedAsset);
    api.fetchSemanticCatalog.mockResolvedValue([
      {
        id: "portrait",
        displayName: "人像",
        categoryGroup: "scene",
        threshold: 0.2,
        isPrimaryCategory: true,
        taxonomyVersion: "photo-organizer-taxonomy-v2",
      },
      {
        id: "landscape",
        displayName: "风景",
        categoryGroup: "scene",
        threshold: 0.2,
        isPrimaryCategory: true,
        taxonomyVersion: "photo-organizer-taxonomy-v2",
      },
    ]);
    api.fetchClassificationRegistry.mockResolvedValue([
      {
        id: "primary_category",
        displayName: "拍摄题材",
        kind: "single",
        filterable: true,
        supportsManualOverride: true,
        supportsRestoreAuto: true,
      },
    ]);
    api.fetchSemanticGroups.mockResolvedValue([
      { labelId: "portrait", displayName: "人像", categoryGroup: "scene", assetCount: 1 },
    ]);
    api.updateClassificationOverride.mockImplementation(async () => {
      api.fetchAssetDetail.mockResolvedValue(updatedAsset);
      return updatedAsset;
    });

    render(<App />);
    const assetButton = await screen.findByRole("button", { name: "晚霞.png" });
    await user.click(assetButton);
    const details = await screen.findByRole("complementary", { name: "图片详情" });
    await user.click(within(details).getByRole("button", { name: "手动修改" }));
    await waitFor(() => expect(api.fetchSemanticGroups).toHaveBeenCalled());
    const groupRequestCount = api.fetchSemanticGroups.mock.calls.length;

    const primarySelect = within(details).getAllByRole("combobox")[0];
    await user.selectOptions(primarySelect, "photo_landscape");
    await user.click(within(details).getByRole("button", { name: "保存" }));
    await waitFor(() =>
      expect(api.updateClassificationOverride).toHaveBeenCalledWith(
        12,
        "primary_category",
        "photo_landscape",
      ),
    );

    await waitFor(() =>
      expect(api.fetchSemanticGroups.mock.calls.length).toBeGreaterThan(groupRequestCount),
    );
    await waitFor(() => expect(api.fetchAssets.mock.calls.length).toBeGreaterThan(1));
  });

  it("opens the query review context without exposing the unavailable Faces tab", async () => {
    const user = userEvent.setup();
    api.fetchLibraries.mockResolvedValue([library]);
    api.fetchAssets.mockResolvedValue({ items: [asset], total: 1, page: 1, pageSize: 200 });
    render(<App />);

    await screen.findByRole("button", { name: "晚霞.png" });
    await user.click(screen.getByRole("button", { name: "AI 搜索" }));

    const workflow = screen.getByRole("region", { name: "查找与审阅" });
    expect(workflow).toBeInTheDocument();
    expect(within(workflow).getByText("AI 搜索")).toBeInTheDocument();
    expect(within(workflow).getByText("本地语义检索")).toBeInTheDocument();
    expect(screen.queryByRole("navigation", { name: "工作流工具" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Faces" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "查找与审阅" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "AI 搜索" })).toHaveClass("is-active");
    expect(within(workflow).getByRole("textbox", { name: "本地 AI 搜索" })).toHaveFocus();
    expect(screen.getByRole("button", { name: "关闭 AI 搜索" })).toBeInTheDocument();
    expect(screen.getByRole("region", { name: "查找与审阅" })).toHaveClass(
      "workflow-workspace-floating-search",
    );
    expect(screen.queryByRole("separator", { name: "调整查找与审阅高度" })).not.toBeInTheDocument();

    await user.click(within(workflow).getByRole("button", { name: "关闭 AI 搜索" }));
    expect(screen.queryByRole("textbox", { name: "本地 AI 搜索" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "AI 搜索" })).not.toHaveClass("is-active");
  });

  it("keeps the main grid and detail context visible while opening a review tool", async () => {
    const user = userEvent.setup();
    api.fetchLibraries.mockResolvedValue([library]);
    api.fetchAssets.mockResolvedValue({
      items: [asset, secondAsset],
      total: 2,
      page: 1,
      pageSize: 200,
    });
    render(<App />);

    const firstSelection = await screen.findByRole("button", { name: "选择 晚霞.png" });
    const secondSelection = screen.getByRole("button", { name: "选择 海边.png" });
    await user.click(firstSelection);
    fireEvent.click(secondSelection, { ctrlKey: true });
    await user.click(screen.getByRole("button", { name: "比较" }));

    expect(screen.getByRole("region", { name: "查找与审阅" })).toBeInTheDocument();
    expect(screen.getByRole("region", { name: "图片网格" })).toBeInTheDocument();
    expect(screen.getByRole("complementary", { name: "图片详情" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "双图 / 四图比较" })).toBeInTheDocument();
    expect(screen.queryByRole("navigation", { name: "工作流工具" })).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "返回图库" }));
    expect(screen.queryByRole("heading", { name: "双图 / 四图比较" })).not.toBeInTheDocument();
    expect(screen.getByRole("region", { name: "图片网格" })).toBeInTheDocument();
  });

  it("preserves an explicit selection or query scope through organization preview and back", async () => {
    const user = userEvent.setup();
    api.fetchLibraries.mockResolvedValue([library]);
    api.fetchAssets.mockResolvedValue({ items: [asset], total: 1, page: 1, pageSize: 200 });
    render(<App />);

    await screen.findByRole("button", { name: "晚霞.png" });
    await user.click(screen.getByRole("button", { name: "选择 晚霞.png" }));
    await user.click(screen.getByRole("button", { name: "整理预览" }));

    expect(screen.getByRole("region", { name: "整理预览工作区" })).toBeInTheDocument();
    expect(screen.getByText("显式选择 · 1 张")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "返回图库" }));
    expect(screen.getByRole("region", { name: "图片网格" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "取消选择 晚霞.png" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );

    await user.click(screen.getByRole("button", { name: "清除选择" }));
    await user.click(screen.getByRole("button", { name: "整理预览" }));
    expect(screen.getByText("当前查询 · 1 张")).toBeInTheDocument();
  });

  it("keeps the explicit selection when adding images to collections", async () => {
    const user = userEvent.setup();
    const collection = {
      id: 3,
      name: "旅行",
      description: "",
      createdAt: "2026-08-06T10:00:00Z",
      updatedAt: "2026-08-06T10:00:00Z",
      assetCount: 0,
      parentCollectionId: null,
      collectionKind: "manual" as const,
      systemKey: null,
      displayOrder: 1,
    };
    api.fetchLibraries.mockResolvedValue([library]);
    api.fetchAssets.mockResolvedValue({ items: [asset], total: 1, page: 1, pageSize: 200 });
    api.fetchCollections.mockResolvedValue([collection]);
    api.addAssetsToCollections.mockResolvedValue([collection]);
    render(<App />);

    await screen.findByRole("button", { name: "晚霞.png" });
    await user.click(screen.getByRole("button", { name: "选择 晚霞.png" }));
    const trigger = screen.getByRole("button", { name: "加入收藏" });
    await user.click(trigger);

    const dialog = screen.getByRole("dialog", { name: "加入收藏" });
    const initialChoice = within(dialog).getByRole("checkbox", { name: /旅行/ });
    await waitFor(() => expect(initialChoice).toHaveFocus());
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog", { name: "加入收藏" })).not.toBeInTheDocument();
    await waitFor(() => expect(trigger).toHaveFocus());

    await user.click(trigger);
    const reopenedDialog = screen.getByRole("dialog", { name: "加入收藏" });
    await user.click(within(reopenedDialog).getByRole("checkbox", { name: /旅行/ }));
    await user.click(within(reopenedDialog).getByRole("button", { name: "确定" }));

    expect(api.addAssetsToCollections).toHaveBeenCalledWith([3], [asset.id]);
    expect(screen.getByRole("button", { name: "清除选择" })).toBeInTheDocument();
  });
  it("keeps the current selection while focusing a search result in the detail panel", async () => {
    const user = userEvent.setup();
    api.fetchLibraries.mockResolvedValue([library]);
    api.fetchAssets.mockResolvedValue({
      items: [asset, secondAsset],
      total: 2,
      page: 1,
      pageSize: 200,
    });
    api.searchLocalImages.mockResolvedValue({
      query: "海边",
      normalizedQuery: "海边",
      embeddedAssetCount: 2,
      items: [{ ...secondAsset, similarity: 0.94 }],
    });
    render(<App />);

    await screen.findByRole("button", { name: "晚霞.png" });
    await user.click(screen.getByRole("button", { name: "选择 晚霞.png" }));
    await user.click(screen.getByRole("button", { name: "AI 搜索" }));

    const workflow = screen.getByRole("region", { name: "查找与审阅" });
    const searchInput = within(workflow).getByRole("textbox", { name: "本地 AI 搜索" });
    await user.type(searchInput, "海边");
    await user.click(within(workflow).getByRole("button", { name: "本地搜索" }));
    await within(workflow).findByText("模型查询：海边 · 已分析 2 张");
    await user.click(within(workflow).getByRole("button", { name: /^海边\.png 94%$/ }));

    expect(screen.getByRole("button", { name: "取消选择 晚霞.png" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    expect(within(workflow).getByText(/显式选择范围/)).toBeInTheDocument();
  });

  it("lets search results share selection and manual marks with the main gallery", async () => {
    const user = userEvent.setup();
    api.fetchLibraries.mockResolvedValue([library]);
    api.fetchAssets.mockResolvedValue({
      items: [asset, secondAsset],
      total: 2,
      page: 1,
      pageSize: 200,
    });
    api.searchLocalImages.mockResolvedValue({
      query: "海边",
      normalizedQuery: "海边",
      embeddedAssetCount: 2,
      items: [{ ...secondAsset, similarity: 0.94 }],
    });
    render(<App />);

    await screen.findByRole("button", { name: "晚霞.png" });
    await user.click(screen.getByRole("button", { name: "AI 搜索" }));
    const workflow = screen.getByRole("region", { name: "查找与审阅" });
    const searchInput = within(workflow).getByRole("textbox", { name: "本地 AI 搜索" });
    await user.type(searchInput, "海边");
    await user.click(within(workflow).getByRole("button", { name: "本地搜索" }));
    await within(workflow).findByText("模型查询：海边 · 已分析 2 张");

    await user.click(within(workflow).getByRole("button", { name: "选择 海边.png" }));
    await user.click(within(workflow).getByRole("button", { name: "3 星" }));
    await user.click(within(workflow).getByRole("button", { name: "蓝色" }));

    expect(within(workflow).getByRole("button", { name: "取消选择 海边.png" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    expect(api.updateAssetRating).toHaveBeenCalledWith(secondAsset.id, 3);
    expect(api.updateAssetColorLabel).toHaveBeenCalledWith(secondAsset.id, "blue");
  });

  it("returns a double-clicked search result to its focused card in the gallery", async () => {
    const user = userEvent.setup();
    api.fetchLibraries.mockResolvedValue([library]);
    api.fetchAssets.mockResolvedValue({
      items: [asset, secondAsset],
      total: 2,
      page: 1,
      pageSize: 200,
    });
    api.searchLocalImages.mockResolvedValue({
      query: "海边",
      normalizedQuery: "海边",
      embeddedAssetCount: 2,
      items: [{ ...secondAsset, similarity: 0.94 }],
    });
    render(<App />);

    await screen.findByRole("button", { name: "晚霞.png" });
    await user.click(screen.getByRole("button", { name: "AI 搜索" }));
    const workflow = screen.getByRole("region", { name: "查找与审阅" });
    const searchInput = within(workflow).getByRole("textbox", { name: "本地 AI 搜索" });
    await user.type(searchInput, "海边");
    await user.click(within(workflow).getByRole("button", { name: "本地搜索" }));
    await within(workflow).findByText("模型查询：海边 · 已分析 2 张");

    await user.dblClick(within(workflow).getByRole("button", { name: /^海边\.png 94%$/ }));

    await waitFor(() => {
      expect(screen.queryByRole("textbox", { name: "本地 AI 搜索" })).not.toBeInTheDocument();
      expect(screen.getByRole("button", { name: "海边.png，当前图片" })).toBeInTheDocument();
    });
  });

  it("renames a manual collection from the unified library tree", async () => {
    const user = userEvent.setup();
    const collection = {
      id: 3,
      name: "旅行",
      description: "",
      createdAt: "2026-08-06T10:00:00Z",
      updatedAt: "2026-08-06T10:00:00Z",
      assetCount: 1,
      parentCollectionId: null,
      collectionKind: "manual" as const,
      systemKey: null,
      displayOrder: 1,
    };
    api.fetchLibraries.mockResolvedValue([library]);
    api.fetchCollections.mockResolvedValue([collection]);
    api.fetchBrowseNodes.mockResolvedValue([
      { kind: "collection", collection, children: [] },
      { kind: "source", library, children: [] },
    ]);
    api.renameCollection.mockResolvedValue({ ...collection, name: "旅途" });
    render(<App />);

    await screen.findByTitle("旅行");
    await user.click(screen.getByRole("button", { name: "旅行收藏夹菜单" }));
    await user.click(screen.getByRole("button", { name: "重命名" }));

    const dialog = screen.getByRole("dialog", { name: "重命名收藏夹" });
    const input = within(dialog).getByRole("textbox", { name: "收藏夹名称" });
    await user.clear(input);
    await user.type(input, "旅途");
    await user.click(within(dialog).getByRole("button", { name: "确定" }));

    await waitFor(() => expect(api.renameCollection).toHaveBeenCalledWith(3, "旅途"));
  });
  it("makes favorites and collections browse sources without replacing the main grid", async () => {
    const user = userEvent.setup();
    api.fetchLibraries.mockResolvedValue([library]);
    api.fetchAssets.mockResolvedValue({ items: [asset], total: 1, page: 1, pageSize: 200 });
    api.fetchBrowseNodes.mockResolvedValue([
      {
        kind: "collection",
        collection: {
          id: 100,
          name: "默认收藏",
          description: "",
          createdAt: "2026-08-06T10:00:00Z",
          updatedAt: "2026-08-06T10:00:00Z",
          assetCount: 1,
          parentCollectionId: null,
          collectionKind: "system_favorites",
          systemKey: "default_favorites",
          displayOrder: -1,
        },
        children: [],
      },
    ]);
    render(<App />);

    await screen.findByRole("button", { name: "晚霞.png" });
    await user.click(screen.getByTitle("默认收藏"));

    await waitFor(() =>
      expect(api.fetchAssets).toHaveBeenLastCalledWith(
        expect.objectContaining({
          libraryId: null,
          filter: expect.objectContaining({ favoriteOnly: true, collectionId: null }),
        }),
      ),
    );
    const sourceButton = screen.getByTitle(library.sourcePath);
    const favoriteButton = document.querySelector<HTMLButtonElement>(
      "button.nav-row[title='默认收藏']",
    );
    expect(favoriteButton).not.toBeNull();
    expect(favoriteButton).toHaveClass("is-active");
    expect(sourceButton).not.toHaveClass("is-active");
    expect(document.querySelector(".app-identity strong")).toHaveTextContent("默认收藏");
    expect(screen.getByRole("region", { name: "图片网格" })).toBeInTheDocument();
    expect(screen.getByRole("complementary", { name: "图片详情" })).toBeInTheDocument();

    await user.click(sourceButton);
    await waitFor(() =>
      expect(api.fetchAssets).toHaveBeenLastCalledWith(
        expect.objectContaining({
          libraryId: library.id,
          filter: expect.objectContaining({ favoriteOnly: false, collectionId: null }),
        }),
      ),
    );
    expect(favoriteButton).not.toHaveClass("is-active");
    expect(sourceButton).toHaveClass("is-active");
    expect(document.querySelector(".app-identity strong")).toHaveTextContent(library.name);
  });

  it("applies the photographic tone and capture-date ranges from the sidebar", async () => {
    api.fetchLibraries.mockResolvedValue([library]);
    api.fetchAssets.mockResolvedValue({ items: [asset], total: 1, page: 1, pageSize: 200 });
    render(<App />);

    await screen.findByRole("button", { name: "晚霞.png" });
    expect(screen.getByText("影调与颜色")).toBeInTheDocument();
    expect(screen.queryByText("来源")).not.toBeInTheDocument();
    const sidebarFilterArea = document.querySelector(".sidebar-filter-area");
    expect(sidebarFilterArea?.firstElementChild).toHaveClass("sidebar-tone-color-section");
    expect(sidebarFilterArea?.querySelector(".sidebar-source-section")).toBeNull();
    expect(screen.getAllByText("0% — 100%")).toHaveLength(2);

    act(() => {
      fireEvent.change(screen.getByRole("slider", { name: "亮度最低百分比" }), {
        target: { value: "25" },
      });
      fireEvent.change(screen.getByLabelText("拍摄日期开始"), {
        target: { value: "2026-01-01" },
      });
    });

    await waitFor(() =>
      expect(api.fetchAssets).toHaveBeenLastCalledWith(
        expect.objectContaining({
          filter: expect.objectContaining({
            brightnessMin: 0.25,
            brightnessMax: null,
            capturedFrom: "2026-01-01",
            capturedTo: null,
          }),
        }),
      ),
    );
  });

  it("keeps the main context and selection while reviewing duplicate groups", async () => {
    const user = userEvent.setup();
    api.fetchLibraries.mockResolvedValue([library]);
    api.fetchAssets.mockResolvedValue({
      items: [asset, secondAsset],
      total: 2,
      page: 1,
      pageSize: 200,
    });
    api.fetchDuplicateGroups.mockResolvedValue([
      {
        fingerprint: "duplicate-fingerprint",
        assets: [asset, secondAsset],
        totalBytes: asset.fileSize + secondAsset.fileSize,
        reclaimableBytes: secondAsset.fileSize,
      },
    ]);
    render(<App />);

    await screen.findByRole("button", { name: "选择 晚霞.png" });
    await user.click(screen.getByRole("button", { name: "选择 晚霞.png" }));
    fireEvent.click(screen.getByRole("button", { name: "选择 海边.png" }), { ctrlKey: true });
    await user.click(screen.getByRole("button", { name: "重复审阅" }));

    expect(await screen.findByRole("heading", { name: "精确重复审阅" })).toBeInTheDocument();
    expect(screen.getByRole("region", { name: "图片网格" })).toBeInTheDocument();
    expect(screen.getByRole("complementary", { name: "图片详情" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "返回图库" }));
    expect(screen.getByRole("button", { name: "取消选择 晚霞.png" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
  });

  it("keeps the explicit selection while focusing a similar-image result", async () => {
    const user = userEvent.setup();
    api.fetchLibraries.mockResolvedValue([library]);
    api.fetchAssets.mockResolvedValue({
      items: [asset, secondAsset],
      total: 2,
      page: 1,
      pageSize: 200,
    });
    api.fetchSimilarAssets.mockResolvedValue([{ ...secondAsset, similarity: 0.91 }]);
    render(<App />);

    await screen.findByRole("button", { name: "选择 晚霞.png" });
    await user.click(screen.getByRole("button", { name: "选择 晚霞.png" }));
    await user.click(screen.getByRole("button", { name: "找相似" }));
    const workflow = screen.getByRole("region", { name: "查找与审阅" });
    await user.click(within(workflow).getByRole("button", { name: "查找当前图片的相似项" }));
    await within(workflow).findByRole("button", { name: /海边\.png 91%/ });
    await user.click(within(workflow).getByRole("button", { name: /海边\.png 91%/ }));

    expect(screen.getByRole("button", { name: "取消选择 晚霞.png" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    expect(within(workflow).getByText(/显式选择范围/)).toBeInTheDocument();
  });

  it("keeps a multi-selection when moving from similar review to compare", async () => {
    const user = userEvent.setup();
    api.fetchLibraries.mockResolvedValue([library]);
    api.fetchAssets.mockResolvedValue({
      items: [asset, secondAsset],
      total: 2,
      page: 1,
      pageSize: 200,
    });
    api.fetchSimilarAssets.mockResolvedValue([{ ...secondAsset, similarity: 0.91 }]);
    render(<App />);

    await screen.findByRole("button", { name: "选择 晚霞.png" });
    await user.click(screen.getByRole("button", { name: "选择 晚霞.png" }));
    fireEvent.click(screen.getByRole("button", { name: "选择 海边.png" }), { ctrlKey: true });
    await user.click(screen.getByRole("button", { name: "找相似" }));

    const workflow = screen.getByRole("region", { name: "查找与审阅" });
    await user.click(within(workflow).getByRole("button", { name: "查找当前图片的相似项" }));
    await within(workflow).findByRole("button", { name: /海边\.png 91%/ });
    await user.click(screen.getByRole("button", { name: "比较" }));

    expect(await screen.findByRole("heading", { name: "双图 / 四图比较" })).toBeInTheDocument();
    expect(api.fetchPreview).toHaveBeenCalledWith(12, "screen", 1600, 1200);
    expect(api.fetchPreview).toHaveBeenCalledWith(13, "screen", 1600, 1200);
    expect(screen.getByRole("button", { name: "取消选择 晚霞.png" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    expect(screen.getByRole("button", { name: "取消选择 海边.png" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
  });

  it("marks the focused review result and restores the multi-selection on back", async () => {
    const user = userEvent.setup();
    api.fetchLibraries.mockResolvedValue([library]);
    api.fetchAssets.mockResolvedValue({
      items: [asset, secondAsset],
      total: 2,
      page: 1,
      pageSize: 200,
    });
    api.fetchSimilarAssets.mockResolvedValue([{ ...secondAsset, similarity: 0.91 }]);
    render(<App />);

    await screen.findByRole("button", { name: "选择 晚霞.png" });
    await user.click(screen.getByRole("button", { name: "选择 晚霞.png" }));
    fireEvent.click(screen.getByRole("button", { name: "选择 海边.png" }), { ctrlKey: true });
    await user.click(screen.getByRole("button", { name: "找相似" }));

    const workflow = screen.getByRole("region", { name: "查找与审阅" });
    await user.click(within(workflow).getByRole("button", { name: "查找当前图片的相似项" }));
    await within(workflow).findByRole("button", { name: /海边\.png 91%/ });
    await user.click(screen.getByRole("button", { name: "比较" }));

    const details = screen.getByRole("complementary", { name: "图片详情" });
    await user.click(within(details).getByRole("button", { name: "4 星" }));
    expect(api.updateAssetRating).toHaveBeenCalledWith(13, 4);

    await user.click(screen.getByRole("button", { name: "返回图库" }));
    expect(screen.getByRole("region", { name: "图片网格" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "取消选择 晚霞.png" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    expect(screen.getByRole("button", { name: "取消选择 海边.png" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
  });

  it("returns from non-destructive edit without losing the selected asset", async () => {
    const user = userEvent.setup();
    api.fetchLibraries.mockResolvedValue([library]);
    api.fetchAssets.mockResolvedValue({ items: [asset], total: 1, page: 1, pageSize: 200 });
    render(<App />);

    await screen.findByRole("button", { name: "选择 晚霞.png" });
    await user.click(screen.getByRole("button", { name: "选择 晚霞.png" }));
    await user.click(screen.getByRole("button", { name: "编辑副本" }));

    expect(await screen.findByRole("heading", { name: "非破坏性配方" })).toBeInTheDocument();
    expect(screen.getByRole("region", { name: "图片网格" })).toBeInTheDocument();
    expect(screen.getByRole("complementary", { name: "图片详情" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "返回图库" }));
    expect(screen.getByRole("button", { name: "取消选择 晚霞.png" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
  });

  it("restores manual marking on the card without pinning the overlay to selection", async () => {
    const user = userEvent.setup();
    api.fetchLibraries.mockResolvedValue([library]);
    api.fetchAssets.mockResolvedValue({
      items: [asset, secondAsset],
      total: 2,
      page: 1,
      pageSize: 200,
    });
    render(<App />);

    const firstCard = await screen.findByRole("button", { name: "晚霞.png" });
    const firstShell = firstCard.closest<HTMLElement>(".asset-card-shell");
    const secondCard = screen.getByRole("button", { name: "海边.png" });
    const secondShell = secondCard.closest<HTMLElement>(".asset-card-shell");
    expect(firstShell).not.toBeNull();
    expect(secondShell).not.toBeNull();

    const cardMarks = within(firstShell as HTMLElement);
    expect(cardMarks.getByRole("group", { name: "星级" })).toBeInTheDocument();
    expect(cardMarks.getByRole("group", { name: "色标" })).toBeInTheDocument();
    expect(cardMarks.getByRole("button", { name: "3 星" })).toBeInTheDocument();
    expect(cardMarks.getByRole("button", { name: "红色" })).toBeInTheDocument();

    await user.click(firstCard);
    const details = await screen.findByRole("complementary", { name: "图片详情" });
    expect(within(details).getByRole("group", { name: "星级" })).toBeInTheDocument();
    expect(within(details).getByRole("group", { name: "色标" })).toBeInTheDocument();

    await user.click(cardMarks.getByRole("button", { name: "3 星" }));
    expect(api.updateAssetRating).toHaveBeenCalledWith(asset.id, 3);
    await user.click(cardMarks.getByRole("button", { name: "红色" }));
    expect(api.updateAssetColorLabel).toHaveBeenCalledWith(asset.id, "red");
  });

  it("updates the detail panel when another grid image is focused or selected", async () => {
    const user = userEvent.setup();
    api.fetchLibraries.mockResolvedValue([library]);
    api.fetchAssets.mockResolvedValue({
      items: [asset, secondAsset],
      total: 2,
      page: 1,
      pageSize: 200,
    });
    render(<App />);

    const firstCard = await screen.findByRole("button", { name: "晚霞.png" });
    const secondCard = screen.getByRole("button", { name: "海边.png" });
    await user.click(firstCard);
    const details = screen.getByRole("complementary", { name: "图片详情" });
    expect(within(details).getByRole("heading", { name: "晚霞.png" })).toBeInTheDocument();

    await user.click(secondCard);
    await waitFor(() =>
      expect(within(details).getByRole("heading", { name: "海边.png" })).toBeInTheDocument(),
    );

    await user.click(screen.getByRole("button", { name: "选择 海边.png" }));
    expect(within(details).getByRole("heading", { name: "海边.png" })).toBeInTheDocument();
  });

  it("synchronizes single-preview marks and toggles color shortcuts", async () => {
    const user = userEvent.setup();
    api.fetchLibraries.mockResolvedValue([library]);
    api.fetchAssets.mockResolvedValue({ items: [asset], total: 1, page: 1, pageSize: 200 });
    render(<App />);

    const firstCard = await screen.findByRole("button", { name: "晚霞.png" });
    await user.dblClick(firstCard);
    const details = screen.getByRole("complementary", { name: "图片详情" });

    fireEvent.keyDown(window, { key: "3" });
    await waitFor(() => expect(api.updateAssetRating).toHaveBeenCalledWith(12, 3));
    expect(within(details).getByRole("button", { name: "3 星" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    expect(within(details).getByText("3 星")).toBeInTheDocument();

    fireEvent.keyDown(window, { key: "6" });
    await waitFor(() => expect(api.updateAssetColorLabel).toHaveBeenCalledWith(12, "red"));
    expect(within(details).getByRole("button", { name: "红色" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );

    fireEvent.keyDown(window, { key: "6" });
    await waitFor(() => expect(api.updateAssetColorLabel).toHaveBeenLastCalledWith(12, null));
    expect(within(details).getByRole("button", { name: "红色" })).toHaveAttribute(
      "aria-pressed",
      "false",
    );
    expect(within(details).getByText("未设置")).toBeInTheDocument();
  });

  it("keeps the star filter as one Lightroom-style rating choice", async () => {
    const user = userEvent.setup();
    api.fetchLibraries.mockResolvedValue([library]);
    api.fetchAssets.mockResolvedValue({ items: [asset], total: 1, page: 1, pageSize: 200 });
    render(<App />);

    const manualMarkBar = await screen.findByRole("toolbar", { name: "人工标记筛选" });
    expect(manualMarkBar.closest(".content-toolbar-manual")).not.toBeNull();
    expect(manualMarkBar.closest(".content-toolbar")).not.toBeNull();
    expect(manualMarkBar.closest(".sidebar-filter-area")).toBeNull();
    expect(within(manualMarkBar).queryByText("人工标记筛选")).not.toBeInTheDocument();
    const manualColorGroup = within(manualMarkBar).getByRole("group", { name: "按色标筛选" });
    expect(within(manualColorGroup).getByRole("button", { name: "红色" })).toHaveStyle(
      "background-color: #d66b6b",
    );
    const gridResults = document.querySelector(".grid-workspace-results");
    expect(gridResults).not.toBeNull();
    expect(gridResults).toHaveStyle("--grid-column-count: 6");
    fireEvent.wheel(gridResults as HTMLElement, { ctrlKey: true, deltaY: -100 });
    expect(gridResults).toHaveStyle("--grid-column-count: 4");
    fireEvent.wheel(gridResults as HTMLElement, { ctrlKey: true, deltaY: 100 });
    expect(gridResults).toHaveStyle("--grid-column-count: 6");
    const thirdStar = within(manualMarkBar).getByRole("button", { name: "3 星及以上" });
    const secondStar = within(manualMarkBar).getByRole("button", { name: "2 星及以上" });
    const fourthStar = within(manualMarkBar).getByRole("button", { name: "4 星及以上" });

    await user.click(thirdStar);
    expect(thirdStar).toHaveAttribute("aria-pressed", "true");
    expect(secondStar).toHaveAttribute("aria-pressed", "false");
    expect(fourthStar).toHaveAttribute("aria-pressed", "false");
    expect(thirdStar).toHaveClass("is-active");
    expect(secondStar).toHaveClass("is-active");

    await user.hover(fourthStar);
    expect(thirdStar).toHaveClass("is-active");
    expect(fourthStar).toHaveClass("is-active");
    await user.unhover(fourthStar);

    expect(api.fetchAssets).toHaveBeenLastCalledWith(
      expect.objectContaining({ filter: expect.objectContaining({ ratings: [3] }) }),
    );

    await user.click(screen.getByRole("button", { name: "筛选 1" }));
    const filterDialog = screen.getByRole("dialog", { name: "当前条件" });
    expect(filterDialog).toHaveTextContent("星级");
    expect(filterDialog).toHaveTextContent("3 星及以上");

    await user.click(
      within(filterDialog).getByRole("button", {
        name: "移除筛选条件：星级 3 星及以上",
      }),
    );
    expect(thirdStar).toHaveAttribute("aria-pressed", "false");
  });

  it("keeps the manual mark filter bar available when a filter has no results", async () => {
    api.fetchLibraries.mockResolvedValue([library]);
    api.fetchAssets.mockResolvedValue({ items: [], total: 0, page: 1, pageSize: 200 });
    render(<App />);

    const manualMarkBar = await screen.findByRole("toolbar", { name: "人工标记筛选" });
    expect(manualMarkBar.closest(".content-toolbar-manual")).not.toBeNull();
    expect(screen.getByText("没有符合条件的图片")).toBeInTheDocument();
    expect(within(manualMarkBar).getByRole("button", { name: "3 星及以上" })).toBeInTheDocument();
  });

  it("places analysis status filters above the image results", async () => {
    const user = userEvent.setup();
    api.fetchLibraries.mockResolvedValue([{ ...library, semanticPendingCount: 1 }]);
    api.fetchAssets.mockResolvedValue({ items: [asset], total: 1, page: 1, pageSize: 200 });
    render(<App />);

    const statusBar = await screen.findByRole("region", { name: "分析状态筛选" });
    expect(statusBar.closest(".center-workspace")).not.toBeNull();
    expect(screen.queryByText("更多筛选")).not.toBeInTheDocument();

    const failedFilter = within(statusBar).getByRole("button", { name: "分析失败" });
    await user.click(failedFilter);
    expect(failedFilter).toHaveAttribute("aria-pressed", "true");
    expect(api.fetchAssets).toHaveBeenLastCalledWith(
      expect.objectContaining({
        filter: expect.objectContaining({ analysisStatus: "failed" }),
      }),
    );
  });

  it("renders an explicit source-derived library tree without folder navigation", async () => {
    const childLibrary: LibrarySummary = {
      ...library,
      id: 8,
      name: "子图库",
      rootPath: "C:\\fixtures\\中文 图库\\子图库",
      sourcePath: "C:\\fixtures\\中文 图库\\子图库",
      sourceIdentityKey: "c:/fixtures/中文 图库/子图库",
      parentLibraryId: library.id,
      presentCount: 1,
      assetCount: 1,
    };
    api.fetchLibraries.mockResolvedValue([library, childLibrary]);
    render(<App />);

    expect(screen.queryByText("原始文件夹")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "折叠左侧面板" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "折叠右侧面板" })).not.toBeInTheDocument();
    expect(document.querySelector(".panel-toggle")).toBeNull();
    const importButton = screen.getByRole("button", { name: "添加图库或收藏夹" });
    expect(importButton).toBeInTheDocument();
    expect(importButton.closest(".panel-titlebar")).not.toBeNull();
    expect(document.querySelector(".library-area-heading")).toBeNull();
    expect(await screen.findByText("子图库")).toBeInTheDocument();
  });

  it("collapses and expands child libraries from the tree arrow", async () => {
    const user = userEvent.setup();
    const childLibrary: LibrarySummary = {
      ...library,
      id: 8,
      name: "子图库",
      rootPath: "C:\\fixtures\\中文 图库\\子图库",
      sourcePath: "C:\\fixtures\\中文 图库\\子图库",
      sourceIdentityKey: "c:/fixtures/中文 图库/子图库",
      parentLibraryId: library.id,
      presentCount: 1,
      assetCount: 1,
    };
    api.fetchLibraries.mockResolvedValue([library, childLibrary]);
    render(<App />);

    expect(await screen.findByText("子图库")).toBeInTheDocument();
    const collapseButton = screen.getByRole("button", { name: `折叠 ${library.name}` });
    expect(collapseButton).toHaveAttribute("aria-expanded", "true");
    await user.click(collapseButton);

    expect(screen.queryByText("子图库")).not.toBeInTheDocument();
    const expandButton = screen.getByRole("button", { name: `展开 ${library.name}` });
    expect(expandButton).toHaveAttribute("aria-expanded", "false");
    await user.click(expandButton);

    expect(await screen.findByText("子图库")).toBeInTheDocument();
  });

  it("updates scan progress and sends cancellation", async () => {
    const user = userEvent.setup();
    api.chooseLibraryFolder.mockResolvedValue("C:\\fixtures\\scan");
    render(<App />);
    await screen.findByRole("heading", { name: "建立本地图片库" });
    await user.click(screen.getAllByRole("button", { name: "选择照片文件夹" })[0]);

    act(() => {
      progressListener?.({
        taskId: "task-1",
        libraryId: 7,
        status: "running",
        stage: "processing",
        discovered: 20,
        processed: 4,
        succeeded: 3,
        failed: 1,
        skipped: 2,
        missing: 0,
        currentPath: "C:\\fixtures\\scan\\four.png",
        error: null,
      });
    });

    const taskTrigger = await screen.findByRole("button", { name: "查看后台任务" });
    expect(taskTrigger).toHaveAttribute("aria-expanded", "false");
    await user.click(taskTrigger);
    expect(taskTrigger).toHaveAttribute("aria-expanded", "true");
    expect(await screen.findByText("发现 20")).toBeInTheDocument();
    expect(screen.getByText("失败 1")).toBeInTheDocument();
    expect(screen.queryByText("Timing (cumulative)")).not.toBeInTheDocument();
    const scanPanel = document.querySelector(".scan-panel");
    expect(scanPanel?.closest(".task-status-popover")).not.toBeNull();
    expect(scanPanel?.closest(".center-column")).toBeNull();
    expect(screen.getByRole("progressbar", { name: "扫描进度" })).toHaveAttribute(
      "aria-valuenow",
      "20",
    );
    await user.click(screen.getByRole("button", { name: "取消扫描" }));
    expect(api.cancelLibraryScan).toHaveBeenCalledWith("task-1");
  });

  it("retains completed scan diagnostics until dismissed and keeps later scans visible", async () => {
    const user = userEvent.setup();
    api.chooseLibraryFolder.mockResolvedValue("C:\\fixtures\\scan");
    render(<App />);
    await screen.findByRole("heading", { name: "建立本地图片库" });
    await waitFor(() => expect(progressListener).toBeDefined());

    act(() => {
      progressListener?.({
        taskId: "task-1",
        libraryId: 7,
        status: "running",
        stage: "processing",
        discovered: 3,
        processed: 2,
        succeeded: 2,
        failed: 0,
        skipped: 0,
        missing: 0,
        currentPath: null,
        error: null,
      });
    });

    const trigger = await screen.findByRole("button", { name: "查看后台任务" });
    await user.click(trigger);
    vi.useFakeTimers();
    try {
      act(() => {
        progressListener?.({
          taskId: "task-1",
          libraryId: 7,
          status: "completed",
          stage: "completed",
          discovered: 3,
          processed: 3,
          succeeded: 3,
          failed: 0,
          skipped: 0,
          missing: 0,
          currentPath: null,
          error: null,
          performance: {
            discoveryUs: 1_000,
            ownershipLookupUs: 1_000,
            metadataLookupUs: 2_000,
            fingerprintUs: 3_000,
            imageProcessingUs: 4_000,
            exifUs: 0,
            sourceDimensionUs: 0,
            decodeUs: 2_000,
            sourceDecodeUs: 0,
            thumbnailDecodeUs: 2_000,
            resizeUs: 0,
            featureAnalysisUs: 1_000,
            thumbnailWriteUs: 0,
            databaseWriteUs: 1_000,
            processedFiles: 3,
            skippedFiles: 0,
            failedFiles: 0,
          },
        });
      });
      act(() => vi.advanceTimersByTime(1_000));

      expect(screen.getByRole("button", { name: "查看后台任务" })).toBeInTheDocument();
      expect(screen.getByRole("group", { name: "扫描性能诊断" })).toBeInTheDocument();
      const dismissButton = screen.getByRole("button", { name: "关闭扫描状态" });
      expect(dismissButton).toHaveAttribute("title", "关闭扫描状态");
      fireEvent.click(dismissButton);
      expect(screen.queryByRole("button", { name: "查看后台任务" })).not.toBeInTheDocument();

      act(() => {
        progressListener?.({
          taskId: "task-2",
          libraryId: 7,
          status: "running",
          stage: "processing",
          discovered: 4,
          processed: 1,
          succeeded: 1,
          failed: 0,
          skipped: 0,
          missing: 0,
          currentPath: null,
          error: null,
        });
      });
      const nextTrigger = screen.getByRole("button", { name: "查看后台任务" });
      expect(nextTrigger).toHaveTextContent("导入中");
      fireEvent.click(nextTrigger);
      expect(screen.getByText("发现 4")).toBeInTheDocument();

      act(() => {
        progressListener?.({
          taskId: "task-2",
          libraryId: 7,
          status: "completed",
          stage: "completed",
          discovered: 4,
          processed: 4,
          succeeded: 4,
          failed: 0,
          skipped: 0,
          missing: 0,
          currentPath: null,
          error: null,
        });
      });
      act(() => {
        progressListener?.({
          taskId: "task-3",
          libraryId: 7,
          status: "running",
          stage: "processing",
          discovered: 5,
          processed: 2,
          succeeded: 2,
          failed: 0,
          skipped: 0,
          missing: 0,
          currentPath: null,
          error: null,
        });
      });

      expect(screen.getByRole("button", { name: "查看后台任务" })).toHaveTextContent("导入中");
      expect(screen.queryByText("发现 4")).not.toBeInTheDocument();
      fireEvent.click(screen.getByRole("button", { name: "查看后台任务" }));
      expect(screen.getByText("发现 5")).toBeInTheDocument();
    } finally {
      vi.useRealTimers();
    }
  });

  it("resets scan cancellation when the backend declines the request", async () => {
    const user = userEvent.setup();
    api.chooseLibraryFolder.mockResolvedValue("C:\\fixtures\\scan");
    api.cancelLibraryScan.mockResolvedValueOnce({ taskId: "task-1", accepted: false });
    render(<App />);
    await screen.findByRole("heading", { name: "建立本地图片库" });

    act(() => {
      progressListener?.({
        taskId: "task-1",
        libraryId: 7,
        status: "running",
        stage: "processing",
        discovered: 2,
        processed: 1,
        succeeded: 1,
        failed: 0,
        skipped: 0,
        missing: 0,
        currentPath: "C:\\fixtures\\scan\\one.png",
        error: null,
      });
    });

    await user.click(await screen.findByRole("button", { name: "查看后台任务" }));
    await user.click(screen.getByRole("button", { name: "取消扫描" }));
    await waitFor(() => expect(screen.getByRole("button", { name: "取消扫描" })).toBeEnabled());
    expect(screen.getByRole("alert")).toHaveTextContent("扫描任务已经结束，无法再次取消。");
  });

  it("serializes semantic controls and ignores a late response after terminal progress", async () => {
    const user = userEvent.setup();
    render(<App />);
    await waitFor(() => expect(semanticProgressListener).toBeDefined());

    const runningProgress: SemanticProgress = {
      jobId: "semantic-1",
      libraryId: 7,
      status: "running",
      total: 2,
      processed: 0,
      completed: 0,
      failed: 0,
      skipped: 0,
      currentAssetId: null,
      currentPath: null,
      executionBackend: "cpu",
      modelName: "SigLIP2",
      modelVersion: "test",
      error: null,
    };
    act(() => {
      semanticProgressListener?.(runningProgress);
    });

    const taskTrigger = await screen.findByRole("button", { name: "查看后台任务" });
    await user.click(taskTrigger);
    let resolvePause: ((value: { jobId: string; accepted: boolean }) => void) | undefined;
    api.pauseSemanticAnalysis.mockImplementationOnce(
      () =>
        new Promise<{ jobId: string; accepted: boolean }>((resolve) => {
          resolvePause = resolve;
        }),
    );

    const pauseButton = screen.getByRole("button", { name: "暂停" });
    await user.click(pauseButton);
    expect(api.pauseSemanticAnalysis).toHaveBeenCalledOnce();
    expect(pauseButton).toBeDisabled();
    await user.click(pauseButton);
    expect(api.pauseSemanticAnalysis).toHaveBeenCalledOnce();

    act(() => {
      semanticProgressListener?.({
        ...runningProgress,
        status: "completed",
        processed: 2,
        completed: 2,
      });
    });
    await waitFor(() =>
      expect(screen.queryByRole("button", { name: "查看后台任务" })).not.toBeInTheDocument(),
    );

    expect(resolvePause).toBeDefined();
    await act(async () => {
      resolvePause?.({ jobId: "semantic-1", accepted: true });
      await Promise.resolve();
    });
    expect(screen.queryByRole("button", { name: "查看后台任务" })).not.toBeInTheDocument();
  });

  it("keeps an accepted pause when an older semantic fetch returns running", async () => {
    const user = userEvent.setup();
    api.fetchLibraries.mockResolvedValue([library]);
    let resolveStaleFetch: ((progress: SemanticProgress | null) => void) | undefined;
    api.fetchSemanticProgress.mockResolvedValueOnce(null).mockImplementationOnce(
      () =>
        new Promise<SemanticProgress | null>((resolve) => {
          resolveStaleFetch = resolve;
        }),
    );
    render(<App />);
    await waitFor(() => expect(api.fetchSemanticProgress).toHaveBeenCalledTimes(1));
    await waitFor(() => {
      expect(progressListener).toBeDefined();
      expect(semanticProgressListener).toBeDefined();
    });

    const runningProgress: SemanticProgress = {
      jobId: "semantic-1",
      libraryId: 7,
      status: "running",
      total: 2,
      processed: 0,
      completed: 0,
      failed: 0,
      skipped: 0,
      currentAssetId: null,
      currentPath: null,
      executionBackend: "cpu",
      modelName: "SigLIP2",
      modelVersion: "test",
      error: null,
    };
    act(() => {
      semanticProgressListener?.(runningProgress);
    });

    act(() => {
      progressListener?.({
        taskId: "task-refresh",
        libraryId: 7,
        status: "completed",
        stage: "completed",
        discovered: 0,
        processed: 0,
        succeeded: 0,
        failed: 0,
        skipped: 0,
        missing: 0,
        currentPath: null,
        error: null,
      });
    });
    await waitFor(() => expect(api.fetchSemanticProgress).toHaveBeenCalledTimes(2));
    expect(resolveStaleFetch).toBeDefined();

    await user.click(await screen.findByRole("button", { name: "查看后台任务" }));
    api.pauseSemanticAnalysis.mockResolvedValueOnce({ jobId: "semantic-1", accepted: true });
    await user.click(screen.getByRole("button", { name: "暂停" }));
    await waitFor(() => expect(screen.getByRole("button", { name: "继续" })).toBeInTheDocument());

    await act(async () => {
      resolveStaleFetch?.(runningProgress);
      await Promise.resolve();
    });
    expect(screen.getByRole("button", { name: "继续" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "暂停" })).not.toBeInTheDocument();
  });

  it("moves a library to the root through drag and drop", async () => {
    const childLibrary: LibrarySummary = {
      ...library,
      id: 8,
      name: "子图库",
      rootPath: "C:\\fixtures\\中文 图库\\子图库",
      sourcePath: "C:\\fixtures\\中文 图库\\子图库",
      sourceIdentityKey: "c:/fixtures/中文 图库/子图库",
      parentLibraryId: library.id,
    };
    api.fetchLibraries.mockResolvedValue([library, childLibrary]);
    render(<App />);
    await screen.findByText("子图库");

    const sourceButton = screen.getByTitle(childLibrary.sourcePath);
    const rootDropTarget = screen.getByText("拖到这里移出当前父图库");
    fireEvent.pointerDown(sourceButton, { button: 0, pointerId: 1, clientX: 10, clientY: 10 });
    fireEvent.pointerMove(rootDropTarget, { pointerId: 1, clientX: 30, clientY: 30 });
    expect(sourceButton.closest(".library-tree-row")).toHaveClass("is-dragging");
    expect(rootDropTarget).toHaveClass("library-root-drop-target");
    fireEvent.pointerUp(rootDropTarget, { pointerId: 1, clientX: 30, clientY: 30 });

    await waitFor(() => expect(api.setLibraryParent).toHaveBeenCalledWith(8, null));
  });

  it("moves a top-level library onto another library", async () => {
    const targetLibrary: LibrarySummary = {
      ...library,
      id: 9,
      name: "另一个图库",
      rootPath: "D:\\fixtures\\另一个图库",
      sourcePath: "D:\\fixtures\\另一个图库",
      sourceIdentityKey: "d:/fixtures/另一个图库",
      parentLibraryId: null,
    };
    api.fetchLibraries.mockResolvedValue([library, targetLibrary]);
    render(<App />);
    await screen.findByTitle(targetLibrary.sourcePath);

    const sidebar = screen.getByRole("complementary", { name: "图库与筛选" });
    const libraryButtonBySourcePath = (sourcePath: string) =>
      Array.from(sidebar.querySelectorAll<HTMLButtonElement>("button[title]")).find(
        (button) => button.title === sourcePath,
      );
    const sourceButton = libraryButtonBySourcePath(library.sourcePath);
    const targetButton = libraryButtonBySourcePath(targetLibrary.sourcePath);
    expect(sourceButton).toBeDefined();
    expect(targetButton).toBeDefined();
    const targetRow = targetButton?.closest(".library-tree-row");
    expect(targetRow).not.toBeNull();
    fireEvent.pointerDown(sourceButton as HTMLElement, {
      button: 0,
      pointerId: 2,
      clientX: 10,
      clientY: 10,
    });
    await act(async () => {
      fireEvent.pointerMove(targetRow as HTMLElement, {
        pointerId: 2,
        clientX: 30,
        clientY: 30,
      });
    });
    expect(sourceButton?.closest(".library-tree-row")).toHaveClass("is-dragging");
    await waitFor(() => expect(targetRow).toHaveClass("is-drag-over"));
    fireEvent.pointerUp(targetRow as HTMLElement, {
      pointerId: 2,
      clientX: 30,
      clientY: 30,
    });

    await waitFor(() => expect(api.setLibraryParent).toHaveBeenCalledWith(7, 9));
  });

  it("moves an asset to another library without moving the source file", async () => {
    const targetLibrary: LibrarySummary = {
      ...library,
      id: 9,
      name: "另一个图库",
      rootPath: "D:\\fixtures\\另一个图库",
      sourcePath: "D:\\fixtures\\另一个图库",
      sourceIdentityKey: "d:/fixtures/另一个图库",
      parentLibraryId: null,
    };
    api.fetchLibraries.mockResolvedValue([library, targetLibrary]);
    api.fetchAssets.mockResolvedValue({ items: [asset], total: 1, page: 1, pageSize: 200 });
    render(<App />);

    const assetCard = await screen.findByRole("button", { name: asset.fileName });
    const sidebar = screen.getByRole("complementary", { name: "图库与筛选" });
    const targetButton = Array.from(
      sidebar.querySelectorAll<HTMLButtonElement>("button[title]"),
    ).find((button) => button.title === targetLibrary.sourcePath);
    const targetRow = targetButton?.closest(".library-tree-row");
    expect(targetRow).not.toBeNull();

    fireEvent.pointerDown(assetCard, {
      button: 0,
      pointerId: 3,
      clientX: 10,
      clientY: 10,
    });
    fireEvent.pointerMove(targetRow as HTMLElement, {
      pointerId: 3,
      clientX: 30,
      clientY: 30,
    });
    expect(targetRow).toHaveClass("is-asset-drag-over");
    fireEvent.pointerUp(targetRow as HTMLElement, {
      pointerId: 3,
      clientX: 30,
      clientY: 30,
    });

    await waitFor(() => expect(api.assignAssetToLibrary).toHaveBeenCalledWith(12, 9));
    expect(asset.absolutePath).toBe("C:\\fixtures\\中文 图库\\晚霞.png");
  });

  it("moves all selected assets to another library", async () => {
    const targetLibrary: LibrarySummary = {
      ...library,
      id: 9,
      name: "另一个图库",
      rootPath: "D:\\fixtures\\另一个图库",
      sourcePath: "D:\\fixtures\\另一个图库",
      sourceIdentityKey: "d:/fixtures/另一个图库",
      parentLibraryId: null,
    };
    api.fetchLibraries.mockResolvedValue([library, targetLibrary]);
    api.fetchAssets.mockResolvedValue({
      items: [asset, secondAsset, thirdAsset],
      total: 3,
      page: 1,
      pageSize: 200,
    });
    render(<App />);

    const firstCheck = await screen.findByRole("button", { name: "选择 晚霞.png" });
    const thirdCheck = screen.getByRole("button", { name: "选择 山谷.png" });
    fireEvent.click(firstCheck);
    fireEvent.click(thirdCheck, { shiftKey: true });
    expect(await screen.findByText("已选择 3 张")).toBeInTheDocument();

    const secondCard = screen.getByRole("button", { name: "海边.png" });
    const targetButton = Array.from(
      screen
        .getByRole("complementary", { name: "图库与筛选" })
        .querySelectorAll<HTMLButtonElement>("button[title]"),
    ).find((button) => button.title === targetLibrary.sourcePath);
    const targetRow = targetButton?.closest(".library-tree-row");
    expect(targetRow).not.toBeNull();

    fireEvent.pointerDown(secondCard, {
      button: 0,
      pointerId: 4,
      clientX: 10,
      clientY: 10,
    });
    fireEvent.pointerMove(targetRow as HTMLElement, {
      pointerId: 4,
      clientX: 30,
      clientY: 30,
    });
    fireEvent.pointerUp(targetRow as HTMLElement, {
      pointerId: 4,
      clientX: 30,
      clientY: 30,
    });

    await waitFor(() => {
      expect(api.assignAssetToLibrary).toHaveBeenNthCalledWith(1, 12, 9);
      expect(api.assignAssetToLibrary).toHaveBeenNthCalledWith(2, 13, 9);
      expect(api.assignAssetToLibrary).toHaveBeenNthCalledWith(3, 14, 9);
    });
  });

  it("keeps a fully successful scan visible until the status is dismissed", async () => {
    const user = userEvent.setup();
    api.chooseLibraryFolder.mockResolvedValue("C:\\fixtures\\successful-scan");
    render(<App />);
    await screen.findByRole("heading", { name: "建立本地图片库" });
    await user.click(screen.getAllByRole("button", { name: "选择照片文件夹" })[0]);

    act(() => {
      progressListener?.({
        taskId: "task-1",
        libraryId: 7,
        status: "completed",
        stage: "completed",
        discovered: 3,
        processed: 3,
        succeeded: 3,
        failed: 0,
        skipped: 0,
        missing: 0,
        currentPath: null,
        error: null,
      });
    });

    const taskTrigger = await screen.findByRole("button", { name: "查看后台任务" });
    await user.click(taskTrigger);
    expect(screen.getByRole("progressbar", { name: "扫描进度" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "关闭扫描状态" }));
    expect(screen.queryByRole("progressbar", { name: "扫描进度" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "查看后台任务" })).not.toBeInTheDocument();
  });

  it("requests a new stable sort when the user changes the sort field", async () => {
    api.fetchLibraries.mockResolvedValue([library]);
    api.fetchAssets.mockResolvedValue({ items: [asset], total: 1, page: 1, pageSize: 200 });
    render(<App />);
    await screen.findByRole("button", { name: "晚霞.png" });

    fireEvent.change(screen.getByLabelText("排序"), { target: { value: "brightness" } });

    await waitFor(() =>
      expect(api.fetchAssets).toHaveBeenLastCalledWith(
        expect.objectContaining({ libraryId: 7, sort: "brightness", direction: "desc" }),
      ),
    );
  });

  it("debounces text search and ignores a result invalidated by the next edit", async () => {
    const staleResultAsset = { ...asset, id: 991, fileName: "过时结果.png" };
    let resolveStaleSearch:
      | ((result: {
          items: AssetListItem[];
          total: number;
          page: number;
          pageSize: number;
        }) => void)
      | null = null;
    api.fetchLibraries.mockResolvedValue([library]);
    api.fetchAssets.mockImplementation((query: { filter: { search: string | null } }) => {
      if (query.filter.search === "晚霞") {
        return new Promise((resolve) => {
          resolveStaleSearch = resolve;
        });
      }
      if (query.filter.search === "海边") {
        return Promise.resolve({ items: [secondAsset], total: 1, page: 1, pageSize: 120 });
      }
      return Promise.resolve({ items: [asset], total: 1, page: 1, pageSize: 120 });
    });
    render(<App />);
    expect(await screen.findByRole("button", { name: "晚霞.png" })).toBeInTheDocument();

    vi.useFakeTimers();
    const searchInput = screen.getByRole("textbox", { name: "搜索图片" });
    fireEvent.change(searchInput, { target: { value: "晚霞" } });
    act(() => vi.advanceTimersByTime(249));
    expect(api.fetchAssets).toHaveBeenCalledTimes(1);
    act(() => vi.advanceTimersByTime(1));
    expect(api.fetchAssets).toHaveBeenCalledTimes(2);
    expect(api.fetchAssets).toHaveBeenLastCalledWith(
      expect.objectContaining({ filter: expect.objectContaining({ search: "晚霞" }) }),
    );

    fireEvent.change(searchInput, { target: { value: "海边" } });
    if (!resolveStaleSearch) throw new Error("the stale search request did not start");
    await act(async () => {
      resolveStaleSearch?.({ items: [staleResultAsset], total: 1, page: 1, pageSize: 120 });
      await Promise.resolve();
    });
    expect(screen.queryByRole("button", { name: "过时结果.png" })).not.toBeInTheDocument();

    await act(async () => {
      vi.advanceTimersByTime(250);
      await Promise.resolve();
    });
    expect(api.fetchAssets).toHaveBeenCalledTimes(3);
    expect(api.fetchAssets).toHaveBeenLastCalledWith(
      expect.objectContaining({ filter: expect.objectContaining({ search: "海边" }) }),
    );
    vi.useRealTimers();
    expect(await screen.findByRole("button", { name: "海边.png" })).toBeInTheDocument();
  });

  it("loads grid results continuously without gallery pagination", async () => {
    api.fetchLibraries.mockResolvedValue([library]);
    api.fetchAssets.mockImplementation((query: { page: number }) =>
      Promise.resolve(
        query.page === 1
          ? { items: [asset], total: 2, page: 1, pageSize: 1 }
          : { items: [secondAsset], total: 2, page: 2, pageSize: 1 },
      ),
    );
    render(<App />);

    expect(await screen.findByRole("button", { name: "晚霞.png" })).toBeInTheDocument();
    expect(api.fetchAssets).toHaveBeenCalledWith(
      expect.objectContaining({ page: 1, pageSize: 120 }),
    );
    const results = document.querySelector<HTMLElement>(".grid-workspace-results");
    expect(results).not.toBeNull();
    if (!results) throw new Error("grid results are missing");
    Object.defineProperties(results, {
      clientHeight: { configurable: true, value: 720 },
      scrollHeight: { configurable: true, value: 1_440 },
      scrollTop: { configurable: true, writable: true, value: 720 },
    });
    fireEvent.scroll(results);
    expect(await screen.findByRole("button", { name: "海边.png" })).toBeInTheDocument();
    expect(screen.queryByRole("navigation", { name: "图库分页" })).not.toBeInTheDocument();
    expect(api.fetchAssets).toHaveBeenCalledWith(expect.objectContaining({ page: 2 }));
  });

  it("surfaces startup errors without hiding the import affordance", async () => {
    api.fetchLibraries.mockRejectedValue(new Error("database unavailable"));
    render(<App />);
    expect(await screen.findByRole("alert")).toHaveTextContent("database unavailable");
    expect(screen.getAllByRole("button", { name: "选择照片文件夹" }).length).toBeGreaterThan(0);
  });

  it("supports explicit multi-selection, blank clearing, and single-image zoom", async () => {
    const user = userEvent.setup();
    api.fetchLibraries.mockResolvedValue([library]);
    api.fetchAssets.mockResolvedValue({
      items: [asset, secondAsset],
      total: 2,
      page: 1,
      pageSize: 200,
    });
    render(<App />);

    const first = await screen.findByRole("button", { name: "晚霞.png" });
    const second = screen.getByRole("button", { name: "海边.png" });
    await user.click(first);
    await waitFor(() => expect(first).toHaveAttribute("aria-current", "true"));
    await user.click(screen.getByRole("button", { name: "选择 晚霞.png" }));
    const selectionActions = screen.getByRole("group", { name: "选择操作" });
    expect(selectionActions).toHaveTextContent("清除选择");
    expect(selectionActions).toHaveTextContent("批量修正");
    expect(selectionActions.nextElementSibling).toHaveClass("topbar-browse-controls");
    expect(selectionActions.nextElementSibling?.querySelector(".segmented")).not.toBeNull();
    expect(screen.queryByRole("button", { name: "分析选中" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "整理预览" })).toBeEnabled();
    fireEvent.change(screen.getByLabelText("搜索图片"), { target: { value: "晚霞" } });
    await user.click(screen.getByRole("button", { name: "筛选 1" }));
    const filterDialog = screen.getByRole("dialog", { name: "当前条件" });
    expect(filterDialog).toHaveTextContent("搜索");
    expect(filterDialog).toHaveTextContent("晚霞");
    await user.click(within(filterDialog).getByRole("button", { name: "移除筛选条件：搜索 晚霞" }));
    expect(screen.getByText("当前没有筛选条件")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "筛选" }));
    fireEvent.change(screen.getByLabelText("搜索图片"), { target: { value: "海边" } });
    await user.click(screen.getByRole("button", { name: "筛选 1" }));
    await user.click(
      within(screen.getByRole("dialog", { name: "当前条件" })).getByRole("button", {
        name: "清除筛选",
      }),
    );
    expect(screen.getByText("当前没有筛选条件")).toBeInTheDocument();
    fireEvent.click(second, { ctrlKey: true });
    expect(await screen.findByText("已选择 2 张")).toBeInTheDocument();
    fireEvent.click(first, { ctrlKey: true });
    await waitFor(() => expect(screen.getByText("已选择 1 张")).toBeInTheDocument());
    fireEvent.click(second, { shiftKey: true });
    expect(await screen.findByText("已选择 2 张")).toBeInTheDocument();
    await user.click(screen.getAllByRole("button", { name: "清除选择" })[0]);
    expect(screen.queryByText("已选择 2 张")).not.toBeInTheDocument();

    await user.dblClick(first);
    const filmstrip = await screen.findByLabelText("胶片栏");
    expect(filmstrip).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "晚霞.png" })).toHaveAttribute(
      "aria-current",
      "true",
    );
    expect(screen.getByRole("button", { name: "晚霞.png" })).toHaveClass("is-active");
    expect(
      screen.getByRole("toolbar", { name: "人工标记筛选" }).closest(".content-toolbar"),
    ).not.toBeNull();
    expect(document.querySelector(".photo-app")).not.toHaveClass("has-batch-classification");
    const singleSelection = screen.getByRole("button", { name: "选择 晚霞.png" });
    expect(singleSelection).toHaveClass("single-selection-toggle");
    expect(singleSelection).toHaveClass("single-selection-toolbar-toggle");
    expect(singleSelection.closest(".content-toolbar")).not.toBeNull();
    expect(singleSelection.closest(".single-canvas")).toBeNull();
    expect(singleSelection.querySelector(".single-selection-mark")).not.toBeNull();
    await user.click(singleSelection);
    expect(singleSelection).toHaveAttribute("aria-pressed", "true");
    expect(await screen.findByText("已选择 1 张")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "取消选择 晚霞.png" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    const filmstripSecond = within(filmstrip).getByRole("button", { name: "海边.png" });
    fireEvent.click(filmstripSecond, { ctrlKey: true });
    await waitFor(() => expect(filmstripSecond).toHaveAttribute("aria-pressed", "true"));
    expect(filmstripSecond).toHaveClass("is-selected");
    expect(filmstripSecond).not.toHaveAttribute("aria-current");
    expect(screen.getByRole("button", { name: "晚霞.png" })).toHaveAttribute(
      "aria-current",
      "true",
    );
    expect(await screen.findByText("已选择 2 张")).toBeInTheDocument();
    fireEvent.wheel(filmstrip, { deltaY: 120, deltaX: 0 });
    expect(filmstrip.scrollLeft).toBe(120);
    expect(screen.getByLabelText("图片导航图")).toBeInTheDocument();
    expect(screen.queryByLabelText("预览缩放工具")).not.toBeInTheDocument();
    expect(api.fetchAssets).toHaveBeenLastCalledWith(
      expect.objectContaining({ page: 1, pageSize: 120 }),
    );
    expect(screen.queryByRole("navigation", { name: "图库分页" })).not.toBeInTheDocument();
    await waitFor(() => expect(api.fetchPreview).toHaveBeenCalledWith(12, "original"));
    await waitFor(() => expect(api.fetchPreview).toHaveBeenCalledWith(13, "original"));
    const previewStage = screen.getByAltText(asset.fileName).closest<HTMLElement>(".zoom-stage");
    expect(previewStage).not.toBeNull();
    const getBoundingClientRect = vi.spyOn(previewStage as HTMLElement, "getBoundingClientRect");
    getBoundingClientRect.mockReturnValue({
      width: 640,
      height: 480,
      top: 0,
      right: 640,
      bottom: 480,
      left: 0,
      x: 0,
      y: 0,
      toJSON: () => ({}),
    });
    fireEvent.resize(window);
    const zoomLabel = document.querySelector<HTMLElement>(".preview-navigator-zoom-label");
    expect(zoomLabel).not.toBeNull();
    await waitFor(() => expect(zoomLabel?.textContent).toBe("50.67%"));
    const previewImage = screen.getByAltText(asset.fileName) as HTMLImageElement;
    Object.defineProperty(previewImage, "naturalWidth", { configurable: true, value: 800 });
    Object.defineProperty(previewImage, "naturalHeight", { configurable: true, value: 1200 });
    fireEvent.load(previewImage);
    expect(zoomLabel).toHaveTextContent("50.67%");
    const zoomBeforeWheel = zoomLabel?.textContent;
    fireEvent.doubleClick(await screen.findByAltText(asset.fileName), {
      clientX: 160,
      clientY: 120,
    });
    await waitFor(() => expect(screen.getByText("100%")).toBeInTheDocument());
    expect(previewImage.style.transform).toContain("155.78947368421052px");
    fireEvent.doubleClick(await screen.findByAltText(asset.fileName));
    await waitFor(() => expect(zoomLabel?.textContent).toBe(zoomBeforeWheel));
    fireEvent.wheel(previewStage as HTMLElement, { deltaY: -120 });
    await waitFor(() => expect(zoomLabel?.textContent).not.toBe(zoomBeforeWheel));
    expect(
      api.fetchPreview.mock.calls.filter((call) => call[0] === 12 && call[1] === "original"),
    ).toHaveLength(1);
    expect(screen.queryByRole("combobox", { name: "缩放比例" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "放大预览" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "缩小预览" })).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "海边.png" }));
    await waitFor(() => expect(zoomLabel?.textContent).toBe(zoomBeforeWheel));
    expect(
      api.fetchPreview.mock.calls.filter((call) => call[0] === 13 && call[1] === "original"),
    ).toHaveLength(1);
    expect(screen.getByRole("button", { name: "海边.png" })).toHaveAttribute(
      "aria-current",
      "true",
    );
    await waitFor(() =>
      expect(screen.getByRole("heading", { name: secondAsset.fileName })).toBeInTheDocument(),
    );
    await waitFor(() =>
      expect(document.querySelector<HTMLImageElement>(".zoom-stage .preview-image")?.alt).toBe(
        secondAsset.fileName,
      ),
    );
    await user.keyboard("{ArrowLeft}");
    await waitFor(() =>
      expect(screen.getByRole("heading", { name: asset.fileName })).toBeInTheDocument(),
    );
    expect(screen.getByRole("button", { name: asset.fileName })).toHaveAttribute(
      "aria-current",
      "true",
    );
    await user.keyboard("{ArrowRight}");
    await waitFor(() =>
      expect(screen.getByRole("heading", { name: secondAsset.fileName })).toBeInTheDocument(),
    );
    expect(screen.getByRole("button", { name: secondAsset.fileName })).toHaveAttribute(
      "aria-current",
      "true",
    );
    await user.keyboard("{Escape}");
    expect(screen.getByLabelText("图片网格")).toBeInTheDocument();
    getBoundingClientRect.mockRestore();
  });

  it("resizes both side panels from border hit areas with accessible controls", async () => {
    api.fetchLibraries.mockResolvedValue([library]);
    api.fetchAssets.mockResolvedValue({ items: [asset], total: 1, page: 1, pageSize: 200 });
    render(<App />);

    const leftHandle = await screen.findByRole("separator", { name: "调整左侧面板宽度" });
    const rightHandle = screen.getByRole("separator", { name: "调整右侧面板宽度" });
    const sidebarHandle = screen.getByRole("separator", { name: "调整图库与筛选高度" });
    expect(leftHandle).toHaveClass("panel-resize-handle-left");
    expect(rightHandle).toHaveClass("panel-resize-handle-right");
    expect(leftHandle).toHaveAttribute("title", "拖动调整左侧图库与筛选宽度");
    expect(rightHandle).toHaveAttribute("title", "拖动调整右侧信息宽度");
    expect(sidebarHandle).toHaveClass("sidebar-vertical-resize-handle");
    expect(sidebarHandle).toHaveAttribute("aria-orientation", "horizontal");
    expect(sidebarHandle).toHaveAttribute("aria-valuenow", "50");
    expect(sidebarHandle).toHaveAttribute("aria-valuetext", "图库与筛选各占一半");
    expect(leftHandle).toHaveAttribute("aria-valuenow", "270");
    expect(rightHandle).toHaveAttribute("aria-valuenow", "320");

    act(() => fireEvent.keyDown(leftHandle, { key: "ArrowRight" }));
    expect(leftHandle).toHaveAttribute("aria-valuenow", "286");

    act(() => fireEvent.keyDown(rightHandle, { key: "ArrowLeft" }));
    expect(rightHandle).toHaveAttribute("aria-valuenow", "336");

    const sidebar = sidebarHandle.closest<HTMLElement>(".left-panel");
    const libraryModule = sidebar?.querySelector<HTMLElement>(".sidebar-library-module");
    const filterModule = sidebar?.querySelector<HTMLElement>(".sidebar-filter-module");
    expect(libraryModule).not.toBeNull();
    expect(filterModule).not.toBeNull();
    const libraryRect = vi
      .spyOn(libraryModule as HTMLElement, "getBoundingClientRect")
      .mockReturnValue({ height: 400 } as DOMRect);
    const filterRect = vi
      .spyOn(filterModule as HTMLElement, "getBoundingClientRect")
      .mockReturnValue({ height: 400 } as DOMRect);

    act(() => fireEvent.keyDown(sidebarHandle, { key: "ArrowDown" }));
    expect(sidebarHandle).toHaveAttribute("aria-valuenow", "52");
    libraryRect.mockRestore();
    filterRect.mockRestore();
  });

  it("toggles favorites and refreshes the default favorite count", async () => {
    const user = userEvent.setup();
    const favoriteCollection = {
      id: 100,
      name: "默认收藏",
      description: "",
      createdAt: "2026-08-06T10:00:00Z",
      updatedAt: "2026-08-06T10:00:00Z",
      assetCount: 0,
      parentCollectionId: null,
      collectionKind: "system_favorites" as const,
      systemKey: "default_favorites",
      displayOrder: -1,
    };
    let favoritePersisted = false;
    api.fetchLibraries.mockResolvedValue([library]);
    api.fetchAssets.mockResolvedValue({ items: [asset], total: 1, page: 1, pageSize: 200 });
    api.setAssetFavorite.mockImplementation(async (_assetId: number, favorite: boolean) => {
      favoritePersisted = favorite;
      return favorite;
    });
    api.fetchCollections.mockImplementation(async () => [
      { ...favoriteCollection, assetCount: favoritePersisted ? 1 : 0 },
    ]);
    api.fetchBrowseNodes.mockImplementation(async () => [
      {
        kind: "collection",
        collection: { ...favoriteCollection, assetCount: favoritePersisted ? 1 : 0 },
        children: [],
      },
    ]);
    api.fetchFavoriteAssetIds.mockImplementation(async () => (favoritePersisted ? [asset.id] : []));
    render(<App />);

    expect(await screen.findByTitle("默认收藏")).toHaveTextContent("0");

    const browseRequestsBeforeFavorite = api.fetchBrowseNodes.mock.calls.length;
    const favorite = await screen.findByRole("button", { name: "收藏 晚霞.png" });
    expect(favorite).toHaveAttribute("aria-pressed", "false");
    await user.click(favorite);

    expect(api.setAssetFavorite).toHaveBeenCalledWith(asset.id, true);
    expect(screen.getByRole("button", { name: "取消收藏 晚霞.png" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    await waitFor(() => expect(screen.getByTitle("默认收藏")).toHaveTextContent("1"));
    expect(api.fetchBrowseNodes.mock.calls.length).toBeGreaterThan(browseRequestsBeforeFavorite);
    expect(api.updateAssetRating).not.toHaveBeenCalled();
  });

  it("preserves a newer library selection and its selected assets when removal completes", async () => {
    const user = userEvent.setup();
    const libraryB: LibrarySummary = {
      ...library,
      id: 8,
      name: "图库 B",
      rootPath: "C:\\fixtures\\图库 B",
      sourcePath: "C:\\fixtures\\图库 B",
      sourceIdentityKey: "c:/fixtures/图库 b",
    };
    const libraryC: LibrarySummary = {
      ...library,
      id: 9,
      name: "图库 C",
      rootPath: "C:\\fixtures\\图库 C",
      sourcePath: "C:\\fixtures\\图库 C",
      sourceIdentityKey: "c:/fixtures/图库 c",
    };
    const cAsset: AssetListItem = {
      ...asset,
      id: 99,
      libraryId: libraryC.id,
      absolutePath: `${libraryC.sourcePath}\\相机C.png`,
      relativePath: "相机C.png",
      fileName: "相机C.png",
    };
    let resolveRemoval: ((removed: boolean) => void) | undefined;
    const removal = new Promise<boolean>((resolve) => {
      resolveRemoval = resolve;
    });
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(true);
    api.fetchLibraries.mockResolvedValue([library, libraryB, libraryC]);
    api.fetchAssets.mockImplementation((query: { libraryId: number | null }) =>
      Promise.resolve(
        query.libraryId === libraryC.id
          ? { items: [cAsset], total: 1, page: 1, pageSize: 200 }
          : { items: [asset], total: 1, page: 1, pageSize: 200 },
      ),
    );
    api.removeLibrary.mockImplementationOnce(() => removal);
    render(<App />);

    await screen.findByRole("button", { name: asset.fileName });
    await user.click(screen.getByRole("button", { name: "中文 图库图库菜单" }));
    await user.click(screen.getByRole("button", { name: "从图库移除" }));
    await waitFor(() => expect(api.removeLibrary).toHaveBeenCalledWith(library.id));

    const libraryCButton = sidebarButtonWithTitle(libraryC.sourcePath);
    if (!libraryCButton) throw new Error("library C is missing from the sidebar");
    await user.click(libraryCButton);
    await user.click(await screen.findByRole("button", { name: `选择 ${cAsset.fileName}` }));
    expect(await screen.findByText("已选择 1 张")).toBeInTheDocument();

    if (!resolveRemoval) throw new Error("the deferred library removal did not start");
    await act(async () => {
      resolveRemoval?.(true);
    });

    await waitFor(() =>
      expect(sidebarButtonWithTitle(libraryC.sourcePath)).toHaveClass("is-active"),
    );
    expect(sidebarButtonWithTitle(libraryB.sourcePath)).not.toHaveClass("is-active");
    expect(screen.getByText("已选择 1 张")).toBeInTheDocument();
    confirm.mockRestore();
  });

  it("preserves a newer Favorites source when library removal completes", async () => {
    const user = userEvent.setup();
    const favoriteNode = {
      kind: "collection" as const,
      collection: {
        id: 100,
        name: "默认收藏",
        description: "",
        createdAt: "2026-08-06T10:00:00Z",
        updatedAt: "2026-08-06T10:00:00Z",
        assetCount: 1,
        parentCollectionId: null,
        collectionKind: "system_favorites" as const,
        systemKey: "default_favorites",
        displayOrder: -1,
      },
      children: [],
    };
    let resolveRemoval: ((removed: boolean) => void) | undefined;
    const removal = new Promise<boolean>((resolve) => {
      resolveRemoval = resolve;
    });
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(true);
    api.fetchLibraries.mockResolvedValue([library]);
    api.fetchBrowseNodes.mockResolvedValue([favoriteNode]);
    api.fetchAssets.mockResolvedValue({ items: [asset], total: 1, page: 1, pageSize: 200 });
    api.removeLibrary.mockImplementationOnce(() => removal);
    render(<App />);

    await screen.findByRole("button", { name: asset.fileName });
    await user.click(screen.getByRole("button", { name: "中文 图库图库菜单" }));
    await user.click(screen.getByRole("button", { name: "从图库移除" }));
    await waitFor(() => expect(api.removeLibrary).toHaveBeenCalledWith(library.id));

    const favoritesButton = sidebarButtonWithTitle("默认收藏");
    if (!favoritesButton) throw new Error("the Favorites source is missing from the sidebar");
    await user.click(favoritesButton);
    await waitFor(() => expect(favoritesButton).toHaveClass("is-active"));
    expect(api.fetchAssets).toHaveBeenLastCalledWith(
      expect.objectContaining({
        libraryId: null,
        filter: expect.objectContaining({ favoriteOnly: true, collectionId: null }),
      }),
    );

    if (!resolveRemoval) throw new Error("the deferred library removal did not start");
    await act(async () => {
      resolveRemoval?.(true);
    });

    await waitFor(() => expect(sidebarButtonWithTitle("默认收藏")).toHaveClass("is-active"));
    expect(sidebarButtonWithTitle(library.sourcePath)).toBeUndefined();
    confirm.mockRestore();
  });

  it("preserves a newer collection source when library removal completes", async () => {
    const user = userEvent.setup();
    const collectionNode = {
      kind: "collection" as const,
      collection: {
        id: 101,
        name: "旅行收藏夹",
        description: "",
        createdAt: "2026-08-06T10:00:00Z",
        updatedAt: "2026-08-06T10:00:00Z",
        assetCount: 1,
        parentCollectionId: null,
        collectionKind: "manual" as const,
        systemKey: null,
        displayOrder: 0,
      },
      children: [],
    };
    let resolveRemoval: ((removed: boolean) => void) | undefined;
    const removal = new Promise<boolean>((resolve) => {
      resolveRemoval = resolve;
    });
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(true);
    api.fetchLibraries.mockResolvedValue([library]);
    api.fetchBrowseNodes.mockResolvedValue([collectionNode]);
    api.fetchAssets.mockResolvedValue({ items: [asset], total: 1, page: 1, pageSize: 200 });
    api.removeLibrary.mockImplementationOnce(() => removal);
    render(<App />);

    await screen.findByRole("button", { name: asset.fileName });
    await user.click(screen.getByRole("button", { name: "中文 图库图库菜单" }));
    await user.click(screen.getByRole("button", { name: "从图库移除" }));
    await waitFor(() => expect(api.removeLibrary).toHaveBeenCalledWith(library.id));

    const collectionButton = sidebarButtonWithTitle("旅行收藏夹");
    if (!collectionButton) throw new Error("the collection source is missing from the sidebar");
    await user.click(collectionButton);
    await waitFor(() => expect(collectionButton).toHaveClass("is-active"));
    expect(api.fetchAssets).toHaveBeenLastCalledWith(
      expect.objectContaining({
        libraryId: null,
        filter: expect.objectContaining({ favoriteOnly: false, collectionId: 101 }),
      }),
    );

    if (!resolveRemoval) throw new Error("the deferred library removal did not start");
    await act(async () => {
      resolveRemoval?.(true);
    });

    await waitFor(() => expect(sidebarButtonWithTitle("旅行收藏夹")).toHaveClass("is-active"));
    expect(sidebarButtonWithTitle(library.sourcePath)).toBeUndefined();
    confirm.mockRestore();
  });

  it("falls back when the currently selected library is removed", async () => {
    const user = userEvent.setup();
    const fallbackLibrary: LibrarySummary = {
      ...library,
      id: 8,
      name: "备用图库",
      rootPath: "C:\\fixtures\\备用图库",
      sourcePath: "C:\\fixtures\\备用图库",
      sourceIdentityKey: "c:/fixtures/备用图库",
    };
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(true);
    let availableLibraries = [library, fallbackLibrary];
    api.fetchLibraries.mockImplementation(async () => availableLibraries);
    api.removeLibrary.mockImplementation(async (libraryId: number) => {
      const nextLibraries = availableLibraries.filter((item) => item.id !== libraryId);
      const removed = nextLibraries.length !== availableLibraries.length;
      availableLibraries = nextLibraries;
      return removed;
    });
    render(<App />);

    await waitFor(() => expect(sidebarButtonWithTitle(library.sourcePath)).toBeDefined());
    await user.click(screen.getByRole("button", { name: "中文 图库图库菜单" }));
    await user.click(screen.getByRole("button", { name: "从图库移除" }));

    await waitFor(() =>
      expect(sidebarButtonWithTitle(fallbackLibrary.sourcePath)).toHaveClass("is-active"),
    );
    expect(sidebarButtonWithTitle(library.sourcePath)).toBeUndefined();
    confirm.mockRestore();
  });

  it("removes a library through its menu without touching source files", async () => {
    const user = userEvent.setup();
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(true);
    api.fetchLibraries.mockResolvedValue([library]);
    api.fetchAssets.mockResolvedValue({ items: [asset], total: 1, page: 1, pageSize: 200 });
    render(<App />);

    await screen.findByRole("button", { name: "晚霞.png" });
    await user.click(screen.getByRole("button", { name: "中文 图库图库菜单" }));
    expect(screen.getByRole("menu")).toBeInTheDocument();
    fireEvent.pointerDown(document.body);
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "中文 图库图库菜单" }));
    await user.click(screen.getByRole("button", { name: "从图库移除" }));

    expect(api.removeLibrary).toHaveBeenCalledWith(7);
    expect(confirm).toHaveBeenCalled();
    confirm.mockRestore();
  });

  it("asks before removing child libraries and removes a cascade from the leaves upward", async () => {
    const user = userEvent.setup();
    const childLibrary: LibrarySummary = {
      ...library,
      id: 8,
      name: "子图库",
      rootPath: "C:\\fixtures\\中文 图库\\子图库",
      sourcePath: "C:\\fixtures\\中文 图库\\子图库",
      sourceIdentityKey: "c:/fixtures/中文 图库/子图库",
      parentLibraryId: library.id,
    };
    const nestedLibrary: LibrarySummary = {
      ...childLibrary,
      id: 9,
      name: "嵌套子图库",
      rootPath: "C:\\fixtures\\中文 图库\\子图库\\嵌套",
      sourcePath: "C:\\fixtures\\中文 图库\\子图库\\嵌套",
      sourceIdentityKey: "c:/fixtures/中文 图库/子图库/嵌套",
      parentLibraryId: childLibrary.id,
    };
    const confirm = vi.spyOn(window, "confirm").mockReturnValueOnce(true).mockReturnValueOnce(true);
    api.fetchLibraries.mockResolvedValue([library, childLibrary, nestedLibrary]);
    render(<App />);

    await screen.findByText("嵌套子图库");
    await user.click(screen.getByRole("button", { name: "中文 图库图库菜单" }));
    await user.click(screen.getByRole("button", { name: "从图库移除" }));

    await waitFor(() => expect(api.removeLibrary).toHaveBeenCalledTimes(3));
    expect(api.removeLibrary).toHaveBeenNthCalledWith(1, nestedLibrary.id);
    expect(api.removeLibrary).toHaveBeenNthCalledWith(2, childLibrary.id);
    expect(api.removeLibrary).toHaveBeenNthCalledWith(3, library.id);
    expect(confirm).toHaveBeenNthCalledWith(2, expect.stringContaining("2 个子图库"));
    confirm.mockRestore();
  });

  it("keeps child libraries when the parent removal asks to remove them and the answer is no", async () => {
    const user = userEvent.setup();
    const childLibrary: LibrarySummary = {
      ...library,
      id: 8,
      name: "子图库",
      rootPath: "C:\\fixtures\\中文 图库\\子图库",
      sourcePath: "C:\\fixtures\\中文 图库\\子图库",
      sourceIdentityKey: "c:/fixtures/中文 图库/子图库",
      parentLibraryId: library.id,
    };
    const confirm = vi
      .spyOn(window, "confirm")
      .mockReturnValueOnce(true)
      .mockReturnValueOnce(false);
    api.fetchLibraries.mockResolvedValue([library, childLibrary]);
    render(<App />);

    await screen.findByText("子图库");
    await user.click(screen.getByRole("button", { name: "中文 图库图库菜单" }));
    await user.click(screen.getByRole("button", { name: "从图库移除" }));

    await waitFor(() => expect(api.removeLibrary).toHaveBeenCalledTimes(1));
    expect(api.removeLibrary).toHaveBeenCalledWith(library.id);
    expect(confirm).toHaveBeenNthCalledWith(2, expect.stringContaining("仅移除当前图库"));
    confirm.mockRestore();
  });
});
