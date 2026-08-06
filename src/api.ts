import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";

import type {
  AssetPage,
  AssetFilter,
  FolderSummary,
  LibrarySummary,
  ScanProgress,
  SemanticGroupSummary,
  SemanticLabelDescriptor,
  SemanticProgress,
  SemanticRuntimeStatus,
  SortDirection,
  SortField,
} from "./types";
import type { VisualFixture } from "./test/visual-fixture";

const desktopRuntime = isTauri();
const loadVisualFixture = import.meta.env.DEV
  ? async (): Promise<LoadedVisualFixture | null> => {
      const tools = await import("./test/visual-fixture");
      const fixture = tools.visualFixtureFromSearch(window.location.search);
      return fixture ? { fixture, tools } : null;
    }
  : null;
let visualFixturePromise: Promise<LoadedVisualFixture | null> | null = null;

interface LoadedVisualFixture {
  fixture: VisualFixture;
  tools: typeof import("./test/visual-fixture");
}

function getVisualFixture(): Promise<LoadedVisualFixture | null> {
  visualFixturePromise ??= loadVisualFixture?.() ?? Promise.resolve(null);
  return visualFixturePromise;
}

export async function chooseLibraryFolder(): Promise<string | null> {
  if (!desktopRuntime) return null;
  const selection = await open({
    directory: true,
    multiple: false,
    title: "选择要导入的图片文件夹",
  });
  return typeof selection === "string" ? selection : null;
}

export async function fetchLibraries(): Promise<LibrarySummary[]> {
  const visual = await getVisualFixture();
  if (visual?.fixture.startupError) throw new Error(visual.fixture.startupError);
  if (visual) return visual.fixture.libraries;
  if (!desktopRuntime) return [];
  return invoke<LibrarySummary[]>("list_libraries");
}

export async function fetchAssets(options: {
  libraryId: number;
  sort: SortField;
  direction: SortDirection;
  page: number;
  pageSize?: number;
  filter: AssetFilter;
}): Promise<AssetPage> {
  const visual = await getVisualFixture();
  if (visual) {
    return visual.tools.fixtureAssetPage(visual.fixture, {
      sort: options.sort,
      direction: options.direction,
      page: options.page,
      pageSize: options.pageSize ?? 200,
      filter: options.filter,
    });
  }
  if (!desktopRuntime) {
    return { items: [], total: 0, page: options.page, pageSize: options.pageSize ?? 200 };
  }
  return invoke<AssetPage>("list_assets", {
    libraryId: options.libraryId,
    sort: options.sort,
    direction: options.direction,
    page: options.page,
    pageSize: options.pageSize ?? 200,
    filter: options.filter,
  });
}

export async function startLibraryScan(rootPath: string): Promise<{ taskId: string }> {
  if (!desktopRuntime) throw new Error("文件夹导入仅在 PhotoOrganizer 桌面应用中可用。");
  return invoke<{ taskId: string }>("start_scan", { rootPath });
}

export async function cancelLibraryScan(
  taskId: string,
): Promise<{ taskId: string; accepted: boolean }> {
  if (!desktopRuntime) return { taskId, accepted: false };
  return invoke<{ taskId: string; accepted: boolean }>("cancel_scan", { taskId });
}

export async function fetchThumbnail(assetId: number): Promise<string> {
  const visual = await getVisualFixture();
  if (visual) return visual.tools.fixtureThumbnail(assetId);
  if (!desktopRuntime) throw new Error(`桌面缩略图 ${assetId} 在浏览器预览中不可用。`);
  return invoke<string>("get_thumbnail_data_url", { assetId });
}

export async function fetchSemanticStatus(): Promise<SemanticRuntimeStatus> {
  const visual = await getVisualFixture();
  if (visual) return visual.fixture.semanticStatus;
  if (!desktopRuntime) {
    return {
      status: "model_unavailable",
      message: "浏览器预览不加载本地模型。",
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
    };
  }
  return invoke<SemanticRuntimeStatus>("get_semantic_status");
}

