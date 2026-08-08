# Checkpoint A — Source Boundary + Nested Library

状态：NOT_STARTED

本阶段建立唯一 Asset identity、SourceRoot 安全边界、source-derived Library hierarchy、Most Specific ownership、递归 Browse Scope 和 scoped scan。完成后必须提交并停止，不能自动进入 Checkpoint B。

## 1. Goal

交付以下能力：

- SourcePath 与 AppDataRoot 的 Windows-aware 安全边界。
- 只有显式导入目录才建立 Library。
- 显式嵌套 SourceRoot 自动建立最近祖先 hierarchy。
- Parent-first 和 Child-first 得到一致结果。
- 一个物理源文件只有一个逻辑 Asset。
- Asset owner 始终是 Most Specific Imported Library。
- 删除 Child、Parent、Middle Library 时正确重新归属。
- Parent Browse 使用 current + all descendants。
- Parent Rescan 遇到 descendant SourceRoot 时 prune。
- Sidebar 只显示 source-derived Library tree，不显示磁盘目录树。

## 2. Non-goals

- 不实现 Manual Classification。
- 不实现 Effective Classification Resolver。
- 不移除或重构 Preview 的 previewAssetId；该内容属于 Checkpoint C。
- 不升级 Semantic 模型或 Dominant Color 算法。
- 不实现 Export Preview 或 COPY。
- 不实现 arbitrary Library Group、Collection 或拖拽分组。
- 不改变源文件、源目录或用户真实照片。
- 不创建本 Runbook 之外的 Migration。

## 3. Preconditions

- 当前仓库和 SQLite schema 0001-0004 可正常启动。
- 现有 scanner、thumbnail、EXIF 和基础 imaging 测试通过。
- 仅使用 test-data/ fixture 进行文件系统验证。
- 产品规则中 Parent Browse = current + all descendants 已冻结。
- 产品规则中 OriginalDirectory 仍只属于 Export Context，不在本阶段处理。

## 4. Architecture Invariants

- sourcePath 是用户可读路径；sourceIdentityKey 是 Windows-aware canonical identity。
- sourceIdentityKey 负责唯一性、包含关系和 SourceRoot boundary。
- SourceRoot 与 AppDataRoot 不得重叠。
- SourceRoot 之间可以嵌套；未显式导入的磁盘子目录不是 Library。
- parentLibraryId 是系统推导值，不能由 UI 任意设置。
- Asset identity 不能由 libraryId + 原始字符串路径定义。
- Asset owner 变化不得改变 Asset ID、fingerprint、thumbnail、preview、metadata 或分类数据。
- Browse Scope 不会复制 Asset，也不会改变 Asset owner。
- Parent Rescan 不得遍历或隐式重扫显式 descendant SourceRoot。
- Parent Browse、list、count、group、filter、pagination 必须使用相同 recursive Library Scope。
- 删除 Library 只改变 PhotoOrganizer 索引，不修改 SourceRoot。

## 5. Current Implementation

当前实现仍是 flat Library，关键位置如下：

- [src/types.ts](E:/Code/Codex/photo-organizer/src/types.ts)
  - LibrarySummary 只有 id、rootPath、scan 状态和统计字段。
  - AssetFilter 仍有 folderPrefix 和 semanticState。
- [src/App.tsx](E:/Code/Codex/photo-organizer/src/App.tsx)
  - currentLibraryId 驱动 fetchAssets。
  - importFolder 调用 chooseLibraryFolder 和 startLibraryScan。
  - rescanLibrary 直接把 library.rootPath 传给 startLibraryScan。
- [src/api.ts](E:/Code/Codex/photo-organizer/src/api.ts)
  - fetchLibraries、fetchAssets、fetchLibraryFolders、startLibraryScan 仍使用旧 flat API。
- [src/components/Sidebar.tsx](E:/Code/Codex/photo-organizer/src/components/Sidebar.tsx)
  - libraries 作为 flat list 展示。
  - folders、FolderTreeNode 和 buildFolderTree 构建磁盘目录树。
- [src-tauri/src/models.rs](E:/Code/Codex/photo-organizer/src-tauri/src/models.rs)
  - LibrarySummary 只有 root_path。
  - AssetFilter 仍有 folder_prefix。
