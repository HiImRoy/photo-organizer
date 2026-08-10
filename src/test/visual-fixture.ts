import { emptyEffectiveClassification } from "../types";
import type {
  AssetListItem,
  AssetFilter,
  AssetPage,
  FolderSummary,
  LibrarySummary,
  ScanProgress,
  SemanticGroupSummary,
  SemanticLabelDescriptor,
  SemanticProgress,
  SemanticRuntimeStatus,
  SortDirection,
  SortField,
} from "../types";

export type VisualFixtureScenario = "library" | "scanning" | "error";

export interface VisualFixture {
  scenario: VisualFixtureScenario;
  libraries: LibrarySummary[];
  assets: AssetListItem[];
  progress: ScanProgress | null;
  semanticProgress: SemanticProgress | null;
  semanticStatus: SemanticRuntimeStatus;
  semanticCatalog: SemanticLabelDescriptor[];
  semanticGroups: SemanticGroupSummary[];
  folders: FolderSummary[];
  startupError: string | null;
}

const rootPath = "C:\\test-data\\专业界面验收图库";

const semanticCatalog: SemanticLabelDescriptor[] = [
  ["portrait", "人像"],
  ["group", "多人"],
  ["landscape", "风景"],
  ["architecture", "建筑"],
  ["indoor", "室内"],
  ["street", "街道"],
  ["vehicle", "车辆"],
  ["product", "静物"],
  ["food", "食品"],
  ["animal", "动物"],
  ["document", "文档"],
  ["night", "夜景"],
  ["flower", "花卉"],
  ["abstract", "抽象"],
].map(([id, displayName]) => ({
  id,
  displayName,
  categoryGroup: [
    "portrait",
    "group",
    "landscape",
    "architecture",
    "product",
    "food",
    "animal",
    "abstract",
  ].includes(id)
    ? "scene"
    : ["vehicle", "flower", "mountain", "water", "forest"].includes(id)
      ? "subject"
      : "context",
  threshold: 0.16,
  isPrimaryCategory: [
    "portrait",
    "group",
    "landscape",
    "architecture",
    "product",
    "food",
    "animal",
    "document",
    "abstract",
  ].includes(id),
  taxonomyVersion: "photo-organizer-taxonomy-v2",
}));

const library: LibrarySummary = {
  id: 9100,
  rootPath,
  name: "专业界面验收图库",
  sourcePath: rootPath,
  sourceIdentityKey: "c:/test-data/专业界面验收图库",
  parentLibraryId: null,
  displayOrder: 0,
  createdAt: "2026-08-01T08:00:00Z",
  lastScanAt: "2026-08-07T02:42:00Z",
  status: "ready",
  assetCount: 18,
  presentCount: 18,
  missingCount: 0,
  semanticPendingCount: 7,
};

const names = [
  "青岛海岸_晨雾_001.jpg",
  "建筑立面_几何阴影_002.webp",
  "产品静物_蓝色玻璃_003.png",
  "山谷与云层_004.jpg",
  "城市夜景_雨后街道_005.jpg",
  "森林步道_006.webp",
  "白色陶瓷器皿_007.png",
  "海面与防波堤_008.jpg",
  "展厅产品拍摄_超长文件名用于验证省略与布局稳定性_009.jpg",
  "红色建筑细节_010.webp",
  "湖畔薄雾_011.jpg",
  "工作室布光测试_012.png",
  "沙丘纹理_013.jpg",
  "室内空间_014.webp",
  "蓝调时刻_015.jpg",
  "玻璃幕墙_016.png",
  "岩石与海浪_017.jpg",
  "深色产品背景_018.webp",
] as const;

const palettes = [
  ["#94a8b8", "#d6dfe4", "#3e6273", "#d9b27d"],
  ["#64717e", "#c9d0d5", "#303842", "#9fa9b1"],
  ["#426b86", "#b8d0dc", "#244254", "#d8e2e7"],
  ["#81919c", "#d9d9d2", "#465860", "#b39a78"],
  ["#263746", "#657b8b", "#18242e", "#bb785e"],
  ["#516b5b", "#aab8a8", "#263b31", "#d4c6a7"],
] as const;

