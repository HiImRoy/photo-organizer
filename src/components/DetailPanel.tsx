import { useState } from "react";

import {
  auxiliaryTagOptions,
  canonicalClassificationValue,
  classificationSourceLabel,
  classificationValueLabel,
  classificationValuesLabel,
  primaryCategoryOptions,
  SATURATION_OPTIONS,
  TONE_OPTIONS,
} from "../classificationLabels";
import { formatBytes, formatDate, formatPercent } from "../format";
import { MANUAL_COLOR_LABEL_OPTIONS } from "../types";
import type {
  AssetListItem,
  ClassificationFieldDescriptor,
  ManualColorLabel,
  SemanticLabelDescriptor,
  SemanticRuntimeStatus,
  SubjectRuntimeStatus,
} from "../types";
import { ColorSwatches } from "./ColorSwatches";
import { ImageHistogram } from "./ImageHistogram";
import { PanelIcon, PlayIcon } from "./Icons";
import { PreviewNavigator, type PreviewNavigatorProps } from "./PreviewNavigator";
import { RatingStars } from "./RatingStars";
import { Thumbnail } from "./Thumbnail";

interface DetailPanelProps {
  asset: AssetListItem | null;
  semanticStatus: SemanticRuntimeStatus | null;
  subjectStatus?: SubjectRuntimeStatus | null;
  previewNavigator: PreviewNavigatorProps | null;
  onReanalyze: (asset: AssetListItem) => void;
  classificationRegistry?: ClassificationFieldDescriptor[];
  catalog?: SemanticLabelDescriptor[];
  onUpdateClassification?: (assetId: number, field: string, value: string | string[]) => void;
  onUpdateTagOverride?: (assetId: number, tagId: string, state: "add" | "remove") => void;
  onRestoreAuto?: (assetId: number, field?: string) => void;
  onUpdateRating?: (assetId: number, rating: number) => void;
  onUpdateColorLabel?: (assetId: number, colorLabel: ManualColorLabel | null) => void;
}