export async function prepareSemanticModel(): Promise<SemanticRuntimeStatus> {
  if (!desktopRuntime) throw new Error("模型准备仅在桌面应用中可用。");
  return invoke<SemanticRuntimeStatus>("prepare_semantic_model");
}

export async function fetchSemanticCatalog(): Promise<SemanticLabelDescriptor[]> {
  const visual = await getVisualFixture();
  if (visual) return visual.fixture.semanticCatalog;
  if (!desktopRuntime) return [];
  return invoke<SemanticLabelDescriptor[]>("get_semantic_catalog");
}

export async function fetchLibraryFolders(libraryId: number): Promise<FolderSummary[]> {
  const visual = await getVisualFixture();
  if (visual) return visual.fixture.folders;
  if (!desktopRuntime) return [];
  return invoke<FolderSummary[]>("list_library_folders", { libraryId });
}

export async function fetchSemanticGroups(libraryId: number): Promise<SemanticGroupSummary[]> {
  const visual = await getVisualFixture();
  if (visual) return visual.fixture.semanticGroups;
  if (!desktopRuntime) return [];
  return invoke<SemanticGroupSummary[]>("list_semantic_groups", { libraryId });
}

export async function fetchSemanticProgress(libraryId: number): Promise<SemanticProgress | null> {
  const visual = await getVisualFixture();
  if (visual) return visual.fixture.semanticProgress;
  if (!desktopRuntime) return null;
  return invoke<SemanticProgress | null>("get_semantic_progress", { libraryId });
}

export async function startSemanticAnalysis(
  libraryId: number,
  force = false,
): Promise<{ jobId: string }> {
  if (!desktopRuntime) throw new Error("语义分析仅在桌面应用中可用。");
  return invoke<{ jobId: string }>("start_semantic_analysis", { libraryId, force });
}

export async function reanalyzeAsset(
  libraryId: number,
  assetId: number,
): Promise<{ jobId: string }> {
  if (!desktopRuntime) throw new Error("语义分析仅在桌面应用中可用。");
  return invoke<{ jobId: string }>("reanalyze_asset", { libraryId, assetId });
}

export async function pauseSemanticAnalysis(jobId: string) {
  if (!desktopRuntime) return { jobId, accepted: false };
  return invoke<{ jobId: string; accepted: boolean }>("pause_semantic_analysis", { jobId });
}

export async function resumeSemanticAnalysis(jobId: string) {
  if (!desktopRuntime) return { jobId, accepted: false };
  return invoke<{ jobId: string; accepted: boolean }>("resume_semantic_analysis", { jobId });
}

export async function cancelSemanticAnalysis(jobId: string) {
  if (!desktopRuntime) return { jobId, accepted: false };
  return invoke<{ jobId: string; accepted: boolean }>("cancel_semantic_analysis", { jobId });
}

export async function subscribeScanProgress(
  onProgress: (progress: ScanProgress) => void,
): Promise<UnlistenFn> {
  const visual = await getVisualFixture();
  if (visual?.fixture.progress) {
    const progress = visual.fixture.progress;
    const timer = window.setTimeout(() => onProgress(progress), 80);
    return () => window.clearTimeout(timer);
  }
  if (!desktopRuntime) return () => undefined;
  return listen<ScanProgress>("scan-progress", (event) => onProgress(event.payload));
}

export async function subscribeSemanticProgress(
  onProgress: (progress: SemanticProgress) => void,
): Promise<UnlistenFn> {
  const visual = await getVisualFixture();
  if (visual?.fixture.semanticProgress) {
    const progress = visual.fixture.semanticProgress;
    const timer = window.setTimeout(() => onProgress(progress), 100);
    return () => window.clearTimeout(timer);
  }
  if (!desktopRuntime) return () => undefined;
  return listen<SemanticProgress>("semantic-progress", (event) => onProgress(event.payload));
}
