import { useEffect, useRef, useState } from "react";

import {
  chooseOrganizationTargetFolder,
  createOrganizationPlanV2,
  grantOrganizationTargetParent,
  isDesktopRuntime,
  listRecentOrganizationPlansV2,
  readOrganizationPlanV2,
} from "../api";
import { formatBytes, formatDate } from "../format";
import { CopyExportJobPanel } from "./CopyExportJobPanel";
import type {
  AssetQuery,
  CopyExportConflictPolicy,
  CopyExportLayout,
  CopyExportRules,
  CreateOrganizationPlanV2Request,
  MissingCaptureTimePolicy,
  OrganizationPlanItemV2,
  OrganizationPlanV2,
  OrganizationPlanScopeV2,
  OrganizationTreeNode,
  RecentOrganizationPlanV2,
  TargetParentGrantResponse,
} from "../types";
import "./organizationWorkspace.css";

const ITEM_PAGE_SIZE = 100;
const DEFAULT_PLAN_IPC_READY = isDesktopRuntime();
const DEFAULT_COPY_JOB_IPC_READY = isDesktopRuntime();

const defaultRules: CopyExportRules = {
  version: "copy-export-rules-v1",
  layout: "preserve_original",
  conflictPolicy: "block",
  missingCaptureTimePolicy: "block",
};

interface CopyExportWorkspaceProps {
  query: AssetQuery;
  selectedAssetIds: number[];
  matchedCount: number;
  onClose: () => void;
  planIpcReady?: boolean;
  copyJobIpcReady?: boolean;
}

type RequestState = "idle" | "choosing" | "granting" | "creating" | "reading";

