import { StrictMode, type ComponentProps } from "react";
import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { OrganizationWorkspace } from "./OrganizationWorkspace";
import { emptyAssetFilter, type LibrarySummary, type OrganizationPlan } from "../types";

const api = vi.hoisted(() => ({
  chooseOrganizationTargetFolder: vi.fn(),
  exportOrganizationManifest: vi.fn(),
  previewOrganizationPlan: vi.fn(),
}));

vi.mock("../api", () => api);

const library: LibrarySummary = {
  id: 4,
  rootPath: "C:\\fixtures\\中文 图库",
  name: "中文 图库",
  sourcePath: "C:\\fixtures\\中文 图库",
  sourceIdentityKey: "c:/fixtures/中文 图库",
  parentLibraryId: null,
  displayOrder: 0,
  createdAt: "2026-08-06T10:00:00Z",
  lastScanAt: "2026-08-06T10:10:00Z",
  status: "ready",
  assetCount: 2,
  presentCount: 2,
  missingCount: 0,
  semanticPendingCount: 0,
};

const testQuery = {
  version: 1 as const,
  libraryId: library.id,
  filter: emptyAssetFilter,
  sort: "file_name" as const,
  direction: "asc" as const,
  page: 1,
  pageSize: 120,
};

const plan: OrganizationPlan = {
  summary: {
    planId: "plan-1",
    libraryId: 4,
    sourceRoot: library.rootPath,
    targetRoot: "D:\\整理预览",
    scope: "filtered",
    itemCount: 1,
    conflictCount: 1,
    errorCount: 0,
    warningCount: 1,
    estimatedBytes: 2048,
    targetAvailableBytes: null,
    generatedAt: "2026-08-07T10:00:00Z",
    status: "has_warnings",
    sourceSnapshot: "snapshot",
    rules: {
      version: "organization-rules-v1",
      levels: [
        { kind: "year", fallback: "modification_time" },
        { kind: "primary_semantic", fallback: "unknown" },
      ],
      template: "{original_stem}_{sequence:0000}",
      sequenceStart: 1,
      sequenceWidth: 4,
      missingFallback: "unknown",
      conflictStrategy: "sequence",
    },
  },
  items: [
    {
      ordinal: 1,
      assetId: 22,
      sourcePath: "C:\\fixtures\\中文 图库\\晚霞😀.jpg",
      sourceRelativePath: "晚霞😀.jpg",
      sourceFingerprint: "aabbccdd",
      targetRelativePath: "2026\\sunset\\晚霞😀_0001.jpg",
      targetPath: "D:\\整理预览\\2026\\sunset\\晚霞😀_0001.jpg",
      fileSize: 2048,
      status: "warning",
      variables: { semantic: "sunset", sequence: "0001" },
      issues: [
        {
          code: "duplicate_target",
          severity: "warning",
          sourcePath: "C:\\fixtures\\中文 图库\\晚霞😀.jpg",
          targetPath: "D:\\整理预览\\2026\\sunset\\晚霞😀.jpg",
          detail: "多个源文件映射到同一目标路径。",
        },
      ],
    },
  ],
  tree: {
    name: "整理预览",
    relativePath: "",
    fileCount: 1,
    byteCount: 2048,
    children: [{ name: "2026", relativePath: "2026", fileCount: 1, byteCount: 2048, children: [] }],
  },
};

type WorkspaceProps = ComponentProps<typeof OrganizationWorkspace>;

const workspaceProps: WorkspaceProps = {
  library,
  selectedAssetIds: [22],
  filteredCount: 1,
  scopeInput: { kind: "selection", query: testQuery, assetIds: [22] },
  scopeDescription: {
    kind: "selection",
    label: "已选择 1 张",
    count: 1,
    isExplicitSelection: true,
  },
  onClose: vi.fn(),
};