- [src-tauri/src/scanner.rs](E:/Code/Codex/photo-organizer/src-tauri/src/scanner.rs)
  - validate_scan_root 只做当前 canonicalize 检查。
  - scan_library 使用 WalkDir::follow_links(false)，但没有 descendant SourceRoot prune。
  - upsert_processed_asset 按当前扫描入口写入 owner。
- [src-tauri/src/db.rs](E:/Code/Codex/photo-organizer/src-tauri/src/db.rs)
  - begin_scan、complete_scan、upsert_processed_asset、list_libraries、remove_library 和 list_assets 是主要入口。
  - list_assets 当前按 a.library_id = ? 查询。
  - complete_scan 按当前 library generation 处理 missing。
  - list_library_folders 从 relative_path 重新构建目录树。
- [src-tauri/src/ipc.rs](E:/Code/Codex/photo-organizer/src-tauri/src/ipc.rs)
  - start_scan 负责验证并启动 scanner。
  - open_library_in_explorer 目前接受原始 root_path 字符串。
- [src-tauri/migrations/0001_initial.sql](E:/Code/Codex/photo-organizer/src-tauri/migrations/0001_initial.sql)
  - libraries.root_path UNIQUE。
  - assets 使用 UNIQUE(library_id, absolute_path)。
- [src-tauri/src/paths.rs](E:/Code/Codex/photo-organizer/src-tauri/src/paths.rs)
  - 创建 AppData database、thumbnail、preview、log 路径。

## 6. Target State

### Domain Model

Library 包含：

    id
    name
    sourcePath
    sourceIdentityKey
    parentLibraryId
    displayOrder
    scan/status metadata

Asset 包含：

    id
    ownerLibraryId
    absolutePath
    assetIdentityKey
    relativePath
    fingerprint
    objective metadata

### DB Model

- libraries.source_identity_key UNIQUE。
- libraries.parent_library_id 指向系统推导的最近祖先。
- assets.asset_identity_key UNIQUE。
- assets.library_id 是当前最具体 owner。
- parent_library_id 不提供任意用户编辑 API。

### React State

- libraries 从后端获得 hierarchy metadata 和 recursive display count。
- currentLibraryId 仍可作为当前节点 ID，但请求语义是 current + descendants。
- folders 和 folderPrefix 不再用于 Sidebar navigation；完全删除属于 B 的 Filter cleanup，可在 A 中移除旧目录 API 依赖。

### IPC

目标 API 语义：

- importLibrary 或等价的显式导入入口。
- listLibraries 返回 source-derived tree metadata。
- rescanLibrary 接收 libraryId，不接受前端任意路径。
- removeLibrary 接收 libraryId 并先做 ownership reconciliation。
- openLibraryInExplorer 接收 libraryId。
- listAssets、count、group 使用统一 Library Scope。

### Rust Data Flow

    user import
      ↓
    source identity validation
      ↓
    Library upsert
      ↓
    parent resolver
      ↓
    ownership reconciliation
      ↓
    scoped scanner with descendant prune
      ↓
    owner-based missing reconciliation

### UI Behavior

- Sidebar 每个节点都是用户显式导入过的 SourceRoot。
- Sidebar count 是当前节点及所有 descendants 的总 Asset 数。
- 不显示 Original Folder、All Folders 或磁盘目录树。
- 点击 Parent 显示 Parent 和全部 descendant Assets。

## 7. Detailed Implementation Steps

### A1 — Windows-aware Source Identity / Path Safety

Goal：建立 sourcePath、sourceIdentityKey、assetIdentityKey 和三类路径边界的统一实现。

- Files to change：新增路径身份 domain module；修改 [src-tauri/src/scanner.rs](E:/Code/Codex/photo-organizer/src-tauri/src/scanner.rs)、[src-tauri/src/paths.rs](E:/Code/Codex/photo-organizer/src-tauri/src/paths.rs)、[src-tauri/src/ipc.rs](E:/Code/Codex/photo-organizer/src-tauri/src/ipc.rs)。
- DB/schema impact：无立即 schema 写入；为 A2/A3 预备 identity 生成规则。
- API impact：扫描和导入返回明确的 source boundary 错误；Explorer API 的输入契约改为 library ID 的设计。
- React state impact：仅处理导入错误和 unavailable 状态，不引入新业务状态。
- Rust/domain impact：实现 Windows 大小写、分隔符、dot segment、UNC、long-path、Unicode 和 reparse point 处理；SourceRoot/AppDataRoot overlap 双向拒绝。
- Tests to add/update：path unit、Unicode path、UNC、long path、case equivalence、source/appdata/export boundary、symlink/junction。
- Completion condition：同一 Windows identity 只能得到一个稳定 key；不安全路径 fail closed。
- Dependency：依赖当前 paths 和 scanner 读取逻辑；完成后才能开始 A2。

