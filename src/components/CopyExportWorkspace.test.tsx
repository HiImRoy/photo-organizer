import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

const api = vi.hoisted(() => ({
  isDesktopRuntime: vi.fn(() => false),
  chooseOrganizationTargetFolder: vi.fn(),
  grantOrganizationTargetParent: vi.fn(),
  createOrganizationPlanV2: vi.fn(),
  readOrganizationPlanV2: vi.fn(),
  listRecentOrganizationPlansV2: vi.fn(),
}));

vi.mock("../api", () => api);

import { CopyExportWorkspace } from "./CopyExportWorkspace";
import { assetQueryFromV1, createAssetQueryV1 } from "../query";
import type {
  CopyExportRules,
  OrganizationPlanItemV2,
  OrganizationPlanSummaryV2,
  OrganizationPlanV2,
  OrganizationPlanScopeV2,
  RecentOrganizationPlanV2,
} from "../types";

const query = assetQueryFromV1(createAssetQueryV1(null));
const queryScope: OrganizationPlanScopeV2 = { kind: "query", query };
const rules: CopyExportRules = {
  version: "copy-export-rules-v1",
  layout: "preserve_original",
  conflictPolicy: "block",
  missingCaptureTimePolicy: "block",
};

const summary: OrganizationPlanSummaryV2 = {
  planId: "plan-42",
  revision: "rev-1",
  status: "ready",
  scope: queryScope,
  targetRoot: "D:/导出/整理结果",
  matchedCount: 101,
  eligibleCount: 99,
  skippedCount: 1,
  blockedCount: 1,
  estimatedBytes: 4096,
  issues: [],
  snapshotBlake3: "snapshot-hash",
  createdAt: "2026-10-19T02:00:00Z",
  rules,
  tree: {
    name: "整理结果",
    relativePath: "",
    fileCount: 99,
    byteCount: 4096,
    children: [
      {
        name: "海边",
        relativePath: "海边",
        fileCount: 4,
        byteCount: 2048,
        children: [],
      },
    ],
  },
};

const item: OrganizationPlanItemV2 = {
  ordinal: 1,
  assetId: 12,
  libraryId: 7,
  sourceRootPath: "C:/图库",
  sourcePath: "C:/图库/海边/晚霞.png",
  sourceRelativePath: "海边/晚霞.png",
  sourceFingerprintBlake3: "source-hash",
  sourceSize: 4096,
  sourceModifiedAt: 1_760_000_000,
  captureTime: "2026-08-05T18:30:00",
  classificationRevision: 3,
  targetRelativePath: "海边/晚霞.png",
  targetPath: "D:/导出/整理结果/海边/晚霞.png",
  status: "ready",
  issues: [],
};

const planPage = (itemPage: number): OrganizationPlanV2 => ({
  summary,
  items: itemPage === 1 ? [item] : [{ ...item, ordinal: 101, assetId: 13 }],
  itemTotal: 101,
  itemPage,
  itemPageSize: 100,
});

beforeEach(() => {
  vi.clearAllMocks();
  api.chooseOrganizationTargetFolder.mockResolvedValue("D:/导出");
  api.grantOrganizationTargetParent.mockResolvedValue({
    grantId: "grant-1",
    displayPath: "D:/导出",
  });
  api.createOrganizationPlanV2.mockResolvedValue(summary);
  api.readOrganizationPlanV2.mockImplementation((_planId: string, page: number) =>
    Promise.resolve(planPage(page)),
  );
  api.listRecentOrganizationPlansV2.mockResolvedValue([
    {
      planId: summary.planId,
      revision: summary.revision,
      status: summary.status,
      targetRoot: summary.targetRoot,
      matchedCount: summary.matchedCount,
      eligibleCount: summary.eligibleCount,
      blockedCount: summary.blockedCount,
      createdAt: summary.createdAt,
    } satisfies RecentOrganizationPlanV2,
  ]);
});