export function CopyExportWorkspace({
  query,
  selectedAssetIds,
  matchedCount,
  onClose,
  planIpcReady = DEFAULT_PLAN_IPC_READY,
  copyJobIpcReady = DEFAULT_COPY_JOB_IPC_READY,
}: CopyExportWorkspaceProps) {
  const [scopeKind, setScopeKind] = useState<"query" | "explicit_assets">(
    selectedAssetIds.length > 0 ? "explicit_assets" : "query",
  );
  const [layout, setLayout] = useState<CopyExportLayout>("preserve_original");
  const [conflictPolicy, setConflictPolicy] = useState<CopyExportConflictPolicy>("block");
  const [missingCaptureTimePolicy, setMissingCaptureTimePolicy] =
    useState<MissingCaptureTimePolicy>("block");
  const [parentPath, setParentPath] = useState("");
  const [parentGrant, setParentGrant] = useState<TargetParentGrantResponse | null>(null);
  const [childName, setChildName] = useState("整理结果");
  const [requestState, setRequestState] = useState<RequestState>("idle");
  const [plan, setPlan] = useState<OrganizationPlanV2 | null>(null);
  const [planQueryKey, setPlanQueryKey] = useState<string | null>(null);
  const [requestQueryKey, setRequestQueryKey] = useState<string | null>(null);
  const [selectedOrdinal, setSelectedOrdinal] = useState<number | null>(null);
  const [recentPlans, setRecentPlans] = useState<RecentOrganizationPlanV2[]>([]);
  const [selectedRecentPlanId, setSelectedRecentPlanId] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [previewGeneration, setPreviewGeneration] = useState(0);
  const mountedRef = useRef(true);
  const requestGenerationRef = useRef(0);
  const planIpcReadyRef = useRef(planIpcReady);
  const queryKey = JSON.stringify(query);
  const previousQueryKeyRef = useRef(queryKey);
  planIpcReadyRef.current = planIpcReady;

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      requestGenerationRef.current += 1;
    };
  }, []);

  useEffect(() => {
    if (!planIpcReady) return undefined;
    let active = true;
    void listRecentOrganizationPlansV2()
      .then((items) => {
        if (active) setRecentPlans(items);
      })
      .catch((reason: unknown) => {
        if (active) setNotice(`无法读取已保存的预览：${messageFrom(reason)}`);
      });
    return () => {
      active = false;
    };
  }, [planIpcReady]);

  useEffect(() => {
    if (previousQueryKeyRef.current === queryKey) return;
    previousQueryKeyRef.current = queryKey;
    if (scopeKind !== "query") return;
    requestGenerationRef.current += 1;
  }, [queryKey, scopeKind]);

  const scope: OrganizationPlanScopeV2 =
    scopeKind === "explicit_assets"
      ? { kind: "explicit_assets", assetIds: [...new Set(selectedAssetIds)] }
      : { kind: "query", query };
  const staleQueryPlan = plan !== null && planQueryKey !== null && planQueryKey !== queryKey;
  const visiblePlan = staleQueryPlan ? null : plan;
  const currentRequestState =
    scopeKind === "query" &&
    requestQueryKey !== null &&
    requestQueryKey !== queryKey &&
    (requestState === "creating" || requestState === "reading")
      ? "idle"
      : requestState;
  const staleQueryRequest =
    scopeKind === "query" &&
    requestQueryKey !== null &&
    requestQueryKey !== queryKey &&
    (requestState === "creating" || requestState === "reading");
  const effectiveNotice =
    staleQueryPlan || staleQueryRequest ? "当前筛选已变化；旧预览已清除，请重新生成。" : notice;
  const selectedItem = visiblePlan?.items.find((item) => item.ordinal === selectedOrdinal) ?? null;
  const builderDisabled = currentRequestState !== "idle";
  const displayedScope = visiblePlan?.summary.scope ?? scope;
  const displayedMatchedCount = visiblePlan?.summary.matchedCount ?? matchedCount;
  const displayedSelectedCount =
    displayedScope.kind === "explicit_assets"
      ? displayedScope.assetIds.length
      : selectedAssetIds.length;
  const canCreatePlan =
    planIpcReady &&
    parentGrant !== null &&
    childName.trim().length > 0 &&
    (scopeKind === "query" || selectedAssetIds.length > 0) &&
    currentRequestState === "idle";
  const pageCount = visiblePlan
    ? Math.max(1, Math.ceil(visiblePlan.itemTotal / visiblePlan.itemPageSize))
    : 0;

  async function selectParentDirectory() {
    setError(null);
    setNotice(null);
    setRequestState("choosing");
    try {
      const selectedPath = await chooseOrganizationTargetFolder();
      if (!mountedRef.current || !selectedPath) return;
      invalidateCurrentPlan();
      setParentPath(selectedPath);
      setParentGrant(null);
      if (!planIpcReadyRef.current) {
        setNotice("整理预览仅在 PhotoOrganizer 桌面应用中可用；浏览器预览不会创建计划。");
        return;
      }
      setRequestState("granting");
      const grant = await grantOrganizationTargetParent(selectedPath);
      if (mountedRef.current) {
        setParentGrant(grant);
        setParentPath(grant.displayPath);
      }
    } catch (reason) {
      if (mountedRef.current) setError(messageFrom(reason));
    } finally {
      if (mountedRef.current) setRequestState("idle");
    }
  }

  function invalidateCurrentPlan() {
    const hadPreview = plan !== null || requestState === "creating" || requestState === "reading";
    requestGenerationRef.current += 1;
    setPreviewGeneration((generation) => generation + 1);
    setPlan(null);
    setPlanQueryKey(null);
    setSelectedOrdinal(null);
    setSelectedRecentPlanId("");
    if (requestState === "creating" || requestState === "reading") setRequestState("idle");
    if (hadPreview) setNotice("设置已更改；旧预览已清除，请重新生成。");
  }

  function updateBuilderValue<T>(setter: (value: T) => void, value: T) {
    invalidateCurrentPlan();
    setter(value);
  }

  async function generatePlan() {
    if (!canCreatePlan || !parentGrant) return;
    const generation = ++requestGenerationRef.current;
    setPreviewGeneration((value) => value + 1);
    setPlanQueryKey(scope.kind === "query" ? queryKey : null);
    setRequestQueryKey(queryKey);
    setRequestState("creating");
    setError(null);
    setNotice(null);
    setPlan(null);
    const request: CreateOrganizationPlanV2Request = {
      scope,
      target: { parentGrantId: parentGrant.grantId, childName: childName.trim() },
      rules: { ...defaultRules, layout, conflictPolicy, missingCaptureTimePolicy },
    };
    try {
      const summary = await createOrganizationPlanV2(request);
      if (!mountedRef.current || generation !== requestGenerationRef.current) return;
      setRequestState("reading");
      const snapshot = await readOrganizationPlanV2(summary.planId, 1, ITEM_PAGE_SIZE);
      if (!mountedRef.current || generation !== requestGenerationRef.current) return;
      setPlan(snapshot);
      setSelectedRecentPlanId(summary.planId);
      setSelectedOrdinal(snapshot.items[0]?.ordinal ?? null);
      const recent = await listRecentOrganizationPlansV2();
      if (mountedRef.current && generation === requestGenerationRef.current) setRecentPlans(recent);
    } catch (reason) {
      if (mountedRef.current && generation === requestGenerationRef.current) {
        setError(messageFrom(reason));
      }
    } finally {
      if (mountedRef.current && generation === requestGenerationRef.current) {
        setRequestState("idle");
      }
    }
  }

  async function restorePlan(planId: string) {
    if (!planIpcReady || !planId) return;
    const generation = ++requestGenerationRef.current;
    setPreviewGeneration((value) => value + 1);
    setPlanQueryKey(null);
    setRequestQueryKey(queryKey);
    setParentGrant(null);
    setParentPath("");
    setSelectedRecentPlanId(planId);
    setRequestState("reading");
    setError(null);
    try {
      const snapshot = await readOrganizationPlanV2(planId, 1, ITEM_PAGE_SIZE);
      if (!mountedRef.current || generation !== requestGenerationRef.current) return;
      setPlan(snapshot);
      setPlanQueryKey(null);
      setScopeKind(snapshot.summary.scope.kind);
      setLayout(snapshot.summary.rules.layout);
      setConflictPolicy(snapshot.summary.rules.conflictPolicy);
      setMissingCaptureTimePolicy(snapshot.summary.rules.missingCaptureTimePolicy);
      setSelectedOrdinal(snapshot.items[0]?.ordinal ?? null);
    } catch (reason) {
      if (mountedRef.current && generation === requestGenerationRef.current) {
        setError(messageFrom(reason));
      }
    } finally {
      if (mountedRef.current && generation === requestGenerationRef.current) {
        setRequestState("idle");
      }
    }
  }

  async function readPlanPage(page: number) {
    if (!visiblePlan || page < 1 || page > pageCount || page === visiblePlan.itemPage) return;
    const generation = ++requestGenerationRef.current;
    setRequestQueryKey(queryKey);
    setRequestState("reading");
    setError(null);
    try {
      const snapshot = await readOrganizationPlanV2(
        visiblePlan.summary.planId,
        page,
        ITEM_PAGE_SIZE,
      );
      if (!mountedRef.current || generation !== requestGenerationRef.current) return;
      setPlan(snapshot);
      setSelectedOrdinal(snapshot.items[0]?.ordinal ?? null);
    } catch (reason) {
      if (mountedRef.current && generation === requestGenerationRef.current) {
        setError(messageFrom(reason));
      }
    } finally {
      if (mountedRef.current && generation === requestGenerationRef.current) {
        setRequestState("idle");
      }
    }
  }

  return (
    <section className="organization-workspace" aria-label="安全复制预览工作区">
      <div className="organization-safety-banner">
        <span className="safety-dot" aria-hidden="true" />
        <strong>复制到新目录 · 原文件保持不变</strong>
        <span className="organization-scope-chip">
          {displayedScope.kind === "query"
            ? `全部匹配 ${displayedMatchedCount.toLocaleString()} 张`
            : `已选择 ${displayedSelectedCount.toLocaleString()} 张`}
        </span>
        <button type="button" className="tool-button" onClick={onClose}>
          返回图库
        </button>
      </div>
      {error ? (
        <div className="organization-message is-error" role="alert">
          {error}
        </div>
      ) : null}
      {effectiveNotice ? (
        <div className="organization-message" role="status">
          {effectiveNotice}
        </div>
      ) : null}

      <div className="organization-columns">
        <aside className="organization-controls" aria-label="复制范围与规则">
          <div className="organization-panel-heading">
            <h2>新建复制预览</h2>
          </div>

          <fieldset className="organization-fieldset" disabled={builderDisabled}>
            <legend>图片范围</legend>
            <label>
              <input
                type="radio"
                name="copy-scope"
                checked={scopeKind === "query"}
                onChange={() => updateBuilderValue(setScopeKind, "query")}
              />
              当前全部匹配 <span>{displayedMatchedCount.toLocaleString()}</span>
            </label>
            <label>
              <input
                type="radio"
                name="copy-scope"
                checked={scopeKind === "explicit_assets"}
                disabled={
                  selectedAssetIds.length === 0 && displayedScope.kind !== "explicit_assets"
                }
                onChange={() => updateBuilderValue(setScopeKind, "explicit_assets")}
              />
              只复制已选图片 <span>{displayedSelectedCount.toLocaleString()}</span>
            </label>
          </fieldset>

          <div className="organization-control-group">
            <label htmlFor="copy-target-parent">新目录放在</label>
            <div className="organization-target-input">
              <input
                id="copy-target-parent"
                value={parentPath}
                readOnly
                placeholder="选择一个现有父目录"
              />
              <button
                type="button"
                className="tool-button"
                disabled={currentRequestState !== "idle"}
                onClick={() => void selectParentDirectory()}
              >
                {currentRequestState === "choosing"
                  ? "打开中…"
                  : currentRequestState === "granting"
                    ? "验证中…"
                    : "选择"}
              </button>
            </div>
            <small>应用只会在此处新建你指定的子目录。</small>
          </div>

          <div className="organization-control-group">
            <label htmlFor="copy-target-child">新目录名称</label>
            <input
              id="copy-target-child"
              className="template-input"
              value={childName}
              disabled={builderDisabled}
              onChange={(event) => updateBuilderValue(setChildName, event.target.value)}
              placeholder="例如：周末旅行"
            />
          </div>

          <div className="organization-control-group">
            <label htmlFor="copy-layout">目录布局</label>
            <select
              id="copy-layout"
              value={layout}
              disabled={builderDisabled}
              onChange={(event) =>
                updateBuilderValue(setLayout, event.target.value as CopyExportLayout)
              }
            >
              <option value="preserve_original">保留原目录和原文件名</option>
              <option value="capture_year_month">按拍摄年月整理</option>
            </select>
          </div>

          <details className="organization-advanced-options">
            <summary>高级规则</summary>
            <div className="organization-rule-row">
              <label>
                同名文件
                <select
                  value={conflictPolicy}
                  disabled={builderDisabled}
                  onChange={(event) =>
                    updateBuilderValue(
                      setConflictPolicy,
                      event.target.value as CopyExportConflictPolicy,
                    )
                  }
                >
                  <option value="block">阻止并提示</option>
                  <option value="skip">跳过并列出</option>
                  <option value="sequence">追加序号</option>
                  <option value="short_hash">追加短标识</option>
                </select>
              </label>
              <label>
                缺少拍摄时间
                <select
                  value={missingCaptureTimePolicy}
                  disabled={builderDisabled}
                  onChange={(event) =>
                    updateBuilderValue(
                      setMissingCaptureTimePolicy,
                      event.target.value as MissingCaptureTimePolicy,
                    )
                  }
                >
                  <option value="block">阻止并提示</option>
                  <option value="preserve_original_folders">保留原目录</option>
                  <option value="unknown_folder">放入“未知”</option>
                  <option value="skip">跳过并列出</option>
                </select>
              </label>
            </div>
          </details>

          <button
            type="button"
            className="organization-generate primary-action"
            disabled={!canCreatePlan}
            onClick={() => void generatePlan()}
          >
            {currentRequestState === "creating" || currentRequestState === "reading"
              ? "正在生成预览…"
              : "生成只读预览"}
          </button>
          {!planIpcReady ? (
            <p className="organization-space-note" role="status">
              整理预览仅在 PhotoOrganizer 桌面应用中可用。
            </p>
          ) : null}
        </aside>

        <section className="organization-preview" aria-label="复制目标预览">
          <div className="organization-preview-heading">
            <div>
              <h2>目标预览</h2>
              <span>{visiblePlan?.summary.targetRoot ?? "生成预览后查看完整目录和文件名"}</span>
            </div>
            <div className="organization-actions">
              <label className="sr-only" htmlFor="recent-copy-plan">
                恢复已保存的预览
              </label>
              <select
                id="recent-copy-plan"
                value={staleQueryPlan ? "" : selectedRecentPlanId}
                disabled={!planIpcReady || currentRequestState !== "idle"}
                onChange={(event) => void restorePlan(event.target.value)}
                aria-label="恢复已保存的预览"
              >
                <option value="">恢复已保存的预览</option>
                {recentPlans.map((recent) => (
                  <option value={recent.planId} key={recent.planId}>
                    {formatDate(recent.createdAt)} · {recent.targetRoot} · {recent.matchedCount} 张
                  </option>
                ))}
              </select>
            </div>
          </div>

          {visiblePlan ? (
            <>
              <div className="organization-summary" aria-label="预览统计">
                <Metric label="匹配" value={visiblePlan.summary.matchedCount} />
                <Metric label="可复制" value={visiblePlan.summary.eligibleCount} />
                <Metric label="跳过" value={visiblePlan.summary.skippedCount} />
                <Metric label="阻止" value={visiblePlan.summary.blockedCount} />
              </div>
              <div className="organization-message" aria-label="冻结的预览设置">
                冻结设置：
                {visiblePlan.summary.rules.layout === "preserve_original"
                  ? "保留原目录和原文件名"
                  : "按拍摄年月整理"}
                · 同名文件：{conflictPolicyLabel(visiblePlan.summary.rules.conflictPolicy)} ·
                缺少拍摄时间：
                {missingTimePolicyLabel(visiblePlan.summary.rules.missingCaptureTimePolicy)}
              </div>
              <div className="organization-tree" aria-label="目标目录结构">
                <strong>目标目录</strong>
                <TreeList node={visiblePlan.summary.tree} />
              </div>
              <div className="organization-mapping-heading">
                <strong>逐项目标与文件名</strong>
                <span>
                  第 {visiblePlan.itemPage} / {pageCount || 1} 页 · 共{" "}
                  {visiblePlan.itemTotal.toLocaleString()} 项
                </span>
              </div>
              <div className="organization-mapping-table" role="table" aria-label="复制文件清单">
                <div className="organization-mapping-row is-header" role="row">
                  <span role="columnheader">源文件</span>
                  <span role="columnheader">目标文件</span>
                  <span role="columnheader">状态</span>
                </div>
                {visiblePlan.items.map((item) => (
                  <button
                    type="button"
                    className={
                      selectedOrdinal === item.ordinal
                        ? "organization-mapping-row is-selected"
                        : "organization-mapping-row"
                    }
                    key={`${item.ordinal}:${item.assetId}`}
                    role="row"
                    onClick={() => setSelectedOrdinal(item.ordinal)}
                  >
                    <span role="cell" title={item.sourcePath}>
                      {item.sourceRelativePath}
                    </span>
                    <span role="cell" title={item.targetPath}>
                      {item.targetRelativePath}
                    </span>
                    <span className="mapping-status" role="cell">
                      {planItemStatusLabel(item.status)}
                    </span>
                  </button>
                ))}
                {visiblePlan.items.length === 0 ? (
                  <div className="organization-empty is-compact">
                    <strong>此预览没有文件</strong>
                  </div>
                ) : null}
              </div>
              <div className="organization-actions" aria-label="预览分页">
                <button
                  type="button"
                  className="tool-button"
                  disabled={currentRequestState !== "idle" || visiblePlan.itemPage <= 1}
                  onClick={() => void readPlanPage(visiblePlan.itemPage - 1)}
                >
                  上一页
                </button>
                <button
                  type="button"
                  className="tool-button"
                  disabled={currentRequestState !== "idle" || visiblePlan.itemPage >= pageCount}
                  onClick={() => void readPlanPage(visiblePlan.itemPage + 1)}
                >
                  下一页
                </button>
              </div>
            </>
          ) : (
            <div className="organization-empty">
              <strong>先设置范围和目标</strong>
              <span>生成预览前不会创建目录或复制文件。</span>
            </div>
          )}
          <CopyExportJobPanel
            plan={visiblePlan?.summary ?? null}
            planGeneration={previewGeneration}
            jobIpcReady={copyJobIpcReady}
          />
        </section>

        <aside className="organization-detail" aria-label="所选计划项目详情">
          <div className="organization-preview-heading">
            <div>
              <h2>项目详情</h2>
              <span>{selectedItem?.sourceRelativePath ?? "选择一行查看文件映射"}</span>
            </div>
          </div>
          {selectedItem ? <PlanItemDetail item={selectedItem} /> : null}
        </aside>
      </div>
    </section>
  );
}

