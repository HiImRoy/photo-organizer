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
      modelName: "TinyCLIP",
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
  progressListener = undefined;
  api.chooseLibraryFolder.mockResolvedValue(null);
  api.fetchLibraries.mockResolvedValue([]);
  api.fetchAssets.mockResolvedValue({ items: [], total: 0, page: 1, pageSize: 200 });
  api.fetchAssetDetail.mockResolvedValue(null);
  api.fetchFavoriteAssetIds.mockResolvedValue([]);
  api.fetchFavoriteAssets.mockResolvedValue([]);
  api.fetchCollections.mockResolvedValue([]);
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
      name: "TinyCLIP",
      version: "test",
      analysisVersion: "test",
      license: "MIT",
      installed: true,
      modelSizeBytes: 24_281_512,
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

describe("PhotoOrganizer application shell", () => {
  it("switches to the light theme and persists the day-mode choice", async () => {
    const user = userEvent.setup();
    render(<App />);

    const app = document.querySelector<HTMLElement>(".photo-app");
    const themeToggle = await screen.findByRole("button", { name: "切换到白天模式" });
    expect(app).not.toHaveClass("theme-light");

    await user.click(themeToggle);

    expect(app).toHaveClass("theme-light");
    expect(window.localStorage.getItem("photo-organizer-theme")).toBe("light");
    expect(screen.getByRole("button", { name: "切换到深色模式" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
  });

  it("shows the first-run empty state and import action", async () => {
    render(<App />);
    expect(await screen.findByRole("heading", { name: "建立本地图片库" })).toBeInTheDocument();
    expect(screen.getAllByRole("button", { name: "导入图片文件夹" }).length).toBeGreaterThan(0);
    expect(screen.getByText(/浏览过程不会修改原始图片/)).toBeInTheDocument();
    expect(screen.getByText("原图只读")).toBeInTheDocument();
    expect(screen.getByText("语义模型未就绪")).toBeInTheDocument();
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
    const categorySection = screen.getByText("拍摄题材").closest(".panel-section");
    const countBadge = categorySection?.querySelector(".panel-section-heading small");

    expect(countBadge).toHaveClass("is-placeholder");
    await user.click(categoryButton);

    expect(countBadge).not.toHaveClass("is-placeholder");
    expect(countBadge).toHaveTextContent("1");
  });

  it("opens the folder chooser and starts a scan", async () => {
    const user = userEvent.setup();
    api.chooseLibraryFolder.mockResolvedValue("C:\\fixtures\\emoji 😀");
    render(<App />);
    await screen.findByRole("heading", { name: "建立本地图片库" });

    await user.click(screen.getAllByRole("button", { name: "导入图片文件夹" })[0]);

    expect(api.chooseLibraryFolder).toHaveBeenCalledOnce();
    expect(screen.getByRole("dialog", { name: "确认导入方式" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "开始导入" }));
    expect(api.startLibraryScan).toHaveBeenCalledWith("C:\\fixtures\\emoji 😀", {
      includeSubfolders: false,
    });
    expect(await screen.findByText("准备图库")).toBeInTheDocument();
  });

  it("restores a library, renders the grid, and opens details", async () => {
    const user = userEvent.setup();
    api.fetchSemanticStatus.mockResolvedValue({
      status: "ready",
      message: "ready",
      model: {
        name: "TinyCLIP",
        version: "test",
        analysisVersion: "test",
        license: "MIT",
        installed: true,
        modelSizeBytes: 24_281_512,
        modelSha256: "test",
        supportedBackends: ["cpu"],
      },
      selectedBackend: "cpu",
    });
    api.fetchLibraries.mockResolvedValue([library]);
    api.fetchAssets.mockResolvedValue({ items: [asset], total: 1, page: 1, pageSize: 200 });
    render(<App />);

    const assetButton = await screen.findByRole("button", { name: "晚霞.png" });
    expect(await screen.findByText("1200 × 800")).toBeInTheDocument();
    expect(assetButton).toHaveAttribute("aria-pressed", "false");
    await user.click(assetButton);

    expect(screen.getByRole("complementary", { name: "图片详情" })).toBeInTheDocument();
    expect(screen.getByText("直方图")).toBeInTheDocument();
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
    expect(await screen.findByRole("button", { name: "分析" })).toBeInTheDocument();
    await waitFor(() => expect(assetButton).toHaveAttribute("aria-pressed", "true"));
    expect(screen.getByText("1200 × 800")).toBeInTheDocument();
    expect(screen.getByText("#D76A52")).toBeInTheDocument();
    await waitFor(() => expect(api.fetchThumbnail).toHaveBeenCalledWith(12));
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
    await user.click(screen.getByRole("button", { name: "查找与审阅" }));

    expect(screen.getByRole("region", { name: "查找与审阅" })).toBeInTheDocument();
    expect(screen.getByText("QUERY / REVIEW CONTEXT")).toBeInTheDocument();
    expect(screen.getByText("当前查询")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Faces" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "返回图库" })).toBeInTheDocument();
  });

  it("applies Lightroom-style marks to one image or the selected images", async () => {
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

    await user.click(within(firstShell as HTMLElement).getByRole("button", { name: "4 星" }));
    expect(api.updateAssetRating).toHaveBeenCalledWith(12, 4);
    expect(firstShell).toHaveClass("has-rating", "rating-4");

    await user.click(screen.getByRole("button", { name: "选择 晚霞.png" }));
    fireEvent.click(screen.getByRole("button", { name: "选择 海边.png" }), { ctrlKey: true });
    await user.click(within(firstShell as HTMLElement).getByRole("button", { name: "蓝色" }));

    expect(api.updateAssetColorLabel).toHaveBeenCalledWith(12, "blue");
    expect(api.updateAssetColorLabel).toHaveBeenCalledWith(13, "blue");
    expect(firstShell).toHaveClass("has-color-label", "color-label-blue");
    expect(secondShell).toHaveClass("has-color-label", "color-label-blue");
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
    expect(manualMarkBar.closest(".grid-workspace-shell")).not.toBeNull();
    expect(manualMarkBar.closest(".sidebar-filter-area")).toBeNull();
    const thirdStar = within(manualMarkBar).getByRole("button", { name: "3 星及以上" });
    const secondStar = within(manualMarkBar).getByRole("button", { name: "2 星及以上" });
    const fourthStar = within(manualMarkBar).getByRole("button", { name: "4 星及以上" });

    await user.click(thirdStar);
    expect(thirdStar).toHaveAttribute("aria-pressed", "true");
    expect(secondStar).toHaveAttribute("aria-pressed", "false");
    expect(fourthStar).toHaveAttribute("aria-pressed", "false");
    expect(thirdStar).toHaveClass("is-active");
    expect(secondStar).toHaveClass("is-active");
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
    expect(manualMarkBar.closest(".grid-workspace-shell")).not.toBeNull();
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
    const importButton = screen.getByRole("button", { name: "＋ 导入图库" });
    expect(importButton).toBeInTheDocument();
    expect(importButton.closest(".panel-titlebar")).not.toBeNull();
    expect(document.querySelector(".library-area-heading")).toBeNull();
    expect(await screen.findByText("子图库")).toBeInTheDocument();
  });

  it("updates scan progress and sends cancellation", async () => {
    const user = userEvent.setup();
    api.chooseLibraryFolder.mockResolvedValue("C:\\fixtures\\scan");
    render(<App />);
    await screen.findByRole("heading", { name: "建立本地图片库" });
    await user.click(screen.getAllByRole("button", { name: "导入图片文件夹" })[0]);

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
    fireEvent.pointerMove(targetRow as HTMLElement, {
      pointerId: 2,
      clientX: 30,
      clientY: 30,
    });
    expect(sourceButton?.closest(".library-tree-row")).toHaveClass("is-dragging");
    expect(targetRow).toHaveClass("is-drag-over");
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
    await screen.findByRole("heading", { name: "建立本地图片库" });
    await user.click(screen.getAllByRole("button", { name: "导入图片文件夹" })[0]);

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

  it("loads all grid results on one surface without gallery pagination", async () => {
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
    expect(await screen.findByRole("button", { name: "海边.png" })).toBeInTheDocument();
    expect(screen.queryByRole("navigation", { name: "图库分页" })).not.toBeInTheDocument();
    expect(api.fetchAssets).toHaveBeenCalledWith(expect.objectContaining({ page: 2 }));
  });

  it("surfaces startup errors without hiding the import affordance", async () => {
    api.fetchLibraries.mockRejectedValue(new Error("database unavailable"));
    render(<App />);
    expect(await screen.findByRole("alert")).toHaveTextContent("database unavailable");
    expect(screen.getAllByRole("button", { name: "导入图片文件夹" }).length).toBeGreaterThan(0);
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
    expect(selectionActions.nextElementSibling).toHaveClass("segmented");
    expect(screen.queryByRole("button", { name: "分析选中" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "整理预览" })).toHaveClass("primary-action");
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
    expect(
      screen.getByRole("toolbar", { name: "人工标记筛选" }).closest(".single-workspace"),
    ).not.toBeNull();
    expect(document.querySelector(".photo-app")).not.toHaveClass("has-batch-classification");
    const singleSelection = screen.getByRole("button", { name: "选择 晚霞.png" });
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
      expect.objectContaining({ page: 1, pageSize: 500 }),
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
    expect(leftHandle).toHaveClass("panel-resize-handle-left");
    expect(rightHandle).toHaveClass("panel-resize-handle-right");
    expect(leftHandle).toHaveAttribute("aria-valuenow", "270");
    expect(rightHandle).toHaveAttribute("aria-valuenow", "320");

    act(() => fireEvent.keyDown(leftHandle, { key: "ArrowRight" }));
    expect(leftHandle).toHaveAttribute("aria-valuenow", "286");

    act(() => fireEvent.keyDown(rightHandle, { key: "ArrowLeft" }));
    expect(rightHandle).toHaveAttribute("aria-valuenow", "336");
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
});