export function DetailPanel({
  asset,
  semanticStatus,
  subjectStatus,
  previewNavigator,
  onReanalyze,
  classificationRegistry,
  catalog,
  onUpdateClassification,
  onUpdateTagOverride,
  onRestoreAuto,
  onUpdateRating,
  onUpdateColorLabel,
}: DetailPanelProps) {
  return (
    <aside className="right-panel" aria-label="图片详情">
      <div className="right-panel-card">
        <div className="panel-titlebar">
          <strong>信息</strong>
        </div>
        {!asset ? (
          <div className="details-empty">
            <PanelIcon width="25" height="25" />
            <strong>未选择图片</strong>
          </div>
        ) : (
          <div className="details-scroll">
            {previewNavigator ? (
              <PreviewNavigator {...previewNavigator} placement="panel" />
            ) : (
              <div className="detail-preview">
                <Thumbnail asset={asset} />
              </div>
            )}
            <h2 title={asset.fileName}>{asset.fileName}</h2>
            <div className="detail-path" title={asset.relativePath}>
              {asset.relativePath}
            </div>

            <DetailSection title="文件">
              <dl className="property-list">
                <Property label="格式" value={asset.extension.toUpperCase()} />
                <Property
                  label="尺寸"
                  value={asset.width && asset.height ? `${asset.width} × ${asset.height}` : "—"}
                />
                <Property label="大小" value={formatBytes(asset.fileSize)} />
                <Property label="拍摄时间" value={formatDate(asset.captureTime)} />
              </dl>
            </DetailSection>

            <DetailSection title="直方图" trailing="缩略图">
              <ImageHistogram key={asset.id} asset={asset} />
            </DetailSection>

            <DetailSection title="拍摄信息">
              <dl className="property-list">
                <Property
                  label="相机"
                  value={[asset.cameraMake, asset.cameraModel].filter(Boolean).join(" ") || "—"}
                />
                <Property label="镜头" value={asset.lensModel ?? "—"} />
                <Property label="曝光" value={asset.exposureTime ?? "—"} />
                <Property label="光圈" value={asset.aperture ? `f/${asset.aperture}` : "—"} />
                <Property label="感光度" value={asset.iso?.toString() ?? "—"} />
                <Property
                  label="焦距"
                  value={asset.focalLength ? `${asset.focalLength} mm` : "—"}
                />
              </dl>
            </DetailSection>

            <ManualMarkEditor
              asset={asset}
              onUpdateRating={onUpdateRating}
              onUpdateColorLabel={onUpdateColorLabel}
            />

            <DetailSection
              title="影调与色彩"
              trailing={asset.analysisStatus === "completed" ? "已完成" : asset.analysisStatus}
            >
              <div className="analysis-grid">
                <Metric label="亮度" value={formatPercent(asset.brightness)} />
                <Metric label="对比度" value={formatPercent(asset.contrast)} />
                <Metric label="饱和度" value={formatPercent(asset.saturation)} />
                <Metric label="色度" value={formatPercent(asset.chroma)} />
                <Metric label="有彩色占比" value={formatPercent(asset.dominantColorCoverage)} />
                <Metric label="中性色占比" value={formatPercent(asset.neutralRatio)} />
                <Metric label="影调" value={classificationValueLabel(asset.toneLabel, "tone")} />
              </div>
              <div className="dominant-row">
                <span>主色</span>
                {asset.dominantColor ? <i style={{ background: asset.dominantColor }} /> : null}
                <strong>{asset.dominantColor ?? "—"}</strong>
                <small>{classificationValueLabel(asset.dominantColorCategory, "color")}</small>
              </div>
              {asset.colorPalette ? (
                <div className="accent-palette-detail">
                  <PaletteDetailRow
                    label="强调色"
                    candidates={asset.colorPalette.prominentPalette}
                  />
                  <PaletteDetailRow
                    label="面积色"
                    candidates={asset.colorPalette.coveragePalette}
                  />
                  <small className="accent-palette-version">
                    {asset.colorPalette.algorithmVersion}
                  </small>
                </div>
              ) : null}
            </DetailSection>

            <ClassificationEditor
              key={`${asset.id}:${asset.classification.revision}`}
              asset={asset}
              classificationRegistry={classificationRegistry}
              catalog={catalog}
              onUpdateClassification={onUpdateClassification}
              onUpdateTagOverride={onUpdateTagOverride}
              onRestoreAuto={onRestoreAuto}
            />

            <DetailSection title="语义标签" trailing={semanticStateLabel(asset.semanticStatus)}>
              {asset.semanticLabels.length ? (
                <div className="semantic-detail-list">
                  {isVirtualAbstract(asset) ? (
                    <div className="semantic-rejection-state">
                      <span>
                        抽象艺术
                        <small>低置信归类</small>
                      </span>
                      <strong>—</strong>
                      <p>没有任何模型相似度分数达到可靠题材条件，已归入抽象艺术。</p>
                    </div>
                  ) : null}
                  {asset.semanticLabels.map((label) => (
                    <div key={label.labelId}>
                      <span>
                        {label.displayName}
                        <small>{semanticGroupLabel(label.categoryGroup)}</small>
                      </span>
                      <strong>{label.similarity.toFixed(3)}</strong>
                      <i
                        style={{ width: `${Math.max(2, Math.min(100, label.similarity * 250))}%` }}
                      />
                    </div>
                  ))}
                  <p>数值为模型相似度，不代表准确率或概率。</p>
                </div>
              ) : (
                <div className="semantic-empty">
                  {asset.semanticStatus === "failed"
                    ? asset.semanticError
                    : isVirtualAbstract(asset)
                      ? "分析完成，但没有达到可靠置信度；已归入抽象艺术。"
                      : "尚无真实语义分析结果"}
                </div>
              )}
              <dl className="property-list model-properties">
                <Property
                  label="模型"
                  value={asset.semanticLabels[0]?.modelName ?? semanticStatus?.model.name ?? "—"}
                />
                <Property
                  label="版本"
                  value={
                    asset.semanticLabels[0]?.modelVersion ?? semanticStatus?.model.version ?? "—"
                  }
                />
                <Property label="题材候选" value={semanticStatus?.topicModel?.name ?? "未启用"} />
                <Property
                  label="后端"
                  value={semanticStatus?.selectedBackend ? "本地计算" : "未启用"}
                />
                <Property label="主体模型" value={subjectStatus?.model.name ?? "—"} />
                <Property
                  label="人像辅助"
                  value={
                    subjectStatus?.faceModel.installed ? subjectStatus.faceModel.name : "未启用"
                  }
                />
              </dl>
              <button
                className="primary-action detail-reanalyze-action"
                type="button"
                onClick={() => onReanalyze(asset)}
                disabled={
                  semanticStatus?.status !== "ready" || asset.analysisStatus !== "completed"
                }
              >
                <PlayIcon width="14" height="14" />
                重新分析此图片
              </button>
            </DetailSection>
          </div>
        )}
      </div>
    </aside>
  );
}

