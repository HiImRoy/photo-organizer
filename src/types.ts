export type SortField =
  "file_name" | "capture_time" | "modified_time" | "brightness" | "saturation";

export type SortDirection = "asc" | "desc";
export type SemanticMatchMode = "any" | "all";
export type ViewMode = "grid" | "single";
export type ClassificationSource = "none" | "auto" | "manual" | "mixed";
export type ManualColorLabel = "red" | "yellow" | "green" | "blue" | "purple";

export const MANUAL_COLOR_LABEL_OPTIONS: ReadonlyArray<{
  id: ManualColorLabel;
  label: string;
  color: string;
}> = [
  { id: "red", label: "红色", color: "#d66b6b" },
  { id: "yellow", label: "黄色", color: "#d7b65f" },
  { id: "green", label: "绿色", color: "#72a37b" },
  { id: "blue", label: "蓝色", color: "#6f94c5" },
  { id: "purple", label: "紫色", color: "#9d7fc1" },
];

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
  categoryGroup: string;
  similarity: number;
  threshold: number;
  modelName: string;
  modelVersion: string;
  analysisVersion: string;
  taxonomyVersion: string;
  analyzedAt: string;
  isManual: boolean;
  isPrimary: boolean;
}

export interface ColorCandidate {
  rank: number;
  color: string;
  category: string;
  areaCoverage: number;
  saliencyCoverage: number;
  localContrast: number;
  chroma: number;
  spatialCoherence: number;
}