function Metric({ label, value }: { label: string; value: number }) {
  return (
    <div className="organization-metric">
      <span>{label}</span>
      <strong>{value.toLocaleString()}</strong>
    </div>
  );
}

function TreeList({ node }: { node: OrganizationTreeNode }) {
  return (
    <ul>
      <li>
        <span className="tree-node-label">
          <strong>{node.name || "新目录"}</strong>
          <span>
            {node.fileCount.toLocaleString()} 张 · {formatBytes(node.byteCount)}
          </span>
        </span>
        {node.children.length > 0 ? (
          <ul>
            {node.children.map((child) => (
              <li key={child.relativePath}>
                <TreeList node={child} />
              </li>
            ))}
          </ul>
        ) : null}
      </li>
    </ul>
  );
}

function PlanItemDetail({ item }: { item: OrganizationPlanItemV2 }) {
  return (
    <>
      <div className="path-card">
        <strong>源文件</strong>
        <code>{item.sourcePath}</code>
      </div>
      <div className="path-card">
        <strong>复制目标</strong>
        <code>{item.targetPath}</code>
      </div>
      <div className="variable-list">
        <div>
          <span>状态</span>
          <code>{planItemStatusLabel(item.status)}</code>
        </div>
        <div>
          <span>大小</span>
          <code>{formatBytes(item.sourceSize)}</code>
        </div>
        <div>
          <span>拍摄时间</span>
          <code>{item.captureTime ?? "未记录"}</code>
        </div>
        <div>
          <span>来源目录</span>
          <code>{item.sourceRootPath}</code>
        </div>
      </div>
      {item.issues.length > 0 ? (
        <div className="issue-list" aria-label="此项目的问题">
          {item.issues.map((issue, index) => (
            <div className="organization-issue" key={`${issue.code}:${index}`}>
              <strong>{issue.code}</strong>
              <span>{issue.detail}</span>
            </div>
          ))}
        </div>
      ) : null}
    </>
  );
}

function planItemStatusLabel(status: OrganizationPlanItemV2["status"]) {
  if (status === "ready") return "可复制";
  if (status === "skipped") return "已跳过";
  return "阻止";
}

function conflictPolicyLabel(policy: CopyExportConflictPolicy) {
  const labels: Record<CopyExportConflictPolicy, string> = {
    block: "阻止并提示",
    skip: "跳过并列出",
    sequence: "追加序号",
    short_hash: "追加短标识",
  };
  return labels[policy];
}

function missingTimePolicyLabel(policy: MissingCaptureTimePolicy) {
  const labels: Record<MissingCaptureTimePolicy, string> = {
    block: "阻止并提示",
    preserve_original_folders: "保留原目录",
    unknown_folder: "放入未知目录",
    skip: "跳过并列出",
  };
  return labels[policy];
}

function messageFrom(reason: unknown) {
  return reason instanceof Error ? reason.message : String(reason);
}
