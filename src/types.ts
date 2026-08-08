export type SortField =
  "file_name" | "capture_time" | "modified_time" | "brightness" | "saturation";

export type SortDirection = "asc" | "desc";
export type SemanticMatchMode = "any" | "all";
export type ViewMode = "grid" | "single";
export type ClassificationSource = "none" | "auto" | "manual" | "mixed";

export interface ClassificationFieldState<T> {
  auto: T | null;
  manual: T | null;
  effective: T | null;
  source: ClassificationSource;
}

export interface AuxiliaryTagState {
  auto: string[];
  manualAdditions: string[];
  manualRemovals: string[];
  effective: string[];
  source: ClassificationSource;
}

export interface EffectiveClassification {
  revision: number;
  primaryCategory: ClassificationFieldState<string>;
  auxiliaryTags: AuxiliaryTagState;
  tone: ClassificationFieldState<string>;
  dominantColorCategories: ClassificationFieldState<string[]>;
  saturationLevel: ClassificationFieldState<string>;
}

export function emptyEffectiveClassification(revision = 0): EffectiveClassification {
  const emptyField = (): ClassificationFieldState<string> => ({
    auto: null,
    manual: null,
    effective: null,
    source: "none",
  });
  return {
    revision,
    primaryCategory: emptyField(),
    auxiliaryTags: {
      auto: [],
      manualAdditions: [],
      manualRemovals: [],
      effective: [],
      source: "none",
    },
    tone: emptyField(),
    dominantColorCategories: {
      auto: null,
      manual: null,
      effective: null,
      source: "none",
    },
    saturationLevel: emptyField(),
  };
}

export interface ClassificationFieldDescriptor {
  id: string;
  displayName: string;
  kind: "single" | "multi" | string;
  filterable: boolean;
  supportsManualOverride: boolean;
  supportsRestoreAuto: boolean;
}

export interface LibrarySummary {
  id: number;
  rootPath: string;
  name: string;
  sourcePath: string;
  sourceIdentityKey: string;
  parentLibraryId: number | null;
  displayOrder: number;
  createdAt: string;
  lastScanAt: string | null;
  status: string;
  assetCount: number;
  presentCount: number;
  missingCount: number;
  semanticPendingCount: number;
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
  chroma: number | null;
  saturationLabel: string | null;
  dominantColor: string | null;
  dominantColorCategory: string | null;
  neutralRatio: number | null;
  dominantColorCoverage: number | null;
  semanticStatus: string;
  semanticError: string | null;
  semanticAnalyzedAt: string | null;
  semanticLabels: SemanticLabelResult[];
  classification: EffectiveClassification;
}

/** Grid and detail use separate names even while sharing the stable asset fields. */
export type AssetGridItem = AssetListItem;
export type AssetDetail = AssetListItem;

export interface AssetPage {
  items: AssetListItem[];
  total: number;
  page: number;
  pageSize: number;
}

export interface AssetFilter {
  search: string | null;
  primaryCategories: string[];
  auxiliaryTags: string[];
  semanticMatch: SemanticMatchMode;
  toneLabels: string[];
  colorCategories: string[];
  saturationLevels: string[];
  brightnessMin: number | null;
  brightnessMax: number | null;
  saturationMin: number | null;
  saturationMax: number | null;
  capturedFrom: string | null;
  capturedTo: string | null;
  analysisStatus: "not_analyzed" | "failed" | "completed" | null;
}

export const emptyAssetFilter: AssetFilter = {
  search: null,
  primaryCategories: [],
  auxiliaryTags: [],
  semanticMatch: "any",
  toneLabels: [],
  colorCategories: [],
  saturationLevels: [],
  brightnessMin: null,
  brightnessMax: null,
  saturationMin: null,
  saturationMax: null,
  capturedFrom: null,
  capturedTo: null,
  analysisStatus: null,
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

export interface ScanPerformance {
  discoveryUs: number;
  ownershipLookupUs: number;
  metadataLookupUs: number;
  fingerprintUs: number;
  imageProcessingUs: number;
  exifUs: number;
  decodeUs: number;
  resizeUs: number;
  featureAnalysisUs: number;
  thumbnailWriteUs: number;
  databaseWriteUs: number;
  processedFiles: number;
  skippedFiles: number;
  failedFiles: number;
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
  performance?: ScanPerformance;
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
  isPrimaryCategory: boolean;
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

export type OrganizationScope = "all" | "filtered" | "selected";
export type OrganizationLevelKind =
  | "year"
  | "month"
  | "day"
  | "original_directory"
  | "primary_semantic"
  | "tone"
  | "dominant_color"
  | "saturation";
export type OrganizationMissingFallback = "modification_time" | "unknown" | "skip" | "block";
export type OrganizationConflictStrategy = "skip" | "sequence" | "short_hash";

export interface OrganizationLevel {
  kind: OrganizationLevelKind;
  fallback: OrganizationMissingFallback;
}

export interface OrganizationRules {
  version: string;
  levels: OrganizationLevel[];
  template: string;
  sequenceStart: number;
  sequenceWidth: number;
  missingFallback: OrganizationMissingFallback;
  conflictStrategy: OrganizationConflictStrategy;
}

export interface OrganizationPlanRequest {
  libraryId: number;
  targetRoot: string;
  scope: OrganizationScope;
  filter: AssetFilter;
  selectedAssetIds: number[];
  rules: OrganizationRules;
}

export type OrganizationItemStatus = "ready" | "warning" | "error" | "skipped_conflict";
export type OrganizationIssueSeverity = "warning" | "error";

export interface OrganizationIssue {
  code: string;
  severity: OrganizationIssueSeverity;
  sourcePath: string | null;
  targetPath: string | null;
  detail: string;
}

export interface OrganizationPlanItem {
  ordinal: number;
  assetId: number;
  sourcePath: string;
  sourceRelativePath: string;
  sourceFingerprint: string;
  targetRelativePath: string;
  targetPath: string;
  fileSize: number;
  status: OrganizationItemStatus;
  variables: Record<string, string>;
  issues: OrganizationIssue[];
}

export interface OrganizationTreeNode {
  name: string;
  relativePath: string;
  fileCount: number;
  byteCount: number;
  children: OrganizationTreeNode[];
}

export interface OrganizationPlanSummary {
  planId: string;
  libraryId: number;
  sourceRoot: string;
  targetRoot: string;
  scope: OrganizationScope;
  itemCount: number;
  conflictCount: number;
  errorCount: number;
  warningCount: number;
  estimatedBytes: number;
  targetAvailableBytes: number | null;
  generatedAt: string;
  status: string;
  sourceSnapshot: string;
  rules: OrganizationRules;
}

export interface OrganizationPlan {
  summary: OrganizationPlanSummary;
  items: OrganizationPlanItem[];
  tree: OrganizationTreeNode;
}
