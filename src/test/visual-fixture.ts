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
  subjectStatus?: import("../types").SubjectRuntimeStatus;
  semanticCatalog: SemanticLabelDescriptor[];
  semanticGroups: SemanticGroupSummary[];
  folders: FolderSummary[];
  startupError: string | null;
}

const rootPath = "C:\\test-data\\专业界面验收图库";

const semanticCatalog: SemanticLabelDescriptor[] = [
  ["photo_landscape", "风光自然"],
  ["photo_urban", "城市街拍"],
  ["photo_architecture", "建筑与空间"],
  ["photo_food", "美食餐饮"],
  ["photo_commercial", "商业与静物"],
  ["photo_indoor", "室内与生活"],
  ["photo_travel", "旅行人文"],
  ["photo_event", "活动与运动"],
  ["photo_transport", "交通与汽车"],
  ["photo_plant", "植物与园艺"],
  ["photo_documentary", "纪实与工业"],
  ["indoor", "室内"],
  ["outdoor", "室外"],
  ["person", "人物"],
  ["group", "多人"],
  ["portrait", "人像"],
  ["animal", "动物"],
  ["pet", "宠物"],
  ["vehicle", "车辆"],
  ["food", "食品"],
  ["plant", "植物"],
].map(([id, displayName]) => ({
  id,
  displayName,
  categoryGroup: id.startsWith("photo_")
    ? "scene"
    : ["person", "group", "portrait", "animal", "pet", "vehicle", "food", "plant"].includes(id)
      ? "subject"
      : "context",
  threshold: id === "portrait" ? 0.65 : 0.16,
  isPrimaryCategory: id.startsWith("photo_"),
  taxonomyVersion:
    id.startsWith("photo_") || id === "indoor" || id === "outdoor"
      ? "photo-organizer-photography-topics-v1"
      : "photo-organizer-subject-tags-v1",
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
            modelName: "Places365-ResNet18",
            modelVersion: "onnx-2026-08-10",
            error: null,
          }
        : null,
    semanticStatus: {
      status: "ready",
      message:
        "Places365 ResNet-18 已就绪；拍摄题材使用本地 365 类模型，向量搜索使用本地 TinyCLIP。",
      model: {
        name: "Places365-ResNet18",
        version: "onnx-2026-08-10",
        analysisVersion: "photo-organizer-semantic-places365-photography-v1",
        license: "MIT",
        installed: true,
        modelSizeBytes: 45_575_731,
        modelSha256: "3c3cd0d42693e2957fcaa0bc365ce78e169a2e1162356742adfbd11077e8f7bf",
        supportedBackends: ["cpu"],
      },
      selectedBackend: "cpu",
    },
    subjectStatus: {
      status: "ready",
      message: "PicoDet 主体检测与 YuNet 人像辅助模型均已就绪。",
      model: {
        name: "PicoDet-S-COCO",
        version: "onnx-2026-08-10",
        analysisVersion: "photo-organizer-subject-picodet-yunet-v1",
        license: "Apache-2.0",
        installed: true,
        modelSizeBytes: 4_792_914,
        modelSha256: "09fc88131be8ad224f13739a5cf8fc838600d76a77539af7f0400fa90506c5f3",
        supportedBackends: ["cpu"],
      },
      faceModel: {
        name: "YuNet-FaceDetector",
        version: "onnx-2023mar",
        analysisVersion: "photo-organizer-subject-picodet-yunet-v1",
        license: "MIT",
        installed: true,
        modelSizeBytes: 232_589,
        modelSha256: "8f2383e4dd3cfbb4553ea8718107fc0423210dc964f9f4280604804ed2552fa4",
        supportedBackends: ["cpu"],
      },
      selectedBackend: "cpu",
    },
    semanticCatalog,
    semanticGroups: [
      {
        labelId: "photo_landscape",
        displayName: "风光自然",
        categoryGroup: "scene",
        assetCount: 7,
      },
      {
        labelId: "photo_architecture",
        displayName: "建筑与空间",
        categoryGroup: "scene",
        assetCount: 4,
      },
      { labelId: "photo_food", displayName: "美食餐饮", categoryGroup: "scene", assetCount: 4 },
      { labelId: "outdoor", displayName: "室外", categoryGroup: "context", assetCount: 11 },
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
  const ids: ReadonlyArray<readonly [string, string]> = [
    ["photo_landscape", "风光自然"],
    ["photo_architecture", "建筑与空间"],
    ["photo_food", "美食餐饮"],
    ["photo_plant", "植物与园艺"],
    ["photo_urban", "城市街拍"],
    ["photo_indoor", "室内与生活"],
  ];
  const primary = ids[index % ids.length];
  const secondary = index % 4 === 0 ? (["outdoor", "室外"] as const) : null;
  return [primary, secondary]
    .filter((label): label is readonly [string, string] => label !== null)
    .map(([labelId, displayName], rank) => ({
      labelId,
      displayName,
      similarity: 0.29 - rank * 0.04,
      threshold: 0.16,
      modelName: "Places365-ResNet18",
      modelVersion: "onnx-2026-08-10",
      analysisVersion: "photo-organizer-semantic-places365-photography-v1",
      taxonomyVersion: "photo-organizer-photography-topics-v1",
      analyzedAt: "2026-08-07T03:12:00Z",
      isManual: false,
      isPrimary: rank === 0,
      categoryGroup: labelId.startsWith("photo_") ? "scene" : "context",
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
