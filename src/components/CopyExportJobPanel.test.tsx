import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

const api = vi.hoisted(() => ({
  cancelCopyExport: vi.fn(),
  confirmCopyExportRollback: vi.fn(),
  getCopyExportJob: vi.fn(),
  getCopyExportRuntimeStatus: vi.fn(),
  listRecentCopyExportJobs: vi.fn(),
  preflightCopyExport: vi.fn(),
  previewCopyExportResume: vi.fn(),
  previewCopyExportRollback: vi.fn(),
  reconcileCopyExportItem: vi.fn(),
  resumeCopyExport: vi.fn(),
  startCopyExport: vi.fn(),
}));

vi.mock("../api", () => api);

import { CopyExportJobPanel } from "./CopyExportJobPanel";
import { assetQueryFromV1, createAssetQueryV1 } from "../query";
import { formatBytes } from "../format";
import type {
  CopyExportJobView,
  CopyExportPreflight,
  OrganizationPlanSummaryV2,
  RecentCopyExportJob,
} from "../types";

const plan: OrganizationPlanSummaryV2 = {
  planId: "plan-1",
  revision: "revision-1",
  status: "ready",
  scope: { kind: "query", query: assetQueryFromV1(createAssetQueryV1(null)) },
  targetRoot: "D:/导出/整理结果",
  matchedCount: 3,
  eligibleCount: 3,
  skippedCount: 0,
  blockedCount: 0,
  estimatedBytes: 4096,
  issues: [],
  snapshotBlake3: "snapshot-1",
  createdAt: "2026-10-19T02:00:00Z",
  rules: {
    version: "copy-export-rules-v1",
    layout: "preserve_original",
    conflictPolicy: "block",
    missingCaptureTimePolicy: "block",
  },
  tree: { name: "整理结果", relativePath: "", fileCount: 3, byteCount: 4096, children: [] },
};

const preflight: CopyExportPreflight = {
  resourceId: plan.planId,
  operation: "copy",
  authorizationToken: "once-only-token",
  expiresAtUnixMs: Date.now() + 300_000,
  targetRoot: plan.targetRoot,
  planRevision: plan.revision,
  snapshotBlake3: plan.snapshotBlake3,
  itemCount: 3,
  estimatedBytes: 4096,
  blockedCount: 0,
};

const completedJob: CopyExportJobView = {
  jobId: "job-1",
  planId: plan.planId,
  targetRoot: plan.targetRoot,
  state: "completed",
  progressCompletedItems: 3,
  progressTotalItems: 3,
  progressCompletedBytes: 4096,
  progressTotalBytes: 4096,
  currentItemOrdinal: null,
  cancelRequested: false,
  createdAt: "2026-10-19T02:00:00Z",
  updatedAt: "2026-10-19T02:01:00Z",
  items: [],
  itemTotal: 3,
  itemPage: 1,
  itemPageSize: 100,
};

const cancelledJob: CopyExportJobView = {
  ...completedJob,
  state: "cancelled",
  progressCompletedItems: 1,
  progressTotalItems: 3,
};

const partialJob: CopyExportJobView = {
  ...completedJob,
  state: "partial",
  progressCompletedItems: 0,
  progressTotalItems: 1,
  items: [
    {
      ordinal: 0,
      assetId: 81,
      state: "reconcile_required",
      attempts: 1,
      targetRelativePath: "原目录/照片.jpg",
      sourceSnapshot: {
        identity: null,
        sizeBytes: 1024,
        modifiedTime: "133719300000000000",
        blake3: "source-hash",
      },
      partialTargetIdentity: null,
      partialSnapshot: null,
      outputIdentity: null,
    },
  ],
  itemTotal: 1,
};

function recentJob(job: CopyExportJobView): RecentCopyExportJob {
  return {
    jobId: job.jobId,
    planId: job.planId,
    targetRoot: job.targetRoot,
    state: job.state,
    progressCompletedItems: job.progressCompletedItems,
    progressTotalItems: job.progressTotalItems,
    createdAt: job.createdAt,
    updatedAt: job.updatedAt,
  };
}

beforeEach(() => {
  vi.clearAllMocks();
  api.listRecentCopyExportJobs.mockResolvedValue([]);
  api.preflightCopyExport.mockResolvedValue(preflight);
  api.startCopyExport.mockResolvedValue({ jobId: "job-1", accepted: true });
  api.getCopyExportRuntimeStatus.mockResolvedValue(null);
  api.getCopyExportJob.mockResolvedValue(completedJob);
  api.cancelCopyExport.mockResolvedValue(true);
  api.previewCopyExportResume.mockResolvedValue({
    ...preflight,
    operation: "resume",
    resourceId: "job-1",
  });
  api.resumeCopyExport.mockResolvedValue({ jobId: "job-1", accepted: true });
  api.previewCopyExportRollback.mockResolvedValue({
    ...preflight,
    operation: "rollback",
    resourceId: "job-1",
  });
  api.confirmCopyExportRollback.mockResolvedValue({ jobId: "job-1", accepted: true });
  api.reconcileCopyExportItem.mockResolvedValue(undefined);
});

