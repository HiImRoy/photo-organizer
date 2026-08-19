import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import App from "./App";
import { emptyEffectiveClassification } from "./types";
import type { AssetListItem, LibrarySummary, ScanProgress } from "./types";

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

let progressListener: ((progress: ScanProgress) => void) | undefined;

beforeEach(() => {
  vi.clearAllMocks();
  window.localStorage.removeItem("photo-organizer-theme");
  window.localStorage.removeItem("photo-organizer-settings");
  progressListener = undefined;
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
  api.subscribeSemanticStatus.mockResolvedValue(() => undefined);
  api.subscribeSubjectStatus.mockResolvedValue(() => undefined);
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
  api.subscribeSemanticProgress.mockResolvedValue(vi.fn());
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
    expect(within(actionGroup).getAllByRole("button").at(-1)).toHaveTextContent(/分析|装载模型/);
  });

  it("shows the first-run empty state and import action", async () => {
    render(<App />);
    expect(await screen.findByRole("heading", { name: "从一个文件夹开始" })).toBeInTheDocument();
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
    expect(screen.getByText("风光自然", { selector: ".group-heading strong" })).toBeInTheDocument();
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
    await screen.findByRole("heading", { name: "从一个文件夹开始" });

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
    expect(await screen.findByText("准备图库")).toBeInTheDocument();
  });

  it("can exclude child-folder images while importing a folder", async () => {
    const user = userEvent.setup();
    api.chooseLibraryFolder.mockResolvedValue("C:\\fixtures\\root-only");
    render(<App />);

    await screen.findByRole("heading", { name: "从一个文件夹开始" });
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

  it("opens persistent settings for shortcuts and thumbnail pipeline limits", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(screen.getByRole("button", { name: "打开设置" }));
    const dialog = screen.getByRole("dialog", { name: "设置" });
    expect(dialog).toBeInTheDocument();

    await user.click(within(dialog).getByRole("tab", { name: /性能/ }));
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
      expect(stored.importWorkerCount).toBe(1);
      expect(stored.analysisBatchSize).toBe(8);
      expect(stored.shortcuts.ratings["3"]).toBe("q");
    });
  });

  it("switches between grid and single preview with the view shortcuts", async () => {
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

    await screen.findByRole("button", { name: viewAsset.fileName });
    const gridButton = screen.getByRole("button", { name: "网格视图" });
    const singleButton = screen.getByRole("button", { name: "单图预览" });
    expect(gridButton).toHaveClass("is-active");
    expect(singleButton).not.toHaveClass("is-active");

    fireEvent.keyDown(window, { key: "f" });
    await waitFor(() => expect(document.querySelector(".single-workspace")).not.toBeNull());
    expect(singleButton).toHaveClass("is-active");
    expect(gridButton).not.toHaveClass("is-active");

    fireEvent.keyDown(window, { key: "g" });
    await waitFor(() => expect(screen.getByLabelText("图片网格")).toBeInTheDocument());
    expect(gridButton).toHaveClass("is-active");
    expect(singleButton).not.toHaveClass("is-active");
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
    expect(assetButton).toHaveAttribute("aria-pressed", "false");
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
    await waitFor(() => expect(assetButton).toHaveAttribute("aria-pressed", "true"));
    expect(screen.getByText("1200 × 800")).toBeInTheDocument();
    expect(screen.getByText("#D76A52")).toBeInTheDocument();
    await waitFor(() => expect(api.fetchThumbnail).toHaveBeenCalledWith(12));
  });

  it("loads SigLIP 2 when the default topic model is prepared", async () => {
    const user = userEvent.setup();
    api.fetchLibraries.mockResolvedValue([library]);
    api.fetchAssets.mockResolvedValue({ items: [asset], total: 1, page: 1, pageSize: 200 });
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "装载模型" }));

    await waitFor(() => expect(api.prepareSemanticModel).toHaveBeenCalledWith("siglip2-base"));
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
          manual: "landscape",
          effective: "landscape",
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
    await user.selectOptions(primarySelect, "landscape");
    await user.click(within(details).getByRole("button", { name: "保存" }));
    await waitFor(() =>
      expect(api.updateClassificationOverride).toHaveBeenCalledWith(
        12,
        "primary_category",
        "landscape",
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

    const assetButton = await screen.findByRole("button", { name: "晚霞.png" });
    await user.click(screen.getByRole("button", { name: "选择 晚霞.png" }));
    await user.click(screen.getByRole("button", { name: "整理预览" }));

    expect(screen.getByRole("region", { name: "整理预览工作区" })).toBeInTheDocument();
    expect(screen.getByText("显式选择 · 1 张")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "返回图库" }));
    expect(screen.getByRole("region", { name: "图片网格" })).toBeInTheDocument();
    expect(assetButton).toHaveAttribute("aria-pressed", "true");

    await user.click(screen.getByRole("button", { name: "清除选择" }));
    await user.click(screen.getByRole("button", { name: "整理预览" }));
    expect(screen.getByText("当前查询 · 1 张")).toBeInTheDocument();
  });

  it("keeps the explicit selection when choosing a collection for the add-selection action", async () => {
    const user = userEvent.setup();
    const collection = {
      id: 3,
      name: "旅行",
      description: "",
      createdAt: "2026-08-06T10:00:00Z",
      updatedAt: "2026-08-06T10:00:00Z",
      assetCount: 0,
      assets: [],
    };
    api.fetchLibraries.mockResolvedValue([library]);
    api.fetchAssets.mockResolvedValue({ items: [asset], total: 1, page: 1, pageSize: 200 });
    api.fetchCollections.mockResolvedValue([collection]);
    api.fetchCollection.mockResolvedValue(collection);
    api.addAssetsToCollection.mockResolvedValue(collection);
    render(<App />);

    await screen.findByRole("button", { name: "晚霞.png" });
    await user.click(screen.getByRole("button", { name: "选择 晚霞.png" }));
    await user.click(screen.getByRole("button", { name: "加入集合" }));

    const workflow = screen.getByRole("region", { name: "查找与审阅" });
    const assetRequestCountBeforeTargetSelection = api.fetchAssets.mock.calls.length;
    await user.click(within(workflow).getByRole("button", { name: /旅行/ }));

    const addButton = await within(workflow).findByRole("button", { name: "加入已选 1 张" });
    expect(addButton).toBeEnabled();
    expect(api.fetchAssets.mock.calls.length).toBe(assetRequestCountBeforeTargetSelection);
    await user.click(addButton);

    expect(api.addAssetsToCollection).toHaveBeenCalledWith(3, [asset.id]);
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
          filter: expect.objectContaining({ favoriteOnly: true, collectionId: null }),
        }),
      ),
    );
    expect(screen.getByRole("region", { name: "图片网格" })).toBeInTheDocument();
    expect(screen.getByRole("complementary", { name: "图片详情" })).toBeInTheDocument();
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
    const gridZoom = screen.getByRole("slider", { name: "每行图片数" });
    expect(gridZoom).toHaveValue("6");
    expect(gridZoom).toHaveAttribute("min", "2");
    expect(gridZoom).toHaveAttribute("max", "12");
    expect(gridZoom).toHaveAttribute("step", "2");
    fireEvent.change(gridZoom, { target: { value: "10" } });
    expect(gridZoom).toHaveValue("10");
    expect(screen.getByText("10 张")).toBeInTheDocument();
    const gridResults = document.querySelector(".grid-workspace-results");
    expect(gridResults).not.toBeNull();
    fireEvent.wheel(gridResults as HTMLElement, { ctrlKey: true, deltaY: -100 });
    expect(gridZoom).toHaveValue("8");
    fireEvent.change(gridZoom, { target: { value: "12" } });
    fireEvent.wheel(gridResults as HTMLElement, { ctrlKey: true, deltaY: 100 });
    expect(gridZoom).toHaveValue("12");
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
    await screen.findByRole("heading", { name: "从一个文件夹开始" });
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

    expect(await screen.findByText("发现 20")).toBeInTheDocument();
    expect(screen.getByText("失败 1")).toBeInTheDocument();
    expect(screen.queryByText("Timing (cumulative)")).not.toBeInTheDocument();
    const scanPanel = document.querySelector(".scan-panel");
    expect(scanPanel?.parentElement).toHaveClass("center-column");
    expect(scanPanel?.closest(".center-workspace")).toBeNull();
    expect(screen.getByRole("progressbar", { name: "扫描进度" })).toHaveAttribute(
      "aria-valuenow",
      "20",
    );
    await user.click(screen.getByRole("button", { name: "取消扫描" }));
    expect(api.cancelLibraryScan).toHaveBeenCalledWith("task-1");
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
    expect(rootDropTarget).toHaveClass("is-drag-over");
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

  it("dismisses a fully successful scan automatically", async () => {
    const user = userEvent.setup();
    api.chooseLibraryFolder.mockResolvedValue("C:\\fixtures\\successful-scan");
    render(<App />);
    await screen.findByRole("heading", { name: "从一个文件夹开始" });
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

    expect(screen.getByRole("progressbar", { name: "扫描进度" })).toBeInTheDocument();
    await waitFor(
      () => expect(screen.queryByRole("progressbar", { name: "扫描进度" })).not.toBeInTheDocument(),
      { timeout: 1_500 },
    );
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
    await waitFor(() => expect(first).toHaveAttribute("aria-pressed", "true"));
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
    fireEvent.doubleClick(await screen.findByAltText(asset.fileName));
    await waitFor(() => expect(screen.getByText("100%")).toBeInTheDocument());
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

  it("toggles favorites independently from the existing star rating", async () => {
    const user = userEvent.setup();
    api.fetchLibraries.mockResolvedValue([library]);
    api.fetchAssets.mockResolvedValue({ items: [asset], total: 1, page: 1, pageSize: 200 });
    render(<App />);

    const favorite = await screen.findByRole("button", { name: "收藏 晚霞.png" });
    expect(favorite).toHaveAttribute("aria-pressed", "false");
    await user.click(favorite);

    expect(api.setAssetFavorite).toHaveBeenCalledWith(asset.id, true);
    expect(screen.getByRole("button", { name: "取消收藏 晚霞.png" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    expect(api.updateAssetRating).not.toHaveBeenCalled();
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
