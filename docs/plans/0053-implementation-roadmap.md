# 0053 统一本地来源与收藏夹模型：初期执行规划

状态：Phase 1、Phase 2 已实现；Phase 3 统一图库导航第一轮已实现，收藏成员批量操作与旧逻辑清理待后续阶段。

关联方案：[0053 统一本地来源与收藏夹模型重构](./0053-unified-library-and-favorite-folders.md)

## 实施记录

2026-08-19 已完成 Phase 1 的第一轮代码：

- 新增 0016 SQLite migration，重建 `collections` 为 v2 并保留旧成员关系；
- 创建唯一默认收藏系统叶节点，迁移旧 `is_favorite`，并让爱心与默认收藏在事务中同步；
- 将旧 `asset_library_assignments` 转换为普通 Collection，保留旧表作短期兼容；
- 按真实 Source 路径恢复物理父子关系，Source 查询不再使用 assignment 覆盖 `assets.library_id`；
- 为 Collection summary 暴露系统类型、父节点、系统键和排序字段；
- 新增 v15 数据库迁移夹具，覆盖同名冲突、旧 assignment、手工 Source 关系、默认收藏和重复初始化；
- Rust library tests：94 passed。
- 新增 AssetQuery v2 和 `query_assets` IPC，统一 All、Source、Favorites、Collection 的分页/计数查询；旧 `list_assets` 保留适配入口；
- 默认收藏列表和 Collection 详情改为复用 Repository 的统一查询执行器；
- TypeScript V1 查询状态在 API 边界转换为 v2 root，新增 Source/Favorites/Collection 转换测试。
- 整理预览在旧单 Source 入口增加跨 Source Collection/默认收藏拦截，避免静默丢弃其它来源的成员。
- 新增 `BrowseNode` 统一返回 Source 与 Collection 树，默认收藏始终作为首个系统叶节点返回。
- 新增 `list_browse_nodes` IPC；收藏夹创建支持选择普通收藏夹父节点，并拒绝挂到默认收藏下。
- 左侧“图库”模块改为统一树：默认收藏、普通收藏夹和本地来源使用不同图标；“来源”筛选区移除。
- “导入图库”改为“＋ 添加”，区分导入本地来源与新建收藏夹；新建收藏夹可选择父节点。
- 收藏夹/默认收藏在没有当前 Source 时也可以进入统一 AssetQuery 查询，避免跨来源收藏被 UI 门槛拦截。

尚未实现：收藏夹重命名/移动/删除树操作、加入/移动/移出收藏夹的统一批量 UI，以及 0054 整理导出改造。

## 1. 规划结论

0053 不能从侧栏 UI 先改起。当前后端仍把以下几种职责混在一起：

- `libraries` 代表本地来源，但 `parent_library_id` 仍允许手工关系；
- `assets.library_id` 是真实来源，但部分查询通过 `asset_library_assignments` 覆盖它；
- `assets.is_favorite` 与 `collections / collection_assets` 是两套收藏状态；
- `AssetFilter.favoriteOnly / collectionId` 与 `AssetQueryV1.libraryId` 共同表达浏览范围；
- 收藏夹接口和主图库接口返回不同 DTO、使用不同 SQL；
- 整理模块仍假设一个查询只属于一个 `library_id`。

因此采用以下顺序：

```text
冻结数据契约
  ↓
迁移 Source / Collection 数据
  ↓
统一 AssetQuery 查询边界
  ↓
接入主界面图库树
  ↓
实现收藏、移出、移动操作
  ↓
清理旧兼容路径并回归
```

在 Phase 1～2 未通过前，不重排主界面 UI，不删除旧字段，不修改整理导出模型。

## 2. 当前实现基线

### 2.1 数据库

- `0011_photo_workflow_mvp.sql` 创建了全局唯一名称的 `collections`，没有父节点、系统类型和排序字段。
- 同一迁移创建 `assets.is_favorite`，当前爱心不写入 `collection_assets`。
- `0008_asset_library_assignments.sql` 创建一对一的旧虚拟图库归属。
- `0005_library_source_hierarchy.sql` 与 `0007_manual_library_hierarchy.sql` 允许 Source 层级存在来源关系和手工关系。
- 当前最高迁移版本为 15，0053 需要新增后续迁移；不能改写已发布迁移文件。