const assets: AssetListItem[] = names.map((fileName, index) => {
  const palette = palettes[index % palettes.length];
  const extension = fileName.split(".").at(-1) ?? "jpg";
  const labels = semanticLabelsFor(index);
  const toneLabel = index % 3 === 0 ? "low_key" : index % 3 === 1 ? "balanced" : "high_key";
  const saturationLabel = index % 3 === 0 ? "low" : index % 3 === 1 ? "medium" : "high";
  const dominantColorCategory = ["blue", "gray", "cyan", "orange"][index % 4];
  return {
    id: 9200 + index,
    libraryId: library.id,
    absolutePath: `${rootPath}\\${fileName}`,
    relativePath: fileName,
    fileName,
    extension,
    fileSize: 1_800_000 + index * 183_271,
    modifiedAt: Date.parse(
      `2026-08-${String((index % 6) + 1).padStart(2, "0")}T${String(8 + (index % 10)).padStart(2, "0")}:24:00Z`,
    ),
    width: index % 3 === 0 ? 4032 : 3000,
    height: index % 3 === 0 ? 3024 : 2250,
    orientation: 1,
    captureTime: `2026-08-${String((index % 6) + 1).padStart(2, "0")}T${String(6 + (index % 12)).padStart(2, "0")}:18:00`,
    cameraMake: index % 2 === 0 ? "FUJIFILM" : "SONY",
    cameraModel: index % 2 === 0 ? "X-T5" : "ILCE-7M4",
    lensModel: index % 2 === 0 ? "XF16-55mmF2.8" : "FE 35mm F1.4 GM",
    exposureTime: index % 4 === 0 ? "1/125" : "1/250",
    aperture: index % 3 === 0 ? 2.8 : 5.6,
    iso: index % 5 === 0 ? 800 : 200,
    focalLength: index % 2 === 0 ? 35 : 50,
    fileStatus: "present",
    scanStatus: "indexed",
    analysisStatus: "completed",
    errorMessage: null,
    thumbnailAvailable: true,
    brightness: 0.28 + ((index * 7) % 58) / 100,
    contrast: 0.31 + ((index * 5) % 43) / 100,
    toneLabel,
    saturation: 0.22 + ((index * 9) % 54) / 100,
    chroma: 0.18 + ((index * 11) % 58) / 100,
    saturationLabel,
    dominantColor: palette[index % palette.length],
    dominantColorCategory,
    neutralRatio: 0.18,
    dominantColorCoverage: 0.52,
    semanticStatus: "completed",
    semanticError: null,
    semanticAnalyzedAt: "2026-08-07T03:12:00Z",
    rating: index === 0 ? 4 : index === 5 ? 2 : 0,
    colorLabel: index === 0 ? "red" : index === 4 ? "blue" : null,
    semanticLabels: labels,
    classification: fixtureClassification(
      labels,
      toneLabel,
      dominantColorCategory,
      saturationLabel,
    ),
  };
});