function PaletteDetailRow({
  label,
  candidates,
}: {
  label: string;
  candidates: NonNullable<AssetListItem["colorPalette"]>["prominentPalette"];
}) {
  if (!candidates.length) return null;
  return (
    <div className="accent-palette-row">
      <span>{label}</span>
      <div className="accent-palette-candidates">
        {candidates.map((candidate) => (
          <span className="accent-palette-candidate" key={`${label}-${candidate.rank}`}>
            <i style={{ background: candidate.color }} title={candidate.color} />
            <strong>{classificationValueLabel(candidate.category, "color")}</strong>
            <small>
              面积 {formatPercent(candidate.areaCoverage)} · 显著{" "}
              {formatPercent(candidate.saliencyCoverage)}
            </small>
          </span>
        ))}
      </div>
    </div>
  );
}

function ManualMarkEditor({
  asset,
  onUpdateRating,
  onUpdateColorLabel,
}: {
  asset: AssetListItem;
  onUpdateRating?: (assetId: number, rating: number) => void;
  onUpdateColorLabel?: (assetId: number, colorLabel: ManualColorLabel | null) => void;
}) {
  return (
    <DetailSection title="人工标记">
      <div className="manual-mark-editor">
        <div className="manual-mark-row">
          <span>星级</span>
          <div className="manual-rating-controls">
            <RatingStars
              className="rating-stars"
              value={asset.rating}
              ariaLabel="星级"
              buttonLabel={(rating) => `${rating} 星`}
              onChange={(rating) =>
                onUpdateRating?.(asset.id, asset.rating === rating ? 0 : rating)
              }
            />
            <small>{asset.rating ? `${asset.rating} 星` : "未评级"}</small>
          </div>
        </div>
        <div className="manual-mark-row">
          <span>色标</span>
          <div className="manual-color-label-controls" role="group" aria-label="色标">
            {MANUAL_COLOR_LABEL_OPTIONS.map((option) => {
              const isActive = asset.colorLabel === option.id;
              return (
                <button
                  type="button"
                  key={option.id}
                  className={isActive ? "is-active" : ""}
                  data-manual-color-label={option.id}
                  style={{ backgroundColor: option.color }}
                  aria-label={option.label}
                  aria-pressed={isActive}
                  onClick={() => onUpdateColorLabel?.(asset.id, isActive ? null : option.id)}
                />
              );
            })}
            <small>
              {asset.colorLabel
                ? MANUAL_COLOR_LABEL_OPTIONS.find((option) => option.id === asset.colorLabel)?.label
                : "未设置"}
            </small>
          </div>
        </div>
      </div>
    </DetailSection>
  );
}