### 2.2 Rust 后端

- `src-tauri/src/db.rs` 的 `list_assets` 以 `library_id` 为主入口。
- `LIBRARY_SCOPE_FILTER` 仍用 `asset_library_assignments` 优先覆盖 `assets.library_id`。
- 文件夹统计、语义分组和部分重复查询也沿用旧 Source 覆盖逻辑。
- `src-tauri/src/workflow.rs` 独立实现 favorite 和 collection 查询。
- `set_favorite` 目前只更新 `assets.is_favorite`，未与默认收藏夹建立事务同步。
- 现有 `collections` 接口只支持扁平列表、创建、删除、单集合加入和移除。

### 2.3 前端

- `src/types.ts` 的 `AssetQueryV1` 只有 `libraryId`，`AssetFilter` 同时含有 `favoriteOnly` 和 `collectionId`。
- `src/App.tsx` 通过这两个 filter 字段切换收藏和集合来源。
- `src/components/Sidebar.tsx` 已有图库树，但收藏夹来自独立的集合区块，不是统一树。
- `src/components/WorkflowWorkspace.tsx` 仍把集合当作独立工作台功能。
- `src/api.ts` 目前使用多个以 `libraryId` 为参数的 favorite / collection API。

## 3. 设计冻结项

以下内容在实现前必须保持不变：

1. 不新建平行的 `sources` 表。现有 `libraries` 表继续作为物理 Source 的存储，产品和查询层使用 Source 术语，内部旧字段暂保留 `library_id`。
2. `assets.library_id` 永远是真实来源，收藏和拖拽不能修改它。
3. Collection 只通过 `collection_assets` 形成虚拟归档，不创建磁盘目录，不写入源文件。
4. 默认收藏是唯一的 `system_favorites` 叶节点，心形状态以它的成员关系为真源，`assets.is_favorite` 只作为过渡镜像。
5. 0053 不修改 `organization_plans`、目标目录树、冲突检查、复制执行器和导出目录规则。
6. 跨 Source 的收藏夹允许浏览、筛选、搜索和收藏；当前整理功能必须阻止跨 Source 范围，不偷偷选择第一个 Source。
7. 所有批量加入收藏必须由后端根据 `AssetQuery` 执行，前端不能为了操作而加载全部 asset ID，也不能触发原图解码或重新分析。

## 4. 分阶段执行计划

### Phase 0：契约冻结与迁移夹具

目标：在写生产代码前，把旧数据到新模型的边界变成可测试的固定输入。

工作项：

- 建立 `BrowseRoot`、`AssetQuery`、`CollectionKind`、`CollectionSummary` 的 Rust/TypeScript 对照表。
- 确定新查询 envelope 的版本号，并写 V1 到新 envelope 的兼容转换规则。
- 建立 SQLite 临时数据库 fixture，至少覆盖：
  - 多个真实 Source；
  - Source 后代和旧 manual Source relationship；
  - 旧 `is_favorite`；
  - 多个旧 collection；
  - assignment 指向其他 Source、同 Source 和不存在目标的异常数据；
  - 同名 Source 与 Collection；
  - 缺失源文件和 Unicode 路径。
- 为迁移前后建立关系计数清单：Asset 数、真实来源、旧收藏、旧 assignment、收藏成员和缺失成员。
- 起草 ADR，记录“保留 `libraries` 作为 Source 存储”和“0053 不动整理导出”的理由。

退出条件：产品概念、迁移异常处理、查询 root 类型和旧字段兼容方式都能由 fixture 表达；不改业务代码。

### Phase 1：数据库迁移与真实 Source 语义

目标：让数据库先满足 Source / Collection 的基本不变量。

建议新增：`0016_unified_source_collection.sql`，并配合幂等的 Rust 数据迁移辅助函数。不能修改 0001～0015。

工作项：