function renderWorkspace(overrides: Partial<WorkspaceProps> = {}) {
  return render(<OrganizationWorkspace {...workspaceProps} {...overrides} />);
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((nextResolve) => {
    resolve = nextResolve;
  });
  return { promise, resolve };
}

const planTwo: OrganizationPlan = {
  ...plan,
  summary: {
    ...plan.summary,
    planId: "plan-2",
    targetRoot: "D:\\第二次整理预览",
  },
  tree: {
    ...plan.tree,
    name: "第二次整理预览",
  },
};

describe("OrganizationWorkspace", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("explains directory dimensions and limits modification-time fallback to date levels", async () => {
    const user = userEvent.setup();
    render(
      <OrganizationWorkspace
        library={library}
        selectedAssetIds={[22]}
        filteredCount={1}
        scopeInput={{ kind: "selection", query: testQuery, assetIds: [22] }}
        scopeDescription={{
          kind: "selection",
          label: "已选择 1 张",
          count: 1,
          isExplicitSelection: true,
        }}
        onClose={vi.fn()}
      />,
    );

    expect(screen.getByText("目录内容")).toBeInTheDocument();
    expect(screen.getByText("缺失时")).toBeInTheDocument();
    const firstDimension = screen.getByRole("combobox", { name: "第 1 层目录维度" });
    const firstFallback = screen.getByRole("combobox", { name: "拍摄年份缺失时" });
    expect(within(firstDimension).getByRole("option", { name: "拍摄年份" })).toBeInTheDocument();
    expect(
      within(firstFallback).getByRole("option", { name: "用文件修改时间" }),
    ).toBeInTheDocument();

    await user.selectOptions(firstDimension, "primary_semantic");

    const semanticFallback = screen.getAllByRole("combobox", { name: "拍摄题材缺失时" })[0];
    expect(semanticFallback).toHaveValue("unknown");
    expect(
      within(semanticFallback).queryByRole("option", { name: "用文件修改时间" }),
    ).not.toBeInTheDocument();
    expect(screen.getAllByText(/按图片的主要题材分目录/).length).toBeGreaterThan(0);
  });

  it("inherits an explicit selection and generates a read-only mapping", async () => {
    const user = userEvent.setup();
    api.previewOrganizationPlan.mockResolvedValue(plan);
    api.exportOrganizationManifest.mockResolvedValue("D:\\整理预览.json");
    render(
      <OrganizationWorkspace
        library={library}
        selectedAssetIds={[22]}
        filteredCount={1}
        scopeInput={{ kind: "selection", query: testQuery, assetIds: [22] }}
        scopeDescription={{
          kind: "selection",
          label: "已选择 1 张",
          count: 1,
          isExplicitSelection: true,
        }}
        onClose={vi.fn()}
      />,
    );

    expect(screen.getByText("只读预览 · 不会修改源文件")).toBeInTheDocument();
    expect(screen.getByText("尚未生成")).toBeInTheDocument();
    expect(screen.getByRole("region", { name: "整理预览工作区" })).toBeInTheDocument();
    await user.type(screen.getByLabelText("目标根目录"), "D:\\整理预览");
    await user.click(screen.getByRole("button", { name: "生成整理预览" }));

    expect(api.previewOrganizationPlan).toHaveBeenCalledWith(
      expect.objectContaining({
        targetRoot: "D:\\整理预览",
        scope: "selected",
        filter: emptyAssetFilter,
      }),
    );
    expect(await screen.findByText("晚霞😀.jpg")).toBeInTheDocument();
    expect(screen.getByText("冲突")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "导出 JSON" }));
    expect(api.exportOrganizationManifest).toHaveBeenCalledWith(plan, "json");
    expect(screen.getByText("只读预览 · 不会修改源文件")).toBeInTheDocument();
  });

  it("re-arms the mounted guard after StrictMode effect replay", async () => {
    const user = userEvent.setup();
    api.previewOrganizationPlan.mockResolvedValue(plan);
    render(
      <StrictMode>
        <OrganizationWorkspace {...workspaceProps} />
      </StrictMode>,
    );

    await user.type(screen.getByLabelText("目标根目录"), "D:\\严格模式");
    await user.click(screen.getByRole("button", { name: "生成整理预览" }));

    expect(await screen.findByText("晚霞😀.jpg")).toBeInTheDocument();
  });

  it("hides an invalidated plan for every organization input change", async () => {
    const user = userEvent.setup();
    api.previewOrganizationPlan.mockResolvedValue(plan);
    const view = renderWorkspace();
    const target = screen.getByLabelText("目标根目录");

    await user.type(target, "D:\\起始目标");
    await user.click(screen.getByRole("button", { name: "生成整理预览" }));
    expect(await screen.findByText("晚霞😀.jpg")).toBeInTheDocument();

    const expectPlanInvalidated = () => {
      expect(screen.getByText("尚未生成")).toBeInTheDocument();
      expect(screen.queryByRole("button", { name: "导出 JSON" })).not.toBeInTheDocument();
    };
    const regenerate = async () => {
      await user.click(screen.getByRole("button", { name: "生成整理预览" }));
      expect(await screen.findByText("晚霞😀.jpg")).toBeInTheDocument();
    };

    await user.clear(target);
    await user.type(target, "D:\\新目标");
    expectPlanInvalidated();
    await regenerate();

    await user.selectOptions(screen.getByRole("combobox", { name: "第 1 层目录维度" }), "month");
    expectPlanInvalidated();
    await regenerate();

    view.rerender(
      <OrganizationWorkspace
        {...workspaceProps}
        selectedAssetIds={[23]}
        scopeInput={{ kind: "selection", query: testQuery, assetIds: [23] }}
      />,
    );
    expectPlanInvalidated();
    await regenerate();

    const filteredQuery = {
      ...testQuery,
      filter: { ...emptyAssetFilter, search: "海边" },
    };
    view.rerender(
      <OrganizationWorkspace
        {...workspaceProps}
        selectedAssetIds={[23]}
        scopeInput={{ kind: "selection", query: filteredQuery, assetIds: [23] }}
      />,
    );
    expectPlanInvalidated();
    await regenerate();

    const secondLibrary: LibrarySummary = {
      ...library,
      id: 5,
      rootPath: "D:\\fixtures\\另一个图库",
      name: "另一个图库",
      sourcePath: "D:\\fixtures\\另一个图库",
      sourceIdentityKey: "d:/fixtures/另一个图库",
    };
    const secondLibraryQuery = { ...filteredQuery, libraryId: secondLibrary.id };
    view.rerender(
      <OrganizationWorkspace
        {...workspaceProps}
        library={secondLibrary}
        selectedAssetIds={[23]}
        scopeInput={{ kind: "selection", query: secondLibraryQuery, assetIds: [23] }}
      />,
    );
    expectPlanInvalidated();
    await regenerate();
  });

  it("ignores out-of-order previews and an older finally cannot clear newer busy", async () => {
    const user = userEvent.setup();
    const first = deferred<OrganizationPlan>();
    const second = deferred<OrganizationPlan>();
    api.previewOrganizationPlan
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(second.promise);
    renderWorkspace();
    const target = screen.getByLabelText("目标根目录");

    await user.type(target, "D:\\第一次");
    await user.click(screen.getByRole("button", { name: "生成整理预览" }));
    await waitFor(() => expect(api.previewOrganizationPlan).toHaveBeenCalledTimes(1));

    await user.clear(target);
    await user.type(target, "D:\\第二次");
    expect(screen.getByText("尚未生成")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "导出 JSON" })).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "生成整理预览" }));
    await waitFor(() => expect(api.previewOrganizationPlan).toHaveBeenCalledTimes(2));

    await act(async () => {
      first.resolve(plan);
      await Promise.resolve();
    });
    expect(screen.getByRole("button", { name: "正在生成…" })).toBeInTheDocument();
    expect(screen.queryByText("晚霞😀.jpg")).not.toBeInTheDocument();

    await act(async () => {
      second.resolve(planTwo);
      await Promise.resolve();
    });
    expect(await screen.findByText("晚霞😀.jpg")).toBeInTheDocument();
    expect(screen.getByText("第二次整理预览")).toBeInTheDocument();
  });

  it("does not revive a previous plan when inputs change from A to B and back to A", async () => {
    const user = userEvent.setup();
    api.previewOrganizationPlan.mockResolvedValue(plan);
    renderWorkspace();
    const target = screen.getByLabelText("目标根目录");

    await user.type(target, "D:\\A");
    await user.click(screen.getByRole("button", { name: "生成整理预览" }));
    expect(await screen.findByText("晚霞😀.jpg")).toBeInTheDocument();

    await user.clear(target);
    await user.type(target, "D:\\B");
    expect(screen.getByText("尚未生成")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "导出 JSON" })).not.toBeInTheDocument();

    await user.clear(target);
    await user.type(target, "D:\\A");
    expect(screen.getByText("尚未生成")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "导出 JSON" })).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "生成整理预览" }));

    expect(await screen.findByText("晚霞😀.jpg")).toBeInTheDocument();
    expect(api.previewOrganizationPlan).toHaveBeenCalledTimes(2);
    expect(api.previewOrganizationPlan).toHaveBeenLastCalledWith(
      expect.objectContaining({ targetRoot: "D:\\A" }),
    );
  });

  it("ignores an export response after the plan inputs become stale", async () => {
    const user = userEvent.setup();
    const pendingExport = deferred<string>();
    api.previewOrganizationPlan.mockResolvedValue(plan);
    api.exportOrganizationManifest.mockReturnValue(pendingExport.promise);
    renderWorkspace();
    const target = screen.getByLabelText("目标根目录");

    await user.type(target, "D:\\导出基准");
    await user.click(screen.getByRole("button", { name: "生成整理预览" }));
    expect(await screen.findByText("晚霞😀.jpg")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "导出 JSON" }));
    expect(api.exportOrganizationManifest).toHaveBeenCalledWith(plan, "json");

    await user.clear(target);
    await user.type(target, "D:\\导出后变更");
    expect(screen.queryByRole("button", { name: "导出 JSON" })).not.toBeInTheDocument();

    await act(async () => {
      pendingExport.resolve("D:\\过期.json");
      await Promise.resolve();
    });
    expect(screen.queryByText(/已导出/)).not.toBeInTheDocument();
  });

  it("ignores preview and export completions after unmount", async () => {
    const user = userEvent.setup();
    const pendingPreview = deferred<OrganizationPlan>();
    api.previewOrganizationPlan.mockReturnValueOnce(pendingPreview.promise);
    const previewView = renderWorkspace();
    const previewTarget = screen.getByLabelText("目标根目录");

    await user.type(previewTarget, "D:\\卸载预览");
    await user.click(screen.getByRole("button", { name: "生成整理预览" }));
    await waitFor(() => expect(api.previewOrganizationPlan).toHaveBeenCalledTimes(1));
    previewView.unmount();
    await act(async () => {
      pendingPreview.resolve(plan);
      await Promise.resolve();
    });

    const pendingExport = deferred<string>();
    api.previewOrganizationPlan.mockResolvedValueOnce(plan);
    api.exportOrganizationManifest.mockReturnValueOnce(pendingExport.promise);
    const exportView = renderWorkspace();
    const exportTarget = screen.getByLabelText("目标根目录");
    await user.type(exportTarget, "D:\\卸载导出");
    await user.click(screen.getByRole("button", { name: "生成整理预览" }));
    expect(await screen.findByText("晚霞😀.jpg")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "导出 JSON" }));
    exportView.unmount();
    await act(async () => {
      pendingExport.resolve("D:\\卸载.json");
      await Promise.resolve();
    });
  });
});