### A2 — Library Source Schema Migration

Goal：把 flat root_path 模型升级为 sourcePath、sourceIdentityKey、name、system-derived parentLibraryId。

- Files to change：[src/types.ts](E:/Code/Codex/photo-organizer/src/types.ts)、[src-tauri/src/models.rs](E:/Code/Codex/photo-organizer/src-tauri/src/models.rs)、[src-tauri/src/db.rs](E:/Code/Codex/photo-organizer/src-tauri/src/db.rs)、[src-tauri/src/ipc.rs](E:/Code/Codex/photo-organizer/src-tauri/src/ipc.rs)；未来新增 0005 migration。
- DB/schema impact：新增 libraries.name、source_path、source_identity_key、parent_library_id、display_order；保留 root_path 兼容读取直到迁移完成。
- API impact：LibrarySummary 返回 name、sourcePath、parentLibraryId 和 recursive count 所需字段。
- React state impact：LibrarySummary 类型和 selected library label 不再从 rootPath basename 临时推导。
- Rust/domain impact：实现同 identity 去重、父关系重算和 parent 防御性校验。
- Tests to add/update：migration backfill、same identity import、parent hierarchy、self-parent/cycle rejection。
- Completion condition：旧数据库能安全打开并得到 source-derived hierarchy；同一源根重复导入不增加 Library。
- Dependency：A1 必须完成；不得在 A2 同时实现 Asset ownership。

### A3 — Global Asset Identity Migration

Goal：把 Asset 唯一身份从 libraryId + absolutePath 改为全局稳定的 assetIdentityKey。

- Files to change：[src-tauri/src/db.rs](E:/Code/Codex/photo-organizer/src-tauri/src/db.rs)、[src-tauri/src/models.rs](E:/Code/Codex/photo-organizer/src-tauri/src/models.rs)；未来新增 0006 migration。
- DB/schema impact：新增 assets.asset_identity_key UNIQUE；处理旧表 unique constraint；保留 absolute_path 和 relative_path。
- API impact：Asset DTO 增加 owner、identity 和 source-relative 信息；前端只消费唯一 Asset。
- React state impact：无新用户交互；刷新时不因 owner 变化丢失 active Asset。
- Rust/domain impact：对现有 canonical duplicate 选择稳定 survivor，迁移关联 tone、color、semantic、thumbnail 和 job 引用。
- Tests to add/update：duplicate migration、foreign key reassignment、Asset ID preservation、fingerprint preservation。
- Completion condition：同一物理 source identity 在数据库中最多一行。
- Dependency：A2 的 source identity semantics 必须稳定；A3 完成前不得做 ownership reconciliation。

### A4 — System-derived Library Parent Resolver

Goal：根据已导入 SourceRoot 的最近祖先建立 parentLibraryId。

- Files to change：[src-tauri/src/db.rs](E:/Code/Codex/photo-organizer/src-tauri/src/db.rs)、[src-tauri/src/models.rs](E:/Code/Codex/photo-organizer/src-tauri/src/models.rs)；可能新增 resolver module。
- DB/schema impact：写入 parent_library_id；创建 parent index；不增加用户可编辑关系表。
- API impact：listLibraries 返回 hierarchy；不提供 setLibraryParent 或 clearLibraryParent。
- React state impact：Sidebar 接收 parentLibraryId 和 flat rows，或后端直接返回 tree。
- Rust/domain impact：使用 sourceIdentityKey 的组件边界比较；严格排除自身；事务内重算全体关系。
- Tests to add/update：A/B/C/D 无限深度、同级 root、非嵌套 root、删除中间节点的关系预测。
- Completion condition：任何关系都能由 SourceRoot 自动重建；无 cycle。
- Dependency：A2 和 A1 完成。

