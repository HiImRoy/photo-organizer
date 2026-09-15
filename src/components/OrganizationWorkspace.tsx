import { useEffect, useRef, useState } from "react";

import {
  chooseOrganizationTargetFolder,
  exportOrganizationManifest,
  previewOrganizationPlan,
} from "../api";
import { formatBytes } from "../format";
import type {
  LibrarySummary,
  OrganizationConflictStrategy,
  OrganizationLevel,
  OrganizationLevelKind,
  OrganizationMissingFallback,
  OrganizationPlan,
  OrganizationPlanItem,
  OrganizationPlanRequest,
  OrganizationRules,
  AssetScopeDescription,
  AssetScopeInputV1,
  OrganizationScope,
} from "../types";
import "./organizationWorkspace.css";

const levelOptions: Array<{
  value: OrganizationLevelKind;
  label: string;
  description: string;
  example: string;
}> = [
  { value: "year", label: "拍摄年份", description: "从拍摄时间读取年份", example: "2025" },
  { value: "month", label: "拍摄月份", description: "从拍摄时间读取月份", example: "01" },
  { value: "day", label: "拍摄日期", description: "从拍摄时间读取日期", example: "13" },
  {
    value: "original_directory",
    label: "原始目录",
    description: "保留源文件所在的相对目录",
    example: "旅行/海边",
  },
  {
    value: "primary_semantic",
    label: "拍摄题材",
    description: "按图片的主要题材分目录",
    example: "人物、建筑",
  },
  { value: "tone", label: "影调", description: "按图片的影调分目录", example: "高调、均衡" },
  {
    value: "dominant_color",
    label: "主色",
    description: "按图片的主色分目录",
    example: "蓝色、绿色",
  },
  {
    value: "saturation",
    label: "饱和度",
    description: "按图片的饱和度等级分目录",
    example: "低饱和、中饱和",
  },
];

const levelLabels = Object.fromEntries(
  levelOptions.map(({ value, label }) => [value, label]),
) as Record<OrganizationLevelKind, string>;

const fallbackOptions: Array<{
  value: OrganizationMissingFallback;
  label: string;
  description: string;
}> = [
  {
    value: "modification_time",
    label: "用文件修改时间",
    description: "没有拍摄日期时，用文件修改时间补上这一层（仅日期维度）",
  },
  {
    value: "unknown",
    label: "放入“未知”",
    description: "保留这一层，并把缺失值写成“未知”",
  },
  {
    value: "skip",
    label: "跳过这一层",
    description: "缺失时不创建这一层，继续处理后面的维度",
  },
  {
    value: "block",
    label: "阻止该文件",
    description: "缺失时将图片标记为错误，不生成有效目标路径",
  },
];

const globalFallbackLabels: Record<OrganizationMissingFallback, string> = {
  modification_time: "拍摄时间用文件修改时间",
  unknown: "使用“未知”",
  skip: "留空并继续",
  block: "阻止该文件",
};

const dateLevelKinds = new Set<OrganizationLevelKind>(["year", "month", "day"]);

function isDateLevelKind(kind: OrganizationLevelKind) {
  return dateLevelKinds.has(kind);
}

function fallbackOptionsForLevel(kind: OrganizationLevelKind) {
  return isDateLevelKind(kind)
    ? fallbackOptions
    : fallbackOptions.filter((option) => option.value !== "modification_time");
}

function levelOptionFor(kind: OrganizationLevelKind) {
  return levelOptions.find((option) => option.value === kind) ?? levelOptions[0];
}

function fallbackOptionFor(fallback: OrganizationMissingFallback) {
  return fallbackOptions.find((option) => option.value === fallback) ?? fallbackOptions[1];
}

const defaultLevels: OrganizationLevel[] = [
  { kind: "year", fallback: "modification_time" },
  { kind: "month", fallback: "modification_time" },
  { kind: "primary_semantic", fallback: "unknown" },
];

