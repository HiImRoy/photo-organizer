import { useMemo, useState } from "react";

import type {
  AssetFilter,
  FolderSummary,
  LibrarySummary,
  SemanticGroupSummary,
  SemanticLabelDescriptor,
  SemanticRuntimeStatus,
} from "../types";
import { ChevronIcon, FolderIcon, LibraryIcon, PanelIcon, ShieldIcon } from "./Icons";

interface SidebarProps {
  collapsed: boolean;
  libraries: LibrarySummary[];
  selectedLibraryId: number | null;
  folders: FolderSummary[];
  groups: SemanticGroupSummary[];
  catalog: SemanticLabelDescriptor[];
  filter: AssetFilter;
  semanticStatus: SemanticRuntimeStatus | null;
  onToggle: () => void;
  onSelectLibrary: (id: number) => void;
  onFilterChange: (filter: AssetFilter) => void;
}

const tones = [
  ["low_key", "低调"],
  ["balanced", "均衡"],
  ["high_key", "高调"],
] as const;

const colors = [
  ["red", "红"],
  ["orange", "橙"],
  ["yellow", "黄"],
  ["green", "绿"],
  ["cyan", "青"],
  ["blue", "蓝"],
  ["purple", "紫"],
  ["neutral", "中性"],
] as const;