1. 重建 `collections` 为 v2，增加：
   - `parent_collection_id`；
   - `collection_kind`，允许 `manual` 和 `system_favorites`；
   - `system_key`；
   - `display_order`；
   - 保留现有名称、说明和时间字段。
2. 使用部分唯一索引实现真正的同级名称唯一：
   - 普通父节点下按 `parent_collection_id + name COLLATE NOCASE` 唯一；
   - 根级 Collection 单独按 `name COLLATE NOCASE` 唯一；
   - `default_favorites` 的 `system_key` 全局最多一个。
3. 创建唯一的默认收藏系统叶节点，迁移所有 `assets.is_favorite = 1` 的成员关系。
4. 将旧 `collections` 原样复制为根级普通收藏夹，并保留旧 collection ID 的成员关系。
5. 将旧 `asset_library_assignments` 转换为普通收藏夹和 `collection_assets`：
   - `assets.library_id` 原值不动；
   - assignment 的目标图库生成或复用迁移收藏夹；
   - `assigned_at` 保留为 `added_at`；
   - 迁移后查询不再读取 assignment 覆盖 Source；
   - 旧表首阶段保留为只读兼容数据，待 Phase 5 清理。
6. 校正 Source 树：
   - 能由真实路径推导的父子关系改为 `source`；
   - 无法从真实路径恢复的旧 manual 关系提升为根级；
   - 不再允许新的手工 Source 父子关系。
7. 对迁移过程增加事务、失败回滚、重复执行和迁移后计数校验。

待确认的迁移细节：当多个 assignment 目标或旧 collection 产生相同父级名称时，必须采用确定性的名称冲突策略，并在迁移报告中记录；不得静默覆盖。

测试重点：

- `library_id` 在迁移前后逐 Asset 一致；
- 收藏关系、心形关系和 assignment 信息没有静默丢失；
- 默认收藏不会重复创建；
- 父收藏夹循环和默认收藏作为父节点都会被拒绝；
- 迁移失败时 schema 和数据整体回滚；
- 离线源文件只变为 missing，不删除收藏关系。

退出条件：旧数据库可以安全升级到 v2；所有 Source 查询都能证明不再受 assignment 覆盖影响。

### Phase 2：统一 AssetQuery 与后端查询

目标：用一个查询边界承载 Source、Collection、All 和现有筛选，而不是继续扩展 `libraryId + favoriteOnly + collectionId`。

建议模型：

```text
BrowseRoot
  source { libraryId }
  collection { collectionId }
  all

AssetQuery
  root
  includeDescendants
  filter
  sort
  page
  pageSize
```

工作项：

- 在 Rust 和 TypeScript 中建立新查询类型；旧 `AssetQueryV1` 先由适配器转换，不立即删除。
- 将 Source 查询改为只看 `assets.library_id` 和已规范化的物理子 Source。
- 将 Collection 查询实现为递归 Collection CTE，父节点聚合后按 `asset_id` 去重。
- 将默认收藏转换为普通 Collection root，爱心查询不再有第二套 SQL 语义。
- 统一分页、计数、排序、AssetFilter 和缺失文件处理。
- 保证跨 Source Collection 返回每张 Asset 自己的 `library_id`，不生成伪造的单一 Source。
- 把文件夹统计、语义分组、AI 搜索范围逐步接到查询解析器；涉及单 Source 的旧接口先通过兼容适配器工作。
- 为批量加入收藏提供基于 `AssetQuery` 的后端事务入口。
- 为 Organization 保留边界检查：解析结果包含多个 Source 时返回明确的“不支持跨来源整理”错误。

测试重点：

- Source、Source 后代、Collection、Collection 后代、All 的总数和分页一致；
- 多个收藏夹聚合后 Asset 不重复；
- 同一筛选条件在 Source 与 Collection root 上结果一致；
- assignment 数据不会改变 Source 结果；
- 旧 V1 查询在适配器下保持现有行为；
- 跨 Source 收藏夹不会进入旧单 Source OrganizationPlan。