const defaultRules: OrganizationRules = {
  version: "organization-rules-v1",
  levels: defaultLevels,
  template: "{capture_time:yyyyMMdd_HHmmss}_{semantic}_{original_stem}_{sequence:0000}",
  sequenceStart: 1,
  sequenceWidth: 4,
  missingFallback: "unknown",
  conflictStrategy: "sequence",
};

type OrganizationPlanSnapshot = {
  key: string;
  plan: OrganizationPlan;
};

function organizationSnapshotKey(
  libraryId: number,
  targetRoot: string,
  scope: OrganizationScope,
  scopeInput: AssetScopeInputV1,
  selectedAssetIds: number[],
  rules: OrganizationRules,
) {
  return JSON.stringify({
    libraryId,
    targetRoot: targetRoot.trim(),
    scope,
    filter: scopeInput.query.filter,
    selectedAssetIds,
    rules,
  });
}

interface OrganizationWorkspaceProps {
  library: LibrarySummary;
  selectedAssetIds: number[];
  filteredCount: number;
  scopeInput: AssetScopeInputV1;
  scopeDescription: AssetScopeDescription;
  onClose: () => void;
}

export function OrganizationWorkspace({
  library,
  selectedAssetIds,
  filteredCount,
  scopeInput,
  scopeDescription,
  onClose,
}: OrganizationWorkspaceProps) {
  const [targetRoot, setTargetRoot] = useState("");
  const [scope, setScope] = useState<OrganizationScope>(() =>
    scopeInput.kind === "selection" ? "selected" : "filtered",
  );
  const [rules, setRules] = useState<OrganizationRules>(defaultRules);
  const [planSnapshot, setPlanSnapshot] = useState<OrganizationPlanSnapshot | null>(null);
  const [selectedItemState, setSelectedItem] = useState<OrganizationPlanItem | null>(null);
  const [dragIndex, setDragIndex] = useState<number | null>(null);
  const [busyRequestKey, setBusyRequestKey] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const currentSnapshotKey = organizationSnapshotKey(
    library.id,
    targetRoot,
    scope,
    scopeInput,
    selectedAssetIds,
    rules,
  );
  const plan = planSnapshot?.key === currentSnapshotKey ? planSnapshot.plan : null;
  const selectedItem = plan ? selectedItemState : null;
  const busy = busyRequestKey === currentSnapshotKey;
  const mountedRef = useRef(true);
  const currentSnapshotKeyRef = useRef(currentSnapshotKey);
  const planSnapshotRef = useRef(planSnapshot);
  const previousSnapshotKeyRef = useRef(currentSnapshotKey);
  const planRequestVersionRef = useRef(0);
  const exportRequestVersionRef = useRef(0);

  useEffect(() => {
    currentSnapshotKeyRef.current = currentSnapshotKey;
    planSnapshotRef.current = planSnapshot;
  }, [currentSnapshotKey, planSnapshot]);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      planRequestVersionRef.current += 1;
      exportRequestVersionRef.current += 1;
    };
  }, []);

  useEffect(() => {
    if (previousSnapshotKeyRef.current === currentSnapshotKey) return;
    previousSnapshotKeyRef.current = currentSnapshotKey;
    planRequestVersionRef.current += 1;
    exportRequestVersionRef.current += 1;
    setPlanSnapshot(null);
    setBusyRequestKey(null);
    setMessage(null);
    setError(null);
  }, [currentSnapshotKey]);

  async function chooseTarget() {
    setError(null);
    try {
      const path = await chooseOrganizationTargetFolder();
      if (mountedRef.current && path) setTargetRoot(path);
    } catch (reason) {
      if (mountedRef.current) setError(messageFrom(reason));
    }
  }

  async function generatePlan() {
    const requestVersion = planRequestVersionRef.current + 1;
    planRequestVersionRef.current = requestVersion;
    const requestSnapshotKey = currentSnapshotKey;
    setError(null);
    setMessage(null);
    if (!targetRoot.trim()) {
      setBusyRequestKey(null);
      setError("请先选择或输入目标根目录。");
      return;
    }
    if (scope === "selected" && selectedAssetIds.length === 0) {
      setBusyRequestKey(null);
      setError("当前没有选中的图片，无法生成“用户选中”范围的预览。");
      return;
    }
    setBusyRequestKey(requestSnapshotKey);
    const request: OrganizationPlanRequest = {
      libraryId: library.id,
      targetRoot: targetRoot.trim(),
      scope,
      filter: scopeInput.query.filter,
      selectedAssetIds,
      rules,
    };
    try {
      const nextPlan = await previewOrganizationPlan(request);
      if (
        !mountedRef.current ||
        requestVersion !== planRequestVersionRef.current ||
        currentSnapshotKeyRef.current !== requestSnapshotKey
      ) {
        return;
      }
      setPlanSnapshot({ key: requestSnapshotKey, plan: nextPlan });
      setSelectedItem(nextPlan.items[0] ?? null);
      setMessage("整理预览已更新。");
    } catch (reason) {
      if (
        mountedRef.current &&
        requestVersion === planRequestVersionRef.current &&
        currentSnapshotKeyRef.current === requestSnapshotKey
      ) {
        setError(messageFrom(reason));
      }
    } finally {
      if (mountedRef.current && requestVersion === planRequestVersionRef.current) {
        setBusyRequestKey(null);
      }
    }
  }

  function updateLevel(index: number, next: Partial<OrganizationLevel>) {
    setRules((current) => ({
      ...current,
      levels: current.levels.map((level, levelIndex) => {
        if (levelIndex !== index) return level;
        const updated: OrganizationLevel = { ...level, ...next };
        if (!isDateLevelKind(updated.kind) && updated.fallback === "modification_time") {
          updated.fallback = "unknown";
        }
        return updated;
      }),
    }));
  }

  function moveLevel(from: number, to: number) {
    if (to < 0 || to >= rules.levels.length) return;
    setRules((current) => {
      const levels = [...current.levels];
      const [level] = levels.splice(from, 1);
      levels.splice(to, 0, level);
      return { ...current, levels };
    });
  }

  function removeLevel(index: number) {
    setRules((current) => ({
      ...current,
      levels: current.levels.filter((_, levelIndex) => levelIndex !== index),
    }));
  }

  async function exportPlan(format: "json" | "csv") {
    const requestPlan = plan;
    const requestSnapshotKey = currentSnapshotKey;
    if (!requestPlan) return;
    const requestVersion = exportRequestVersionRef.current + 1;
    exportRequestVersionRef.current = requestVersion;
    setError(null);
    try {
      const path = await exportOrganizationManifest(requestPlan, format);
      if (
        mountedRef.current &&
        requestVersion === exportRequestVersionRef.current &&
        currentSnapshotKeyRef.current === requestSnapshotKey &&
        planSnapshotRef.current?.key === requestSnapshotKey &&
        planSnapshotRef.current?.plan === requestPlan
      ) {
        if (path) setMessage(`已导出 ${format.toUpperCase()} 只读清单：${path}`);
      }
    } catch (reason) {
      if (
        mountedRef.current &&
        requestVersion === exportRequestVersionRef.current &&
        currentSnapshotKeyRef.current === requestSnapshotKey &&
        planSnapshotRef.current?.key === requestSnapshotKey &&
        planSnapshotRef.current?.plan === requestPlan
      ) {
        setError(messageFrom(reason));
      }
    }
  }

  return (
    <section className="organization-workspace" aria-label="整理预览工作区">
      <div className="organization-safety-banner">
        <span className="safety-dot" aria-hidden="true" />
        <strong>只读预览 · 不会修改源文件</strong>
        <span className="organization-scope-chip" title={scopeDescription.label}>
          {scopeInput.kind === "selection" ? "显式选择" : "当前查询"} · {scopeDescription.count} 张
        </span>
        <button type="button" onClick={onClose}>
          返回图库
        </button>
      </div>
      {error ? (
        <div className="organization-message is-error" role="alert">
          {error}
        </div>
      ) : null}
      {message ? (
        <div className="organization-message" role="status">
          {message}
        </div>
      ) : null}

      <div className="organization-columns">
        <aside className="organization-controls" aria-label="整理规则">
          <div className="organization-panel-heading">
            <h2>整理方案</h2>
          </div>

          <fieldset className="organization-fieldset">
            <legend>图片范围</legend>
            <label>
              <input
                type="radio"
                checked={scope === "filtered"}
                onChange={() => setScope("filtered")}
              />
              当前筛选结果 <span>{filteredCount}</span>
            </label>
            <label>
              <input type="radio" checked={scope === "all"} onChange={() => setScope("all")} />
              全部图片 <span>{library.presentCount}</span>
            </label>
            <label>
              <input
                type="radio"
                checked={scope === "selected"}
                onChange={() => setScope("selected")}
              />
              用户选中 <span>{selectedAssetIds.length}</span>
            </label>
          </fieldset>

          <div className="organization-control-group">
            <label htmlFor="organization-target">目标根目录</label>
            <div className="organization-target-input">
              <input
                id="organization-target"
                value={targetRoot}
                onChange={(event) => setTargetRoot(event.target.value)}
                placeholder="选择目标目录（不会创建）"
              />
              <button type="button" onClick={() => void chooseTarget()}>
                选择
              </button>
            </div>
          </div>

          <div className="organization-control-group">
            <div className="organization-label-row">
              <label>目录维度顺序</label>
              <small>拖动或使用箭头</small>
            </div>
            <div className="organization-level-columns" aria-hidden="true">
              <span>目录内容</span>
              <span>缺失时</span>
            </div>
            <div className="organization-levels">
              {rules.levels.map((level, index) => {
                const levelOption = levelOptionFor(level.kind);
                const fallbackOption = fallbackOptionFor(level.fallback);
                return (
                  <div className="organization-level-entry" key={`${level.kind}-${index}`}>
                    <div
                      className={`organization-level${dragIndex === index ? " is-dragging" : ""}`}
                      draggable
                      onDragStart={() => setDragIndex(index)}
                      onDragOver={(event) => event.preventDefault()}
                      onDrop={() => {
                        if (dragIndex !== null) moveLevel(dragIndex, index);
                        setDragIndex(null);
                      }}
                      onDragEnd={() => setDragIndex(null)}
                    >
                      <span className="drag-handle" aria-hidden="true">
                        ⋮⋮
                      </span>
                      <strong>{index + 1}</strong>
                      <select
                        aria-label={`第 ${index + 1} 层目录维度`}
                        title={`${levelOption.description}，例如：${levelOption.example}`}
                        value={level.kind}
                        onChange={(event) =>
                          updateLevel(index, { kind: event.target.value as OrganizationLevelKind })
                        }
                      >
                        {levelOptions.map((option) => (
                          <option key={option.value} value={option.value}>
                            {option.label}
                          </option>
                        ))}
                      </select>
                      <select
                        aria-label={`${levelLabels[level.kind]}缺失时`}
                        title={fallbackOption.description}
                        value={level.fallback}
                        onChange={(event) =>
                          updateLevel(index, {
                            fallback: event.target.value as OrganizationMissingFallback,
                          })
                        }
                      >
                        {fallbackOptionsForLevel(level.kind).map((option) => (
                          <option key={option.value} value={option.value}>
                            {option.label}
                          </option>
                        ))}
                      </select>
                      <button
                        type="button"
                        aria-label={`上移第 ${index + 1} 层`}
                        disabled={index === 0}
                        onClick={() => moveLevel(index, index - 1)}
                      >
                        ↑
                      </button>
                      <button
                        type="button"
                        aria-label={`下移第 ${index + 1} 层`}
                        disabled={index === rules.levels.length - 1}
                        onClick={() => moveLevel(index, index + 1)}
                      >
                        ↓
                      </button>
                      <button
                        type="button"
                        aria-label={`删除第 ${index + 1} 层`}
                        onClick={() => removeLevel(index)}
                      >
                        ×
                      </button>
                    </div>
                    <small className="organization-level-note">
                      {levelOption.description} · {fallbackOption.label}
                    </small>
                  </div>
                );
              })}
            </div>
            <button
              className="subtle-button"
              type="button"
              onClick={() =>
                setRules((current) => ({
                  ...current,
                  levels: [...current.levels, { kind: "day", fallback: "modification_time" }],
                }))
              }
            >
              + 添加维度
            </button>
          </div>

          <div className="organization-control-group">
            <label htmlFor="organization-template">文件命名模板</label>
            <input
              id="organization-template"
              className="template-input"
              value={rules.template}
              onChange={(event) =>
                setRules((current) => ({ ...current, template: event.target.value }))
              }
            />
            <small>
              可用变量：capture_time · camera · lens · original_name · semantic · tone ·
              dominant_color · saturation · sequence · short_hash
            </small>
          </div>

          <div className="organization-rule-row">
            <label>
              <span>缺失元数据时</span>
              <select
                aria-label="缺失元数据时"
                value={rules.missingFallback}
                onChange={(event) =>
                  setRules((current) => ({
                    ...current,
                    missingFallback: event.target.value as OrganizationMissingFallback,
                  }))
                }
              >
                {Object.entries(globalFallbackLabels).map(([value, label]) => (
                  <option key={value} value={value}>
                    {label}
                  </option>
                ))}
              </select>
              <small>修改时间仅替代拍摄时间。</small>
            </label>
            <label>
              重名策略
              <select
                value={rules.conflictStrategy}
                onChange={(event) =>
                  setRules((current) => ({
                    ...current,
                    conflictStrategy: event.target.value as OrganizationConflictStrategy,
                  }))
                }
              >
                <option value="sequence">添加序号</option>
                <option value="short_hash">添加 short hash</option>
                <option value="skip">跳过</option>
              </select>
            </label>
          </div>
          <div className="organization-rule-row">
            <label>
              序号起点
              <input
                type="number"
                min="1"
                value={rules.sequenceStart}
                onChange={(event) =>
                  setRules((current) => ({
                    ...current,
                    sequenceStart: Math.max(1, Number(event.target.value) || 1),
                  }))
                }
              />
            </label>
            <label>
              序号宽度
              <input
                type="number"
                min="1"
                max="12"
                value={rules.sequenceWidth}
                onChange={(event) =>
                  setRules((current) => ({
                    ...current,
                    sequenceWidth: Math.min(12, Math.max(1, Number(event.target.value) || 1)),
                  }))
                }
              />
            </label>
          </div>
          <button
            className="primary-action organization-generate"
            type="button"
            disabled={busy}
            onClick={() => void generatePlan()}
          >
            {busy ? "正在生成…" : "生成整理预览"}
          </button>
        </aside>

        <main className="organization-preview" aria-label="目标目录树和映射">
          <div className="organization-preview-heading">
            <div>
              <h2>目标目录树</h2>
            </div>
            {plan ? (
              <div className="organization-actions">
                <button type="button" onClick={() => void exportPlan("json")}>
                  导出 JSON
                </button>
                <button type="button" onClick={() => void exportPlan("csv")}>
                  导出 CSV
                </button>
              </div>
            ) : null}
          </div>
          {plan ? (
            <>
              <div className="organization-summary">
                <SummaryMetric label="文件" value={plan.summary.itemCount.toLocaleString()} />
                <SummaryMetric
                  label="冲突"
                  value={plan.summary.conflictCount.toLocaleString()}
                  tone={plan.summary.conflictCount ? "warning" : undefined}
                />
                <SummaryMetric
                  label="错误"
                  value={plan.summary.errorCount.toLocaleString()}
                  tone={plan.summary.errorCount ? "error" : undefined}
                />
                <SummaryMetric label="预计空间" value={formatBytes(plan.summary.estimatedBytes)} />
              </div>
              {plan.summary.targetAvailableBytes === null ? (
                <div className="organization-space-note">
                  未探测目标卷空间；预计空间按源文件大小合计。
                </div>
              ) : null}
              <div className="organization-tree">
                <TreeNode node={plan.tree} depth={0} />
              </div>
              <div className="organization-mapping-heading">
                <strong>源文件 → 规划目标文件</strong>
                <span>{plan.items.length} 条完整映射</span>
              </div>
              <div
                className="organization-mapping-table"
                role="table"
                aria-label="源文件到目标文件映射"
              >
                <div className="organization-mapping-row is-header">
                  <span>源文件</span>
                  <span>规划目标</span>
                  <span>状态</span>
                </div>
                {plan.items.map((item) => (
                  <button
                    type="button"
                    className={`organization-mapping-row${selectedItem?.assetId === item.assetId ? " is-selected" : ""}`}
                    key={`${item.assetId}-${item.ordinal}`}
                    onClick={() => setSelectedItem(item)}
                  >
                    <span title={item.sourcePath}>{item.sourceRelativePath}</span>
                    <span title={item.targetPath}>{item.targetRelativePath}</span>
                    <span className={`mapping-status is-${item.status}`}>
                      {item.status === "ready"
                        ? "可规划"
                        : item.status === "warning"
                          ? "需注意"
                          : item.status === "skipped_conflict"
                            ? "跳过"
                            : "错误"}
                    </span>
                  </button>
                ))}
              </div>
            </>
          ) : (
            <div className="organization-empty">
              <strong>尚未生成</strong>
            </div>
          )}
        </main>

        <aside className="organization-detail" aria-label="整理预览详情">
          <div className="organization-preview-heading">
            <div>
              <h2>路径检查</h2>
            </div>
          </div>
          {selectedItem ? (
            <>
              <div className="path-card">
                <small>源路径</small>
                <code>{selectedItem.sourcePath}</code>
                <small>规划目标</small>
                <code>{selectedItem.targetPath}</code>
              </div>
              <div className="variable-list">
                <strong>模板变量</strong>
                {Object.entries(selectedItem.variables).map(([key, value]) => (
                  <div key={key}>
                    <span>{key}</span>
                    <code>{value || "（空）"}</code>
                  </div>
                ))}
              </div>
              <div className="issue-list">
                <strong>检查结果</strong>
                {selectedItem.issues.length ? (
                  selectedItem.issues.map((issue, index) => (
                    <div
                      className={`organization-issue is-${issue.severity}`}
                      key={`${issue.code}-${index}`}
                    >
                      <span>{issue.severity === "error" ? "错误" : "提示"}</span>
                      <p>{issue.detail}</p>
                    </div>
                  ))
                ) : (
                  <span className="no-issues">没有发现路径问题。</span>
                )}
              </div>
            </>
          ) : (
            <div className="organization-empty is-compact">
              <strong>选择映射后查看路径</strong>
            </div>
          )}
        </aside>
      </div>
    </section>
  );
}

function SummaryMetric({ label, value, tone }: { label: string; value: string; tone?: string }) {
  return (
    <div className={`organization-metric${tone ? ` is-${tone}` : ""}`}>
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}

function TreeNode({ node, depth }: { node: OrganizationPlan["tree"]; depth: number }) {
  return (
    <div className="tree-node" style={{ paddingLeft: `${depth * 14}px` }}>
      <div className="tree-node-label">
        <span className={node.children.length ? "tree-folder" : "tree-file"}>
          {node.children.length ? "▾" : "•"}
        </span>
        <strong title={node.relativePath}>{node.name}</strong>
        <span>
          {node.fileCount} 张 · {formatBytes(node.byteCount)}
        </span>
      </div>
      {node.children.map((child) => (
        <TreeNode key={child.relativePath} node={child} depth={depth + 1} />
      ))}
    </div>
  );
}

function messageFrom(reason: unknown): string {
  if (reason instanceof Error) return reason.message;
  if (typeof reason === "string") return reason;
  return "整理预览失败，请查看应用日志。";
}