export function Sidebar(props: SidebarProps) {
  const {
    collapsed,
    libraries,
    selectedLibraryId,
    folders,
    groups,
    catalog,
    filter,
    semanticStatus,
    onToggle,
    onSelectLibrary,
    onFilterChange,
  } = props;
  const [expandedFolders, setExpandedFolders] = useState<Set<string>>(new Set());
  const folderTree = useMemo(() => buildFolderTree(folders), [folders]);
  const selectedLibrary = libraries.find((library) => library.id === selectedLibraryId) ?? null;

  if (collapsed) {
    return (
      <aside className="left-panel is-collapsed" aria-label="图库与筛选">
        <button className="panel-toggle" type="button" onClick={onToggle} aria-label="展开左侧面板">
          <PanelIcon width="17" height="17" />
        </button>
        <LibraryIcon width="18" height="18" />
        <FolderIcon width="18" height="18" />
      </aside>
    );
  }

  return (
    <aside className="left-panel" aria-label="图库与筛选">
      <div className="panel-titlebar">
        <strong>资料库</strong>
        <button className="panel-toggle" type="button" onClick={onToggle} aria-label="折叠左侧面板">
          <PanelIcon width="17" height="17" />
        </button>
      </div>

      <PanelSection title="图库">
        <div className="nav-list">
          {libraries.map((library) => (
            <button
              type="button"
              className={library.id === selectedLibraryId ? "nav-row is-active" : "nav-row"}
              key={library.id}
              onClick={() => onSelectLibrary(library.id)}
              title={library.rootPath}
            >
              <LibraryIcon width="15" height="15" />
              <span>{library.rootPath.split(/[\\/]/).at(-1) || library.rootPath}</span>
              <small>{library.presentCount}</small>
            </button>
          ))}
        </div>
      </PanelSection>

      <PanelSection title="原始文件夹">
        <div className="nav-list compact">
          <button
            type="button"
            className={!filter.folderPrefix ? "nav-row is-active" : "nav-row"}
            onClick={() => onFilterChange({ ...filter, folderPrefix: null })}
          >
            <FolderIcon width="14" height="14" />
            <span>全部目录</span>
          </button>
          {selectedLibrary ? (
            <button
              type="button"
              className={!filter.folderPrefix ? "nav-row is-active" : "nav-row"}
              onClick={() => onFilterChange({ ...filter, folderPrefix: null })}
              title={selectedLibrary.rootPath}
            >
              <LibraryIcon width="14" height="14" />
              <span>
                {selectedLibrary.rootPath.split(/[\\/]/).at(-1) || selectedLibrary.rootPath}
              </span>
              <small>{selectedLibrary.presentCount}</small>
            </button>
          ) : null}
          {folderTree.map((node) => (
            <FolderTreeNode
              key={node.relativePath}
              node={node}
              depth={0}
              expanded={expandedFolders.has(node.relativePath)}
              onToggle={() =>
                setExpandedFolders((current) => {
                  const next = new Set(current);
                  if (next.has(node.relativePath)) next.delete(node.relativePath);
                  else next.add(node.relativePath);
                  return next;
                })
              }
              selectedPath={filter.folderPrefix}
              onSelect={(path) => onFilterChange({ ...filter, folderPrefix: path })}
              expandedFolders={expandedFolders}
              setExpandedFolders={setExpandedFolders}
            />
          ))}
        </div>
      </PanelSection>

      <PanelSection title="快捷入口">
        <div className="nav-list compact">
          <FilterRow
            active={filter.semanticState === "not_analyzed"}
            label="尚未语义分析"
            onClick={() =>
              onFilterChange({
                ...filter,
                semanticState: filter.semanticState === "not_analyzed" ? null : "not_analyzed",
              })
            }
          />
          <FilterRow
            active={filter.semanticState === "failed"}
            label="分析失败"
            onClick={() =>
              onFilterChange({
                ...filter,
                semanticState: filter.semanticState === "failed" ? null : "failed",
              })
            }
          />
        </div>
      </PanelSection>

      <PanelSection
        title="内容标签"
        trailing={filter.semanticLabels.length ? `${filter.semanticLabels.length}` : undefined}
      >
        <div className="chip-grid">
          {catalog
            .filter((label) => label.isPrimaryCategory)
            .map((label) => {
              const active = filter.semanticLabels.includes(label.id);
              const count = groups.find((group) => group.labelId === label.id)?.assetCount;
              return (
                <button
                  type="button"
                  className={active ? "filter-chip is-active" : "filter-chip"}
                  key={label.id}
                  onClick={() =>
                    onFilterChange({
                      ...filter,
                      semanticLabels: toggleValue(filter.semanticLabels, label.id),
                    })
                  }
                >
                  {label.displayName}
                  {count ? <small>{count}</small> : null}
                </button>
              );
            })}
        </div>
        {filter.semanticLabels.length > 1 ? (
          <div className="match-mode" aria-label="语义标签匹配方式">
            <button
              type="button"
              className={filter.semanticMatch === "any" ? "is-active" : ""}
              onClick={() => onFilterChange({ ...filter, semanticMatch: "any" })}
            >
              任一标签
            </button>
            <button
              type="button"
              className={filter.semanticMatch === "all" ? "is-active" : ""}
              onClick={() => onFilterChange({ ...filter, semanticMatch: "all" })}
            >
              同时包含
            </button>
          </div>
        ) : null}
      </PanelSection>

      <PanelSection title="影调">
        <div className="chip-grid three">
          {tones.map(([id, label]) => (
            <button
              type="button"
              className={filter.toneLabels.includes(id) ? "filter-chip is-active" : "filter-chip"}
              key={id}
              onClick={() =>
                onFilterChange({ ...filter, toneLabels: toggleValue(filter.toneLabels, id) })
              }
            >
              {label}
            </button>
          ))}
        </div>
      </PanelSection>

      <PanelSection title="主色">
        <div className="color-filter-list">
          {colors.map(([id, label]) => (
            <button
              type="button"
              className={
                filter.colorCategories.includes(id) ? "color-filter is-active" : "color-filter"
              }
              key={id}
              onClick={() =>
                onFilterChange({
                  ...filter,
                  colorCategories: toggleValue(filter.colorCategories, id),
                })
              }
              aria-label={`${label}色`}
              title={`${label}色`}
            >
              <i data-color={id} />
            </button>
          ))}
        </div>
      </PanelSection>

      <PanelSection title="数值与时间范围">
        <div className="range-filters">
          <RangePair
            label="亮度"
            min={filter.brightnessMin}
            max={filter.brightnessMax}
            onChange={(brightnessMin, brightnessMax) =>
              onFilterChange({ ...filter, brightnessMin, brightnessMax })
            }
          />
          <RangePair
            label="饱和度"
            min={filter.saturationMin}
            max={filter.saturationMax}
            onChange={(saturationMin, saturationMax) =>
              onFilterChange({ ...filter, saturationMin, saturationMax })
            }
          />
          <label className="date-filter">
            <span>拍摄日期从</span>
            <input
              type="date"
              value={filter.capturedFrom?.slice(0, 10) ?? ""}
              onChange={(event) =>
                onFilterChange({ ...filter, capturedFrom: event.target.value || null })
              }
            />
          </label>
          <label className="date-filter">
            <span>至</span>
            <input
              type="date"
              value={filter.capturedTo?.slice(0, 10) ?? ""}
              onChange={(event) =>
                onFilterChange({ ...filter, capturedTo: event.target.value || null })
              }
            />
          </label>
        </div>
      </PanelSection>

      <div className="left-panel-footer">
        <ShieldIcon width="14" height="14" />
        <span>
          <strong>原图只读</strong> · 索引与模型数据保存在应用目录
        </span>
        <small className={semanticStatus?.status === "ready" ? "status-ready" : ""}>
          {semanticStatus?.status === "ready" ? "TinyCLIP · CPU" : "语义模型未就绪"}
        </small>
      </div>
    </aside>
  );
}

interface FolderTreeNodeData {
  name: string;
  relativePath: string;
  assetCount: number;
  children: FolderTreeNodeData[];
}