function ClassificationEditor({
  asset,
  classificationRegistry,
  catalog = [],
  onUpdateClassification,
  onUpdateTagOverride,
  onRestoreAuto,
}: {
  asset: AssetListItem;
  classificationRegistry?: ClassificationFieldDescriptor[];
  catalog?: SemanticLabelDescriptor[];
  onUpdateClassification?: (assetId: number, field: string, value: string | string[]) => void;
  onUpdateTagOverride?: (assetId: number, tagId: string, state: "add" | "remove") => void;
  onRestoreAuto?: (assetId: number, field?: string) => void;
}) {
  const classification = asset.classification;
  const registryIds = new Set(
    classificationRegistry?.length
      ? classificationRegistry.map((field) => field.id)
      : [
          "primary_category",
          "auxiliary_tags",
          "tone",
          "dominant_color_category",
          "saturation_level",
        ],
  );
  const [editing, setEditing] = useState(false);
  const [primary, setPrimary] = useState(
    canonicalClassificationValue(classification.primaryCategory.effective, "primary") ?? "",
  );
  const [tone, setTone] = useState(classification.tone.effective ?? "");
  const [colors, setColors] = useState<string[]>(
    classification.dominantColorCategories.effective ?? [],
  );
  const [saturation, setSaturation] = useState(classification.saturationLevel.effective ?? "");
  const [tagChoice, setTagChoice] = useState("");
  const primaryOptions = primaryCategoryOptions(catalog, classification.primaryCategory.effective);
  const tagOptions = auxiliaryTagOptions(catalog, classification.auxiliaryTags.effective);

  const save = (field: string, value: string | string[]) => {
    if (!value || (Array.isArray(value) && value.length === 0)) return;
    onUpdateClassification?.(asset.id, field, value);
  };
  const addTag = () => {
    if (!tagChoice) return;
    onUpdateTagOverride?.(asset.id, tagChoice, "add");
    setTagChoice("");
  };

  const summary = [
    `拍摄题材：${classificationValueLabel(classification.primaryCategory.effective, "primary", catalog)}`,
    `影调：${classificationValueLabel(classification.tone.effective, "tone", catalog)}`,
    `主色：${classificationValuesLabel(
      classification.dominantColorCategories.effective,
      "color",
      catalog,
    )}`,
    `饱和度：${classificationValueLabel(
      classification.saturationLevel.effective,
      "saturation",
      catalog,
    )}`,
  ];

  return (
    <DetailSection title="分类" trailing={`版本 ${classification.revision}`}>
      <div className="classification-summary">
        <div className="classification-summary-values">
          {summary.map((value) => (
            <span key={value}>{value}</span>
          ))}
        </div>
        <button
          type="button"
          className="secondary-action"
          aria-expanded={editing}
          onClick={() => setEditing((value) => !value)}
        >
          {editing ? "收起修改" : "手动修改"}
        </button>
      </div>

      {editing ? (
        <div className="classification-editor">
          {registryIds.has("primary_category") ? (
            <ClassificationRow
              label="拍摄题材"
              auto={classificationValueLabel(
                classification.primaryCategory.auto,
                "primary",
                catalog,
              )}
              manual={classificationValueLabel(
                classification.primaryCategory.manual,
                "primary",
                catalog,
              )}
              effective={classificationValueLabel(
                classification.primaryCategory.effective,
                "primary",
                catalog,
              )}
              source={classification.primaryCategory.source}
              control={
                <select value={primary} onChange={(event) => setPrimary(event.target.value)}>
                  <option value="">请选择拍摄题材</option>
                  {primaryOptions.map((option) => (
                    <option value={option.value} key={option.value}>
                      {option.label}
                    </option>
                  ))}
                </select>
              }
              onSave={() => save("primary_category", primary)}
              onRestore={() => onRestoreAuto?.(asset.id, "primary_category")}
            />
          ) : null}
          {registryIds.has("tone") ? (
            <ClassificationRow
              label="影调"
              auto={classificationValueLabel(classification.tone.auto, "tone", catalog)}
              manual={classificationValueLabel(classification.tone.manual, "tone", catalog)}
              effective={classificationValueLabel(classification.tone.effective, "tone", catalog)}
              source={classification.tone.source}
              control={
                <select value={tone} onChange={(event) => setTone(event.target.value)}>
                  <option value="">请选择影调</option>
                  {TONE_OPTIONS.map(([value, label]) => (
                    <option value={value} key={value}>
                      {label}
                    </option>
                  ))}
                </select>
              }
              onSave={() => save("tone", tone)}
              onRestore={() => onRestoreAuto?.(asset.id, "tone")}
            />
          ) : null}
          {registryIds.has("dominant_color_category") ? (
            <ClassificationRow
              label="主色"
              auto={classificationValuesLabel(
                classification.dominantColorCategories.auto,
                "color",
                catalog,
              )}
              manual={classificationValuesLabel(
                classification.dominantColorCategories.manual,
                "color",
                catalog,
              )}
              effective={classificationValuesLabel(
                classification.dominantColorCategories.effective,
                "color",
                catalog,
              )}
              source={classification.dominantColorCategories.source}
              control={<ColorSwatches value={colors} onChange={setColors} ariaLabel="选择主色" />}
              onSave={() => save("dominant_color_category", colors)}
              onRestore={() => onRestoreAuto?.(asset.id, "dominant_color_category")}
            />
          ) : null}
          {registryIds.has("saturation_level") ? (
            <ClassificationRow
              label="饱和度级别"
              auto={classificationValueLabel(
                classification.saturationLevel.auto,
                "saturation",
                catalog,
              )}
              manual={classificationValueLabel(
                classification.saturationLevel.manual,
                "saturation",
                catalog,
              )}
              effective={classificationValueLabel(
                classification.saturationLevel.effective,
                "saturation",
                catalog,
              )}
              source={classification.saturationLevel.source}
              control={
                <select value={saturation} onChange={(event) => setSaturation(event.target.value)}>
                  <option value="">请选择饱和度</option>
                  {SATURATION_OPTIONS.map(([value, label]) => (
                    <option value={value} key={value}>
                      {label}
                    </option>
                  ))}
                </select>
              }
              onSave={() => save("saturation_level", saturation)}
              onRestore={() => onRestoreAuto?.(asset.id, "saturation_level")}
            />
          ) : null}
          {registryIds.has("auxiliary_tags") ? (
            <div className="classification-tags-editor">
              <div className="classification-field-label">辅助标签</div>
              <div className="classification-provenance">
                <span>
                  当前：
                  {classificationValuesLabel(
                    classification.auxiliaryTags.effective,
                    "tag",
                    catalog,
                  )}
                </span>
                <span>来源：{classificationSourceLabel(classification.auxiliaryTags.source)}</span>
              </div>
              <div className="classification-tag-list">
                {classification.auxiliaryTags.effective.map((value) => (
                  <button
                    type="button"
                    className="filter-chip is-active"
                    key={value}
                    title="点击移除此标签"
                    onClick={() => onUpdateTagOverride?.(asset.id, value, "remove")}
                  >
                    {classificationValueLabel(value, "tag", catalog)} ×
                  </button>
                ))}
              </div>
              <div className="classification-tag-add">
                <select value={tagChoice} onChange={(event) => setTagChoice(event.target.value)}>
                  <option value="">请选择要添加的标签</option>
                  {tagOptions.map((option) => (
                    <option value={option.value} key={option.value}>
                      {option.label}
                    </option>
                  ))}
                </select>
                <button
                  type="button"
                  className="secondary-action"
                  onClick={addTag}
                  disabled={!tagChoice}
                >
                  添加标签
                </button>
                <button
                  type="button"
                  className="secondary-action"
                  onClick={() => onRestoreAuto?.(asset.id, "auxiliary_tags")}
                >
                  恢复自动
                </button>
              </div>
            </div>
          ) : null}
        </div>
      ) : null}
    </DetailSection>
  );
}

