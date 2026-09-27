import { fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ComponentProps } from "react";
import { describe, expect, it, vi } from "vitest";

import type { ScanProgress, SemanticProgress } from "../types";
import { BackgroundTaskStatus } from "./BackgroundTaskStatus";

const scanProgress: ScanProgress = {
  taskId: "scan-1",
  libraryId: 7,
  status: "running",
  stage: "processing",
  discovered: 20,
  processed: 4,
  succeeded: 3,
  failed: 1,
  skipped: 2,
  missing: 0,
  currentPath: "C:\\fixtures\\scan\\four.png",
  error: null,
};

const legacyScanPerformance: NonNullable<ScanProgress["performance"]> = {
  discoveryUs: 1_250,
  ownershipLookupUs: 2_000,
  metadataLookupUs: 4_000,
  fingerprintUs: 8_500,
  imageProcessingUs: 20_000,
  exifUs: 0,
  sourceDimensionUs: 0,
  decodeUs: 8_000,
  sourceDecodeUs: 0,
  thumbnailDecodeUs: 8_000,
  resizeUs: 0,
  featureAnalysisUs: 1_250,
  thumbnailWriteUs: 0,
  databaseWriteUs: 3_500,
  processedFiles: 1,
  skippedFiles: 0,
  failedFiles: 0,
};

const scanPerformance: NonNullable<ScanProgress["performance"]> = {
  ...legacyScanPerformance,
  fileMetadataUs: 2_000,
  existingAssetLookupUs: 1_250,
  cacheProbeUs: 0,
  coldFiles: 3,
  reanalyzedFiles: 4,
  skippedFiles: 2,
};

const semanticProgress: SemanticProgress = {
  jobId: "semantic-1",
  libraryId: 7,
  status: "running",
  total: 12,
  processed: 3,
  completed: 3,
  failed: 0,
  skipped: 0,
  currentAssetId: 12,
  currentPath: "C:\\fixtures\\scan\\four.png",
  executionBackend: "cpu",
  modelName: "SigLIP2",
  modelVersion: "test",
  error: null,
};

function renderTaskStatus(overrides: Partial<ComponentProps<typeof BackgroundTaskStatus>> = {}) {
  return render(
    <div className="photo-app">
      <div className="library-stat">
        <BackgroundTaskStatus
          scanProgress={null}
          semanticProgress={null}
          scanRunning={false}
          semanticRunning={false}
          cancellingScan={false}
          onCancelScan={vi.fn()}
          onDismissScan={vi.fn()}
          onPauseResumeSemantic={vi.fn()}
          onCancelSemantic={vi.fn()}
          {...overrides}
        />
      </div>
    </div>,
  );
}

