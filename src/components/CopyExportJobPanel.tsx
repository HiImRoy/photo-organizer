import { useEffect, useRef, useState } from "react";

import {
  cancelCopyExport,
  confirmCopyExportRollback,
  getCopyExportJob,
  getCopyExportRuntimeStatus,
  listRecentCopyExportJobs,
  preflightCopyExport,
  previewCopyExportResume,
  previewCopyExportRollback,
  reconcileCopyExportItem,
  resumeCopyExport,
  startCopyExport,
} from "../api";
import { formatBytes, formatDate } from "../format";
import type {
  CopyExportJobItemView,
  CopyExportJobState,
  CopyExportJobView,
  CopyExportPreflight,
  CopyExportReconcileDecision,
  CopyExportRuntimeStatus,
  OrganizationPlanSummaryV2,
  RecentCopyExportJob,
} from "../types";

const JOB_ITEM_PAGE_SIZE = 100;
const JOB_STATUS_POLL_MS = 1000;

interface CopyExportJobPanelProps {
  plan: OrganizationPlanSummaryV2 | null;
  planGeneration: number;
  jobIpcReady: boolean;
}

export function CopyExportJobPanel({ plan, planGeneration, jobIpcReady }: CopyExportJobPanelProps) {
  const [recentJobs, setRecentJobs] = useState<RecentCopyExportJob[]>([]);
  const [jobId, setJobId] = useState<string | null>(null);
  const [job, setJob] = useState<CopyExportJobView | null>(null);
  const [runtime, setRuntime] = useState<CopyExportRuntimeStatus | null>(null);
  const [preflight, setPreflight] = useState<CopyExportPreflight | null>(null);
  const [preflightPlanGeneration, setPreflightPlanGeneration] = useState<number | null>(null);
  const [selectedReconcile, setSelectedReconcile] = useState<
    Record<number, CopyExportReconcileDecision>
  >({});
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [pollRefresh, setPollRefresh] = useState(0);
  const pollGenerationRef = useRef(0);
  const actionGenerationRef = useRef(0);
  const planGenerationRef = useRef(planGeneration);
  const jobIdRef = useRef(jobId);
  const jobPageRef = useRef(1);

  useEffect(() => {
    planGenerationRef.current = planGeneration;
    jobIdRef.current = jobId;
    jobPageRef.current = job?.itemPage ?? 1;
  }, [job?.itemPage, jobId, planGeneration]);

  useEffect(() => {
    if (!jobIpcReady) return undefined;
    let active = true;
    void listRecentCopyExportJobs()
      .then((items) => {
        if (active) setRecentJobs(items);
      })
      .catch((reason: unknown) => {
        if (active) setError(messageFrom(reason));
      });
    return () => {
      active = false;
    };
  }, [jobIpcReady]);

  useEffect(() => {
    if (!jobIpcReady || !jobId) return undefined;
    let active = true;
    let timer: ReturnType<typeof setTimeout> | null = null;
    const generation = ++pollGenerationRef.current;

    const poll = async () => {
      try {
        const status = await getCopyExportRuntimeStatus(jobId);
        if (!active || generation !== pollGenerationRef.current) return;
        setRuntime(status);
        if (status?.running) {
          timer = setTimeout(() => void poll(), JOB_STATUS_POLL_MS);
          return;
        }
        const requestedPage = jobPageRef.current;
        const latestJob = await getCopyExportJob(jobId, requestedPage, JOB_ITEM_PAGE_SIZE);
        if (!active || generation !== pollGenerationRef.current) return;
        if (latestJob === null || latestJob.itemPage === jobPageRef.current) setJob(latestJob);
        if (latestJob !== null && isCopyJobActive(latestJob.state)) {
          timer = setTimeout(() => void poll(), JOB_STATUS_POLL_MS);
        } else {
          void listRecentCopyExportJobs()
            .then((items) => {
              if (active && generation === pollGenerationRef.current) setRecentJobs(items);
            })
            .catch((reason: unknown) => {
              if (active && generation === pollGenerationRef.current) setError(messageFrom(reason));
            });
        }
      } catch (reason) {
        if (active && generation === pollGenerationRef.current) {
          setError(messageFrom(reason));
          timer = setTimeout(() => void poll(), JOB_STATUS_POLL_MS * 3);
        }
      }
    };

    void poll();
    return () => {
      active = false;
      pollGenerationRef.current += 1;
      if (timer !== null) clearTimeout(timer);
    };
  }, [jobIpcReady, jobId, pollRefresh]);

  async function refreshRecentJobs() {
    const items = await listRecentCopyExportJobs();
    setRecentJobs(items);
  }

  async function loadJob(id: string) {
    actionGenerationRef.current += 1;
    setPreflight(null);
    setPreflightPlanGeneration(null);
    setSelectedReconcile({});
    jobPageRef.current = 1;
    if (!id) {
      setJobId(null);
      setJob(null);
      setRuntime(null);
      return;
    }
    setError(null);
    setNotice(null);
    setJobId(id);
    setJob(null);
    setRuntime(null);
    setBusy(true);
    const requestGeneration = actionGenerationRef.current;
    try {
      const result = await getCopyExportJob(id, 1, JOB_ITEM_PAGE_SIZE);
      if (requestGeneration === actionGenerationRef.current && jobIdRef.current === id) {
        setJob(result);
      }
    } catch (reason) {
      if (requestGeneration === actionGenerationRef.current && jobIdRef.current === id) {
        setError(messageFrom(reason));
      }
    } finally {
      if (requestGeneration === actionGenerationRef.current) setBusy(false);
    }
  }

  async function prepare(operation: "copy" | "resume" | "rollback", resourceId: string) {
    if (!jobIpcReady || busy) return;
    setBusy(true);
    setError(null);
    setNotice(null);
    setPreflight(null);
    setPreflightPlanGeneration(null);
    const requestGeneration = ++actionGenerationRef.current;
    const requestPlanGeneration = planGenerationRef.current;
    try {
      const preview =
        operation === "copy"
          ? await preflightCopyExport(resourceId, operation)
          : operation === "resume"
            ? await previewCopyExportResume(resourceId)
            : await previewCopyExportRollback(resourceId);
      const stillCurrent =
        requestGeneration === actionGenerationRef.current &&
        (operation === "copy"
          ? requestPlanGeneration === planGenerationRef.current
          : jobIdRef.current === resourceId);
      if (stillCurrent) {
        setPreflight(preview);
        setPreflightPlanGeneration(requestPlanGeneration);
      }
    } catch (reason) {
      if (requestGeneration === actionGenerationRef.current) setError(messageFrom(reason));
    } finally {
      if (requestGeneration === actionGenerationRef.current) setBusy(false);
    }
  }

  async function confirmPreparedOperation() {
    if (!preflight || busy) return;
    const prepared = preflight;
    if (
      (prepared.operation === "copy" &&
        (preflightPlanGeneration !== planGenerationRef.current ||
          !plan ||
          prepared.resourceId !== plan.planId ||
          prepared.planRevision !== plan.revision ||
          prepared.snapshotBlake3 !== plan.snapshotBlake3)) ||
      (prepared.operation !== "copy" && prepared.resourceId !== jobIdRef.current)
    ) {
      setPreflight(null);
      setNotice("复制范围或预览已变化；请重新检查并预览。");
      return;
    }
    // Authorization is one-use: consume it locally before making the start attempt.
    setPreflight(null);
    setBusy(true);
    setError(null);
    try {
      const result =
        prepared.operation === "copy"
          ? await startCopyExport(prepared.resourceId, prepared.authorizationToken)
          : prepared.operation === "resume"
            ? await resumeCopyExport(prepared.resourceId, prepared.authorizationToken)
            : await confirmCopyExportRollback(prepared.resourceId, prepared.authorizationToken);
      if (!result.accepted) {
        setNotice("任务未启动；请重新检查并预览后再试。旧授权已失效。");
        return;
      }
      setJob(null);
      setRuntime(null);
      jobPageRef.current = 1;
      setJobId(result.jobId);
      setPollRefresh((value) => value + 1);
      setNotice(operationStartMessage(prepared.operation));
      void refreshRecentJobs().catch((reason: unknown) => {
        setNotice(`任务已启动；刷新任务列表失败：${messageFrom(reason)}`);
      });
    } catch (reason) {
      setError(`${messageFrom(reason)}。请重新生成预览后重试。`);
    } finally {
      setBusy(false);
    }
  }

  async function requestCancel() {
    if (!jobId || busy) return;
    setBusy(true);
    setError(null);
    try {
      const accepted = await cancelCopyExport(jobId);
      setNotice(accepted ? "已请求停止；当前文件会安全收尾。" : "任务已不在运行。正在刷新状态。");
    } catch (reason) {
      setError(messageFrom(reason));
    } finally {
      setBusy(false);
    }
  }

  async function reconcileItem(item: CopyExportJobItemView) {
    if (!jobId || busy) return;
    const selectedJobId = jobId;
    const requestGeneration = actionGenerationRef.current;
    const decision = selectedReconcile[item.ordinal] ?? "keep_quarantined";
    setBusy(true);
    setError(null);
    setNotice(null);
    try {
      await reconcileCopyExportItem(jobId, item.ordinal, decision);
      if (requestGeneration !== actionGenerationRef.current || jobIdRef.current !== selectedJobId) {
        return;
      }
      setNotice(reconcileDecisionMessage(decision));
      const latestJob = await getCopyExportJob(
        selectedJobId,
        jobPageRef.current,
        JOB_ITEM_PAGE_SIZE,
      );
      if (requestGeneration === actionGenerationRef.current && jobIdRef.current === selectedJobId) {
        setJob(latestJob);
      }
    } catch (reason) {
      if (requestGeneration === actionGenerationRef.current && jobIdRef.current === selectedJobId) {
        setError(messageFrom(reason));
      }
    } finally {
      setBusy(false);
    }
  }

  async function changeJobPage(page: number) {
    if (!job || busy || page < 1 || page > Math.ceil(job.itemTotal / job.itemPageSize)) return;
    setBusy(true);
    setError(null);
    const selectedJobId = job.jobId;
    const requestGeneration = actionGenerationRef.current;
    try {
      const nextPage = await getCopyExportJob(selectedJobId, page, JOB_ITEM_PAGE_SIZE);
      if (requestGeneration === actionGenerationRef.current && jobIdRef.current === selectedJobId) {
        jobPageRef.current = nextPage?.itemPage ?? page;
        setJob(nextPage);
      }
    } catch (reason) {
      if (requestGeneration === actionGenerationRef.current && jobIdRef.current === selectedJobId) {
        setError(messageFrom(reason));
      }
    } finally {
      if (requestGeneration === actionGenerationRef.current) setBusy(false);
    }
  }

  const progress = runtime?.progress;
  const runtimeIsRunning = runtime?.running === true;
  const completedItems =
    (runtimeIsRunning ? progress?.completedItems : undefined) ?? job?.progressCompletedItems ?? 0;
  const totalItems =
    (runtimeIsRunning ? progress?.totalItems : undefined) ?? job?.progressTotalItems ?? 0;
  const completedBytes =
    (runtimeIsRunning ? progress?.bytesProcessed : undefined) ?? job?.progressCompletedBytes ?? 0;
  const totalBytes =
    (runtimeIsRunning ? progress?.totalBytes : undefined) ?? job?.progressTotalBytes ?? 0;
  const byteProgressLabel = runtimeIsRunning ? "已处理" : "累计已复制";
  const jobCanCancel = Boolean(
    (job && ["queued", "running", "cancelling"].includes(job.state)) || runtime?.running,
  );
  const jobCanResume = Boolean(
    job &&
    ["cancelled", "partial", "failed", "interrupted", "reconcile_required"].includes(job.state),
  );
  const jobCanRollback = Boolean(
    job && ["completed", "partial", "interrupted", "reconcile_required"].includes(job.state),
  );
  const activePreflight =
    preflight?.operation === "copy" &&
    (preflightPlanGeneration !== planGeneration ||
      !plan ||
      preflight.resourceId !== plan.planId ||
      preflight.planRevision !== plan.revision ||
      preflight.snapshotBlake3 !== plan.snapshotBlake3)
      ? null
      : preflight && preflight.operation !== "copy" && preflight.resourceId !== jobId
        ? null
        : preflight;

  return (
    <section className="organization-job-panel" aria-label="安全复制任务">
      <div className="organization-preview-heading">
        <div>
          <h2>复制任务</h2>
          <span>
            {job
              ? `${copyJobStateLabel(job.state)} · ${formatDate(job.createdAt)}`
              : "复制原文件字节，不转码"}
          </span>
          {job ? <span title={job.targetRoot}>{job.targetRoot}</span> : null}
        </div>
        <label className="organization-actions">
          <span className="sr-only">已保存的复制任务</span>
          <select
            aria-label="已保存的复制任务"
            value={jobId ?? ""}
            disabled={!jobIpcReady || busy}
            onChange={(event) => void loadJob(event.target.value)}
          >
            <option value="">选择任务记录</option>
            {recentJobs.map((recent) => (
              <option key={recent.jobId} value={recent.jobId}>
                {formatDate(recent.createdAt)} · {copyJobStateLabel(recent.state)} ·{" "}
                {recent.progressCompletedItems}/{recent.progressTotalItems} · {recent.targetRoot}
              </option>
            ))}
          </select>
        </label>
      </div>

      {!jobIpcReady ? (
        <p className="organization-space-note" role="status">
          复制任务仅在 PhotoOrganizer 桌面应用中可用。
        </p>
      ) : null}
      {error ? (
        <div className="organization-message is-error" role="alert">
          {error}
        </div>
      ) : null}
      {notice ? (
        <div className="organization-message" role="status">
          {notice}
        </div>
      ) : null}

      <div className="organization-actions" aria-label="复制任务操作">
        <button
          type="button"
          className="primary-action"
          disabled={
            !jobIpcReady || busy || !plan || plan.status !== "ready" || plan.blockedCount > 0
          }
          onClick={() => {
            if (plan) void prepare("copy", plan.planId);
          }}
        >
          检查并准备复制
        </button>
        {jobCanCancel ? (
          <button
            type="button"
            className="tool-button"
            disabled={busy}
            onClick={() => void requestCancel()}
          >
            请求停止
          </button>
        ) : null}
        {jobCanResume ? (
          <button
            type="button"
            className="tool-button"
            disabled={!jobIpcReady || busy}
            onClick={() => void prepare("resume", job!.jobId)}
          >
            重新检查并恢复
          </button>
        ) : null}
        {jobCanRollback ? (
          <button
            type="button"
            className="tool-button"
            disabled={!jobIpcReady || busy}
            onClick={() => void prepare("rollback", job!.jobId)}
          >
            预览撤销
          </button>
        ) : null}
      </div>

      {runtime?.progress ? (
        <p className="organization-message" role="status">
          {progressPhaseLabel(runtime.progress.phase)}
          {runtime.progress.currentOrdinal !== null
            ? ` · 正在处理第 ${runtime.progress.currentOrdinal + 1} 项`
            : ""}
        </p>
      ) : null}
      {runtime?.error ? (
        <div className="organization-message is-error" role="alert">
          {runtime.error}
        </div>
      ) : null}

      {job ? (
        <>
          <div className="organization-summary" aria-label="复制进度">
            <Metric label="已完成" value={completedItems} />
            <Metric label="总项目" value={totalItems} />
            <Metric
              label={byteProgressLabel}
              value={completedBytes}
              display={formatBytes(completedBytes)}
            />
            <Metric label="总大小" value={totalBytes} display={formatBytes(totalBytes)} />
          </div>
          <div className="organization-mapping-table" role="table" aria-label="复制任务项目">
            <div className="organization-mapping-row is-header" role="row">
              <span role="columnheader">项目</span>
              <span role="columnheader">目标位置</span>
              <span role="columnheader">状态</span>
            </div>
            {job.items.map((item) => (
              <div className="organization-mapping-row" role="row" key={item.ordinal}>
                <span role="cell">第 {item.ordinal + 1} 项</span>
                <span role="cell" title={item.targetRelativePath}>
                  {item.targetRelativePath}
                </span>
                <span className="mapping-status" role="cell">
                  {copyItemStateLabel(item.state)}
                  {item.state === "partial" || item.state === "reconcile_required" ? (
                    <span
                      className="organization-actions"
                      aria-label={`第 ${item.ordinal + 1} 项核对操作`}
                    >
                      <select
                        aria-label={`第 ${item.ordinal + 1} 项处理方式`}
                        value={selectedReconcile[item.ordinal] ?? "keep_quarantined"}
                        disabled={busy}
                        onChange={(event) =>
                          setSelectedReconcile((current) => ({
                            ...current,
                            [item.ordinal]: event.target.value as CopyExportReconcileDecision,
                          }))
                        }
                      >
                        <option value="keep_quarantined">保留隔离</option>
                        <option value="retry_when_target_absent">目标已确认不存在后重试</option>
                        <option value="adopt_verified_complete">目标核验完成后认领</option>
                      </select>
                      <button
                        type="button"
                        className="tool-button"
                        disabled={busy}
                        onClick={() => void reconcileItem(item)}
                      >
                        核对此项
                      </button>
                    </span>
                  ) : null}
                </span>
              </div>
            ))}
          </div>
          <div className="organization-actions" aria-label="复制任务分页">
            <button
              type="button"
              className="tool-button"
              disabled={busy || job.itemPage <= 1}
              onClick={() => void changeJobPage(job.itemPage - 1)}
            >
              上一页
            </button>
            <span>
              第 {job.itemPage} / {Math.max(1, Math.ceil(job.itemTotal / job.itemPageSize))} 页 · 共{" "}
              {job.itemTotal} 项
            </span>
            <button
              type="button"
              className="tool-button"
              disabled={busy || job.itemPage >= Math.ceil(job.itemTotal / job.itemPageSize)}
              onClick={() => void changeJobPage(job.itemPage + 1)}
            >
              下一页
            </button>
          </div>
        </>
      ) : (
        <p className="organization-space-note" role="status">
          {plan
            ? `尚无打开的任务。当前计划：${plan.targetRoot} · ${plan.eligibleCount.toLocaleString()} 项待复制。`
            : "生成或恢复计划后可检查复制；已保存任务可从上方列表恢复查看。"}
        </p>
      )}

      {activePreflight ? (
        <div className="modal-backdrop">
          <section
            className="import-library-dialog"
            role="dialog"
            aria-modal="true"
            aria-label={confirmationTitle(activePreflight.operation)}
          >
            <div className="dialog-heading">
              <h2>{confirmationTitle(activePreflight.operation)}</h2>
            </div>
            {activePreflight.operation === "copy" && plan ? (
              <p>
                范围：
                {plan.scope.kind === "query"
                  ? `当前全部匹配 ${plan.matchedCount.toLocaleString()} 张`
                  : `已选 ${plan.scope.assetIds.length.toLocaleString()} 张`}
              </p>
            ) : (
              <p>
                任务：
                {job
                  ? `${copyJobStateLabel(job.state)} · ${formatDate(job.createdAt)}`
                  : "已保存任务"}
              </p>
            )}
            <p className="dialog-path">目标：{activePreflight.targetRoot}</p>
            <p>
              项目：{activePreflight.itemCount.toLocaleString()} · 预计大小：
              {formatBytes(activePreflight.estimatedBytes)}
            </p>
            <p>阻止项：{activePreflight.blockedCount.toLocaleString()}</p>
            <p>授权将在 5 分钟内失效；确认后开始前会再次检查计划和目标目录。</p>
            <div className="dialog-actions">
              <button
                type="button"
                className="tool-button"
                disabled={busy}
                onClick={() => setPreflight(null)}
              >
                返回
              </button>
              <button
                type="button"
                className="primary-action"
                disabled={busy || activePreflight.blockedCount > 0}
                onClick={() => void confirmPreparedOperation()}
              >
                {busy ? "正在提交…" : "确认并继续"}
              </button>
            </div>
          </section>
        </div>
      ) : null}
    </section>
  );
}