function ClassificationRow({
  label,
  auto,
  manual,
  effective,
  source,
  control,
  onSave,
  onRestore,
}: {
  label: string;
  auto: string;
  manual: string;
  effective: string;
  source: string;
  control: React.ReactNode;
  onSave: () => void;
  onRestore: () => void;
}) {
  return (
    <div className="classification-field">
      <div className="classification-field-heading">
        <strong>{label}</strong>
        <small>{classificationSourceLabel(source)}</small>
      </div>
      <div className="classification-provenance">
        <span>自动：{auto}</span>
        <span>手动：{manual}</span>
        <span>当前结果：{effective}</span>
      </div>
      <div className="classification-control">
        {control}
        <button type="button" className="secondary-action" onClick={onSave}>
          保存
        </button>
        <button type="button" className="secondary-action" onClick={onRestore}>
          恢复自动
        </button>
      </div>
    </div>
  );
}

function DetailSection({
  title,
  trailing,
  children,
}: {
  title: string;
  trailing?: string;
  children: React.ReactNode;
}) {
  return (
    <section className="detail-section">
      <div className="detail-section-heading">
        <strong>{title}</strong>
        {trailing ? <small>{trailing}</small> : null}
      </div>
      {children}
    </section>
  );
}

function Property({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <dt>{label}</dt>
      <dd title={value}>{value}</dd>
    </div>
  );
}

function Metric({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}

function semanticStateLabel(status: string) {
  if (status === "completed") return "已完成";
  if (status === "running" || status === "queued") return "分析中";
  if (status === "failed") return "失败";
  return "未分析";
}

function semanticGroupLabel(group: string) {
  switch (group) {
    case "scene":
      return "拍摄题材";
    case "subject":
      return "主体标签";
    case "context":
      return "环境属性";
    default:
      return "语义标签";
  }
}

function isVirtualAbstract(asset: AssetListItem) {
  return (
    asset.semanticStatus === "completed" &&
    asset.classification.primaryCategory.effective === "photo_abstract" &&
    !asset.semanticLabels.some((label) => label.isPrimary)
  );
}
