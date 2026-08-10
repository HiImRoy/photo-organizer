import { useMemo, useState } from "react";

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

const levelLabels: Record<OrganizationLevelKind, string> = {
  year: "年",
  month: "月",
  day: "日",
  original_directory: "原始目录",
  primary_semantic: "主要语义",
  tone: "影调",
  dominant_color: "主色",
  saturation: "饱和度等级",
};

const fallbackLabels: Record<OrganizationMissingFallback, string> = {
  modification_time: "修改时间回退",
  unknown: "未知",
  skip: "跳过维度",
  block: "阻止此项",
};

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
  const [plan, setPlan] = useState<OrganizationPlan | null>(null);
  const [selectedItem, setSelectedItem] = useState<OrganizationPlanItem | null>(null);
  const [dragIndex, setDragIndex] = useState<number | null>(null);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const scopeCount = useMemo(() => {
    if (scope === "all") return library.presentCount;
    if (scope === "selected") return selectedAssetIds.length;
    return filteredCount;
  }, [filteredCount, library.presentCount, scope, selectedAssetIds.length]);

  async function chooseTarget() {
    setError(null);
    try {
      const path = await chooseOrganizationTargetFolder();
      if (path) setTargetRoot(path);
    } catch (reason) {
      setError(messageFrom(reason));
    }
  }

  async function generatePlan() {
    setError(null);
    setMessage(null);
    if (!targetRoot.trim()) {
      setError("请先选择或输入目标根目录；dry-run 不会创建该目录。");
      return;
    }
    if (scope === "selected" && selectedAssetIds.length === 0) {
      setError("当前没有选中的图片，无法生成“用户选中”范围的预览。");
      return;
    }
    setBusy(true);
    try {
      const request: OrganizationPlanRequest = {
        libraryId: library.id,
        targetRoot: targetRoot.trim(),
        scope,
        filter: scopeInput.query.filter,
        selectedAssetIds,
        rules,
      };
      const nextPlan = await previewOrganizationPlan(request);
      setPlan(nextPlan);
      setSelectedItem(nextPlan.items[0] ?? null);
      setMessage("整理方案已重新生成。没有创建目标目录，也没有修改源文件。");
    } catch (reason) {
      setError(messageFrom(reason));
    } finally {
      setBusy(false);
    }
  }

  function updateLevel(index: number, next: Partial<OrganizationLevel>) {
    setRules((current) => ({
      ...current,
      levels: current.levels.map((level, levelIndex) =>
        levelIndex === index ? { ...level, ...next } : level,
      ),
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
    if (!plan) return;
    setError(null);
    try {
      const path = await exportOrganizationManifest(plan, format);
      if (path) setMessage(`已导出 ${format.toUpperCase()} dry-run 清单：${path}`);
    } catch (reason) {
      setError(messageFrom(reason));
    }
  }

  return (
    <section className="organization-workspace" aria-label="整理预览工作区">
      <div className="organization-safety-banner">
        <span className="safety-dot" aria-hidden="true" />
        <strong>只读整理预览</strong>
        <span>仅生成源路径 → 规划目标路径映射，不会创建目录、复制、移动、重命名或删除文件。</span>
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
            <div>
              <small>PLANNING MODE</small>
              <h2>整理方案</h2>
            </div>
            <span className="read-only-chip">DRY-RUN</span>
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
            <small>目标目录位于源图库内部时会被阻止。</small>
          </div>

          <div className="organization-control-group">
            <div className="organization-label-row">
              <label>目录维度顺序</label>
              <small>拖动或使用箭头</small>
            </div>
            <div className="organization-levels">
              {rules.levels.map((level, index) => (
                <div
                  key={`${level.kind}-${index}`}
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
                    value={level.kind}
                    onChange={(event) =>
                      updateLevel(index, { kind: event.target.value as OrganizationLevelKind })
                    }
                  >
                    {Object.entries(levelLabels).map(([value, label]) => (
                      <option key={value} value={value}>
                        {label}
                      </option>
                    ))}
                  </select>
                  <select
                    aria-label={`${levelLabels[level.kind]}缺失回退`}
                    value={level.fallback}
                    onChange={(event) =>
                      updateLevel(index, {
                        fallback: event.target.value as OrganizationMissingFallback,
                      })
                    }
                  >
                    {Object.entries(fallbackLabels).map(([value, label]) => (
                      <option key={value} value={value}>
                        {label}
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
              ))}
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
              变量：capture_time、camera、lens、original_name、semantic、tone、dominant_color、saturation、sequence、short_hash。
            </small>
          </div>

          <div className="organization-rule-row">
            <label>
              缺失元数据
              <select
                value={rules.missingFallback}
                onChange={(event) =>
                  setRules((current) => ({
                    ...current,
                    missingFallback: event.target.value as OrganizationMissingFallback,
                  }))
                }
              >
                {Object.entries(fallbackLabels).map(([value, label]) => (
                  <option key={value} value={value}>
                    {label}
                  </option>
                ))}
              </select>
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
          <span className="organization-scope-note">
            当前范围预计 {scopeCount.toLocaleString()} 张；结果来自 SQLite 查询层。
          </span>
        </aside>

        <main className="organization-preview" aria-label="目标目录树和映射">
          <div className="organization-preview-heading">
            <div>
              <small>TARGET DIRECTORY PREVIEW</small>
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
                  目标卷可用空间未探测；没有创建探测文件，预计空间仅为源文件大小合计。
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
              <strong>尚未生成整理方案</strong>
              <span>选择目标根目录和目录维度后，生成完整的只读映射。</span>
            </div>
          )}
        </main>

        <aside className="organization-detail" aria-label="整理预览详情">
          <div className="organization-preview-heading">
            <div>
              <small>PATH INSPECTOR</small>
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
              <span>点击中央映射中的文件，查看源路径、目标路径、变量和异常。</span>
            </div>
          )}
          <div className="organization-detail-note">
            <strong>安全边界</strong>
            <span>原始路径、虚拟分类和规划目标路径始终分开。此版本没有执行按钮。</span>
          </div>
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
