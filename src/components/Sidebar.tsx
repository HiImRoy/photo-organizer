import { useEffect, useRef, useState } from "react";

import type {
  AssetFilter,
  CollectionSummary,
  LibrarySummary,
  SemanticGroupSummary,
  SemanticLabelDescriptor,
  SemanticRuntimeStatus,
  SubjectRuntimeStatus,
} from "../types";
import { ColorRangeFilter } from "./ColorRangeFilter";
import { ChevronIcon, LibraryIcon, ShieldIcon } from "./Icons";

interface SidebarProps {
  libraries: LibrarySummary[];
  selectedLibraryId: number | null;
  groups: SemanticGroupSummary[];
  catalog: SemanticLabelDescriptor[];
  filter: AssetFilter;
  semanticStatus: SemanticRuntimeStatus | null;
  subjectStatus?: SubjectRuntimeStatus | null;
  collections?: CollectionSummary[];
  activeCollectionId?: number | null;
  favoriteSourceActive?: boolean;
  onImportLibrary: () => void;
  onSelectLibrary: (id: number) => void;
  onRescanLibrary: (library: LibrarySummary) => void;
  onOpenLibrary: (library: LibrarySummary) => void;
  onShowLibraryInfo: (library: LibrarySummary) => void;
  onRemoveLibrary: (library: LibrarySummary) => void;
  onChangeLibraryParent: (library: LibrarySummary, parentLibraryId: number | null) => void;
  assetDropTargetLibraryId: number | null;
  onFilterChange: (filter: AssetFilter) => void;
  onSelectFavorites?: () => void;
  onSelectCollection?: (collectionId: number) => void;
  onOpenWorkflowTool?: (tool: "collections" | "search") => void;
}

const tones = [
  ["low_key", "低调"],
  ["balanced", "均衡"],
  ["high_key", "高调"],
] as const;

const saturationLevels = [
  ["low", "低饱和"],
  ["medium", "中饱和"],
  ["high", "高饱和"],
] as const;