function FolderTreeNode({
  node,
  depth,
  expanded,
  onToggle,
  selectedPath,
  onSelect,
  expandedFolders,
  setExpandedFolders,
}: {
  node: FolderTreeNodeData;
  depth: number;
  expanded: boolean;
  onToggle: () => void;
  selectedPath: string | null;
  onSelect: (path: string) => void;
  expandedFolders: Set<string>;
  setExpandedFolders: React.Dispatch<React.SetStateAction<Set<string>>>;
}) {
  return (
    <>
      <div className="folder-tree-row" style={{ paddingLeft: `${8 + depth * 14}px` }}>
        <button
          type="button"
          className="folder-tree-expander"
          onClick={onToggle}
          aria-label={expanded ? `折叠 ${node.name}` : `展开 ${node.name}`}
          disabled={!node.children.length}
        >
          <ChevronIcon width="11" height="11" />
        </button>
        <button
          type="button"
          className={selectedPath === node.relativePath ? "nav-row is-active" : "nav-row"}
          onClick={() => onSelect(node.relativePath)}
          title={node.relativePath}
        >
          <FolderIcon width="14" height="14" />
          <span>{node.name}</span>
          <small>{node.assetCount}</small>
        </button>
      </div>
      {expanded
        ? node.children.map((child) => (
            <FolderTreeNode
              key={child.relativePath}
              node={child}
              depth={depth + 1}
              expanded={expandedFolders.has(child.relativePath)}
              onToggle={() =>
                setExpandedFolders((current) => {
                  const next = new Set(current);
                  if (next.has(child.relativePath)) next.delete(child.relativePath);
                  else next.add(child.relativePath);
                  return next;
                })
              }
              selectedPath={selectedPath}
              onSelect={onSelect}
              expandedFolders={expandedFolders}
              setExpandedFolders={setExpandedFolders}
            />
          ))
        : null}
    </>
  );
}

function buildFolderTree(folders: FolderSummary[]): FolderTreeNodeData[] {
  const roots: FolderTreeNodeData[] = [];
  const byPath = new Map<string, FolderTreeNodeData>();
  const sorted = folders
    .filter((folder) => folder.relativePath)
    .slice()
    .sort((left, right) => left.relativePath.localeCompare(right.relativePath));
  for (const folder of sorted) {
    const parts = folder.relativePath.split(/[\\/]+/).filter(Boolean);
    let parent: FolderTreeNodeData | null = null;
    let path = "";
    for (const part of parts) {
      path = path ? `${path}\\${part}` : part;
      let node = byPath.get(path);
      if (!node) {
        node = { name: part, relativePath: path, assetCount: 0, children: [] };
        byPath.set(path, node);
        if (parent) parent.children.push(node);
        else roots.push(node);
      }
      if (
        path === folder.relativePath ||
        path.replaceAll("\\", "/") === folder.relativePath.replaceAll("\\", "/")
      ) {
        node.assetCount = folder.assetCount;
      }
      parent = node;
    }
  }
  return roots;
}

function PanelSection({
  title,
  trailing,
  children,
}: {
  title: string;
  trailing?: string;
  children: React.ReactNode;
}) {
  return (
    <section className="panel-section">
      <div className="panel-section-heading">
        <span>{title}</span>
        {trailing ? <small>{trailing}</small> : null}
      </div>
      {children}
    </section>
  );
}

function FilterRow({
  active,
  label,
  onClick,
}: {
  active: boolean;
  label: string;
  onClick: () => void;
}) {
  return (
    <button type="button" className={active ? "nav-row is-active" : "nav-row"} onClick={onClick}>
      <i className="quick-marker" aria-hidden="true" />
      <span>{label}</span>
    </button>
  );
}

function toggleValue(values: string[], value: string) {
  return values.includes(value) ? values.filter((item) => item !== value) : [...values, value];
}

function RangePair({
  label,
  min,
  max,
  onChange,
}: {
  label: string;
  min: number | null;
  max: number | null;
  onChange: (min: number | null, max: number | null) => void;
}) {
  return (
    <div className="range-pair">
      <span>{label}</span>
      <input
        aria-label={`${label}最小值`}
        type="number"
        min="0"
        max="1"
        step="0.05"
        placeholder="0"
        value={min ?? ""}
        onChange={(event) => onChange(numberOrNull(event.target.value), max)}
      />
      <i>—</i>
      <input
        aria-label={`${label}最大值`}
        type="number"
        min="0"
        max="1"
        step="0.05"
        placeholder="1"
        value={max ?? ""}
        onChange={(event) => onChange(min, numberOrNull(event.target.value))}
      />
    </div>
  );
}

function numberOrNull(value: string) {
  if (!value) return null;
  const number = Number(value);
  return Number.isFinite(number) ? Math.max(0, Math.min(1, number)) : null;
}
