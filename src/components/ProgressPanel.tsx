import type { ScanProgress } from "../types";
import { CloseIcon, PauseIcon } from "./Icons";

interface ProgressPanelProps {
  progress: ScanProgress;
  taskName: string;
  cancelling: boolean;
  onCancel: () => void;
  onDismiss: () => void;
}

const stageLabels: Record<string, string> = {
  preparing: "准备图库",
  discovering: "发现图片",
  processing: "生成索引与缩略图",
  completed: "导入完成",
  cancelled: "导入已取消",
  failed: "导入失败",
};

function formatScanDuration(microseconds: number): string {
  if (microseconds < 1000) return `${microseconds} μs`;
  const milliseconds = microseconds / 1000;
  if (milliseconds < 1000) return `${milliseconds.toFixed(1)} ms`;
  return `${(milliseconds / 1000).toFixed(2)} s`;
}

export function ProgressPanel({
  progress,
  taskName,
  cancelling,
  onCancel,
  onDismiss,
}: ProgressPanelProps) {
  const terminal = ["completed", "cancelled", "failed"].includes(progress.status);
  const ratio = progress.discovered
    ? Math.min(100, Math.round((progress.processed / progress.discovered) * 100))
    : progress.stage === "discovering"
      ? 8
      : 0;
  const operationLabel = terminal ? (stageLabels[progress.stage] ?? progress.stage) : "导入中";
  const performance = progress.performance;
  const hasMetadataLookupBreakdown =
    performance?.fileMetadataUs !== undefined ||
    performance?.existingAssetLookupUs !== undefined ||
    performance?.cacheProbeUs !== undefined;
  const hasProcessingDecisionCounts =
    performance?.coldFiles !== undefined || performance?.reanalyzedFiles !== undefined;

  return (
    <section className={`scan-panel status-${progress.status}`} aria-live="polite">
      <div className="scan-panel-top">
        <div className="scan-panel-copy">
          <div className="section-label" title={taskName}>
            {taskName}
          </div>
          <strong>{operationLabel}</strong>
          {!terminal ? <small>{stageLabels[progress.stage] ?? progress.stage}</small> : null}
        </div>
        <span className="task-panel-percent">{ratio}%</span>
        {terminal ? (
          <button
            className="icon-button"
            type="button"
            onClick={onDismiss}
            aria-label="关闭扫描状态"
            title="关闭扫描状态"
          >
            <CloseIcon width="18" height="18" />
          </button>
        ) : (
          <button className="quiet-button" type="button" onClick={onCancel} disabled={cancelling}>
            <PauseIcon width="16" height="16" />
            {cancelling ? "正在取消…" : "取消扫描"}
          </button>
        )}
      </div>
      <div
        className="progress-track"
        role="progressbar"
        aria-label="扫描进度"
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={ratio}
      >
        <span style={{ width: `${ratio}%` }} />
      </div>
      <div className="scan-counts">
        <span>发现 {progress.discovered}</span>
        <span>完成 {progress.succeeded}</span>
        <span>失败 {progress.failed}</span>
        <span>跳过 {progress.skipped}</span>
        {progress.missing > 0 ? <span>缺失 {progress.missing}</span> : null}
      </div>
      {progress.currentPath && !terminal ? (
        <div className="current-path" title={progress.currentPath}>
          {progress.currentPath}
        </div>
      ) : null}
      {progress.error ? (
        <div className="scan-error" role="alert">
          {progress.error}
        </div>
      ) : null}
      {performance ? (
        <section
          className="scan-performance"
          role="group"
          aria-label="扫描性能诊断"
          aria-live="off"
        >
          <div className="scan-performance-heading">
            <strong>性能计时</strong>
            <span>累计值 · 并行阶段可能重叠</span>
          </div>
          <dl className="scan-performance-grid">
            <div>
              <dt>图片发现</dt>
              <dd>{formatScanDuration(performance.discoveryUs)}</dd>
            </div>
            <div>
              <dt>元数据 / 归属查询</dt>
              <dd>
                {formatScanDuration(performance.metadataLookupUs + performance.ownershipLookupUs)}
              </dd>
            </div>
            {hasMetadataLookupBreakdown ? (
              <>
                <div className="is-child">
                  <dt>其中：文件元数据</dt>
                  <dd>{formatScanDuration(performance.fileMetadataUs ?? 0)}</dd>
                </div>
                <div className="is-child">
                  <dt>其中：已有资源查询</dt>
                  <dd>{formatScanDuration(performance.existingAssetLookupUs ?? 0)}</dd>
                </div>
                <div className="is-child">
                  <dt>其中：缓存探测</dt>
                  <dd>{formatScanDuration(performance.cacheProbeUs ?? 0)}</dd>
                </div>
                <div className="is-child">
                  <dt>其中：归属查询</dt>
                  <dd>{formatScanDuration(performance.ownershipLookupUs)}</dd>
                </div>
              </>
            ) : null}
            <div>
              <dt>读文件 / 指纹</dt>
              <dd>{formatScanDuration(performance.fingerprintUs)}</dd>
            </div>
            <div>
              <dt>图像处理总计</dt>
              <dd>{formatScanDuration(performance.imageProcessingUs)}</dd>
            </div>
            <div className="is-child">
              <dt>其中：缩略图解码</dt>
              <dd>{formatScanDuration(performance.thumbnailDecodeUs)}</dd>
            </div>
            <div className="is-child">
              <dt>其中：特征分析</dt>
              <dd>{formatScanDuration(performance.featureAnalysisUs)}</dd>
            </div>
            <div>
              <dt>数据库写入</dt>
              <dd>{formatScanDuration(performance.databaseWriteUs)}</dd>
            </div>
          </dl>
          {hasProcessingDecisionCounts ? (
            <>
              <div className="scan-performance-heading">
                <strong>处理计数</strong>
              </div>
              <dl className="scan-performance-grid">
                <div>
                  <dt>新文件已判定需处理</dt>
                  <dd>{performance.coldFiles ?? 0} 张</dd>
                </div>
                <div>
                  <dt>已有文件需重分析</dt>
                  <dd>{performance.reanalyzedFiles ?? 0} 张</dd>
                </div>
                <div>
                  <dt>已有文件跳过</dt>
                  <dd>{performance.skippedFiles} 张</dd>
                </div>
              </dl>
            </>
          ) : null}
          <p className="scan-performance-note">
            阶段是累计工作耗时，可能并行重叠，不代表墙钟时长。元数据明细和归属查询已计入查询总计；图像处理子项已计入图像处理总计，不要重复相加。
          </p>
        </section>
      ) : null}
    </section>
  );
}