export function visualFixtureFromSearch(search: string): VisualFixture | null {
  const value = new URLSearchParams(search).get("visual-fixture");
  if (!isScenario(value)) return null;

  return {
    scenario: value,
    libraries: value === "error" ? [] : [library],
    assets: value === "error" ? [] : assets,
    progress:
      value === "scanning"
        ? {
            taskId: "visual-fixture-scan",
            libraryId: library.id,
            status: "running",
            stage: "processing",
            discovered: 124,
            processed: 74,
            succeeded: 71,
            failed: 1,
            skipped: 2,
            missing: 0,
            currentPath: `${rootPath}\\待处理\\建筑立面_075.jpg`,
            error: null,
          }
        : null,
    semanticProgress:
      value === "scanning"
        ? {
            jobId: "visual-semantic-job",
            libraryId: library.id,
            status: "running",
            total: 18,
            processed: 11,
            completed: 11,
            failed: 0,
            skipped: 0,
            currentAssetId: 9211,
            currentPath: `${rootPath}\\工作室布光测试_012.png`,
            executionBackend: "cpu",
            modelName: "TinyCLIP-ViT-8M-16-Text-3M-YFCC15M",
            modelVersion: "onnx-int8-2025-08-06",
            error: null,
          }
        : null,
    semanticStatus: {
      status: "ready",
      message: "TinyCLIP INT8 已通过完整性校验，CPU 执行后端可用。",
      model: {
        name: "TinyCLIP-ViT-8M-16-Text-3M-YFCC15M",
        version: "onnx-int8-2025-08-06",
        analysisVersion: "photo-organizer-semantic-v1",
        license: "MIT",
        installed: true,
        modelSizeBytes: 24_281_512,
        modelSha256: "10921310ddef06557ec1598d1260470a0a4db53f70ffe0deb60b946dcad6d27a",
        supportedBackends: ["cpu"],
      },
      selectedBackend: "cpu",
    },
    semanticCatalog,
    semanticGroups: [
      { labelId: "landscape", displayName: "风景", categoryGroup: "scene", assetCount: 7 },
      { labelId: "architecture", displayName: "建筑", categoryGroup: "scene", assetCount: 4 },
      { labelId: "product", displayName: "产品", categoryGroup: "scene", assetCount: 4 },
      { labelId: "night", displayName: "夜景", categoryGroup: "context", assetCount: 3 },
    ],
    folders: [
      { relativePath: "", assetCount: 18 },
      { relativePath: "精选", assetCount: 8 },
      { relativePath: "待交付", assetCount: 6 },
    ],
    startupError: value === "error" ? "测试夹具：无法打开本地索引，请检查应用日志。" : null,
  };
}

export function fixtureAssetPage(
  fixture: VisualFixture,
  options: {
    sort: SortField;
    direction: SortDirection;
    page: number;
    pageSize: number;
    filter: AssetFilter;
  },
): AssetPage {
  const filtered = fixture.assets.filter((asset) => matchesFilter(asset, options.filter));
  const sorted = [...filtered].sort((left, right) => {
    const comparison = compareAsset(left, right, options.sort);
    if (comparison !== 0) return options.direction === "asc" ? comparison : -comparison;
    return left.fileName.localeCompare(right.fileName, "zh-CN");
  });
  const offset = (options.page - 1) * options.pageSize;
  return {
    items: sorted.slice(offset, offset + options.pageSize),
    total: sorted.length,
    page: options.page,
    pageSize: options.pageSize,
  };
}

function semanticLabelsFor(index: number) {
  const ids = [
    ["landscape", "风景"],
    ["architecture", "建筑"],
    ["product", "静物"],
    ["mountain", "山"],
    ["night", "夜景"],
    ["forest", "森林"],
  ] as const;
  const primary = ids[index % ids.length];
  const secondary = index % 4 === 0 && primary[0] !== "night" ? (["night", "夜景"] as const) : null;
  return [primary, secondary]
    .filter((label): label is (typeof ids)[number] => label !== null)
    .map(([labelId, displayName], rank) => ({
      labelId,
      displayName,
      similarity: 0.29 - rank * 0.04,
      threshold: 0.16,
      modelName: "TinyCLIP-ViT-8M-16-Text-3M-YFCC15M",
      modelVersion: "onnx-int8-2025-08-06",
      analysisVersion: "photo-organizer-semantic-v2",
      taxonomyVersion: "photo-organizer-taxonomy-v2",
      analyzedAt: "2026-08-07T03:12:00Z",
      isManual: false,
      isPrimary: rank === 0,
      categoryGroup: ["landscape", "architecture", "product"].includes(labelId)
        ? "scene"
        : ["mountain", "forest"].includes(labelId)
          ? "subject"
          : "context",
    }));
}

function fixtureClassification(
  labels: ReturnType<typeof semanticLabelsFor>,
  tone: string,
  color: string,
  saturation: string,
) {
  const result = emptyEffectiveClassification();
  const primary = labels.find((label) => label.isPrimary)?.labelId ?? null;
  const auxiliary = labels.filter((label) => !label.isPrimary).map((label) => label.labelId);
  result.primaryCategory = {
    auto: primary,
    manual: null,
    effective: primary,
    source: primary ? "auto" : "none",
  };
  result.auxiliaryTags = {
    auto: auxiliary,
    manualAdditions: [],
    manualRemovals: [],
    effective: auxiliary,
    source: auxiliary.length ? "auto" : "none",
  };
  result.tone = { auto: tone, manual: null, effective: tone, source: "auto" };
  result.dominantColorCategories = {
    auto: [color],
    manual: null,
    effective: [color],
    source: "auto",
  };
  result.saturationLevel = {
    auto: saturation,
    manual: null,
    effective: saturation,
    source: "auto",
  };
  return result;
}