export function Sidebar(props: SidebarProps) {
  const {
    libraries,
    selectedLibraryId,
    groups,
    catalog,
    filter,
    semanticStatus,
    subjectStatus,
    collections = [],
    activeCollectionId = null,
    favoriteSourceActive = false,
    onImportLibrary,
    onSelectLibrary,
    onRescanLibrary,
    onOpenLibrary,
    onShowLibraryInfo,
    onRemoveLibrary,
    onChangeLibraryParent,
    assetDropTargetLibraryId,
    onFilterChange,
    onSelectFavorites,
    onSelectCollection,
    onOpenWorkflowTool,
  } = props;
  const [collapsedLibraryIds, setCollapsedLibraryIds] = useState<Set<number>>(new Set());
  const [openLibraryMenuId, setOpenLibraryMenuId] = useState<number | null>(null);
  const [draggingLibraryId, setDraggingLibraryId] = useState<number | null>(null);
  const [dropTargetId, setDropTargetId] = useState<number | "root" | null>(null);
  const pointerDragRef = useRef<PointerDragState | null>(null);
  const suppressLibraryClickRef = useRef(false);
  const librariesRef = useRef(libraries);
  const onChangeLibraryParentRef = useRef(onChangeLibraryParent);
  const libraryTree = buildLibraryTree(libraries);

  useEffect(() => {
    librariesRef.current = libraries;
    onChangeLibraryParentRef.current = onChangeLibraryParent;
  }, [libraries, onChangeLibraryParent]);

  useEffect(() => {
    const findDropTarget = (event: PointerEvent): number | "root" | null => {
      const pointElement =
        typeof document.elementFromPoint === "function"
          ? document.elementFromPoint(event.clientX, event.clientY)
          : null;
      const element = pointElement ?? event.target;
      if (!(element instanceof Element)) return null;
      const row = element.closest<HTMLElement>("[data-library-drop-id]");
      if (row) {
        const libraryId = Number(row.dataset.libraryDropId);
        return Number.isInteger(libraryId) ? libraryId : null;
      }
      return element.closest("[data-library-root-drop]") ? "root" : null;
    };

    const isValidDropTarget = (sourceId: number, target: number | "root" | null) => {
      if (target === "root") {
        return librariesRef.current.some(
          (library) => library.id === sourceId && library.parentLibraryId !== null,
        );
      }
      return target !== null && canDropLibrary(sourceId, target, librariesRef.current);
    };

    const handlePointerMove = (event: PointerEvent) => {
      const drag = pointerDragRef.current;
      const pointerId = event.pointerId || 1;
      if (!drag || drag.pointerId !== pointerId) return;

      const distance = Math.hypot(event.clientX - drag.startX, event.clientY - drag.startY);
      if (!drag.active && distance < 6) return;

      if (!drag.active) {
        drag.active = true;
        suppressLibraryClickRef.current = true;
        setDraggingLibraryId(drag.libraryId);
      }

      event.preventDefault();
      const target = findDropTarget(event);
      setDropTargetId(isValidDropTarget(drag.libraryId, target) ? target : null);
    };

    const finishPointerDrag = (event: PointerEvent, cancelled: boolean) => {
      const drag = pointerDragRef.current;
      const pointerId = event.pointerId || 1;
      if (!drag || drag.pointerId !== pointerId) return;

      const target = drag.active && !cancelled ? findDropTarget(event) : null;
      const source = librariesRef.current.find((library) => library.id === drag.libraryId);
      if (source && isValidDropTarget(drag.libraryId, target)) {
        onChangeLibraryParentRef.current(source, target === "root" ? null : target);
      }
      if (drag.active) {
        suppressLibraryClickRef.current = true;
        window.setTimeout(() => {
          suppressLibraryClickRef.current = false;
        }, 0);
      }
      pointerDragRef.current = null;
      setDraggingLibraryId(null);
      setDropTargetId(null);
    };

    const handlePointerCancel = (event: PointerEvent) => finishPointerDrag(event, true);
    const handlePointerUp = (event: PointerEvent) => finishPointerDrag(event, false);

    document.addEventListener("pointermove", handlePointerMove, { passive: false });
    document.addEventListener("pointerup", handlePointerUp);
    document.addEventListener("pointercancel", handlePointerCancel);
    return () => {
      document.removeEventListener("pointermove", handlePointerMove);
      document.removeEventListener("pointerup", handlePointerUp);
      document.removeEventListener("pointercancel", handlePointerCancel);
    };
  }, []);

  const beginLibraryPointerDrag = (
    libraryId: number,
    event: React.PointerEvent<HTMLDivElement>,
  ) => {
    if (event.button !== undefined && event.button !== 0 && event.button !== -1) return;
    const target = event.target;
    if (
      target instanceof Element &&
      target.closest(".library-menu-trigger, .library-tree-expander")
    ) {
      return;
    }
    pointerDragRef.current = {
      libraryId,
      pointerId: event.pointerId || 1,
      startX: event.clientX,
      startY: event.clientY,
      active: false,
    };
  };

  const selectLibraryFromRow = (libraryId: number) => {
    if (suppressLibraryClickRef.current) {
      suppressLibraryClickRef.current = false;
      return;
    }
    onSelectLibrary(libraryId);
  };

  useEffect(() => {
    if (openLibraryMenuId === null) return undefined;

    const closeMenuOnOutsidePointer = (event: PointerEvent) => {
      const target = event.target;
      if (!(target instanceof Element)) return;
      if (target.closest(".library-context-menu") || target.closest(".library-menu-trigger")) {
        return;
      }
      setOpenLibraryMenuId(null);
    };

    document.addEventListener("pointerdown", closeMenuOnOutsidePointer);
    return () => document.removeEventListener("pointerdown", closeMenuOnOutsidePointer);
  }, [openLibraryMenuId]);

  return (
    <aside className="left-panel" aria-label="图库与筛选">
      <section
        className="sidebar-module sidebar-library-module"
        aria-labelledby="sidebar-library-title"
      >
        <div className="panel-titlebar">
          <strong id="sidebar-library-title">图库</strong>
          <div className="panel-titlebar-actions">
            <button
              className="library-import-button"
              type="button"
              onClick={onImportLibrary}
              aria-label="＋ 导入图库"
            >
              <LibraryIcon width="13" height="13" />
              <span>＋ 导入图库</span>
            </button>
          </div>
        </div>

        <div className="sidebar-library-area">
          <div className="nav-list library-tree">
            {libraryTree.map((node) => (
              <LibraryTreeNode
                key={node.library.id}
                node={node}
                draggingLibraryId={draggingLibraryId}
                dropTargetId={dropTargetId}
                assetDropTargetLibraryId={assetDropTargetLibraryId}
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
                onSelectLibrary={selectLibraryFromRow}
                onRescanLibrary={onRescanLibrary}
                onOpenLibrary={onOpenLibrary}
                onShowLibraryInfo={onShowLibraryInfo}
                onRemoveLibrary={onRemoveLibrary}
                onChangeLibraryParent={onChangeLibraryParent}
                onPointerDown={beginLibraryPointerDrag}
              />
            ))}
            {libraryTree.length === 0 ? (
              <span className="empty-nav-state">尚未导入图库</span>
            ) : null}
            <div
              className={
                draggingLibraryId === null || dropTargetId !== "root"
                  ? "library-root-drop-target"
                  : "library-root-drop-target is-active is-drag-over"
              }
              data-library-root-drop="true"
            >
              拖到这里移出当前父图库
            </div>
          </div>
        </div>
      </section>

      <section
        className="sidebar-module sidebar-source-module"
        aria-labelledby="sidebar-source-title"
      >
        <div className="sidebar-area-heading">
          <strong id="sidebar-source-title">来源筛选</strong>
          <span>收藏与虚拟集合</span>
        </div>
        <p className="sidebar-source-note">只改变当前图库的显示，不移动或复制原文件。</p>
        <div className="sidebar-source-list">
          <button
            type="button"
            className={favoriteSourceActive ? "source-chip is-active" : "source-chip"}
            onClick={onSelectFavorites}
          >
            <span>收藏</span>
            <small>仅收藏照片</small>
          </button>
          {collections.map((collection) => (
            <button
              type="button"
              key={collection.id}
              className={
                activeCollectionId === collection.id ? "source-chip is-active" : "source-chip"
              }
              onClick={() => onSelectCollection?.(collection.id)}
            >
              <span>{collection.name}</span>
              <small>{collection.assetCount} 张 · 虚拟</small>
            </button>
          ))}
          <button
            type="button"
            className="source-chip source-chip-action"
            onClick={() => onOpenWorkflowTool?.("collections")}
          >
            <span>管理集合</span>
            <small>新建 / 编辑</small>
          </button>
        </div>
      </section>

      <section
        className="sidebar-module sidebar-filter-module"
        aria-labelledby="sidebar-filter-title"
      >
        <div className="sidebar-area-heading">
          <strong id="sidebar-filter-title">分类与筛选</strong>
          <span>按内容属性整理图片</span>
        </div>

        <div className="sidebar-filter-area">
          <SemanticFilterSection
            title="拍摄题材"
            categoryGroup="scene"
            labels={catalog.filter((label) => label.categoryGroup === "scene")}
            filter={filter}
            groups={groups}
            onFilterChange={onFilterChange}
          />

          <SemanticFilterSection
            title="主体标签"
            categoryGroup="subject"
            labels={catalog.filter((label) => label.categoryGroup === "subject")}
            filter={filter}
            groups={groups}
            onFilterChange={onFilterChange}
          />

          <PanelSection title="影调">
            <div className="chip-grid three">
              {tones.map(([id, label]) => (
                <button
                  type="button"
                  className={
                    filter.toneLabels.includes(id) ? "filter-chip is-active" : "filter-chip"
                  }
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

          <PanelSection title="饱和度级别">
            <div className="chip-grid three">
              {saturationLevels.map(([id, label]) => (
                <button
                  type="button"
                  className={
                    filter.saturationLevels.includes(id) ? "filter-chip is-active" : "filter-chip"
                  }
                  key={id}
                  onClick={() =>
                    onFilterChange({
                      ...filter,
                      saturationLevels: toggleValue(filter.saturationLevels, id),
                    })
                  }
                >
                  {label}
                </button>
              ))}
            </div>
          </PanelSection>

          <PanelSection title="颜色范围">
            <ColorRangeFilter
              center={filter.colorHueCenter}
              width={filter.colorHueWidth}
              strictness={filter.colorHueStrictness}
              onChange={(colorHueCenter, colorHueWidth) =>
                onFilterChange({
                  ...filter,
                  colorCategories: [],
                  colorHueCenter,
                  colorHueWidth,
                })
              }
              onStrictnessChange={(colorHueStrictness) =>
                onFilterChange({ ...filter, colorHueStrictness })
              }
            />
          </PanelSection>

          <PanelSection title="影调范围">
            <div className="range-filters">
              <RangePair
                label="亮度"
                description="按画面平均亮度筛选"
                minHint="最暗"
                maxHint="最亮"
                min={filter.brightnessMin}
                max={filter.brightnessMax}
                onChange={(brightnessMin, brightnessMax) =>
                  onFilterChange({ ...filter, brightnessMin, brightnessMax })
                }
              />
              <RangePair
                label="饱和度"
                description="按画面平均色彩强度筛选"
                minHint="近灰阶"
                maxHint="高彩"
                min={filter.saturationMin}
                max={filter.saturationMax}
                onChange={(saturationMin, saturationMax) =>
                  onFilterChange({ ...filter, saturationMin, saturationMax })
                }
              />
            </div>
          </PanelSection>

          <PanelSection title="拍摄日期">
            <DateRangeFilter
              from={filter.capturedFrom}
              to={filter.capturedTo}
              onChange={(capturedFrom, capturedTo) =>
                onFilterChange({ ...filter, capturedFrom, capturedTo })
              }
            />
          </PanelSection>

          <div className="left-panel-footer">
            <ShieldIcon width="14" height="14" />
            <span>
              <strong>原图只读</strong> · 索引与模型数据保存在应用目录
            </span>
            <small className={semanticStatus?.status === "ready" ? "status-ready" : ""}>
              {semanticStatus?.status === "ready"
                ? semanticStatus.topicModel
                  ? `题材候选 · ${semanticStatus.topicModel.name}`
                  : "环境模型 · Places365"
                : "语义模型未就绪"}
            </small>
            <small
              className={
                subjectStatus?.status === "ready" || subjectStatus?.status === "partial"
                  ? "status-ready"
                  : ""
              }
            >
              {subjectStatus?.status === "ready"
                ? "主体模型 · PicoDet + YuNet"
                : subjectStatus?.status === "partial"
                  ? "主体模型 · 人像辅助不可用"
                  : "主体模型未就绪"}
            </small>
          </div>
        </div>
      </section>
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

interface PointerDragState {
  libraryId: number;
  pointerId: number;
  startX: number;
  startY: number;
  active: boolean;
}

function canDropLibrary(
  sourceLibraryId: number,
  targetLibraryId: number,
  libraries: LibrarySummary[],
): boolean {
  if (sourceLibraryId === targetLibraryId) return false;
  const byId = new Map(libraries.map((library) => [library.id, library]));
  let current: number | null = targetLibraryId;
  while (current !== null) {
    if (current === sourceLibraryId) return false;
    current = byId.get(current)?.parentLibraryId ?? null;
  }
  return byId.has(sourceLibraryId) && byId.has(targetLibraryId);
}

function LibraryTreeNode({
  node,
  draggingLibraryId,
  dropTargetId,
  assetDropTargetLibraryId,
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
  onChangeLibraryParent,
  onPointerDown,
}: {
  node: LibraryTreeNodeData;
  draggingLibraryId: number | null;
  dropTargetId: number | "root" | null;
  assetDropTargetLibraryId: number | null;
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
  onChangeLibraryParent: (library: LibrarySummary, parentLibraryId: number | null) => void;
  onPointerDown: (libraryId: number, event: React.PointerEvent<HTMLDivElement>) => void;
}) {
  const { library } = node;
  const menuOpen = openLibraryMenuId === library.id;
  const label = library.name || library.sourcePath;
  const rowClassName = [
    "library-tree-row",
    library.id === dropTargetId ? "is-drag-over" : "",
    library.id === assetDropTargetLibraryId ? "is-asset-drag-over" : "",
    library.id === draggingLibraryId ? "is-dragging" : "",
  ]
    .filter(Boolean)
    .join(" ");
  return (
    <>
      <div
        className={rowClassName}
        style={{ paddingLeft: `${8 + depth * 14}px` }}
        data-library-drop-id={library.id}
        onPointerDown={(event) => onPointerDown(library.id, event)}
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
              disabled={library.parentLibraryId === null}
              onClick={() => {
                onOpenMenu(null);
                onChangeLibraryParent(library, null);
              }}
            >
              移出当前父图库
            </button>
            <button
              type="button"
              className="danger-action"
              onClick={() => {
                onOpenMenu(null);
                onRemoveLibrary(library);
              }}
            >
              从图库移除
            </button>
          </div>
        ) : null}
      </div>
      {expanded
        ? node.children.map((child) => (
            <LibraryTreeNode
              key={child.library.id}
              node={child}
              draggingLibraryId={draggingLibraryId}
              dropTargetId={dropTargetId}
              assetDropTargetLibraryId={assetDropTargetLibraryId}
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
              onChangeLibraryParent={onChangeLibraryParent}
              onPointerDown={onPointerDown}
            />
          ))
        : null}
    </>
  );
}

function PanelSection({
  title,
  trailing,
  reserveTrailing = false,
  children,
}: {
  title: string;
  trailing?: string;
  reserveTrailing?: boolean;
  children: React.ReactNode;
}) {
  return (
    <section className="panel-section">
      <div className="panel-section-heading">
        <span>{title}</span>
        {trailing || reserveTrailing ? (
          <small className={trailing ? undefined : "is-placeholder"} aria-hidden={!trailing}>
            {trailing ?? "0"}
          </small>
        ) : null}
      </div>
      {children}
    </section>
  );
}

function SemanticFilterSection({
  title,
  labels,
  categoryGroup,
  filter,
  groups,
  onFilterChange,
}: {
  title: string;
  labels: SemanticLabelDescriptor[];
  categoryGroup: string;
  filter: AssetFilter;
  groups: SemanticGroupSummary[];
  onFilterChange: (filter: AssetFilter) => void;
}) {
  const primary = categoryGroup === "scene";
  const selectedValues = primary ? filter.primaryCategories : filter.auxiliaryTags;

  return (
    <PanelSection
      title={title}
      trailing={selectedValues.length ? `${selectedValues.length}` : undefined}
      reserveTrailing
    >
      <div className="chip-grid">
        {labels.map((label) => {
          const active = selectedValues.includes(label.id);
          const count = groups.find((group) => group.labelId === label.id)?.assetCount;
          return (
            <button
              type="button"
              className={active ? "filter-chip is-active" : "filter-chip"}
              key={label.id}
              onClick={() =>
                onFilterChange({
                  ...filter,
                  ...(primary
                    ? {
                        primaryCategories:
                          filter.primaryCategories[0] === label.id ? [] : [label.id],
                      }
                    : { auxiliaryTags: toggleValue(filter.auxiliaryTags, label.id) }),
                })
              }
            >
              {label.displayName}
              {count ? <small>{count}</small> : null}
            </button>
          );
        })}
      </div>
      {!primary && filter.auxiliaryTags.length > 1 ? (
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
  );
}

function toggleValue<T>(values: T[], value: T) {
  return values.includes(value) ? values.filter((item) => item !== value) : [...values, value];
}

function RangePair({
  label,
  description,
  minHint,
  maxHint,
  min,
  max,
  onChange,
}: {
  label: string;
  description: string;
  minHint: string;
  maxHint: string;
  min: number | null;
  max: number | null;
  onChange: (min: number | null, max: number | null) => void;
}) {
  const minPercent = Math.round((min ?? 0) * 100);
  const maxPercent = Math.round((max ?? 1) * 100);
  const rangeText = formatPercentRange(min, max);

  function updateMin(value: string) {
    const next = Math.min(Number(value) / 100, maxPercent / 100);
    onChange(next <= 0 ? null : next, max);
  }

  function updateMax(value: string) {
    const next = Math.max(Number(value) / 100, minPercent / 100);
    onChange(min, next >= 1 ? null : next);
  }

  return (
    <div className="range-filter-card">
      <div className="range-filter-heading">
        <div>
          <strong>{label}</strong>
          <span>{description}</span>
        </div>
        <output aria-live="polite">{rangeText}</output>
      </div>
      <div className="range-slider" aria-label={`${label}筛选范围`}>
        <span
          className="range-slider-fill"
          style={{ left: `${minPercent}%`, width: `${Math.max(0, maxPercent - minPercent)}%` }}
        />
        <input
          className="range-slider-input range-slider-min"
          aria-label={`${label}最低百分比`}
          aria-valuetext={`${minPercent}%（${minHint}方向）`}
          type="range"
          min="0"
          max="100"
          step="5"
          value={minPercent}
          onChange={(event) => updateMin(event.target.value)}
        />
        <input
          className="range-slider-input range-slider-max"
          aria-label={`${label}最高百分比`}
          aria-valuetext={`${maxPercent}%（${maxHint}方向）`}
          type="range"
          min="0"
          max="100"
          step="5"
          value={maxPercent}
          onChange={(event) => updateMax(event.target.value)}
        />
      </div>
      <div className="range-slider-scale" aria-hidden="true">
        <span>{minHint}</span>
        <span>0% — 100%</span>
        <span>{maxHint}</span>
      </div>
      <div className="range-filter-summary">
        {min === null && max === null ? "未设置：显示全部图片" : `当前显示：${rangeText}范围内`}
      </div>
    </div>
  );
}

function DateRangeFilter({
  from,
  to,
  onChange,
}: {
  from: string | null;
  to: string | null;
  onChange: (from: string | null, to: string | null) => void;
}) {
  const fromDate = from?.slice(0, 10) ?? "";
  const toDate = to?.slice(0, 10) ?? "";

  return (
    <div className="date-range-filter">
      <p>按照片记录的拍摄日期筛选，包含开始和结束当天。</p>
      <div className="date-range-fields">
        <label>
          <span>从</span>
          <input
            type="date"
            aria-label="拍摄日期开始"
            value={fromDate}
            max={toDate || undefined}
            onChange={(event) => onChange(event.target.value || null, to)}
          />
        </label>
        <span className="date-range-separator">至</span>
        <label>
          <span>到</span>
          <input
            type="date"
            aria-label="拍摄日期结束"
            value={toDate}
            min={fromDate || undefined}
            onChange={(event) => onChange(from, event.target.value || null)}
          />
        </label>
      </div>
      <div className="date-range-footer">
        <span>
          {fromDate || toDate
            ? `${formatDateValue(fromDate, "最早")} — ${formatDateValue(toDate, "最近")}`
            : "未设置日期范围"}
        </span>
        {fromDate || toDate ? (
          <button type="button" onClick={() => onChange(null, null)}>
            清除
          </button>
        ) : null}
      </div>
    </div>
  );
}

function formatPercentRange(min: number | null, max: number | null) {
  const format = (value: number) => `${Math.round(value * 100)}%`;
  if (min !== null && max !== null) return `${format(min)} — ${format(max)}`;
  if (min !== null) return `≥ ${format(min)}`;
  if (max !== null) return `≤ ${format(max)}`;
  return "全部";
}

function formatDateValue(value: string, fallback: string) {
  return value ? value.replace(/-/g, "/") : fallback;
}
