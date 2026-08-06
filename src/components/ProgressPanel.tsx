import type { ScanProgress } from "../types";
import { CloseIcon, PauseIcon } from "./Icons";

interface ProgressPanelProps {
  progress: ScanProgress;
  cancelling: boolean;
  onCancel: () => void;
  onDismiss: () => void;
}

const stageLabels: Record<string, string> = {
  preparing: "准备图库",
  discovering: "发现图片",
  processing: "生成索引与缩略图",
  completed: "扫描完成",
  cancelled: "扫描已取消",
  failed: "扫描失败",
};

export function ProgressPanel({ progress, cancelling, onCancel, onDismiss }: ProgressPanelProps) {
  const terminal = ["completed", "cancelled", "failed"].includes(progress.status);
  const ratio = progress.discovered
    ? Math.min(100, Math.round((progress.processed / progress.discovered) * 100))
    : progress.stage === "discovering"
      ? 8
      : 0;

  return (
    <section className={`scan-panel status-${progress.status}`} aria-live="polite">
      <div className="scan-panel-top">
        <div>
          <div className="section-label">后台任务</div>
          <strong>{stageLabels[progress.stage] ?? progress.stage}</strong>
        </div>
        {terminal ? (
          <button className="icon-button" type="button" onClick={onDismiss} aria-label="关闭状态">
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
    </section>
  );
}