function matchesFilter(asset: AssetListItem, filter: AssetFilter) {
  if (filter.search) {
    const search = filter.search.toLocaleLowerCase();
    if (!asset.fileName.toLocaleLowerCase().includes(search)) return false;
  }
  if (filter.analysisStatus && asset.semanticStatus !== filter.analysisStatus) return false;
  const ratingThreshold = filter.ratings.length > 0 ? Math.max(...filter.ratings) : null;
  if (ratingThreshold !== null && asset.rating < ratingThreshold) return false;
  if (filter.colorLabels.length > 0) {
    if (!asset.colorLabel || !filter.colorLabels.includes(asset.colorLabel)) return false;
  }
  if (filter.primaryCategories.length > 0) {
    if (!filter.primaryCategories.includes(asset.classification.primaryCategory.effective ?? "")) {
      return false;
    }
  }
  if (filter.auxiliaryTags.length > 0) {
    const actual = new Set(asset.classification.auxiliaryTags.effective);
    const matches = filter.auxiliaryTags.map((label) => actual.has(label));
    if (filter.semanticMatch === "all" ? !matches.every(Boolean) : !matches.some(Boolean)) {
      return false;
    }
  }
  if (filter.toneLabels.length > 0 && !filter.toneLabels.includes(asset.toneLabel ?? "")) {
    return false;
  }
  if (
    filter.colorCategories.length > 0 &&
    !(asset.classification.dominantColorCategories.effective ?? []).some((value) =>
      filter.colorCategories.includes(value),
    )
  ) {
    return false;
  }
  if (
    filter.saturationLevels.length > 0 &&
    !filter.saturationLevels.includes(asset.classification.saturationLevel.effective ?? "")
  ) {
    return false;
  }
  if (filter.brightnessMin !== null && (asset.brightness ?? -1) < filter.brightnessMin)
    return false;
  if (filter.brightnessMax !== null && (asset.brightness ?? 2) > filter.brightnessMax) return false;
  if (filter.saturationMin !== null && (asset.saturation ?? -1) < filter.saturationMin)
    return false;
  if (filter.saturationMax !== null && (asset.saturation ?? 2) > filter.saturationMax) return false;
  return true;
}

export function fixtureThumbnail(assetId: number): string {
  const index = Math.max(0, assetId - 9200);
  const palette = palettes[index % palettes.length];
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="640" height="480" viewBox="0 0 640 480">
    <rect width="640" height="480" fill="${palette[0]}"/>
    <rect y="300" width="640" height="180" fill="${palette[2]}"/>
    <circle cx="${110 + (index % 4) * 120}" cy="${96 + (index % 3) * 34}" r="58" fill="${palette[3]}" opacity="0.88"/>
    <path d="M0 340 L150 ${170 + (index % 5) * 18} L268 310 L410 ${150 + (index % 4) * 25} L640 330 V480 H0 Z" fill="${palette[1]}" opacity="0.92"/>
    <path d="M0 390 L190 250 L322 360 L500 230 L640 350 V480 H0 Z" fill="${palette[2]}" opacity="0.78"/>
  </svg>`;
  return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;
}

function isScenario(value: string | null): value is VisualFixtureScenario {
  return value === "library" || value === "scanning" || value === "error";
}

function compareAsset(left: AssetListItem, right: AssetListItem, sort: SortField): number {
  switch (sort) {
    case "capture_time":
      return (left.captureTime ?? "").localeCompare(right.captureTime ?? "");
    case "modified_time":
      return left.modifiedAt - right.modifiedAt;
    case "brightness":
      return (left.brightness ?? -1) - (right.brightness ?? -1);
    case "saturation":
      return (left.saturation ?? -1) - (right.saturation ?? -1);
    case "file_name":
      return left.fileName.localeCompare(right.fileName, "zh-CN");
  }
}
