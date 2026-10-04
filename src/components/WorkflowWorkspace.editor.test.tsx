import { type ComponentProps } from "react";
import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type {
  AssetListItem,
  EditExportPlan,
  EditExportResult,
  EditRecipe,
  EditRollbackPlan,
} from "../types";

const api = vi.hoisted(() => ({
  chooseEditedCopyTarget: vi.fn(),
  executeEditExport: vi.fn(),
  executeEditRollback: vi.fn(),
  previewEditExport: vi.fn(),
  previewEditRollback: vi.fn(),
  renderEditPreview: vi.fn(),
}));

vi.mock("../api", () => api);

import { WorkflowWorkspace } from "./WorkflowWorkspace";
import { emptyEditRecipe, emptyAssetFilter } from "../types";

const asset = {
  id: 42,
  fileName: "编辑照片.jpg",
  width: 1600,
  height: 1200,
} as AssetListItem;

const targetOne = "D:\\exports\\编辑照片-one.jpg";
const targetTwo = "D:\\exports\\编辑照片-two.jpg";

type WorkspaceProps = ComponentProps<typeof WorkflowWorkspace>;

const query = {
  version: 1 as const,
  libraryId: 1,
  filter: emptyAssetFilter,
  sort: "file_name" as const,
  direction: "asc" as const,
  page: 1,
  pageSize: 120,
};

const workspaceProps: WorkspaceProps = {
  libraryId: 1,
  selectedAssetIds: [],
  activeAsset: asset,
  scope: { kind: "selection", query, assetIds: [asset.id] },
  scopeDescription: {
    kind: "selection",
    label: "已选择 1 张",
    count: 1,
    isExplicitSelection: true,
  },
  onSelectAsset: vi.fn(),
  onToggleSelection: vi.fn(),
  onUpdateRating: vi.fn(),
  onUpdateColorLabel: vi.fn(),
  onOpenAsset: vi.fn(),
  onBack: vi.fn(),
  onFavoriteChange: vi.fn(),
  initialTool: "edit",
};

function makeExportPlan(
  planId: string,
  targetPath: string,
  requestedRecipe: EditRecipe = emptyEditRecipe,
  sourceAssetId = asset.id,
): EditExportPlan {
  return {
    planId,
    assetId: sourceAssetId,
    sourcePath: `D:\\图库\\${asset.fileName}`,
    targetPath,
    sourceFingerprint: "source-fingerprint",
    recipe: {
      ...requestedRecipe,
      crop: requestedRecipe.crop ? { ...requestedRecipe.crop } : null,
    },
    status: "ready",
    issues: [],
  };
}

function makeExportResult(planId = "export-plan", targetPath = targetOne): EditExportResult {
  return { planId, targetPath, status: "completed" };
}

function makeRollbackPlan(planId = "export-plan"): EditRollbackPlan {
  return {
    planId,
    targetPath: targetOne,
    targetHash: "generated-file-hash",
    status: "ready",
    issues: [],
  };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((nextResolve) => {
    resolve = nextResolve;
  });
  return { promise, resolve };
}

function renderEditor(overrides: Partial<WorkspaceProps> = {}) {
  return render(<WorkflowWorkspace {...workspaceProps} {...overrides} />);
}

async function requestPlan(user: ReturnType<typeof userEvent.setup>) {
  await user.click(screen.getByRole("button", { name: "预览另存计划" }));
  return screen.findByText(targetOne);
}