### A5 — Most Specific Asset Ownership Resolver

Goal：实现文件到最具体 imported Library 的唯一归属算法。

- Files to change：[src-tauri/src/scanner.rs](E:/Code/Codex/photo-organizer/src-tauri/src/scanner.rs)、[src-tauri/src/db.rs](E:/Code/Codex/photo-organizer/src-tauri/src/db.rs)、[src-tauri/src/models.rs](E:/Code/Codex/photo-organizer/src-tauri/src/models.rs)。
- DB/schema impact：upsert 改用 assetIdentityKey；owner library_id 可变但 Asset ID 不变。
- API impact：scan result 和 Asset DTO 暴露实际 owner，不暴露扫描入口作为 owner 的假设。
- React state impact：Library 浏览通过 scope 获取 Asset；不根据 frontend 当前 Library 改写 owner。
- Rust/domain impact：resolver 在每次 upsert 和 reconciliation 时运行；relative_path 按当前 owner SourceRoot 重算。
- Tests to add/update：Parent-first、Child-first、三层嵌套、文件边界、相邻路径名称相似但不包含的路径。
- Completion condition：相同输入路径、不同扫描入口得到同一 owner 和同一 Asset ID。
- Dependency：A3 和 A4 完成。

### A6 — Parent-first / Child-first Ownership Reconciliation

Goal：导入新 Child 或 Parent 时重新归属已存在 Asset，不重复创建。

- Files to change：[src-tauri/src/db.rs](E:/Code/Codex/photo-organizer/src-tauri/src/db.rs)、[src-tauri/src/scanner.rs](E:/Code/Codex/photo-organizer/src-tauri/src/scanner.rs)、[src-tauri/src/ipc.rs](E:/Code/Codex/photo-organizer/src-tauri/src/ipc.rs)。
- DB/schema impact：事务内更新 owner、relative_path 和 owner-dependent metadata；不删除源文件。
- API impact：显式 import 完成后返回 reconciliation summary；重复导入返回已有 Library。
- React state impact：刷新 libraries、counts、assets；保留仍存在 Asset 的 active/selection ID。
- Rust/domain impact：先插入/确认 Library，再重算 hierarchy，再做受影响 SourceRoot 范围 reconciliation，最后扫描新增文件。
- Tests to add/update：import order convergence、Asset ID preservation、classification/thumbnail preservation、parent recursive count stability。
- Completion condition：Parent-first 和 Child-first 数据库快照等价。
- Dependency：A5 完成。

### A7 — Remove Child / Parent / Middle Library Reconciliation

Goal：删除 Library metadata 时保持源文件不变，并正确 fallback/reparent。

- Files to change：[src/App.tsx](E:/Code/Codex/photo-organizer/src/App.tsx)、[src/api.ts](E:/Code/Codex/photo-organizer/src/api.ts)、[src-tauri/src/ipc.rs](E:/Code/Codex/photo-organizer/src-tauri/src/ipc.rs)、[src-tauri/src/db.rs](E:/Code/Codex/photo-organizer/src-tauri/src/db.rs)。
- DB/schema impact：删除前必须 reconciliation；不能直接依赖当前 remove_library 的 cascade。
- API impact：removeLibrary 返回受影响 Asset、fallback、removed index counts；仍只接收 libraryId。
- React state impact：删除当前节点后重新加载 hierarchy、recursive counts、Asset page 和 active state。
- Rust/domain impact：Child fallback 到最近剩余 ancestor；Parent-only 无剩余 owner 的 Asset 才从索引移除；Middle 删除后子节点连接到最近祖先。
- Tests to add/update：remove child/parent/middle、Asset ID preservation、recursive count stability、source hash unchanged。
- Completion condition：Library metadata 删除不造成没有真实源文件变化的父级 browse 数量异常减少。
- Dependency：A6 完成。

### A8 — Scan Scope / Descendant Library Pruning

Goal：Parent Scan 只扫描自己的 ownership scope，遇到显式 descendant SourceRoot 时 prune。