describe("BackgroundTaskStatus", () => {
  it("keeps scan details collapsed until the compact status is clicked", async () => {
    const user = userEvent.setup();
    renderTaskStatus({
      scanProgress,
      scanRunning: true,
      scanTaskName: "【2023】",
    });

    expect(screen.queryByRole("progressbar", { name: "扫描进度" })).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "查看后台任务" }));

    expect(screen.getByRole("progressbar", { name: "扫描进度" })).toHaveAttribute(
      "aria-valuenow",
      "20",
    );
    expect(screen.getByText("【2023】")).toBeInTheDocument();
    expect(screen.getByText("导入中")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "关闭任务详情" })).not.toBeInTheDocument();
    expect(screen.getByText("发现 20")).toBeInTheDocument();

    fireEvent.pointerDown(document.body);
    expect(screen.queryByRole("dialog", { name: "后台任务详情" })).not.toBeInTheDocument();
  });

  it("omits performance diagnostics when older scan progress has no counters", async () => {
    const user = userEvent.setup();
    renderTaskStatus({ scanProgress, scanRunning: true });

    await user.click(screen.getByRole("button", { name: "查看后台任务" }));

    expect(screen.queryByRole("group", { name: "扫描性能诊断" })).not.toBeInTheDocument();
    expect(screen.getByText("发现 20")).toBeInTheDocument();
  });

  it("keeps aggregate timings when legacy diagnostics lack the stage breakdown", async () => {
    const user = userEvent.setup();
    renderTaskStatus({
      scanProgress: { ...scanProgress, currentPath: null, performance: legacyScanPerformance },
      scanRunning: true,
    });

    await user.click(screen.getByRole("button", { name: "查看后台任务" }));

    const diagnostics = screen.getByRole("group", { name: "扫描性能诊断" });
    const lookupRow = within(diagnostics).getByText("元数据 / 归属查询").parentElement;
    expect(lookupRow).toHaveTextContent("6.0 ms");
    expect(diagnostics).not.toHaveTextContent("文件元数据");
    expect(diagnostics).not.toHaveTextContent("处理计数");
  });

  it("shows cumulative scan timings and identifies nested image-processing metrics", async () => {
    const user = userEvent.setup();
    renderTaskStatus({
      scanProgress: {
        ...scanProgress,
        currentPath: null,
        performance: scanPerformance,
      },
      scanRunning: true,
    });

    await user.click(screen.getByRole("button", { name: "查看后台任务" }));

    const diagnostics = screen.getByRole("group", { name: "扫描性能诊断" });
    expect(diagnostics).toHaveAttribute("aria-live", "off");
    const expectMetric = (label: string, value: string) => {
      const row = within(diagnostics).getByText(label).parentElement;
      expect(row).toHaveTextContent(value);
    };

    expectMetric("图片发现", "1.3 ms");
    expectMetric("元数据 / 归属查询", "6.0 ms");
    expectMetric("其中：文件元数据", "2.0 ms");
    expectMetric("其中：已有资源查询", "1.3 ms");
    expectMetric("其中：缓存探测", "0 μs");
    expectMetric("其中：归属查询", "2.0 ms");
    expectMetric("新文件已判定需处理", "3 张");
    expectMetric("已有文件需重分析", "4 张");
    expectMetric("已有文件跳过", "2 张");
    expectMetric("读文件 / 指纹", "8.5 ms");
    expectMetric("图像处理总计", "20.0 ms");
    expectMetric("其中：缩略图解码", "8.0 ms");
    expectMetric("其中：特征分析", "1.3 ms");
    expectMetric("数据库写入", "3.5 ms");
    expect(diagnostics).toHaveTextContent("可能并行重叠");
    expect(diagnostics).toHaveTextContent("不代表墙钟时长");
    expect(diagnostics).toHaveTextContent("不要重复相加");
    expect(diagnostics).not.toHaveTextContent("C:\\");
  });

  it("preserves full long paths and unbroken errors in scan details", async () => {
    const user = userEvent.setup();
    const longPath = `C:\\Users\\13002\\Pictures\\${"long-folder-name".repeat(24)}\\中文 图片.png`;
    const longError = "decoder-failed-".repeat(32);

    renderTaskStatus({
      scanProgress: { ...scanProgress, currentPath: longPath, error: longError },
      scanRunning: true,
    });

    await user.click(screen.getByRole("button", { name: "查看后台任务" }));

    const pathDetails = screen.getByTitle(longPath);
    expect(pathDetails).toHaveClass("current-path");
    expect(pathDetails).toHaveTextContent(longPath);
    expect(screen.getByRole("alert")).toHaveTextContent(longError);
  });

  it("puts semantic analysis in the same task details surface", async () => {
    const user = userEvent.setup();
    const onPauseResumeSemantic = vi.fn();
    renderTaskStatus({
      semanticProgress,
      semanticRunning: true,
      semanticTaskName: "默认收藏",
      onPauseResumeSemantic,
    });

    await user.click(screen.getByRole("button", { name: "查看后台任务" }));
    expect(screen.getByText("默认收藏")).toBeInTheDocument();
    expect(screen.getByText("分析中")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "暂停" }));
    expect(onPauseResumeSemantic).toHaveBeenCalledOnce();
  });

  it("disables semantic controls while one control request is pending", async () => {
    const user = userEvent.setup();
    renderTaskStatus({
      semanticProgress,
      semanticRunning: true,
      semanticControlBusy: true,
    });

    await user.click(screen.getByRole("button", { name: "查看后台任务" }));
    expect(screen.getByRole("button", { name: "暂停" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "取消" })).toBeDisabled();
  });

  it("keeps interrupted semantic jobs resumable", async () => {
    const user = userEvent.setup();
    const onPauseResumeSemantic = vi.fn();
    renderTaskStatus({
      semanticProgress: { ...semanticProgress, status: "interrupted" },
      semanticRunning: true,
      onPauseResumeSemantic,
    });

    await user.click(screen.getByRole("button", { name: "查看后台任务" }));
    expect(screen.getByText("分析已中断")).toBeInTheDocument();
    const resumeButton = screen.getByRole("button", { name: "继续" });
    expect(resumeButton).toBeEnabled();
    await user.click(resumeButton);
    expect(onPauseResumeSemantic).toHaveBeenCalledOnce();
  });

  it("shows the actual semantic execution backend", async () => {
    const user = userEvent.setup();
    renderTaskStatus({
      semanticProgress: { ...semanticProgress, executionBackend: "direct_ml" },
      semanticRunning: true,
    });

    await user.click(screen.getByRole("button", { name: "查看后台任务" }));
    expect(screen.getByText(/DirectML GPU/)).toBeInTheDocument();
  });
  it("summarizes parallel tasks with a weighted overall progress", async () => {
    const user = userEvent.setup();
    renderTaskStatus({
      scanProgress,
      semanticProgress,
      scanRunning: true,
      semanticRunning: true,
      scanTaskName: "【2023】",
      semanticTaskName: "默认收藏",
    });

    const trigger = screen.getByRole("button", { name: "查看后台任务" });
    expect(trigger).toHaveTextContent("2个任务正在执行");
    const miniProgress = trigger.querySelector<HTMLElement>(".task-status-mini-track i");
    expect(miniProgress).not.toBeNull();
    expect(miniProgress).toHaveStyle({ width: "22%" });

    await user.click(trigger);
    expect(screen.getByText("【2023】")).toBeInTheDocument();
    expect(screen.getByText("默认收藏")).toBeInTheDocument();
  });

  it("keeps the compact fill flush with its track inside the library stat", () => {
    renderTaskStatus({
      scanProgress: { ...scanProgress, discovered: 0, processed: 0 },
      scanRunning: true,
    });

    const trigger = screen.getByRole("button", { name: "查看后台任务" });
    const track = trigger.querySelector<HTMLElement>(".task-status-mini-track");
    const fill = track?.querySelector<HTMLElement>("i");
    expect(track).not.toBeNull();
    expect(fill).not.toBeNull();
    expect(track?.parentElement).toBe(trigger);
    expect(fill?.parentElement).toBe(track);
    expect(fill).toHaveStyle({ width: "0%" });
  });

  it("renders an 87 percent scan progress in the compact status", async () => {
    const user = userEvent.setup();
    renderTaskStatus({
      scanProgress: { ...scanProgress, discovered: 100, processed: 87 },
      scanRunning: true,
    });

    const trigger = screen.getByRole("button", { name: "查看后台任务" });
    expect(trigger).toHaveTextContent("87%");
    const fill = trigger.querySelector<HTMLElement>(".task-status-mini-track i");
    expect(fill).toHaveStyle({ width: "87%" });

    await user.click(trigger);
    expect(screen.getByRole("progressbar", { name: "扫描进度" })).toHaveAttribute(
      "aria-valuenow",
      "87",
    );
  });
});
