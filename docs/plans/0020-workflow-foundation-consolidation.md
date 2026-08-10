# Plan 0020 — Workflow Foundation Consolidation

- 状态：PAUSED_FOR_UI_REMEDIATION（查询契约已保留；旧工作台 IA 不接受，转入 [Plan 0021](0021-lap-ui-integration-remediation.md)）
- 日期：2026-08-10
- 前置：[Checkpoint G](../refactor/checkpoint-g-product-architecture-consolidation.md)
- 产品策略：[next-stage-product-strategy.md](next-stage-product-strategy.md)

## Goal

将当前图库的查询状态和批量范围收敛为可序列化的前端契约，并把现有“智能工作台”重新命名和定位为查找/审阅上下文。此计划不新增业务能力，不创建数据库迁移，不开放文件复制。

## Current facts

- `App.tsx` 的旧查询字段已收敛到 `AssetQueryV1`；selection 仍是独立的用户交互状态。
- `fetchAssets` 已接受 query envelope，并通过兼容 adapter 调用现有 Rust `list_assets`；Organization 从同一 query 读取 filter。
- `WorkflowWorkspace` 已移除不可用的 Faces 一级 tab，但仅改名为“查找与审阅”仍保留独立工作台；该 IA 结果不接受，必须按 Plan 0021 回到主界面各上下文。
- `AssetFilter`/`asset_filter_sql` 是现有可靠查询核心，应继续复用。

## Scope

### 1. AssetQueryV1

新增前端 domain contract：

```ts
AssetQueryV1 {
  version: 1;
  libraryId: number | null;
  filter: AssetFilter;
  sort: SortField;
  direction: SortDirection;
  groupBySemantic: boolean;
  page: number;
  pageSize: number;
}
```

`viewMode` 不属于 query membership；它是浏览呈现状态。`page`/`pageSize` 是运行时分页，不写入未来 Saved View 的持久定义。

### 2. AssetScopeInputV1

第一阶段表达当前已经存在的两个范围：

```ts
type AssetScopeInputV1 =
  | { kind: "query"; query: AssetQueryV1 }
  | { kind: "selection"; assetIds: number[]; query: AssetQueryV1 };
```

Collection、Saved View、Similarity Cluster 和 Duplicate Group 在本阶段只定义可扩展联合类型，不新增解析 IPC；现有工作流保留专用查询实现，但 UI 必须显示其范围语义。

### 3. Existing IA correction（暂停，改由 Plan 0021 执行）

- 之前的改名、隐藏 Faces 和 scope 显示只算临时兼容措施，不算 IA 修复完成。
- 具体的 source/query/review/action 拆分以 [Plan 0021](0021-lap-ui-integration-remediation.md) 为准。
- 不删除现有 Favorite、Collection、Search、Duplicate、Similar、Compare、Edit 能力。

## Out of scope

- Smart Album/Saved View CRUD。
- Pick/Reject/Culling。
- Metadata filter 全量扩展。
- Organization snapshot/schema/COPY。
- Thumbnail batch IPC、virtualization、asset protocol。
- Vector cache、SIMD、mmap、HNSW。
- HEIC、RAW、Video、GPS、Face model。
- 任意源文件写入、移动、删除、重命名或覆盖。

## Files expected to change

- `src/types.ts`
- `src/query.ts`
- `src/query.test.ts`
- `src/App.tsx`
- `src/api.ts`
- `src/components/WorkflowWorkspace.tsx`
- `src/components/OrganizationWorkspace.tsx`
- `src/components/OrganizationWorkspace.test.tsx`
- `docs/refactor/checkpoint-g-product-architecture-consolidation.md`
- `docs/refactor/IMPLEMENTATION_STATUS.md`
- `docs/plans/0021-lap-ui-integration-remediation.md`

不预期修改 migration、Rust query SQL 或生产依赖；如实现中发现必须修改，暂停并更新 ADR/计划。

## Implementation steps

### N1.1 Contract

- 在 `src/types.ts` 定义 `AssetQueryV1`、`AssetScopeInputV1`、`AssetScopeDescription`。
- 添加创建/规范化函数，保证 page >= 1、pageSize 在现有边界内、selection 去重且保持稳定顺序。
- 在 `fetchAssets` 接受 query envelope，同时保留内部兼容 adapter，避免一次性改动 Tauri command。
- 为序列化和规范化添加纯函数测试。