- Files to change：[src-tauri/src/scanner.rs](E:/Code/Codex/photo-organizer/src-tauri/src/scanner.rs)、[src-tauri/src/ipc.rs](E:/Code/Codex/photo-organizer/src-tauri/src/ipc.rs)。
- DB/schema impact：scan session 需要记录 scan owner 和被 prune 的 descendant roots；不改变 child ownership。
- API impact：rescanLibrary 使用 libraryId；不提供 path-based rescan 给前端。
- React state impact：Parent Rescan 的 progress 不显示为 Child Rescan；UI 不误报 child 已重新扫描。
- Rust/domain impact：WalkDir 在进入显式 descendant SourceRoot 前 prune；Most Specific resolver 仍保留为 upsert 兜底。
- Tests to add/update：prune traversal、parent scan 不读取 child file、parent scan 不触发 child generation、nested descendants。
- Completion condition：Parent Rescan 不隐式维护或重扫任何 descendant Library。
- Dependency：A4-A7 完成。

### A9 — Scoped Missing Detection + Scan Concurrency

Goal：missing 只作用于实际扫描的 owner scope，并串行化重叠 SourceRoot 操作。

- Files to change：[src-tauri/src/scanner.rs](E:/Code/Codex/photo-organizer/src-tauri/src/scanner.rs)、[src-tauri/src/db.rs](E:/Code/Codex/photo-organizer/src-tauri/src/db.rs)、可能新增 AppState source lock。
- DB/schema impact：scan generation 或 observed owner set；complete_scan 不得按 parent root 粗暴标记 child-owned Asset missing。
- API impact：并发扫描返回可解释的 busy/cancelled 状态。
- React state impact：scan progress 和 refresh 不覆盖另一个重叠任务的结果。
- Rust/domain impact：对重叠 SourceRoot 使用 per-overlap lock；批次 upsert 和最终 missing reconciliation 使用 transaction。
- Tests to add/update：parent/child concurrent scan、overlapping import、cancel、child missing isolation、non-overlap parallelism。
- Completion condition：父扫描不会标记子 Asset missing；竞争任务不会回写过期 owner。
- Dependency：A8 完成。

### A10 — Recursive Parent Browse Scope + Recursive Counts

Goal：实现 current + all descendants 的统一 browse、count、group、filter 和 pagination scope。

- Files to change：[src-tauri/src/db.rs](E:/Code/Codex/photo-organizer/src-tauri/src/db.rs)、[src-tauri/src/ipc.rs](E:/Code/Codex/photo-organizer/src-tauri/src/ipc.rs)、[src-tauri/src/models.rs](E:/Code/Codex/photo-organizer/src-tauri/src/models.rs)、[src/api.ts](E:/Code/Codex/photo-organizer/src/api.ts)。
- DB/schema impact：recursive CTE 和 parent index；不复制 Asset 行。
- API impact：listAssets、count、semantic groups、organization source list 使用统一 scope；LibrarySummary 主 count 为 recursive count。
- React state impact：fetchAssets 仍传 active Library ID，但后端解释为 recursive scope；Sidebar 数量与 AssetPage.total 同源。
- Rust/domain impact：优先使用 SQLite Recursive CTE，所有 list/count/group/filter 复用同一 scope builder。
- Tests to add/update：A/B/C/D scope、list/count equality、search/filter/sort/group/pagination equality、100 层嵌套边界。
- Completion condition：Parent 点击结果、Sidebar count、AssetPage.total 完全一致。
- Dependency：A4、A7、A9 完成。

### A11 — Sidebar Library Tree

Goal：用 source-derived Library hierarchy 替代当前 flat list 和磁盘 Folder Tree。

- Files to change：[src/components/Sidebar.tsx](E:/Code/Codex/photo-organizer/src/components/Sidebar.tsx)、[src/App.tsx](E:/Code/Codex/photo-organizer/src/App.tsx)、[src/types.ts](E:/Code/Codex/photo-organizer/src/types.ts)、[src/api.ts](E:/Code/Codex/photo-organizer/src/api.ts)。
- DB/schema impact：只消费 A10 的 recursive count；删除 list_library_folders 作为导航依赖。
- API impact：Sidebar Library node 包含 name、parent、recursive count、status 和 source info。
- React state impact：删除 folders、FolderTreeNode、buildFolderTree 和 folderPrefix 导航状态；Library tree 展开状态独立于文件路径。
- Rust/domain impact：listLibraries 只返回已显式导入 Library；不返回磁盘子目录节点。
- Tests to add/update：source-derived tree、explicit nested import、unimported directory invisible、no Original Folder UI、count equality。
- Completion condition：Sidebar 没有原始文件夹、全部目录或磁盘目录树。
- Dependency：A10 完成；FilterState 的完全清理在 B9 继续完成。