退出条件：主图库可以只依赖 AssetQuery 查询，且不需要在 UI 中拼装多套来源状态。

### Phase 3：统一左侧图库导航

目标：把本地来源和收藏夹放在同一个“图库”模块中，但明确它们是两种不同节点。

工作项：

- 统一返回 `BrowseNode` 树，节点类型明确为 Source 或 Collection。
- Source 使用真实目录图标；Collection 使用文件夹加心形图标；默认收藏置顶。
- 统一树行的展开、选中、计数和缺失状态样式，不让 Source 和 Collection 共享错误的拖拽语义。
- 将“来源”筛选入口移除，浏览 root 由树节点直接决定。
- 将原“导入图库”改为“＋ 添加”，菜单区分“导入本地来源”和“新建收藏夹”。
- 导入本地来源仍调用现有扫描配置，并增加是否包含子文件夹图片的明确选项。
- 新建收藏夹支持选择普通收藏夹父节点，可选择把当前 AssetQuery 范围批量加入；范围由后端解析。
- 默认收藏不能重命名、移动、删除或创建子节点。
- Source 节点不允许通过 UI 改父级；收藏夹节点才允许层级操作。

测试重点：

- 首次启动没有本地来源时，入口简洁且可直接导入；
- 默认收藏始终置顶、重启后仍存在；
- Source 与 Collection 同名仍能清楚区分；
- 窄窗口、深色/白色主题和大数量树节点不出现错位或重复外层卡片；
- 展开/折叠只改变对应节点的子树。

退出条件：用户可以从同一棵树进入本地来源或收藏夹，且 UI 不再使用“图库”同时指代两种数据库关系。

### Phase 4：收藏交互和成员操作

目标：让图片管理语义稳定且不会误改磁盘结构。

工作项：

- 爱心：在同一事务中维护默认收藏关系和 `assets.is_favorite` 镜像。
- “加入收藏”：弹出普通收藏夹多选器，可同时加入多个收藏夹；选择默认收藏等价于点亮爱心。
- “从当前收藏夹移除”：只删除当前节点的 direct membership；父节点聚合展示不能含糊移除。
- “移动到其他收藏夹”：只能在普通收藏夹 direct member 上执行，事务内完成目标加入和当前关系删除。
- Source、全局搜索、动态筛选和父收藏夹聚合结果只提供“加入收藏”，不提供歧义的“移动”。
- 允许收藏夹创建、重命名、同级排序、移动、删除；删除有子节点时提供“删除整棵树”或“提升子节点”。
- 图片拖到普通收藏夹执行加入，拖到默认收藏执行爱心；拖到 Source 禁止。
- 所有重复加入幂等；所有成员操作只写数据库关系。
- 主图库、单图预览、多图预览、AI 搜索结果和整理入口使用同一成员操作 API。

测试重点：

- 一个 Asset 可以同时出现在多个普通收藏夹和默认收藏夹；
- 加入普通收藏夹不会误点亮爱心；
- 取消爱心不会移除其他普通收藏夹关系；
- 移动失败时目标和源关系都保持原状；
- missing Asset 的收藏关系仍可显示和清理；
- 快速重复点击不会产生重复关系。

退出条件：用户可以完成“扫描本地来源 → 筛选 → 加入一个或多个收藏夹”的闭环，且原始文件路径完全不变。

### Phase 5：旧逻辑清理与整理兼容

目标：移除旧概念对新产品路径的影响，但保留安全的读取兼容。

工作项：

- 新代码不再把 `asset_library_assignments` 当作 Source 或收藏目标。
- 新 UI 不再直接维护 `favoriteOnly` 和 `collectionId` 两个并行来源状态。
- `set_favorite`、集合查询、工作台集合查询逐步改为 AssetQuery / Collection service。
- 保留 V1 IPC 适配器和旧持久化读取，直到重启、迁移和工作流回归完成。
- 统一计数、列表、搜索和选择状态的刷新机制。
- 为 Organization 增加跨 Source guard，但不改 `organization_plans` 结构，不引入 0054 的 snapshot 或导出执行器。
- 通过全局搜索确认旧 assignment 不再进入 Source SQL；完成后再决定是否删除旧表和旧字段。