### N1.2 Current query state

- `App.tsx` 使用一个 `assetQuery` state 保存当前 library/filter/sort/direction/group/page/pageSize。
- 提供局部更新函数，保持现有控件和行为。
- 数据请求、preview all、Organization props 均从同一个 query 读取。
- viewMode、active asset、selection 保持独立，因为它们不是 query membership。

### N1.3 Scope description

- App 根据当前 query 和 selection 创建 `AssetScopeInputV1`。
- Workflow workspace 和 Organization workspace 接收 scope description，显示来源与数量。
- 现有 Organization request 暂时继续转换为 legacy `scope/filter/selectedAssetIds`，不改 Rust schema；转换函数单测覆盖。

### N1.4 IA correction

- `WorkflowWorkspace` 标签调整为 source/search/review/action 语义。
- Faces 不作为一级入口；model unavailable 状态仍可保留在后端和未来设置入口。
- 加入“返回图库”操作，减少结果点击后上下文丢失。
- 维持现有业务组件，避免在本切片重写编辑器或重复查询后端。

### N1.5 Verification

- TypeScript tests：query normalization、scope description、legacy Organization conversion。
- App tests：query updates still reset page; Organization receives current query; workflow entry label and hidden Faces。
- `format:check`、lint、typecheck、frontend tests、build。
- 只读检查 `git diff --check`，确认没有 migration 和源文件操作。

## Execution record — 2026-08-10

- [x] N1.1：完成 `AssetQueryV1`、`AssetScopeInputV1`、规范化和稳定 selection id 的纯函数契约。
- [x] N1.2：`App.tsx` 使用单一 `assetQuery` state；Grid、preview-all、Organization 均从它读取。
- [x] N1.3（当前切片）：Workflow/Organization 显示 scope；显式 selection 进入 Organization 时默认选择“用户选中”，legacy request 仍由现有 `scope/filter/selectedAssetIds` adapter 发送。
- [ ] N1.4：旧切片仅完成入口改名和 Faces 隐藏；独立工作台 IA 不满足验收，转由 G-UI 修复。
- [x] N1.5 前端验证：format、lint、typecheck、36 个 frontend tests、build 和 `git diff --check` 通过。
- [ ] N1.5 性能基线：尚未生成 1k–100k thumbnail/vector JSON 基线；不得据此引入 virtualization、batch IPC 或 ANN。
- [ ] N1.6 桌面人工验收：Browse → Review → Back 与 Query/Selection → Organization scope preview 尚未在桌面端执行。
- [ ] Rust test/clippy：当前环境没有 `cargo`，无法执行；不把它标记为通过。

## Acceptance criteria

- [x] 当前 Grid 的 library/filter/sort/direction/group/page/pageSize 只有一个 `AssetQueryV1` 状态来源。
- [x] 现有 `fetchAssets`、preview-all 和 Organization scope 显式从该 query 读取。
- [x] Selection 与 current query 有可见的 `AssetScopeDescription`。
- [ ] 不再存在承载多种意图的通用工作台；旧切片只完成改名，未满足该条件。
- [x] Faces 不显示为不可用的一级工作流 tab。
- [x] 不新增数据库 migration、生产依赖或任何真实文件写操作。
- [x] 相关 frontend checks 全部通过。
- [ ] 完整 performance baseline 和桌面人工验收尚未完成。

## Risks and mitigations

- App state 改造容易影响筛选分页：保留现有行为测试，并让规范化函数集中处理 page reset。
- Tauri command 当前仍是旧参数：用前端 adapter，避免扩大 Rust 变更。
- 工作台结果仍有专用后端查询；在 G-UI 完成前不得通过改名假装已完成统一 IA。
- Organization 仍不是 immutable snapshot：文案和类型不使用 confirmed/execute 语义。

## Stop condition

当前 N1 技术切片暂停；先执行 G-UI。G-UI 未通过前，不补 benchmark，也不进入 Culling、Saved View、Backup 或 Organization Safe Copy。