function Metric({ label, value, display }: { label: string; value: number; display?: string }) {
  return (
    <div className="organization-metric">
      <span>{label}</span>
      <strong>{display ?? value.toLocaleString()}</strong>
    </div>
  );
}

function isCopyJobActive(state: CopyExportJobState) {
  return ["queued", "running", "cancelling", "rolling_back"].includes(state);
}

function copyJobStateLabel(state: CopyExportJobState) {
  const labels: Record<CopyExportJobState, string> = {
    queued: "排队中",
    running: "正在复制",
    cancelling: "正在停止",
    cancelled: "已停止",
    completed: "已完成",
    partial: "部分完成",
    failed: "失败",
    interrupted: "中断待核对",
    reconcile_required: "需要核对",
    rolling_back: "正在撤销",
    rolled_back: "已撤销",
  };
  return labels[state];
}

function copyItemStateLabel(state: CopyExportJobItemView["state"]) {
  const labels: Record<CopyExportJobItemView["state"], string> = {
    pending: "等待中",
    copying: "复制中",
    copied: "已复制",
    skipped: "已跳过",
    failed: "失败",
    cancelled: "已停止",
    partial: "未完成，隔离待核对",
    reconcile_required: "隔离待核对",
    rollback_pending: "等待撤销",
    rolled_back: "已撤销",
    rollback_blocked: "撤销受阻",
  };
  return labels[state];
}