describe("WorkflowWorkspace editor export interactions", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    api.renderEditPreview.mockResolvedValue("data:image/jpeg;base64,preview");
    api.chooseEditedCopyTarget.mockResolvedValue(targetOne);
    api.previewEditExport.mockImplementation(
      (assetId: number, targetPath: string, requestedRecipe: EditRecipe) =>
        Promise.resolve(makeExportPlan(`plan-${targetPath}`, targetPath, requestedRecipe, assetId)),
    );
    api.executeEditExport.mockImplementation((planId: string) =>
      Promise.resolve(makeExportResult(planId)),
    );
    api.previewEditRollback.mockImplementation((planId: string) =>
      Promise.resolve(makeRollbackPlan(planId)),
    );
    api.executeEditRollback.mockResolvedValue(makeExportResult());
  });

  it("invalidates a displayed and pending plan when Reset is used", async () => {
    const user = userEvent.setup();
    renderEditor();

    await requestPlan(user);
    await user.click(screen.getByRole("button", { name: "重置配方" }));
    expect(screen.queryByText(targetOne)).not.toBeInTheDocument();

    const pendingTarget = deferred<string | null>();
    api.chooseEditedCopyTarget.mockReturnValueOnce(pendingTarget.promise);
    await user.click(screen.getByRole("button", { name: "旋转 90°" }));
    await user.click(screen.getByRole("button", { name: "预览另存计划" }));
    await waitFor(() => expect(api.chooseEditedCopyTarget).toHaveBeenCalledTimes(2));
    expect(screen.getByRole("button", { name: "准备中…" })).toBeDisabled();
    await user.click(screen.getByRole("button", { name: "重置配方" }));
    expect(screen.getByRole("button", { name: "预览另存计划" })).toBeEnabled();

    await act(async () => {
      pendingTarget.resolve(targetTwo);
    });

    expect(api.previewEditExport).toHaveBeenCalledTimes(1);
    expect(screen.queryByText(targetTwo)).not.toBeInTheDocument();
  });

  it("keeps the newest plan when preview responses resolve out of order", async () => {
    const user = userEvent.setup();
    const firstResponse = deferred<EditExportPlan>();
    const secondResponse = deferred<EditExportPlan>();
    api.chooseEditedCopyTarget.mockResolvedValueOnce(targetOne).mockResolvedValueOnce(targetTwo);
    api.previewEditExport
      .mockReturnValueOnce(firstResponse.promise)
      .mockReturnValueOnce(secondResponse.promise);
    renderEditor();

    await user.click(screen.getByRole("button", { name: "预览另存计划" }));
    await waitFor(() => expect(api.previewEditExport).toHaveBeenCalledTimes(1));
    expect(screen.getByRole("button", { name: "准备中…" })).toBeDisabled();
    await user.click(screen.getByRole("button", { name: "旋转 90°" }));
    await user.click(screen.getByRole("button", { name: "预览另存计划" }));
    await waitFor(() => expect(api.previewEditExport).toHaveBeenCalledTimes(2));

    await act(async () => {
      secondResponse.resolve(
        makeExportPlan("newest-plan", targetTwo, { ...emptyEditRecipe, rotateDegrees: 90 }),
      );
    });
    expect(await screen.findByText(targetTwo)).toBeInTheDocument();

    await act(async () => {
      firstResponse.resolve(makeExportPlan("stale-plan", targetOne));
    });
    expect(screen.getByText(targetTwo)).toBeInTheDocument();
    expect(screen.queryByText(targetOne)).not.toBeInTheDocument();
  });

  it("prevents duplicate export confirmation and rollback execution", async () => {
    const user = userEvent.setup();
    const exportExecution = deferred<EditExportResult>();
    const rollbackExecution = deferred<EditExportResult>();
    api.executeEditExport.mockReturnValue(exportExecution.promise);
    api.executeEditRollback.mockReturnValue(rollbackExecution.promise);
    renderEditor();

    await requestPlan(user);
    await user.dblClick(screen.getByRole("button", { name: "确认另存副本" }));
    expect(api.executeEditExport).toHaveBeenCalledTimes(1);

    await act(async () => {
      exportExecution.resolve(makeExportResult());
    });
    await screen.findByText("副本已创建，可安全回滚");
    await user.click(screen.getByRole("button", { name: "预览撤销" }));
    await screen.findByRole("button", { name: "确认删除该生成副本" });
    await user.dblClick(screen.getByRole("button", { name: "确认删除该生成副本" }));
    expect(api.executeEditRollback).toHaveBeenCalledTimes(1);

    await act(async () => {
      rollbackExecution.resolve(makeExportResult());
    });
    await waitFor(() => expect(screen.queryByText("副本已创建，可安全回滚")).toBeNull());
  });

  it("keeps a completed export rollbackable after recipe edits until dismissed", async () => {
    const user = userEvent.setup();
    renderEditor();

    await requestPlan(user);
    await user.click(screen.getByRole("button", { name: "确认另存副本" }));
    await screen.findByText("副本已创建，可安全回滚");
    await user.click(screen.getByRole("button", { name: "旋转 90°" }));

    expect(screen.getByText(targetOne)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "预览撤销" }));
    expect(await screen.findByRole("button", { name: "确认删除该生成副本" })).toBeInTheDocument();
    expect(api.previewEditRollback).toHaveBeenCalledWith("plan-D:\\exports\\编辑照片-one.jpg");

    await user.click(screen.getByRole("button", { name: "关闭记录" }));
    expect(screen.queryByText("副本已创建，可安全回滚")).not.toBeInTheDocument();
    expect(api.executeEditRollback).not.toHaveBeenCalled();
  });

  it("rejects a rollback preview tied to a different completed export", async () => {
    const user = userEvent.setup();
    api.previewEditRollback.mockResolvedValueOnce(makeRollbackPlan("another-export"));
    renderEditor();

    await requestPlan(user);
    await user.click(screen.getByRole("button", { name: "确认另存副本" }));
    await screen.findByText("副本已创建，可安全回滚");
    await user.click(screen.getByRole("button", { name: "预览撤销" }));

    await waitFor(() =>
      expect(api.previewEditRollback).toHaveBeenCalledWith("plan-D:\\exports\\编辑照片-one.jpg"),
    );
    expect(screen.queryByRole("button", { name: "确认删除该生成副本" })).not.toBeInTheDocument();
    expect(api.executeEditRollback).not.toHaveBeenCalled();
  });
});
