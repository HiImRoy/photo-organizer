import { useEffect, useRef, useState, type CSSProperties, type KeyboardEvent } from "react";

import type {
  AssetFilter,
  BrowseNode,
  CollectionSummary,
  LibrarySummary,
  SemanticGroupSummary,
  SemanticLabelDescriptor,
} from "../types";
import { ColorRangeFilter } from "./ColorRangeFilter";
import { ChevronIcon, FolderIcon, HeartFolderIcon, LibraryIcon } from "./Icons";
import { RangePair } from "./RangePair";

interface SidebarProps {
  libraries: LibrarySummary[];
  browseNodes: BrowseNode[];
  selectedLibraryId: number | null;
  groups: SemanticGroupSummary[];
  catalog: SemanticLabelDescriptor[];
  filter: AssetFilter;
  collections: CollectionSummary[];
  favoriteSourceActive: boolean;
  activeCollectionId: number | null;
  libraryPanelRatio: number | null;
  onLibraryPanelRatioChange: (ratio: number) => void;
  onImportLibrary: () => void;
  onCreateCollection: (name: string, parentCollectionId: number | null) => void;
  onSelectLibrary: (id: number) => void;
  onRescanLibrary: (library: LibrarySummary) => void;
  onOpenLibrary: (library: LibrarySummary) => void;
  onShowLibraryInfo: (library: LibrarySummary) => void;
  onRemoveLibrary: (library: LibrarySummary) => void;
  onChangeLibraryParent: (library: LibrarySummary, parentLibraryId: number | null) => void;
  assetDropTargetLibraryId: number | null;
  onFilterChange: (filter: AssetFilter) => void;
  onSelectFavorites: () => void;
  onSelectCollection: (collectionId: number) => void;
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
    browseNodes,
    selectedLibraryId,
    groups,
    catalog,
    filter,
    collections,
    favoriteSourceActive,
    activeCollectionId,
    libraryPanelRatio,
    onLibraryPanelRatioChange,
    onImportLibrary,
    onCreateCollection,
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
  } = props;
  const [collapsedLibraryIds, setCollapsedLibraryIds] = useState<Set<number>>(new Set());
  const [collapsedCollectionIds, setCollapsedCollectionIds] = useState<Set<number>>(new Set());
  const [openLibraryMenuId, setOpenLibraryMenuId] = useState<number | null>(null);
  const [addMenuOpen, setAddMenuOpen] = useState(false);
  const [createCollectionOpen, setCreateCollectionOpen] = useState(false);
  const [newCollectionName, setNewCollectionName] = useState("");
  const [newCollectionParentId, setNewCollectionParentId] = useState<number | null>(null);
  const [draggingLibraryId, setDraggingLibraryId] = useState<number | null>(null);
  const [dropTargetId, setDropTargetId] = useState<number | "root" | null>(null);
  const pointerDragRef = useRef<PointerDragState | null>(null);
  const suppressLibraryClickRef = useRef(false);
  const librariesRef = useRef(libraries);
  const onChangeLibraryParentRef = useRef(onChangeLibraryParent);
  const libraryTree = buildLibraryTree(libraries);
  const sourceNodes = browseNodes.some((node) => node.kind === "source")
    ? browseNodes
        .filter((node): node is Extract<BrowseNode, { kind: "source" }> => node.kind === "source")
        .map(browseSourceNodeToLibraryTreeNode)
    : libraryTree;
  const collectionNodes = browseNodes.filter(
    (node): node is Extract<BrowseNode, { kind: "collection" }> => node.kind === "collection",
  );
  const manualCollections = collections.filter(
    (collection) => collection.collectionKind === "manual",
  );
  const sidebarStyle =
    libraryPanelRatio === null
      ? undefined
      : ({
          "--sidebar-library-track": `${libraryPanelRatio}fr`,
          "--sidebar-filter-track": `${1 - libraryPanelRatio}fr`,
        } as CSSProperties);

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
    <aside className="left-panel" aria-label="图库与筛选" style={sidebarStyle}>
      <section
        className="sidebar-module sidebar-library-module"
        aria-labelledby="sidebar-library-title"
      >
        <div className="panel-titlebar">
          <strong id="sidebar-library-title">图库</strong>
          <div className="panel-titlebar-actions">
            <div className="sidebar-add-control">
              <button
                className="library-import-button"
                type="button"
                onClick={() => setAddMenuOpen((current) => !current)}
                aria-label="添加图库或收藏夹"
                aria-expanded={addMenuOpen}
              >
                <span>＋ 添加</span>
                <ChevronIcon width="11" height="11" />
              </button>
              {addMenuOpen ? (
                <div className="sidebar-add-menu" role="menu">
                  <button
                    type="button"
                    role="menuitem"
                    onClick={() => {
                      setAddMenuOpen(false);
                      onImportLibrary();
                    }}
                  >
                    <LibraryIcon width="14" height="14" />
                    <span>导入本地来源</span>
                  </button>
                  <button
                    type="button"
                    role="menuitem"
                    onClick={() => {
                      setAddMenuOpen(false);
                      setCreateCollectionOpen(true);
                    }}
                  >
                    <HeartFolderIcon width="14" height="14" />
                    <span>新建收藏夹</span>
                  </button>
                </div>
              ) : null}
            </div>
          </div>
        </div>

        {createCollectionOpen ? (
          <form
            className="sidebar-create-collection"
            onSubmit={(event) => {
              event.preventDefault();
              const name = newCollectionName.trim();
              if (!name) return;
              onCreateCollection(name, newCollectionParentId);
              setNewCollectionName("");
              setNewCollectionParentId(null);
              setCreateCollectionOpen(false);
            }}
          >
            <div className="sidebar-create-collection-row">
              <input
                value={newCollectionName}
                onChange={(event) => setNewCollectionName(event.target.value)}
                placeholder="收藏夹名称"
                maxLength={100}
                aria-label="收藏夹名称"
                autoFocus
              />
              <button type="submit" disabled={!newCollectionName.trim()}>
                创建
              </button>
              <button
                type="button"
                className="sidebar-create-collection-cancel"
                onClick={() => {
                  setCreateCollectionOpen(false);
                  setNewCollectionName("");
                  setNewCollectionParentId(null);
                }}
              >
                取消
              </button>
            </div>
            <label className="sidebar-create-collection-parent">
              <span>放入</span>
              <select
                value={newCollectionParentId ?? ""}
                onChange={(event) =>
                  setNewCollectionParentId(event.target.value ? Number(event.target.value) : null)
                }
              >
                <option value="">顶层收藏夹</option>
                {manualCollections.map((collection) => (
                  <option key={collection.id} value={collection.id}>
                    {collection.name}
                  </option>
                ))}
              </select>
            </label>
          </form>
        ) : null}

        <div className="sidebar-library-area">
          <div className="nav-list library-tree">
            {collectionNodes.map((node) => (
              <CollectionTreeNode
                key={`collection:${node.collection.id}`}
                node={node}
                depth={0}
                expanded={!collapsedCollectionIds.has(node.collection.id)}
                collapsedCollectionIds={collapsedCollectionIds}
                favoriteSourceActive={favoriteSourceActive}
                activeCollectionId={activeCollectionId}
                onToggle={(id) =>
                  setCollapsedCollectionIds((current) => {
                    const next = new Set(current);
                    if (next.has(id)) next.delete(id);
                    else next.add(id);
                    return next;
                  })
                }
                onSelectFavorites={onSelectFavorites}
                onSelectCollection={onSelectCollection}
              />
            ))}
            {collectionNodes.length > 0 && sourceNodes.length > 0 ? (
              <div className="browse-node-divider">本地来源</div>
            ) : null}
            {sourceNodes.map((node) => (
              <LibraryTreeNode
                key={`source:${node.library.id}`}
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
            {sourceNodes.length === 0 && collectionNodes.length === 0 ? (
              <span className="empty-nav-state">尚未导入图库</span>
            ) : null}
            <div
              className={
                draggingLibraryId === null
                  ? "library-root-drop-target"
                  : dropTargetId !== "root"
                    ? "library-root-drop-target is-dragging"
                    : "library-root-drop-target is-active is-drag-over"
              }
              data-library-root-drop="true"
            >
              拖到这里移出当前父图库
            </div>
          </div>
        </div>
      </section>

      <SidebarResizeHandle
        libraryPanelRatio={libraryPanelRatio}
        onChange={onLibraryPanelRatioChange}
      />

      <section
        className="sidebar-module sidebar-filter-module"
        aria-labelledby="sidebar-filter-title"
      >
        <div className="sidebar-area-heading">
          <strong id="sidebar-filter-title">筛选</strong>
        </div>

        <div className="sidebar-filter-area">
          <section
            className="panel-section sidebar-tone-color-section"
            aria-labelledby="sidebar-tone-color-title"
          >
            <div className="panel-section-heading">
              <span id="sidebar-tone-color-title">影调与颜色</span>
            </div>

            <div className="sidebar-filter-subsection">
              <div className="sidebar-filter-subsection-heading">
                <strong>颜色范围</strong>
                {filter.colorHueCenter !== null && filter.colorHueWidth !== null ? (
                  <span>已设定</span>
                ) : null}
              </div>
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
            </div>

            <div className="sidebar-filter-subsection sidebar-tone-range-subsection">
              <div className="sidebar-filter-subsection-heading">
                <strong>影调范围</strong>
                <span>亮度 / 饱和度</span>
              </div>
              <div className="range-filters">
                <RangePair
                  label="亮度"
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
                  minHint="近灰阶"
                  maxHint="高彩"
                  min={filter.saturationMin}
                  max={filter.saturationMax}
                  onChange={(saturationMin, saturationMax) =>
                    onFilterChange({ ...filter, saturationMin, saturationMax })
                  }
                />
              </div>
            </div>
          </section>

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

          <PanelSection title="拍摄日期">
            <DateRangeFilter
              from={filter.capturedFrom}
              to={filter.capturedTo}
              onChange={(capturedFrom, capturedTo) =>
                onFilterChange({ ...filter, capturedFrom, capturedTo })
              }
            />
          </PanelSection>
        </div>
      </section>
    </aside>
  );
}

const SIDEBAR_LIBRARY_MIN_HEIGHT = 150;
const SIDEBAR_FILTER_MIN_HEIGHT = 180;
const SIDEBAR_RESIZE_STEP = 16;

function SidebarResizeHandle({
  libraryPanelRatio,
  onChange,
}: {
  libraryPanelRatio: number | null;
  onChange: (ratio: number) => void;
}) {
  const dragRef = useRef<{
    pointerId: number;
    startY: number;
    startRatio: number;
    availableHeight: number;
    minRatio: number;
    maxRatio: number;
  } | null>(null);

  const readLayout = (handle: HTMLElement) => {
    const panel = handle.closest<HTMLElement>(".left-panel");
    const library = panel?.querySelector<HTMLElement>(":scope > .sidebar-library-module");
    const filter = panel?.querySelector<HTMLElement>(":scope > .sidebar-filter-module");
    if (!panel || !library || !filter) return null;

    const libraryHeight = library.getBoundingClientRect().height;
    const filterHeight = filter.getBoundingClientRect().height;
    const availableHeight = libraryHeight + filterHeight;
    if (!Number.isFinite(availableHeight) || availableHeight <= 0) return null;

    const currentRatio = libraryHeight / availableHeight;
    const minRatio = Math.min(0.5, SIDEBAR_LIBRARY_MIN_HEIGHT / availableHeight);
    const maxRatio = Math.max(0.5, 1 - SIDEBAR_FILTER_MIN_HEIGHT / availableHeight);
    return {
      availableHeight,
      currentRatio,
      minRatio: Math.min(minRatio, maxRatio),
      maxRatio: Math.max(minRatio, maxRatio),
    };
  };

  const clampRatio = (ratio: number, minRatio: number, maxRatio: number) =>
    Math.max(minRatio, Math.min(maxRatio, ratio));

  useEffect(() => {
    const handlePointerMove = (event: PointerEvent) => {
      const drag = dragRef.current;
      if (!drag || drag.pointerId !== (event.pointerId || 1)) return;
      event.preventDefault();
      const clientY = Number.isFinite(event.clientY) ? event.clientY : drag.startY;
      const nextRatio =
        drag.startRatio + (clientY - drag.startY) / Math.max(1, drag.availableHeight);
      onChange(clampRatio(nextRatio, drag.minRatio, drag.maxRatio));
    };
    const finishPointerDrag = (event: PointerEvent) => {
      if (dragRef.current?.pointerId === (event.pointerId || 1)) dragRef.current = null;
    };

    document.addEventListener("pointermove", handlePointerMove, { passive: false });
    document.addEventListener("pointerup", finishPointerDrag);
    document.addEventListener("pointercancel", finishPointerDrag);
    return () => {
      document.removeEventListener("pointermove", handlePointerMove);
      document.removeEventListener("pointerup", finishPointerDrag);
      document.removeEventListener("pointercancel", finishPointerDrag);
    };
  }, [onChange]);

  const handleKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key !== "ArrowUp" && event.key !== "ArrowDown") return;
    event.preventDefault();
    const layout = readLayout(event.currentTarget);
    if (!layout) return;
    const currentRatio = libraryPanelRatio ?? layout.currentRatio;
    const nextRatio =
      currentRatio +
      (event.key === "ArrowDown" ? SIDEBAR_RESIZE_STEP : -SIDEBAR_RESIZE_STEP) /
        layout.availableHeight;
    onChange(clampRatio(nextRatio, layout.minRatio, layout.maxRatio));
  };

  const currentPercent = Math.round((libraryPanelRatio ?? 0.5) * 100);
  return (
    <div
      className="sidebar-vertical-resize-handle"
      role="separator"
      aria-label="调整图库与筛选高度"
      aria-orientation="horizontal"
      aria-valuemin={15}
      aria-valuemax={85}
      aria-valuenow={currentPercent}
      aria-valuetext={
        libraryPanelRatio === null
          ? "图库与筛选各占一半"
          : `图库 ${currentPercent}%，筛选 ${100 - currentPercent}%`
      }
      tabIndex={0}
      onKeyDown={handleKeyDown}
      onPointerDown={(event) => {
        event.preventDefault();
        const layout = readLayout(event.currentTarget);
        if (!layout) return;
        dragRef.current = {
          pointerId: event.pointerId || 1,
          startY: Number.isFinite(event.clientY) ? event.clientY : 0,
          startRatio: layout.currentRatio,
          availableHeight: layout.availableHeight,
          minRatio: layout.minRatio,
          maxRatio: layout.maxRatio,
        };
        event.currentTarget.setPointerCapture?.(event.pointerId);
      }}
    />
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

function browseSourceNodeToLibraryTreeNode(
  node: Extract<BrowseNode, { kind: "source" }>,
): LibraryTreeNodeData {
  return {
    library: node.library,
    children: node.children
      .filter((child): child is Extract<BrowseNode, { kind: "source" }> => child.kind === "source")
      .map(browseSourceNodeToLibraryTreeNode),
  };
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

function CollectionTreeNode({
  node,
  depth,
  expanded,
  collapsedCollectionIds,
  favoriteSourceActive,
  activeCollectionId,
  onToggle,
  onSelectFavorites,
  onSelectCollection,
}: {
  node: Extract<BrowseNode, { kind: "collection" }>;
  depth: number;
  expanded: boolean;
  collapsedCollectionIds: Set<number>;
  favoriteSourceActive: boolean;
  activeCollectionId: number | null;
  onToggle: (id: number) => void;
  onSelectFavorites: () => void;
  onSelectCollection: (collectionId: number) => void;
}) {
  const { collection } = node;
  const isDefaultFavorites = collection.systemKey === "default_favorites";
  const isActive = isDefaultFavorites ? favoriteSourceActive : activeCollectionId === collection.id;
  const label = collection.name || "未命名收藏夹";
  return (
    <>
      <div
        className={`library-tree-row browse-collection-row${isActive ? " is-active" : ""}`}
        style={{ paddingLeft: `${8 + depth * 14}px` }}
        data-browse-collection-id={collection.id}
      >
        <button
          type="button"
          className={`library-tree-expander${expanded ? " is-expanded" : ""}`}
          onClick={() => onToggle(collection.id)}
          aria-label={expanded ? `折叠 ${label}` : `展开 ${label}`}
          aria-expanded={node.children.length > 0 ? expanded : undefined}
          disabled={!node.children.length}
        >
          <ChevronIcon width="11" height="11" />
        </button>
        <button
          type="button"
          className={isActive ? "nav-row is-active" : "nav-row"}
          onClick={() =>
            isDefaultFavorites ? onSelectFavorites() : onSelectCollection(collection.id)
          }
          title={isDefaultFavorites ? "默认收藏" : collection.name}
        >
          {isDefaultFavorites ? (
            <HeartFolderIcon width="15" height="15" />
          ) : (
            <FolderIcon width="15" height="15" />
          )}
          <span>{label}</span>
          <small>{collection.assetCount}</small>
        </button>
      </div>
      {expanded
        ? node.children
            .filter(
              (child): child is Extract<BrowseNode, { kind: "collection" }> =>
                child.kind === "collection",
            )
            .map((child) => (
              <CollectionTreeNode
                key={`collection:${child.collection.id}`}
                node={child}
                depth={depth + 1}
                expanded={!collapsedCollectionIds.has(child.collection.id)}
                collapsedCollectionIds={collapsedCollectionIds}
                favoriteSourceActive={favoriteSourceActive}
                activeCollectionId={activeCollectionId}
                onToggle={onToggle}
                onSelectFavorites={onSelectFavorites}
                onSelectCollection={onSelectCollection}
              />
            ))
        : null}
    </>
  );
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
          className={
            node.children.length > 0
              ? `library-tree-expander${expanded ? " is-expanded" : ""}`
              : "library-tree-expander"
          }
          onClick={() => onToggle(library.id)}
          aria-label={expanded ? `折叠 ${label}` : `展开 ${label}`}
          aria-expanded={node.children.length > 0 ? expanded : undefined}
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
      {fromDate || toDate ? (
        <div className="date-range-footer">
          <span>{`${formatDateValue(fromDate, "最早")} — ${formatDateValue(toDate, "最近")}`}</span>
          <button type="button" onClick={() => onChange(null, null)}>
            清除
          </button>
        </div>
      ) : null}
    </div>
  );
}

function formatDateValue(value: string, fallback: string) {
  return value ? value.replace(/-/g, "/") : fallback;
}