function progressPhaseLabel(phase: NonNullable<CopyExportRuntimeStatus["progress"]>["phase"]) {
  const labels = {
    preparing: "正在准备",
    verifying_source: "正在核对原文件",
    copying: "正在复制",
    rolling_back: "正在撤销生成副本",
    completed: "任务完成",
  };
  return labels[phase];
}

function confirmationTitle(operation: CopyExportPreflight["operation"]) {
  return operation === "copy"
    ? "确认复制原文件"
    : operation === "resume"
      ? "确认恢复复制任务"
      : "确认撤销生成副本";
}

function operationStartMessage(operation: CopyExportPreflight["operation"]) {
  return operation === "copy"
    ? "复制任务已启动。"
    : operation === "resume"
      ? "恢复任务已启动。"
      : "撤销任务已启动。";
}

function reconcileDecisionMessage(decision: CopyExportReconcileDecision) {
  const labels: Record<CopyExportReconcileDecision, string> = {
    retry_when_target_absent: "已记录重试决定；目标仍不存在并通过校验后才会继续。",
    adopt_verified_complete: "已记录认领请求；仅完全匹配的目标会被标记为已完成。",
    keep_quarantined: "此项目仍保持隔离，文件不会被修改。",
  };
  return labels[decision];
}

function messageFrom(reason: unknown) {
  return reason instanceof Error ? reason.message : String(reason);
}