export interface ColorPalette {
  algorithmVersion: string;
  coveragePalette: ColorCandidate[];
  prominentPalette: ColorCandidate[];
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
  colorPalette: ColorPalette | null;
  neutralRatio: number | null;
  dominantColorCoverage: number | null;
  semanticStatus: string;
  semanticError: string | null;
  semanticAnalyzedAt: string | null;
  rating: number;
  colorLabel: ManualColorLabel | null;
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

export type AssetGroupField = Exclude<AssetGroupBy, "none">;

export interface AssetGroupSummary {
  key: string;
  label?: string | null;
  uniqueCount: number;
}

export interface AssetGroupPage {
  groups: AssetGroupSummary[];
  groupCount: number;
  globalUniqueTotal: number;
  page: number;
  pageSize: number;
}

/**
 * The serializable membership query for the current browse surface.
 * Presentation state such as grid/single preview is intentionally separate.
 */
export interface AssetQueryV1 {
  version: 1;
  libraryId: number | null;
  filter: AssetFilter;
  sort: SortField;
  direction: SortDirection;
  page: number;
  pageSize: number;
}

export type AssetQueryRoot =
  | { kind: "all" }
  | { kind: "source"; libraryId: number }
  | { kind: "collection"; collectionId: number }
  | { kind: "favorites" };

/** Unified browse contract. V1 remains as a compatibility boundary for old UI state. */
export interface AssetQuery {
  version: 2;
  root: AssetQueryRoot;
  includeDescendants: boolean;
  filter: AssetFilter;
  sort: SortField;
  direction: SortDirection;
  page: number;
  pageSize: number;
}

/** Presentation-only grouping for the current browse surface. */
export type AssetGroupBy =
  | "none"
  | "primary_category"
  | "auxiliary_tag"
  | "tone"
  | "saturation_level"
  | "dominant_color"
  | "rating";

export type AssetScopeInputV1 =
  | { kind: "query"; query: AssetQueryV1 }
  | { kind: "selection"; query: AssetQueryV1; assetIds: number[] };

export interface AssetScopeDescription {
  kind: AssetScopeInputV1["kind"];
  label: string;
  count: number;
  isExplicitSelection: boolean;
}

export interface AssetFilter {
  search: string | null;
  primaryCategories: string[];
  auxiliaryTags: string[];
  semanticMatch: SemanticMatchMode;
  toneLabels: string[];
  colorCategories: string[];
  saturationLevels: string[];
  ratings: number[];
  colorLabels: ManualColorLabel[];
  colorHueCenter: number | null;
  colorHueWidth: number | null;
  /** Strictness mapped to a 4%–32% whole-image target-hue area threshold. */
  colorHueStrictness: number;
  brightnessMin: number | null;
  brightnessMax: number | null;
  saturationMin: number | null;
  saturationMax: number | null;
  capturedFrom: string | null;
  capturedTo: string | null;
  analysisStatus: "not_analyzed" | "failed" | "completed" | null;
  /** V1 compatibility field; AssetQuery v2 expresses this as root=favorites. */
  favoriteOnly: boolean;
  /** V1 compatibility field; AssetQuery v2 expresses this as root=collection. */
  collectionId: number | null;
}

export const emptyAssetFilter: AssetFilter = {
  search: null,
  primaryCategories: [],
  auxiliaryTags: [],
  semanticMatch: "any",
  toneLabels: [],
  colorCategories: [],
  saturationLevels: [],
  ratings: [],
  colorLabels: [],
  colorHueCenter: null,
  colorHueWidth: null,
  colorHueStrictness: 0.5,
  brightnessMin: null,
  brightnessMax: null,
  saturationMin: null,
  saturationMax: null,
  capturedFrom: null,
  capturedTo: null,
  analysisStatus: null,
  favoriteOnly: false,
  collectionId: null,
};

export interface FolderSummary {
  relativePath: string;
  assetCount: number;
}

export interface SemanticGroupSummary {
  labelId: string;
  displayName: string;
  categoryGroup: string;
  assetCount: number;
}

export interface ScanPerformance {
  discoveryUs: number;
  ownershipLookupUs: number;
  metadataLookupUs: number;
  /** Optional stage detail added after the original aggregate-only payload. */
  fileMetadataUs?: number;
  existingAssetLookupUs?: number;
  cacheProbeUs?: number;
  fingerprintUs: number;
  imageProcessingUs: number;
  exifUs: number;
  sourceDimensionUs: number;
  decodeUs: number;
  sourceDecodeUs: number;
  thumbnailDecodeUs: number;
  resizeUs: number;
  featureAnalysisUs: number;
  thumbnailWriteUs: number;
  databaseWriteUs: number;
  coldFiles?: number;
  reanalyzedFiles?: number;
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

export interface GpuProviderStatus {
  id: string;
  state: string;
  message: string;
}

export interface GpuAdapterInfo {
  index: number;
  name: string;
  vendorId: number;
  deviceId: number;
  dedicatedVramBytes: number;
  sharedSystemMemoryBytes: number;
  isSoftware: boolean;
  isDiscreteCandidate: boolean;
}

export interface GpuCapabilities {
  status: string;
  message: string;
  adapters: GpuAdapterInfo[];
  selectedAdapterIndex: number | null;
  dedicatedGpuAvailable: boolean;
  directml: GpuProviderStatus;
  /** Conservative DirectML batch ceiling derived from the selected adapter's dedicated VRAM. */
  recommendedAnalysisBatchSize: number;
}

export interface SemanticRuntimeStatus {
  status: string;
  message: string;
  model: ModelMetadata;
  topicModel?: ModelMetadata | null;
  selectedBackend: string | null;
}

export interface SubjectRuntimeStatus {
  status: string;
  message: string;
  model: ModelMetadata;
  faceModel: ModelMetadata;
  selectedBackend: string | null;
}

export interface SemanticLabelDescriptor {
  id: string;
  displayName: string;
  categoryGroup: string;
  threshold: number;
  isPrimaryCategory: boolean;
  taxonomyVersion: string;
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

export type CopyExportLayout = "preserve_original" | "capture_year_month";
export type CopyExportConflictPolicy = "block" | "skip" | "sequence" | "short_hash";
export type MissingCaptureTimePolicy =
  "block" | "preserve_original_folders" | "unknown_folder" | "skip";

export interface CopyExportRules {
  version: string;
  layout: CopyExportLayout;
  conflictPolicy: CopyExportConflictPolicy;
  missingCaptureTimePolicy: MissingCaptureTimePolicy;
}

export type OrganizationPlanScopeV2 =
  { kind: "query"; query: AssetQuery } | { kind: "explicit_assets"; assetIds: number[] };

export interface OrganizationTargetV2 {
  parentGrantId: string;
  childName: string;
}

export interface CreateOrganizationPlanV2Request {
  scope: OrganizationPlanScopeV2;
  target: OrganizationTargetV2;
  rules: CopyExportRules;
}

export interface TargetParentGrantResponse {
  grantId: string;
  displayPath: string;
}

export type CopyPlanItemStatus = "ready" | "blocked" | "skipped";
export type CopyPlanStatus = "ready" | "blocked" | "stale" | "consumed";

export interface CopyPlanIssue {
  code: string;
  detail: string;
  assetId: number | null;
  sourcePath: string | null;
  targetPath: string | null;
}

export interface OrganizationPlanItemV2 {
  ordinal: number;
  assetId: number;
  libraryId: number;
  sourceRootPath: string;
  sourcePath: string;
  sourceRelativePath: string;
  sourceFingerprintBlake3: string;
  sourceSize: number;
  sourceModifiedAt: number;
  captureTime: string | null;
  classificationRevision: number;
  targetRelativePath: string;
  targetPath: string;
  status: CopyPlanItemStatus;
  issues: CopyPlanIssue[];
}

export interface OrganizationPlanSummaryV2 {
  planId: string;
  revision: string;
  status: CopyPlanStatus;
  scope: OrganizationPlanScopeV2;
  targetRoot: string;
  matchedCount: number;
  eligibleCount: number;
  skippedCount: number;
  blockedCount: number;
  estimatedBytes: number;
  issues: CopyPlanIssue[];
  snapshotBlake3: string;
  createdAt: string;
  rules: CopyExportRules;
  tree: OrganizationTreeNode;
}

export interface OrganizationPlanV2 {
  summary: OrganizationPlanSummaryV2;
  items: OrganizationPlanItemV2[];
  itemTotal: number;
  itemPage: number;
  itemPageSize: number;
}

export interface RecentOrganizationPlanV2 {
  planId: string;
  revision: string;
  status: CopyPlanStatus;
  targetRoot: string;
  matchedCount: number;
  eligibleCount: number;
  blockedCount: number;
  createdAt: string;
}

export type CopyExportOperation = "copy" | "resume" | "rollback";
export type CopyExportJobState =
  | "queued"
  | "running"
  | "cancelling"
  | "cancelled"
  | "completed"
  | "partial"
  | "failed"
  | "interrupted"
  | "reconcile_required"
  | "rolling_back"
  | "rolled_back";
export type CopyExportJobItemState =
  | "pending"
  | "copying"
  | "copied"
  | "skipped"
  | "failed"
  | "cancelled"
  | "partial"
  | "reconcile_required"
  | "rollback_pending"
  | "rolled_back"
  | "rollback_blocked";
export type CopyExportProgressPhase =
  "preparing" | "verifying_source" | "copying" | "rolling_back" | "completed";

export interface DurableFileIdentity {
  volumeSerial: number;
  fileIndexHigh: number;
  fileIndexLow: number;
  creationFiletime: string;
}

export interface CopyExportFileSnapshot {
  identity: DurableFileIdentity | null;
  sizeBytes: number;
  modifiedTime: string;
  blake3: string;
}

export interface CopyExportCommittedTargetIdentity {
  identity: DurableFileIdentity;
  sizeBytes: number;
  blake3: string;
  finalPath: string;
}

export interface CopyExportPreflight {
  resourceId: string;
  operation: CopyExportOperation;
  authorizationToken: string;
  expiresAtUnixMs: number;
  targetRoot: string;
  planRevision: string;
  snapshotBlake3: string;
  itemCount: number;
  estimatedBytes: number;
  blockedCount: number;
}

export interface CopyExportStartResponse {
  jobId: string;
  accepted: boolean;
}

export interface CopyExportProgress {
  jobId: string;
  phase: CopyExportProgressPhase;
  completedItems: number;
  totalItems: number;
  currentOrdinal: number | null;
  currentAssetId: number | null;
  bytesProcessed: number;
  totalBytes: number;
}

export interface CopyExportRuntimeStatus {
  jobId: string;
  operation: CopyExportOperation;
  running: boolean;
  progress: CopyExportProgress | null;
  error: string | null;
}

export type CopyExportReconcileDecision =
  "retry_when_target_absent" | "adopt_verified_complete" | "keep_quarantined";

export interface CopyExportJobItemView {
  ordinal: number;
  assetId: number;
  state: CopyExportJobItemState;
  attempts: number;
  targetRelativePath: string;
  sourceSnapshot: CopyExportFileSnapshot;
  partialTargetIdentity: DurableFileIdentity | null;
  partialSnapshot: CopyExportFileSnapshot | null;
  outputIdentity: CopyExportCommittedTargetIdentity | null;
}

export interface CopyExportJobView {
  jobId: string;
  planId: string;
  targetRoot: string;
  state: CopyExportJobState;
  progressCompletedItems: number;
  progressTotalItems: number;
  progressCompletedBytes: number;
  progressTotalBytes: number;
  currentItemOrdinal: number | null;
  cancelRequested: boolean;
  createdAt: string;
  updatedAt: string;
  items: CopyExportJobItemView[];
  itemTotal: number;
  itemPage: number;
  itemPageSize: number;
}

export interface RecentCopyExportJob {
  jobId: string;
  planId: string;
  targetRoot: string;
  state: CopyExportJobState;
  progressCompletedItems: number;
  progressTotalItems: number;
  createdAt: string;
  updatedAt: string;
}

export interface WorkflowAsset {
  id: number;
  libraryId: number;
  fileName: string;
  extension: string;
  fileSize: number;
  width: number | null;
  height: number | null;
  captureTime: string | null;
  rating: number;
  colorLabel: ManualColorLabel | null;
  isFavorite: boolean;
  thumbnailAvailable: boolean;
}

export interface CollectionSummary {
  id: number;
  name: string;
  description: string;
  createdAt: string;
  updatedAt: string;
  assetCount: number;
  parentCollectionId: number | null;
  collectionKind: "manual" | "system_favorites";
  systemKey: string | null;
  displayOrder: number;
}

export type CollectionDeleteMode = "deleteSubtree" | "promoteChildren";

export interface CollectionMembershipMutation {
  affectedAssetCount: number;
  skippedAssetCount: number;
}

export type BrowseNode =
  | {
      kind: "source";
      library: LibrarySummary;
      children: BrowseNode[];
    }
  | {
      kind: "collection";
      collection: CollectionSummary;
      children: BrowseNode[];
    };

export interface CollectionDetail extends CollectionSummary {
  assets: WorkflowAsset[];
}

export interface DuplicateGroup {
  fingerprint: string;
  assets: WorkflowAsset[];
  totalBytes: number;
  reclaimableBytes: number;
}

export interface SimilarAsset extends WorkflowAsset {
  similarity: number;
}

export interface LocalSearchResponse {
  query: string;
  normalizedQuery: string;
  embeddedAssetCount: number;
  items: SimilarAsset[];
}

export interface SimilarityCluster {
  id: string;
  assets: SimilarAsset[];
}

export interface SimilarityClusterResponse {
  clusters: SimilarityCluster[];
  embeddedAssetCount: number;
  candidatePairCount: number;
  truncated: boolean;
}

export interface FaceFeatureStatus {
  status: string;
  message: string;
  enabled: boolean;
  modelInstalled: boolean;
  detectionCount: number;
  clusterCount: number;
  privacyNote: string;
}

export interface CropRecipe {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface EditRecipe {
  rotateDegrees: 0 | 90 | 180 | 270;
  flipHorizontal: boolean;
  flipVertical: boolean;
  crop: CropRecipe | null;
  exposure: number;
  contrast: number;
  saturation: number;
}

export const emptyEditRecipe: EditRecipe = {
  rotateDegrees: 0,
  flipHorizontal: false,
  flipVertical: false,
  crop: null,
  exposure: 0,
  contrast: 0,
  saturation: 0,
};

export interface EditExportPlan {
  planId: string;
  assetId: number;
  sourcePath: string;
  targetPath: string;
  sourceFingerprint: string;
  recipe: EditRecipe;
  status: string;
  issues: string[];
}

export interface EditExportResult {
  planId: string;
  targetPath: string;
  status: string;
}

export interface EditRollbackPlan {
  planId: string;
  targetPath: string;
  targetHash: string;
  status: string;
  issues: string[];
}
