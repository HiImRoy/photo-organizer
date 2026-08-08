import { useState } from "react";

import type {
  AssetFilter,
  LibrarySummary,
  SemanticGroupSummary,
  SemanticLabelDescriptor,
  SemanticRuntimeStatus,
} from "../types";
import { ChevronIcon, LibraryIcon, PanelIcon, ShieldIcon } from "./Icons";

interface SidebarProps {
  collapsed: boolean;
  libraries: LibrarySummary[];
  selectedLibraryId: number | null;
  groups: SemanticGroupSummary[];
  catalog: SemanticLabelDescriptor[];
  filter: AssetFilter;
  semanticStatus: SemanticRuntimeStatus | null;
  onToggle: () => void;
  onImportLibrary: () => void;
  onSelectLibrary: (id: number) => void;
  onRescanLibrary: (library: LibrarySummary) => void;
  onOpenLibrary: (library: LibrarySummary) => void;
  onShowLibraryInfo: (library: LibrarySummary) => void;
  onRemoveLibrary: (library: LibrarySummary) => void;
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
    groups,
    catalog,
    filter,
    semanticStatus,
    onToggle,
    onImportLibrary,
    onSelectLibrary,
    onRescanLibrary,
    onOpenLibrary,
    onShowLibraryInfo,
    onRemoveLibrary,
    onFilterChange,
  } = props;
  const [collapsedLibraryIds, setCollapsedLibraryIds] = useState<Set<number>>(new Set());
  const [openLibraryMenuId, setOpenLibraryMenuId] = useState<number | null>(null);
  const libraryTree = buildLibraryTree(libraries);

  if (collapsed) {
    return (
      <aside className="left-panel is-collapsed" aria-label="图库与筛选">
        <button className="panel-toggle" type="button" onClick={onToggle} aria-label="展开左侧面板">
          <PanelIcon width="17" height="17" />
        </button>
        <LibraryIcon width="18" height="18" />
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
        <div className="nav-list library-tree">
          {libraryTree.map((node) => (
            <LibraryTreeNode
              key={node.library.id}
              node={node}
              depth={0}
              expanded={!collapsedLibraryIds.has(node.library.id)}
              collapsedLibraryIds={collapsedLibraryIds}
              selectedLibraryId={selectedLibraryId}
              openLibraryMenuId={openLibraryMenuId}
              onToggle={(id) =>
                setCollapsedLibraryIds((current) => {
                  const next = new Set(current);
                  if (next.has(id)) next.delete(id);
                  else next.add(id);
                  return next;
                })
              }
              onOpenMenu={setOpenLibraryMenuId}
              onSelectLibrary={onSelectLibrary}
              onRescanLibrary={onRescanLibrary}
              onOpenLibrary={onOpenLibrary}
              onShowLibraryInfo={onShowLibraryInfo}
              onRemoveLibrary={onRemoveLibrary}
            />
          ))}
          {libraryTree.length === 0 ? <span className="empty-nav-state">尚未导入图库</span> : null}
          <button className="nav-row import-library-row" type="button" onClick={onImportLibrary}>
            <LibraryIcon width="15" height="15" />
            <span>＋ 导入图库</span>
          </button>
        </div>
      </PanelSection>

      <PanelSection title="更多筛选">
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

interface LibraryTreeNodeData {
  library: LibrarySummary;
  children: LibraryTreeNodeData[];
}

function buildLibraryTree(libraries: LibrarySummary[]): LibraryTreeNodeData[] {
  const nodes = new Map<number, LibraryTreeNodeData>();
  for (const library of libraries) nodes.set(library.id, { library, children: [] });

  const roots: LibraryTreeNodeData[] = [];
  for (const node of nodes.values()) {
    const parent =
      node.library.parentLibraryId === null ? null : nodes.get(node.library.parentLibraryId);
    if (parent && parent.library.id !== node.library.id) parent.children.push(node);
    else roots.push(node);
  }

  const sortNodes = (items: LibraryTreeNodeData[]) => {
    items.sort(
      (left, right) =>
        left.library.displayOrder - right.library.displayOrder ||
        left.library.name.localeCompare(right.library.name, "zh-CN"),
    );
    for (const item of items) sortNodes(item.children);
  };
  sortNodes(roots);
  return roots;
}

function LibraryTreeNode({
  node,
  depth,
  expanded,
  collapsedLibraryIds,
  selectedLibraryId,
  openLibraryMenuId,
  onToggle,
  onOpenMenu,
  onSelectLibrary,
  onRescanLibrary,
  onOpenLibrary,
  onShowLibraryInfo,
  onRemoveLibrary,
}: {
  node: LibraryTreeNodeData;
  depth: number;
  expanded: boolean;
  collapsedLibraryIds: Set<number>;
  selectedLibraryId: number | null;
  openLibraryMenuId: number | null;
  onToggle: (id: number) => void;
  onOpenMenu: (id: number | null) => void;
  onSelectLibrary: (id: number) => void;
  onRescanLibrary: (library: LibrarySummary) => void;
  onOpenLibrary: (library: LibrarySummary) => void;
  onShowLibraryInfo: (library: LibrarySummary) => void;
  onRemoveLibrary: (library: LibrarySummary) => void;
}) {
  const { library } = node;
  const menuOpen = openLibraryMenuId === library.id;
  const label = library.name || library.sourcePath;
  return (
    <>
      <div
        className="library-tree-row"
        style={{ paddingLeft: `${8 + depth * 14}px` }}
        onContextMenu={(event) => {
          event.preventDefault();
          onOpenMenu(library.id);
        }}
      >
        <button
          type="button"
          className="library-tree-expander"
          onClick={() => onToggle(library.id)}
          aria-label={expanded ? `折叠 ${label}` : `展开 ${label}`}
          disabled={!node.children.length}
        >
          <ChevronIcon width="11" height="11" />
        </button>
        <button
          type="button"
          className={library.id === selectedLibraryId ? "nav-row is-active" : "nav-row"}
          onClick={() => onSelectLibrary(library.id)}
          title={library.sourcePath}
        >
          <LibraryIcon width="15" height="15" />
          <span>{library.status === "unavailable" ? `${label}（位置不可用）` : label}</span>
          <small>{library.presentCount}</small>
        </button>
        <button
          type="button"
          className="library-menu-trigger"
          aria-label={`${label}图库菜单`}
          onClick={(event) => {
            event.stopPropagation();
            onOpenMenu(menuOpen ? null : library.id);
          }}
        >
          …
        </button>
        {menuOpen ? (
          <div className="library-context-menu" role="menu">
            <button
              type="button"
              onClick={() => {
                onOpenMenu(null);
                onRescanLibrary(library);
              }}
            >
              重新扫描
            </button>
            <button
              type="button"
              onClick={() => {
                onOpenMenu(null);
                onOpenLibrary(library);
              }}
            >
              在资源管理器中显示
            </button>
            <button
              type="button"
              onClick={() => {
                onOpenMenu(null);
                onShowLibraryInfo(library);
              }}
            >
              图库信息
            </button>
            <button
              type="button"
              className="danger-action"
              onClick={() => {
                onOpenMenu(null);
                onRemoveLibrary(library);
              }}
            >
              从资料库移除
            </button>
          </div>
        ) : null}
      </div>
      {expanded
        ? node.children.map((child) => (
            <LibraryTreeNode
              key={child.library.id}
              node={child}
              depth={depth + 1}
              expanded={!collapsedLibraryIds.has(child.library.id)}
              collapsedLibraryIds={collapsedLibraryIds}
              selectedLibraryId={selectedLibraryId}
              openLibraryMenuId={openLibraryMenuId}
              onToggle={onToggle}
              onOpenMenu={onOpenMenu}
              onSelectLibrary={onSelectLibrary}
              onRescanLibrary={onRescanLibrary}
              onOpenLibrary={onOpenLibrary}
              onShowLibraryInfo={onShowLibraryInfo}
              onRemoveLibrary={onRemoveLibrary}
            />
          ))
        : null}
    </>
  );
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
