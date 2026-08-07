import { formatBytes, formatDate, formatPercent } from "../format";
import type { AssetListItem, SemanticRuntimeStatus } from "../types";
import { PanelIcon, PlayIcon } from "./Icons";
import { Thumbnail } from "./Thumbnail";

interface DetailPanelProps {
  asset: AssetListItem | null;
  collapsed: boolean;
  semanticStatus: SemanticRuntimeStatus | null;
  onToggle: () => void;
  onReanalyze: (asset: AssetListItem) => void;
}

const toneLabels: Record<string, string> = {
  low_key: "低调",
  balanced: "均衡",
  high_key: "高调",
};

export function DetailPanel({
  asset,
  collapsed,
  semanticStatus,
  onToggle,
  onReanalyze,
}: DetailPanelProps) {
  if (collapsed) {
    return (
      <aside className="right-panel is-collapsed" aria-label="图片详情">
        <button className="panel-toggle" type="button" onClick={onToggle} aria-label="展开右侧面板">
          <PanelIcon width="17" height="17" />
        </button>
      </aside>
    );
  }

  return (
    <aside className="right-panel" aria-label="图片详情">
      <div className="panel-titlebar">
        <strong>信息</strong>
        <button
          className="panel-toggle flip"
          type="button"
          onClick={onToggle}
          aria-label="折叠右侧面板"
        >
          <PanelIcon width="17" height="17" />
        </button>
      </div>
      {!asset ? (
        <div className="details-empty">
          <PanelIcon width="25" height="25" />
          <strong>未选择图片</strong>
          <span>选择网格中的图片以查看元数据和分析结果。</span>
        </div>
      ) : (
        <div className="details-scroll">
          <div className="detail-preview">
            <Thumbnail asset={asset} />
          </div>
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

          <DetailSection title="EXIF">
            <dl className="property-list">
              <Property
                label="相机"
                value={[asset.cameraMake, asset.cameraModel].filter(Boolean).join(" ") || "—"}
              />
              <Property label="镜头" value={asset.lensModel ?? "—"} />
              <Property label="曝光" value={asset.exposureTime ?? "—"} />
              <Property label="光圈" value={asset.aperture ? `f/${asset.aperture}` : "—"} />
              <Property label="ISO" value={asset.iso?.toString() ?? "—"} />
              <Property label="焦距" value={asset.focalLength ? `${asset.focalLength} mm` : "—"} />
            </dl>
          </DetailSection>

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
              <Metric
                label="影调"
                value={asset.toneLabel ? (toneLabels[asset.toneLabel] ?? asset.toneLabel) : "—"}
              />
            </div>
            <div className="dominant-row">
              <span>主色</span>
              {asset.dominantColor ? <i style={{ background: asset.dominantColor }} /> : null}
              <strong>{asset.dominantColor ?? "—"}</strong>
              <small>{asset.dominantColorCategory ?? ""}</small>
            </div>
          </DetailSection>

          <DetailSection title="语义标签" trailing={semanticStateLabel(asset.semanticStatus)}>
            {asset.semanticLabels.length ? (
              <div className="semantic-detail-list">
                {asset.semanticLabels.map((label) => (
                  <div key={label.labelId}>
                    <span>
                      {label.displayName}
                      <small>{label.isPrimary ? "一级分类" : "辅助标签"}</small>
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
                {asset.semanticStatus === "failed" ? asset.semanticError : "尚无真实语义分析结果"}
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
              <Property
                label="后端"
                value={semanticStatus?.selectedBackend?.toUpperCase() ?? "未启用"}
              />
            </dl>
            <button
              className="secondary-action full"
              type="button"
              onClick={() => onReanalyze(asset)}
              disabled={semanticStatus?.status !== "ready" || asset.analysisStatus !== "completed"}
            >
              <PlayIcon width="14" height="14" />
              重新分析此图片
            </button>
          </DetailSection>
        </div>
      )}
    </aside>
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
