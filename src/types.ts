export type SortField =
  "file_name" | "capture_time" | "modified_time" | "brightness" | "saturation";

export type SortDirection = "asc" | "desc";
export type SemanticMatchMode = "any" | "all";
export type ViewMode = "grid" | "single";

export interface LibrarySummary {
  id: number;
  rootPath: string;
  createdAt: string;
  lastScanAt: string | null;
  status: string;
  assetCount: number;
  presentCount: number;
  missingCount: number;
}

export interface SemanticLabelResult {
  labelId: string;
  displayName: string;
  similarity: number;
  threshold: number;
  modelName: string;
  modelVersion: string;
  analysisVersion: string;
  analyzedAt: string;
  isManual: boolean;
  isPrimary: boolean;
}

export interface AssetListItem {
  id: number;
  libraryId: number;
  absolutePath: string;
  relativePath: string;
  fileName: string;
  extension: string;
  fileSize: number;
  modifiedAt: number;
  width: number | null;
  height: number | null;
  orientation: number | null;
  captureTime: string | null;
  cameraMake: string | null;
  cameraModel: string | null;
  lensModel: string | null;
  exposureTime: string | null;
  aperture: number | null;
  iso: number | null;
  focalLength: number | null;
  fileStatus: string;
  scanStatus: string;
  analysisStatus: string;
  errorMessage: string | null;
  thumbnailAvailable: boolean;
  brightness: number | null;
  contrast: number | null;
  toneLabel: string | null;
  saturation: number | null;
  saturationLabel: string | null;
  dominantColor: string | null;
  dominantColorCategory: string | null;
  semanticStatus: string;
  semanticError: string | null;
  semanticAnalyzedAt: string | null;
  semanticLabels: SemanticLabelResult[];
}

export interface AssetPage {
  items: AssetListItem[];
  total: number;
  page: number;
  pageSize: number;
}

export interface AssetFilter {
  search: string | null;
  semanticLabels: string[];
  semanticMatch: SemanticMatchMode;
  toneLabels: string[];
  colorCategories: string[];
  brightnessMin: number | null;
  brightnessMax: number | null;
  saturationMin: number | null;
  saturationMax: number | null;
  capturedFrom: string | null;
  capturedTo: string | null;
  folderPrefix: string | null;
  semanticState: "not_analyzed" | "failed" | null;
}

export const emptyAssetFilter: AssetFilter = {
  search: null,
  semanticLabels: [],
  semanticMatch: "any",
  toneLabels: [],
  colorCategories: [],
  brightnessMin: null,
  brightnessMax: null,
  saturationMin: null,
  saturationMax: null,
  capturedFrom: null,
  capturedTo: null,
  folderPrefix: null,
  semanticState: null,
};

export interface FolderSummary {
  relativePath: string;
  assetCount: number;
}

export interface SemanticGroupSummary {
  labelId: string;
  displayName: string;
  assetCount: number;
}

export interface ScanProgress {
  taskId: string;
  libraryId: number | null;
  status: "running" | "completed" | "cancelled" | "failed" | string;
  stage: string;
  discovered: number;
  processed: number;
  succeeded: number;
  failed: number;
  skipped: number;
  missing: number;
  currentPath: string | null;
  error: string | null;
}

export interface ModelMetadata {
  name: string;
  version: string;
  analysisVersion: string;
  license: string | null;
  installed: boolean;
  modelSizeBytes: number | null;
  modelSha256: string | null;
  supportedBackends: string[];
}

export interface SemanticRuntimeStatus {
  status: string;
  message: string;
  model: ModelMetadata;
  selectedBackend: string | null;
}

export interface SemanticLabelDescriptor {
  id: string;
  displayName: string;
  threshold: number;
}

export interface SemanticProgress {
  jobId: string;
  libraryId: number;
  status: string;
  total: number;
  processed: number;
  completed: number;
  failed: number;
  skipped: number;
  currentAssetId: number | null;
  currentPath: string | null;
  executionBackend: string | null;
  modelName: string;
  modelVersion: string;
  error: string | null;
}