describe("CopyExportJobPanel", () => {
  it("shows the frozen copy scope and requires confirmation before starting", async () => {
    const user = userEvent.setup();
    render(<CopyExportJobPanel plan={plan} planGeneration={1} jobIpcReady />);

    await user.click(screen.getByRole("button", { name: "检查并准备复制" }));
    expect(await screen.findByRole("dialog", { name: "确认复制原文件" })).toBeInTheDocument();
    expect(screen.getByText("范围：当前全部匹配 3 张")).toBeInTheDocument();
    expect(screen.getByText("目标：D:/导出/整理结果")).toBeInTheDocument();
    expect(api.startCopyExport).not.toHaveBeenCalled();

    await user.click(screen.getByRole("button", { name: "确认并继续" }));
    await waitFor(() =>
      expect(api.startCopyExport).toHaveBeenCalledWith(plan.planId, "once-only-token"),
    );
    expect(await screen.findByText("已完成")).toBeInTheDocument();
  });

  it("discards a copy authorization when the frozen plan generation changes", async () => {
    const user = userEvent.setup();
    const view = render(<CopyExportJobPanel plan={plan} planGeneration={1} jobIpcReady />);
    await user.click(screen.getByRole("button", { name: "检查并准备复制" }));
    expect(await screen.findByRole("dialog", { name: "确认复制原文件" })).toBeInTheDocument();

    view.rerender(<CopyExportJobPanel plan={null} planGeneration={2} jobIpcReady />);
    expect(screen.queryByRole("dialog", { name: "确认复制原文件" })).not.toBeInTheDocument();
    expect(api.startCopyExport).not.toHaveBeenCalled();
  });

  it("shows running progress and lets the user request a safe stop", async () => {
    const user = userEvent.setup();
    api.getCopyExportRuntimeStatus.mockResolvedValue({
      jobId: "job-1",
      operation: "copy",
      running: true,
      progress: {
        jobId: "job-1",
        phase: "copying",
        completedItems: 1,
        totalItems: 3,
        currentOrdinal: 0,
        currentAssetId: 81,
        bytesProcessed: 512,
        totalBytes: 4096,
      },
      error: null,
    });
    render(<CopyExportJobPanel plan={plan} planGeneration={1} jobIpcReady />);

    await user.click(screen.getByRole("button", { name: "检查并准备复制" }));
    await user.click(await screen.findByRole("button", { name: "确认并继续" }));

    await waitFor(() => expect(api.getCopyExportRuntimeStatus).toHaveBeenCalledWith("job-1"));
    expect(
      Array.from(document.querySelectorAll(".organization-job-panel [role='status']")).some(
        (element) => element.textContent?.includes("正在复制 · 正在处理第 1 项"),
      ),
    ).toBe(true);
    await user.click(screen.getByRole("button", { name: "请求停止" }));
    expect(api.cancelCopyExport).toHaveBeenCalledWith("job-1");
    expect(await screen.findByText("已请求停止；当前文件会安全收尾。")).toBeInTheDocument();
  });

  it("shows persisted copied bytes after cancel cleanup instead of the aborted current-file attempt", async () => {
    const user = userEvent.setup();
    const runningJob: CopyExportJobView = {
      ...completedJob,
      state: "running",
      progressCompletedItems: 1,
      progressCompletedBytes: 1024,
    };
    const cancelledJobAfterCleanup: CopyExportJobView = {
      ...runningJob,
      state: "cancelled",
    };
    const runningStatus = {
      jobId: runningJob.jobId,
      operation: "copy" as const,
      running: true,
      progress: {
        jobId: runningJob.jobId,
        phase: "copying" as const,
        completedItems: 1,
        totalItems: 3,
        currentOrdinal: 1,
        currentAssetId: 82,
        bytesProcessed: 3584,
        totalBytes: 4096,
      },
      error: null,
    };
    api.listRecentCopyExportJobs.mockResolvedValue([recentJob(runningJob)]);
    api.getCopyExportJob
      .mockResolvedValueOnce(runningJob)
      .mockResolvedValueOnce(cancelledJobAfterCleanup);
    api.getCopyExportRuntimeStatus
      .mockResolvedValueOnce(runningStatus)
      .mockResolvedValueOnce({ ...runningStatus, running: false });
    render(<CopyExportJobPanel plan={null} planGeneration={1} jobIpcReady />);

    await user.selectOptions(
      await screen.findByRole("combobox", { name: "已保存的复制任务" }),
      runningJob.jobId,
    );
    expect(await screen.findByText("已处理")).toBeInTheDocument();
    expect(
      screen.getByText("已处理").closest(".organization-metric")?.querySelector("strong"),
    ).toHaveTextContent(formatBytes(3584));

    await user.click(screen.getByRole("button", { name: "请求停止" }));
    expect(api.cancelCopyExport).toHaveBeenCalledWith(runningJob.jobId);
    await waitFor(() => expect(api.getCopyExportRuntimeStatus).toHaveBeenCalledTimes(2), {
      timeout: 2500,
    });
    await waitFor(() => expect(screen.getByText("累计已复制")).toBeInTheDocument());

    const copiedBytes = screen
      .getByText("累计已复制")
      .closest(".organization-metric")
      ?.querySelector("strong");
    expect(copiedBytes).toHaveTextContent(formatBytes(1024));
    expect(copiedBytes).not.toHaveTextContent(formatBytes(3584));
    expect(screen.getByRole("heading", { name: "复制任务" }).parentElement).toHaveTextContent(
      "已停止",
    );
  });

  it("restores a saved cancelled job and requires a fresh preview before resume", async () => {
    const user = userEvent.setup();
    api.listRecentCopyExportJobs.mockResolvedValue([recentJob(cancelledJob)]);
    api.getCopyExportJob.mockResolvedValue(cancelledJob);
    api.previewCopyExportResume.mockResolvedValue({
      ...preflight,
      operation: "resume",
      resourceId: cancelledJob.jobId,
      authorizationToken: "resume-token",
    });
    render(<CopyExportJobPanel plan={null} planGeneration={1} jobIpcReady />);

    await user.selectOptions(
      await screen.findByRole("combobox", { name: "已保存的复制任务" }),
      "job-1",
    );
    expect(await screen.findByRole("button", { name: "重新检查并恢复" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "重新检查并恢复" }));
    expect(await screen.findByRole("dialog", { name: "确认恢复复制任务" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "确认并继续" }));

    await waitFor(() => expect(api.resumeCopyExport).toHaveBeenCalledWith("job-1", "resume-token"));
  });

  it("restores a completed job and requires confirmation for owned-copy rollback", async () => {
    const user = userEvent.setup();
    api.listRecentCopyExportJobs.mockResolvedValue([recentJob(completedJob)]);
    api.getCopyExportJob.mockResolvedValue(completedJob);
    api.previewCopyExportRollback.mockResolvedValue({
      ...preflight,
      operation: "rollback",
      resourceId: completedJob.jobId,
      authorizationToken: "rollback-token",
    });
    render(<CopyExportJobPanel plan={null} planGeneration={1} jobIpcReady />);

    await user.selectOptions(
      await screen.findByRole("combobox", { name: "已保存的复制任务" }),
      "job-1",
    );
    expect(await screen.findByText("已完成")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "预览撤销" }));
    expect(await screen.findByRole("dialog", { name: "确认撤销生成副本" })).toBeInTheDocument();
    expect(api.confirmCopyExportRollback).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "确认并继续" }));

    await waitFor(() =>
      expect(api.confirmCopyExportRollback).toHaveBeenCalledWith("job-1", "rollback-token"),
    );
  });

  it("keeps quarantined item handling explicit and sends its zero-based ordinal unchanged", async () => {
    const user = userEvent.setup();
    api.listRecentCopyExportJobs.mockResolvedValue([recentJob(partialJob)]);
    api.getCopyExportJob.mockResolvedValue(partialJob);
    render(<CopyExportJobPanel plan={null} planGeneration={1} jobIpcReady />);

    await user.selectOptions(
      await screen.findByRole("combobox", { name: "已保存的复制任务" }),
      "job-1",
    );
    expect(await screen.findByText("第 1 项")).toBeInTheDocument();
    await user.selectOptions(
      screen.getByRole("combobox", { name: "第 1 项处理方式" }),
      "keep_quarantined",
    );
    await user.click(screen.getByRole("button", { name: "核对此项" }));

    await waitFor(() =>
      expect(api.reconcileCopyExportItem).toHaveBeenCalledWith("job-1", 0, "keep_quarantined"),
    );
    expect(await screen.findByText("此项目仍保持隔离，文件不会被修改。")).toBeInTheDocument();
  });
});