### A12 — Integration / Desktop Verification

Goal：以真实桌面操作确认 A1-A11 组合行为。

- Files to change：[src/App.test.tsx](E:/Code/Codex/photo-organizer/src/App.test.tsx)、[src/components/OrganizationWorkspace.test.tsx](E:/Code/Codex/photo-organizer/src/components/OrganizationWorkspace.test.tsx)、Rust scanner/db tests、[src/test/visual-fixture.ts](E:/Code/Codex/photo-organizer/src/test/visual-fixture.ts)。
- DB/schema impact：验证迁移后的 fixture DB；不新增业务 schema。
- API impact：验证 import/rescan/remove/list/open Explorer 契约。
- React state impact：验证切换 Parent、Child、删除和 refresh 后 active/selection 不出现悬挂 ID。
- Rust/domain impact：验证 source boundary、ownership、browse scope、prune、missing、concurrency。
- Tests to add/update：完整 Rust、frontend、migration、source integrity 和桌面 smoke。
- Completion condition：所有 Exit Criteria 和 Manual Verification 通过。
- Dependency：A1-A11 全部完成。

## 8. Migration Strategy

本阶段涉及两个未来 migration，但本次只写 Runbook，不创建文件。

### Planned 0005 — Library Source Identity and Hierarchy

- 备份 SQLite。
- 增加 name、source_path、source_identity_key、parent_library_id、display_order。
- 从 root_path backfill sourcePath 和 name。
- 使用 A1 的 identity function backfill sourceIdentityKey。
- 按 sourceIdentityKey 去重 Library。
- 在事务中重建 parentLibraryId。
- 增加 source_identity_key UNIQUE 和 parent index。
- root_path 继续兼容读取，待业务代码迁移后再清理。
- 失败时回滚事务并保留原数据库备份。

### Planned 0006 — Global Asset Identity and Ownership

- 备份 SQLite。
- 增加 asset_identity_key。
- 从 absolute_path 生成 identity key。
- 对重复逻辑 Asset 选择稳定 survivor。
- 合并所有关联表引用后再删除重复索引行。
- 保留 absolute_path、relative_path、fingerprint 和 Asset ID。
- 建立 asset_identity_key UNIQUE。
- owner library_id 变为可更新字段，但 Asset ID 不变。
- 迁移采用 forward-only；回退使用数据库备份。

任何 migration 失败都必须停止应用启动，不能继续使用半迁移 schema，也不能触碰 SourceRoot。

## 9. Automated Tests

### Rust unit

- Windows-aware identity normalization。
- component-boundary containment。
- SourceRoot/AppDataRoot/ExportRoot boundary。
- parent resolver。
- most-specific owner resolver。
- descendant prune。

### Rust integration

- Parent-first / Child-first。
- nested depth unlimited。
- duplicate logical Asset。
- remove Child/Parent/Middle。
- scoped missing。
- overlapping scan lock。

### DB migration

- 0005 backfill。
- 0006 duplicate merge。
- foreign key preservation。
- Asset ID preservation。
- recursive count after import/remove。

### Frontend

- Library tree rendering。
- recursive count consistency。
- Parent selection loads descendants。
- no folder tree navigation。
- remove and refresh state。

### Source integrity

- import、scan、rescan、remove 前后 source hash 不变。
- thumbnail/preview/cache 写入只在 AppData。

### Evaluation

本阶段不改变模型结果；只确认已有分类字段在 owner reassignment 后仍保持关联。

## 10. Manual Verification

使用 test-data/ 下的 Parent、Child、Grandchild fixture：

1. 导入 Parent。
   - 预期：建立一个 Library，Sidebar 显示 Parent。
2. 导入 Child。
   - 预期：建立独立 Library，自动挂到 Parent。
3. 导入 Grandchild。
   - 预期：自动挂到 Child，Parent browse 包含三层。
4. 对比 Parent-first 和 Child-first 数据库结果。
   - 预期：Library hierarchy、Asset ID、owner 和 counts 等价。