退出条件：没有新业务路径依赖旧 assignment；旧接口只剩明确标注的兼容层；整理模块不会被 0053 的跨来源收藏夹误触发。

### Phase 6：回归和人工验收

目标：覆盖产品边界、迁移安全和桌面交互。

自动化：

- Rust 数据库迁移、事务回滚、循环校验、计数、missing、重启恢复测试；
- Rust 查询测试覆盖 Source / Collection / All / descendants / 多 Source 去重；
- TypeScript API 和查询适配器测试；
- Sidebar、收藏多选器、成员操作和错误提示组件测试；
- `cargo test`、前端测试、lint、typecheck、format 和 build。

人工验收：

1. 导入两个本地文件夹，确认 Source 树和真实路径一致。
2. 通过筛选结果创建收藏夹，确认加入的是整个 AssetQuery 范围而非当前已加载缩略图。
3. 同一张图片加入三个收藏夹，确认 Source 不变、三处均可浏览。
4. 点亮/取消爱心，确认默认收藏和爱心始终一致。
5. 断开一个来源目录，确认收藏关系和 missing 数量保留；恢复后重新可见。
6. 建立跨 Source 收藏夹，确认可以浏览、筛选、搜索，但整理入口明确阻止。
7. 重启应用，确认默认收藏、树结构、成员关系和选中状态恢复。
8. 使用中文、空格、Unicode 路径，确认迁移和浏览不丢失。

## 5. 文件变更边界

初期真正开始实现时，预计涉及：

- `src-tauri/migrations/0016_*.sql`；
- `src-tauri/src/db.rs`；
- `src-tauri/src/models.rs`；
- `src-tauri/src/workflow.rs`；
- `src-tauri/src/ipc.rs`；
- `src-tauri/src/lib.rs`；
- `src/types.ts`、`src/query.ts`、`src/api.ts`；
- `src/App.tsx`、`src/components/Sidebar.tsx`、收藏和工具栏相关组件；
- 对应 Rust、TypeScript 和 UI 测试；
- `docs/decisions/` 下的 ADR 和本阶段验收记录。

本规划不授权修改：

- 用户个人照片目录；
- 原图文件和 EXIF；
- `organization_plans` 的数据结构；
- 0054 的导出目录、冲突处理和真实复制执行器；
- 模型、缩略图和分析流程。

## 6. 风险与停工条件

### 高风险

- SQLite 重建 `collections` 时破坏旧成员关系；
- assignment 目标名称冲突导致迁移后收藏夹语义不确定；
- 旧查询仍偷偷使用 assignment，造成 Source 计数和列表不一致；
- 跨 Source Collection 进入只支持单 Source 的整理流程；
- 默认收藏与 `is_favorite` 双写不在同一事务中。

### 必须停工并重新确认的情况

- 需要修改 `assets.library_id` 才能实现收藏或移动；
- 需要扫描、复制、移动或重命名原始文件才能实现收藏夹；
- 需要把 Collection 变成真实文件夹；
- 需要提前改写 OrganizationPlan 或导出执行器；
- 迁移无法在测试数据库中证明可回滚；
- 无法确定旧 assignment 应映射到哪个收藏夹。

## 7. 第一轮实施前的检查清单

只有以下项目全部完成，才进入 Phase 1 编码：

- [ ] 0053 产品方案确认；
- [ ] assignment 名称冲突策略确认；
- [ ] `BrowseRoot / AssetQuery` 的 JSON 契约确认；
- [ ] Source 物理父子关系恢复规则确认；
- [ ] 默认收藏的系统标识和迁移策略确认；
- [ ] v15 数据库迁移 fixture 准备完成；
- [ ] 0053 / 0054 的边界已写入 ADR；
- [ ] 明确本轮不改整理导出，不触碰个人照片目录。

第一轮编码只从 Phase 1 开始：先做迁移和 Repository 测试，通过后再进入查询和 UI；不采用“先做 UI、后补数据模型”的顺序。
