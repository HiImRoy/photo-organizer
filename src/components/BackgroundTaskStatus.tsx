import { useEffect, useRef, useState } from "react";
import type { ScanProgress, SemanticProgress } from "../types";
import { PauseIcon, PlayIcon } from "./Icons";
import { ProgressPanel } from "./ProgressPanel";
import "./backgroundTaskStatus.css";

interface BackgroundTaskStatusProps {
  scanProgress: ScanProgress | null;
  semanticProgress: SemanticProgress | null;
  scanRunning: boolean;
  semanticRunning: boolean;
  cancellingScan: boolean;
  onCancelScan: () => void;
  onDismissScan: () => void;
  onPauseResumeSemantic: () => void;
  onCancelSemantic: () => void;
  scanTaskName?: string | null;
  semanticTaskName?: string | null;
}

function scanProgressRatio(progress: ScanProgress): number {
  if (progress.discovered > 0) {
    return Math.min(100, Math.round((progress.processed / progress.discovered) * 100));
  }
  return progress.stage === "discovering" ? 8 : 0;
}

function semanticProgressRatio(progress: SemanticProgress): number {
  return progress.total > 0
    ? Math.min(100, Math.round((progress.processed / progress.total) * 100))
    : 0;
}

function scanOperationLabel(progress: ScanProgress): string {
  if (progress.status === "completed") return "导入完成";
  if (progress.status === "cancelled") return "导入已取消";
  if (progress.status === "failed") return "导入失败";
  return "导入中";
}

function executionBackendLabel(backend: string | null): string {
  if (backend === "direct_ml") return "DirectML GPU";
  if (backend === "cpu") return "CPU";
  return "本地计算";
}
function semanticLabel(progress: SemanticProgress): string {
  if (progress.status === "paused") return "分析已暂停";
  if (progress.status === "cancelling") return "正在停止分析";
  if (progress.status === "completed") return "分析完成";
  if (progress.status === "failed") return "分析失败";
  return "分析中";
}

function overallTaskProgressRatio(
  scanProgress: ScanProgress | null,
  semanticProgress: SemanticProgress | null,
): number {
  const entries: Array<{ total: number; ratio: number }> = [];
  if (scanProgress) {
    entries.push({
      total: scanProgress.discovered > 0 ? scanProgress.discovered : 100,
      ratio: scanProgressRatio(scanProgress),
    });
  }
  if (semanticProgress) {
    entries.push({
      total: semanticProgress.total > 0 ? semanticProgress.total : 100,
      ratio: semanticProgressRatio(semanticProgress),
    });
  }
  const total = entries.reduce((sum, entry) => sum + entry.total, 0);
  if (total <= 0) return 0;
  const completed = entries.reduce((sum, entry) => sum + (entry.total * entry.ratio) / 100, 0);
  return Math.min(100, Math.round((completed / total) * 100));
}

function SemanticTaskDetails({
  progress,
  taskName,
  onPauseResume,
  onCancel,
}: {
  progress: SemanticProgress;
  taskName: string;
  onPauseResume: () => void;
  onCancel: () => void;
}) {
  const percent = semanticProgressRatio(progress);
  const paused = progress.status === "paused";
  const backendLabel = executionBackendLabel(progress.executionBackend);

  return (
    <section className="semantic-taskbar task-detail" aria-live="polite">
      <div className="task-detail-heading">
        <PlayIcon width="15" height="15" />
        <span>
          <small className="task-scope-label" title={taskName}>
            {taskName}
          </small>
          <strong>{semanticLabel(progress)}</strong>
          <small>
            {percent}% · {progress.processed} / {progress.total} · {backendLabel} · 失败{" "}
            {progress.failed}
          </small>
        </span>
      </div>
      <div className="task-progress">
        <i style={{ width: `${percent}%` }} />
      </div>
      <div className="task-detail-actions">
        <button type="button" onClick={onPauseResume} disabled={progress.status === "cancelling"}>
          {paused ? <PlayIcon width="13" height="13" /> : <PauseIcon width="13" height="13" />}
          {paused ? "继续" : "暂停"}
        </button>
        <button type="button" onClick={onCancel} disabled={progress.status === "cancelling"}>
          取消
        </button>
      </div>
    </section>
  );
}