5. 点击 Parent。
   - 预期：显示 Parent、Child、Grandchild 的全部 Assets。
6. 检查 Sidebar 数量和 AssetPage.total。
   - 预期：完全一致。
7. 重新扫描 Parent。
   - 预期：Child/Grandchild subtree 被 prune，不触发子图库扫描。
8. 重新扫描 Child。
   - 预期：只维护 Child 自己的 scope，Grandchild 被 prune。
9. 删除 Child。
   - 预期：Child Asset fallback 到 Parent 或最近剩余 owner，源文件不变。
10. 删除 Parent。
    - 预期：子 Library 保留并重新挂载，Parent-only 无 owner Asset 才从应用索引移除。
11. 观察带中文、空格和 Unicode 的 SourceRoot。
    - 预期：能导入、比较、扫描和去重。
12. 检查 Sidebar。
    - 预期：没有原始文件夹、全部目录、Folder Tree 或虚拟 Group。

## 11. Exit Criteria

- [ ] SourceRoot 与 AppDataRoot overlap 会被拒绝。
- [ ] 同一 sourceIdentityKey 不会产生重复 Library。
- [ ] 同一 assetIdentityKey 不会产生重复 Asset。
- [ ] Parent-first / Child-first 结果一致。
- [ ] parentLibraryId 只能系统推导。
- [ ] Asset ownership 始终为 Most Specific Library。
- [ ] 删除 Child/Parent/Middle 行为通过测试。
- [ ] Parent Rescan prune descendant，不触发 descendant rescan。
- [ ] Parent Browse、list、count、group、filter、pagination 使用同一 scope。
- [ ] Sidebar count 与 Parent 点击结果一致。
- [ ] 源文件和源目录未被修改。
- [ ] Rust、frontend、migration、source integrity 测试通过。
- [ ] Manual Verification 全部通过。
- [ ] 创建独立 Checkpoint A commit。

任一项失败，A 不得标记完成。

## 12. Expected Files To Change

- [src/types.ts](E:/Code/Codex/photo-organizer/src/types.ts)
- [src/App.tsx](E:/Code/Codex/photo-organizer/src/App.tsx)
- [src/api.ts](E:/Code/Codex/photo-organizer/src/api.ts)
- [src/components/Sidebar.tsx](E:/Code/Codex/photo-organizer/src/components/Sidebar.tsx)
- [src/App.test.tsx](E:/Code/Codex/photo-organizer/src/App.test.tsx)
- [src/test/visual-fixture.ts](E:/Code/Codex/photo-organizer/src/test/visual-fixture.ts)
- [src-tauri/src/models.rs](E:/Code/Codex/photo-organizer/src-tauri/src/models.rs)
- [src-tauri/src/db.rs](E:/Code/Codex/photo-organizer/src-tauri/src/db.rs)
- [src-tauri/src/scanner.rs](E:/Code/Codex/photo-organizer/src-tauri/src/scanner.rs)
- [src-tauri/src/ipc.rs](E:/Code/Codex/photo-organizer/src-tauri/src/ipc.rs)
- [src-tauri/src/paths.rs](E:/Code/Codex/photo-organizer/src-tauri/src/paths.rs)
- 新增 source identity / ownership domain module。
- 未来新增 0005、0006 migration 文件；本次不创建。

## 13. Risks

- 旧数据库存在跨 Library 重复 Asset，合并时可能涉及多张关联表。
- Parent recursive query 如果缺少 parent 和 library indexes，可能影响大型图库。
- 并发扫描可能与已有 task cancellation 状态竞争。
- Windows reparse point 和 Unicode identity semantics 可能无法从 lexical path 安全推断。
- 现有前端 fixture 依赖 flat Library 和 folderPrefix，需要同步更新。
- remove_library 当前 cascade 逻辑不能直接复用，必须先做 reconciliation。
- Parent browse 与 Parent rescan 如果共享旧 scan API，容易发生 scope 混淆。

## 14. Stop Condition

完成 A1-A12 后：

1. 运行全部要求的 Rust、frontend、migration、path 和 source integrity 测试。
2. 完成 Manual Verification。
3. Review diff，确认没有实现 B-F。
4. 更新 IMPLEMENTATION_STATUS.md。
5. 创建 Checkpoint A commit。
6. 停止，等待审核。
