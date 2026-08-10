import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

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
  groupBySemantic: false,
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

describe("OrganizationWorkspace", () => {
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

    expect(screen.getByText("只读整理预览")).toBeInTheDocument();
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
    expect(screen.getByText(/不会创建目录/)).toBeInTheDocument();
  });
});