export function BackgroundTaskStatus({
  scanProgress,
  semanticProgress,
  scanRunning,
  semanticRunning,
  cancellingScan,
  onCancelScan,
  onDismissScan,
  onPauseResumeSemantic,
  onCancelSemantic,
  scanTaskName,
  semanticTaskName,
}: BackgroundTaskStatusProps) {
  const visibleSemanticProgress = semanticRunning ? semanticProgress : null;
  const hasTask = scanProgress !== null || visibleSemanticProgress !== null;
  const [detailsOpen, setDetailsOpen] = useState(false);

  const taskStatusRef = useRef<HTMLSpanElement | null>(null);

  useEffect(() => {
    if (!detailsOpen) return undefined;
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") setDetailsOpen(false);
    };
    const handlePointerDown = (event: PointerEvent) => {
      if (!(event.target instanceof Node) || !taskStatusRef.current?.contains(event.target)) {
        setDetailsOpen(false);
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    document.addEventListener("pointerdown", handlePointerDown);
    return () => {
      window.removeEventListener("keydown", handleKeyDown);
      document.removeEventListener("pointerdown", handlePointerDown);
    };
  }, [detailsOpen]);

  if (!hasTask) return null;

  const resolvedScanTaskName = scanTaskName?.trim() || "当前图库";
  const resolvedSemanticTaskName = semanticTaskName?.trim() || "当前图库";
  const ratio = overallTaskProgressRatio(scanProgress, visibleSemanticProgress);
  const taskEntries: Array<{ name: string; operation: string }> = [];
  const activeTaskEntries: Array<{ name: string; operation: string }> = [];
  if (scanProgress) {
    const entry = {
      name: resolvedScanTaskName,
      operation: scanOperationLabel(scanProgress),
    };
    taskEntries.push(entry);
    if (scanRunning) activeTaskEntries.push(entry);
  }
  if (visibleSemanticProgress) {
    const entry = {
      name: resolvedSemanticTaskName,
      operation: semanticLabel(visibleSemanticProgress),
    };
    taskEntries.push(entry);
    if (semanticRunning) activeTaskEntries.push(entry);
  }
  const taskCount = activeTaskEntries.length > 0 ? activeTaskEntries.length : taskEntries.length;
  const summaryEntries = activeTaskEntries.length > 0 ? activeTaskEntries : taskEntries;
  const taskSummary =
    taskCount > 1
      ? taskCount + "个任务正在执行"
      : (summaryEntries[0]?.name ?? "当前任务") +
        " · " +
        (summaryEntries[0]?.operation ?? "处理中");

  return (
    <span ref={taskStatusRef} className="task-status-anchor">
      <button
        className="task-status-trigger"
        type="button"
        aria-label="查看后台任务"
        aria-expanded={detailsOpen}
        aria-controls="background-task-details"
        title="查看后台任务"
        onClick={() => setDetailsOpen((current) => !current)}
      >
        <span className="task-status-copy">
          <span className="task-status-dot" aria-hidden="true" />
          <span className="task-status-summary" title={taskSummary}>
            {taskSummary}
          </span>
        </span>
        <span className="task-status-mini-track" aria-hidden="true">
          <i style={{ width: ratio + "%" }} />
        </span>
        <span className="task-status-percent">{ratio}%</span>
      </button>
      {detailsOpen ? (
        <div
          id="background-task-details"
          className="task-status-popover"
          role="dialog"
          aria-label="后台任务详情"
        >
          <div className="task-status-popover-header">
            <strong>后台任务</strong>
          </div>
          <div className="task-status-list">
            {scanProgress ? (
              <ProgressPanel
                progress={scanProgress}
                taskName={resolvedScanTaskName}
                cancelling={cancellingScan}
                onCancel={onCancelScan}
                onDismiss={onDismissScan}
              />
            ) : null}
            {visibleSemanticProgress ? (
              <SemanticTaskDetails
                progress={visibleSemanticProgress}
                taskName={resolvedSemanticTaskName}
                onPauseResume={onPauseResumeSemantic}
                onCancel={onCancelSemantic}
              />
            ) : null}
          </div>
        </div>
      ) : null}
    </span>
  );
}
