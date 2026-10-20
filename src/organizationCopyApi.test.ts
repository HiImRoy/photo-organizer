import { beforeEach, describe, expect, it, vi } from "vitest";

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
  isTauri: () => true,
}));

import {
  cancelCopyExport,
  createOrganizationPlanV2,
  confirmCopyExportRollback,
  getCopyExportJob,
  getCopyExportRuntimeStatus,
  grantOrganizationTargetParent,
  listRecentOrganizationPlansV2,
  listRecentCopyExportJobs,
  preflightCopyExport,
  previewCopyExportResume,
  previewCopyExportRollback,
  reconcileCopyExportItem,
  readOrganizationPlanV2,
  resumeCopyExport,
  startCopyExport,
} from "./api";
import { assetQueryFromV1, createAssetQueryV1 } from "./query";
import type { CopyExportRules, CreateOrganizationPlanV2Request } from "./types";

const rules: CopyExportRules = {
  version: "copy-export-rules-v1",
  layout: "preserve_original",
  conflictPolicy: "block",
  missingCaptureTimePolicy: "block",
};

beforeEach(() => invokeMock.mockReset());

describe("safe copy organization IPC clients", () => {
  it("grants the user-selected existing parent directory", async () => {
    invokeMock.mockResolvedValue({ grantId: "grant-1", displayPath: "D:/Exports" });

    await expect(grantOrganizationTargetParent("D:/Exports")).resolves.toEqual({
      grantId: "grant-1",
      displayPath: "D:/Exports",
    });
    expect(invokeMock).toHaveBeenCalledWith("grant_organization_target_parent", {
      path: "D:/Exports",
    });
  });

  it("sends a full query scope and explicit child directory to plan creation", async () => {
    const request: CreateOrganizationPlanV2Request = {
      scope: {
        kind: "query",
        query: assetQueryFromV1({ ...createAssetQueryV1(null), page: 1, pageSize: 120 }),
      },
      target: { parentGrantId: "grant-1", childName: "整理结果" },
      rules,
    };
    invokeMock.mockResolvedValue({ planId: "plan-1", status: "ready" });

    await createOrganizationPlanV2(request);

    expect(invokeMock).toHaveBeenCalledWith("create_organization_plan_v2", request);
  });

  it("reads plan pages and recent plan summaries from the durable store", async () => {
    invokeMock.mockResolvedValue({
      summary: {},
      items: [],
      itemTotal: 0,
      itemPage: 2,
      itemPageSize: 50,
    });
    await readOrganizationPlanV2("plan-1", 2, 50);
    expect(invokeMock).toHaveBeenNthCalledWith(1, "read_organization_plan_v2", {
      planId: "plan-1",
      itemPage: 2,
      itemPageSize: 50,
    });

    invokeMock.mockResolvedValue([]);
    await listRecentOrganizationPlansV2(12);
    expect(invokeMock).toHaveBeenNthCalledWith(2, "list_recent_organization_plans", { limit: 12 });
  });

  it("uses the confirmed preflight and copy job IPC contracts", async () => {
    invokeMock.mockResolvedValue({ authorizationToken: "token-1" });
    await preflightCopyExport("plan-1", "copy");
    expect(invokeMock).toHaveBeenLastCalledWith("copy_export_preflight", {
      resourceId: "plan-1",
      operation: "copy",
    });

    invokeMock.mockResolvedValue({ jobId: "job-1", accepted: true });
    await startCopyExport("plan-1", "token-1");
    expect(invokeMock).toHaveBeenLastCalledWith("copy_export_start", {
      planId: "plan-1",
      authorizationToken: "token-1",
    });

    invokeMock.mockResolvedValue(null);
    await getCopyExportJob("job-1", 2, 100);
    expect(invokeMock).toHaveBeenLastCalledWith("copy_export_get_job", {
      jobId: "job-1",
      itemPage: 2,
      itemPageSize: 100,
    });
    await getCopyExportRuntimeStatus("job-1");
    expect(invokeMock).toHaveBeenLastCalledWith("copy_export_runtime_status", { jobId: "job-1" });
    await listRecentCopyExportJobs(10);
    expect(invokeMock).toHaveBeenLastCalledWith("copy_export_list_recent_jobs", { limit: 10 });
    await cancelCopyExport("job-1");
    expect(invokeMock).toHaveBeenLastCalledWith("copy_export_cancel", { jobId: "job-1" });
  });

  it("keeps resume, item reconciliation, and rollback behind their preview contracts", async () => {
    invokeMock.mockResolvedValue({ authorizationToken: "resume-token" });
    await previewCopyExportResume("job-1");
    expect(invokeMock).toHaveBeenLastCalledWith("copy_export_resume_preview", { jobId: "job-1" });
    await resumeCopyExport("job-1", "resume-token");
    expect(invokeMock).toHaveBeenLastCalledWith("copy_export_resume", {
      jobId: "job-1",
      authorizationToken: "resume-token",
    });

    await reconcileCopyExportItem("job-1", 4, "keep_quarantined");
    expect(invokeMock).toHaveBeenLastCalledWith("copy_export_reconcile_item", {
      jobId: "job-1",
      ordinal: 4,
      decision: "keep_quarantined",
    });
    await previewCopyExportRollback("job-1");
    expect(invokeMock).toHaveBeenLastCalledWith("copy_export_rollback_preview", { jobId: "job-1" });
    await confirmCopyExportRollback("job-1", "rollback-token");
    expect(invokeMock).toHaveBeenLastCalledWith("copy_export_confirm_rollback", {
      jobId: "job-1",
      authorizationToken: "rollback-token",
    });
  });
});