describe("CopyExportWorkspace", () => {
  it("creates and pages a durable full-query preview with frozen path mappings", async () => {
    const user = userEvent.setup();
    render(
      <CopyExportWorkspace
        query={query}
        selectedAssetIds={[12]}
        matchedCount={250}
        planIpcReady
        onClose={vi.fn()}
      />,
    );

    await user.click(screen.getByRole("radio", { name: /当前全部匹配/ }));
    await user.click(screen.getByRole("button", { name: "选择" }));
    await waitFor(() => expect(api.grantOrganizationTargetParent).toHaveBeenCalledWith("D:/导出"));

    await user.clear(screen.getByLabelText("新目录名称"));
    await user.type(screen.getByLabelText("新目录名称"), "旅途");
    await user.click(screen.getByRole("button", { name: "生成只读预览" }));

    await screen.findByText("第 1 / 2 页 · 共 101 项");
    expect(api.createOrganizationPlanV2).toHaveBeenCalledWith({
      scope: { kind: "query", query },
      target: { parentGrantId: "grant-1", childName: "旅途" },
      rules,
    });
    expect(api.readOrganizationPlanV2).toHaveBeenCalledWith("plan-42", 1, 100);
    expect(screen.getByText("99", { selector: ".organization-metric strong" })).toBeInTheDocument();
    expect(screen.getAllByText("海边/晚霞.png").length).toBeGreaterThan(0);
    expect(screen.getByText("D:/导出/整理结果/海边/晚霞.png")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "下一页" }));
    await waitFor(() =>
      expect(api.readOrganizationPlanV2).toHaveBeenLastCalledWith("plan-42", 2, 100),
    );
    expect(screen.getByText("第 2 / 2 页 · 共 101 项")).toBeInTheDocument();

    await user.selectOptions(screen.getByLabelText("目录布局"), "capture_year_month");
    expect(screen.queryByText("第 2 / 2 页 · 共 101 项")).not.toBeInTheDocument();
    expect(screen.getByText(/设置已更改；旧预览已清除/)).toBeInTheDocument();
  });

  it("restores and displays the frozen settings from a saved plan", async () => {
    const user = userEvent.setup();
    const historicalSummary = {
      ...summary,
      rules: { ...rules, layout: "capture_year_month" as const },
    };
    api.readOrganizationPlanV2.mockResolvedValueOnce({
      ...planPage(1),
      summary: historicalSummary,
    });
    render(
      <CopyExportWorkspace
        query={query}
        selectedAssetIds={[]}
        matchedCount={3}
        planIpcReady
        onClose={vi.fn()}
      />,
    );

    await screen.findByRole("option", { name: /D:\/导出/ });
    await user.selectOptions(screen.getByLabelText("恢复已保存的预览"), "plan-42");
    await screen.findByText("第 1 / 2 页 · 共 101 项");
    expect(screen.getByLabelText("目录布局")).toHaveValue("capture_year_month");
    expect(screen.getByLabelText("冻结的预览设置")).toHaveTextContent("按拍摄年月整理");
  });

  it("clears a query-scoped preview when the active filter changes", async () => {
    const user = userEvent.setup();
    const props = {
      query,
      selectedAssetIds: [12],
      matchedCount: 3,
      planIpcReady: true,
      onClose: vi.fn(),
    };
    const view = render(<CopyExportWorkspace {...props} />);

    await user.click(screen.getByRole("radio", { name: /当前全部匹配/ }));
    await user.click(screen.getByRole("button", { name: "选择" }));
    await user.click(screen.getByRole("button", { name: "生成只读预览" }));
    await screen.findByText("第 1 / 2 页 · 共 101 项");

    view.rerender(
      <CopyExportWorkspace
        {...props}
        query={{ ...query, filter: { ...query.filter, ratings: [4] } }}
      />,
    );

    expect(await screen.findByText(/当前筛选已变化；旧预览已清除/)).toBeInTheDocument();
    expect(screen.queryByText("第 1 / 2 页 · 共 101 项")).not.toBeInTheDocument();
  });

  it("keeps plan creation unavailable in browser preview mode", async () => {
    const user = userEvent.setup();
    render(
      <CopyExportWorkspace
        query={query}
        selectedAssetIds={[]}
        matchedCount={3}
        onClose={vi.fn()}
      />,
    );

    await user.click(screen.getByRole("button", { name: "选择" }));
    expect(await screen.findAllByText(/整理预览仅在 PhotoOrganizer 桌面应用中可用/)).toHaveLength(
      2,
    );
    expect(screen.getByRole("button", { name: "生成只读预览" })).toBeDisabled();
    expect(api.grantOrganizationTargetParent).not.toHaveBeenCalled();
    expect(api.createOrganizationPlanV2).not.toHaveBeenCalled();
    expect(api.readOrganizationPlanV2).not.toHaveBeenCalled();
  });
});
