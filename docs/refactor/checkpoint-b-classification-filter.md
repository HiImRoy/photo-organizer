# Checkpoint B — Manual Classification + Effective Filter

状态：IMPLEMENTED_PENDING_MANUAL

实现说明：B1-B12 的代码、数据库、Effective 查询和自动化测试已完成；当前只等待桌面端 Manual Verification，完成后再将状态更新为 COMPLETED。已应用 migration 0009，未实现 Checkpoint C-F。

本阶段建立封闭的 Derived Classification Registry、Manual Override、Effective Classification Resolver 和基于 Effective 的筛选。完成后必须提交并停止，不能自动进入 Checkpoint C。

## 1. Goal

交付以下能力：

- 明确当前所有 Derived Categorical Classification。
- Auto、Manual Override、Effective 三层分离。
- 支持单张图片人工修正。
- 支持 Restore Auto。
- 支持批量人工修正。
- 所有分类显示和筛选使用 Effective。
- analysis status 进入更多筛选，不再作为分类值伪装。
- Grid 使用 summary DTO，DetailPanel 使用 detail DTO。
- 重新分析不删除人工修正。
- 同一 Effective Resolver 服务于 Sidebar、AssetCard、DetailPanel、查询和 Export 上下文。

## 2. Non-goals

- 不升级 Semantic 模型、taxonomy 或 prompt ensemble；属于 Checkpoint D。
- 不改变 Dominant Color 算法；只把现有分类字段纳入 Registry。
- 不实现 Preview resource tiers；属于 Checkpoint C。
- 不实现 Export Snapshot 或 COPY。
- 不把 Auxiliary Tag 作为默认 Export Directory Dimension。
- 不把 Objective Numeric Feature 放进 Derived Classification Registry。
- 不恢复 Folder Tree、folderPrefix 或 Original Folder UI。

## 3. Preconditions

- Checkpoint A 已完成并提交。
- Asset 已具备全局 identity 和正确 owner。
- Parent Browse Scope 已经能返回 current + descendants。
- SourceRoot 仍然只读。
- 当前 Semantic 和 Imaging 结果可以在 test-data/ fixture 上读取。

## 4. Architecture Invariants

### Derived Classification Registry

当前 Registry 是封闭集合：

1. Primary Category。
2. Auxiliary Tags。
3. Tone。
4. Dominant Color Category / Palette（多值分类）。
5. Saturation Level。

每个 Registry 字段都必须支持：

    Auto
    Manual Override
    Effective
    Restore Auto

如果支持筛选，筛选必须使用 Effective。

Dominant Color Category 是多值分类：

- Auto 和 Effective 可以包含多个有序颜色类别；默认筛选语义为匹配任一 Effective 类别。
- 颜色的 RGB、面积占比、显著性占比和空间连续性属于 Imaging Auto detail，不属于 Derived Classification Registry。
- Manual Override 替换整个颜色类别列表；Restore Auto 删除该字段的 override。

以下不属于 Registry：

- brightness、contrast、saturation mean、chroma mean、neutral ratio、color coverage 等 Objective Numeric Feature。
- EXIF、路径、尺寸、文件大小、拍摄时间。
- analysisStatus。

### Classification Semantics

- UNKNOWN 只表示 Semantic 成功执行但 confidence 或 margin 不足。
- FAILED 表示分析流程失败，不能用 UNKNOWN 代替。
- OTHER、UNKNOWN、FAILED 不得混淆。
- B 不改变 D 负责的模型推理规则，但不能引入新的混淆语义。

### Manual Data

- 单值字段使用 nullable override；Dominant Color Category 使用有序列表 override。
- Auxiliary Tags 使用 ADD / REMOVE。
- 重置字段删除对应 override。
- semantic_labels 中旧的 is_manual 和 is_excluded 不再作为业务判断。

## 5. Current Implementation

Checkpoint B 已将上述目标落到当前代码：

- [src-tauri/src/classification.rs](E:/Code/Codex/photo-organizer/src-tauri/src/classification.rs) 提供封闭 Registry、Auto/Manual/Effective 类型、Auxiliary Tag ADD/REMOVE 和唯一 Effective Resolver。
- [src-tauri/migrations/0009_manual_classification_overrides.sql](E:/Code/Codex/photo-organizer/src-tauri/migrations/0009_manual_classification_overrides.sql) 创建单值 override、tag override 和 `classification_revision`。
- [src-tauri/src/models.rs](E:/Code/Codex/photo-organizer/src-tauri/src/models.rs) 拆分 `AssetGridItem`、`AssetDetail` 与组织用途的完整资产模型；Grid DTO 不暴露源文件路径。
- [src-tauri/src/db.rs](E:/Code/Codex/photo-organizer/src-tauri/src/db.rs) 在 list/count/group/filter 中解析 Effective；`folderPrefix` 不再进入 `AssetFilter`，但 `relative_path` 仍保留给源文件和 Organization。
- [src-tauri/src/semantic.rs](E:/Code/Codex/photo-organizer/src-tauri/src/semantic.rs) 保持 UNKNOWN 与 FAILED 分离：低置信度成功返回 UNKNOWN，执行失败清除当前 Auto 结果并返回 FAILED。
- [src-tauri/src/ipc.rs](E:/Code/Codex/photo-organizer/src-tauri/src/ipc.rs)、[src/api.ts](E:/Code/Codex/photo-organizer/src/api.ts) 提供 Registry、Detail、单字段编辑、tag 编辑、Restore Auto 和批量编辑 API。
- [src/components/DetailPanel.tsx](E:/Code/Codex/photo-organizer/src/components/DetailPanel.tsx) 显示 Auto/Manual/Effective/provenance 并支持五类 Registry 字段修正；分类修正区域默认折叠，展开后使用中文下拉/多选控件，不接受原始分类 ID 文本输入；[src/App.tsx](E:/Code/Codex/photo-organizer/src/App.tsx) 提供同样使用选项控件的批量修正入口。
- [src/components/AssetCard.tsx](E:/Code/Codex/photo-organizer/src/components/AssetCard.tsx)、[src/components/Sidebar.tsx](E:/Code/Codex/photo-organizer/src/components/Sidebar.tsx) 使用 Effective 分类和 analysis status 筛选。

## 6. Target State

### Domain Model

    AutoClassification
    ManualClassificationOverride
    ManualTagOverride
    EffectiveClassification

Effective Resolver 是唯一权威实现。

### DB Model

新增逻辑表：

    manual_classification_overrides
    manual_tag_overrides

Asset 具有 classificationRevision。任何会改变 Effective 的 Auto 或 Manual 变更都增加 revision。

### React State

- DetailPanel 使用 AssetDetail。
- Grid 使用 AssetGridItem。
- Manual editor 状态只表示待提交的 patch，不复制完整 Asset。
- filter 只保存内容属性、analysis status、numeric features、sort 和 pagination。
- folderPrefix 被移除。

### IPC

目标 API：

- getAssetDetail(assetId)
- updateClassificationOverride(assetId, patch)
- updateTagOverride(assetId, tagId, ADD | REMOVE)
- restoreAutoClassification(assetId, field?)
- batchUpdateClassification(assetIds, patch)
- listAssets 使用 Effective filter。

### Rust Data Flow

    auto tables + manual tables
              ↓
    EffectiveClassification resolver
              ↓
    grid/detail DTO
              ↓
    SQL filter / count / group / export context

### UI Behavior

- DetailPanel 可编辑所有 Registry 字段。
- Objective Numeric Feature 保持只读。
- UI 显示 Auto、Manual Override 和 Effective provenance。
- Restore Auto 不删除 Auto 结果。
- Analysis Status 出现在更多筛选。

## 7. Detailed Implementation Steps

### B1 — Derived Classification Registry

Goal：将当前五个 categorical derived fields 登记为封闭 Registry。

- Files to change：[src/types.ts](E:/Code/Codex/photo-organizer/src/types.ts)、[src-tauri/src/models.rs](E:/Code/Codex/photo-organizer/src-tauri/src/models.rs)、新增 classification domain module。
- DB/schema impact：可先使用稳定常量；若持久化 taxonomy，预留 taxonomyVersion，但不把 numeric feature 当 Registry。
- API impact：DTO 字段使用 Registry ID，不直接暴露散落字符串常量。
- React state impact：分类控件从 Registry descriptor 生成，不在多个组件复制字段列表。
- Rust/domain impact：定义 Primary Category、Auxiliary Tags、Tone、Dominant Color Category、Saturation Level 的类型、是否单值、是否可筛选。
- Tests to add/update：Registry completeness、numeric exclusion、analysisStatus exclusion、unsupported field rejection。
- Completion condition：新增分类字段如果未登记不能进入 UI、query 或 Export。
- Dependency：Checkpoint A 完成。

### B2 — Manual Override DB Schema

Goal：建立单值和标签人工修正的持久化模型。

- Files to change：[src-tauri/src/db.rs](E:/Code/Codex/photo-organizer/src-tauri/src/db.rs)、[src-tauri/src/models.rs](E:/Code/Codex/photo-organizer/src-tauri/src/models.rs)；已应用 0009 migration（A 已使用 0007、0008）。
- DB/schema impact：manual_classification_overrides 以 asset_id 为主键；manual_tag_overrides 使用 asset_id + tag_id 唯一键，并约束 ADD / REMOVE。
- API impact：增加读写 override 的 IPC 契约，错误时返回字段级错误。
- React state impact：DetailPanel 可以加载 pending override；失败提交不覆盖当前已确认值。
- Rust/domain impact：所有写入在 transaction 内增加 classificationRevision；reset 通过删除 override 实现。
- Tests to add/update：unique constraint、ADD/REMOVE、reset、missing Asset、transaction rollback。
- Completion condition：人工结果重启应用后仍可读取，且源文件未变化。
- Dependency：B1 完成。

### B3 — EffectiveClassification Resolver

Goal：实现 Auto + Manual → Effective 的唯一 Resolver。

- Files to change：[src-tauri/src/models.rs](E:/Code/Codex/photo-organizer/src-tauri/src/models.rs)、[src-tauri/src/db.rs](E:/Code/Codex/photo-organizer/src-tauri/src/db.rs)、新增 resolver module。
- DB/schema impact：Resolver 读取 Auto semantic、tone_features、color_features 和 manual tables。
- API impact：Asset DTO 统一返回 effective、auto、manual provenance。
- React state impact：组件不得自行 coalesce 自动值和人工值。
- Rust/domain impact：单值使用 manual ?? auto；标签使用 auto + ADD - REMOVE；失败时 Auto 为空但已有 Manual 仍可产生 Effective，analysisStatus 继续保持 FAILED。
- Tests to add/update：每个 Registry field 的 auto/manual/effective/restore、OTHER/UNKNOWN/FAILED separation。
- Completion condition：Sidebar、Detail、Card、query 和 export context 全部调用同一个 Resolver。
- Dependency：B2 完成。

### B4 — Grid Summary DTO / Detail DTO

Goal：把当前宽 AssetListItem 拆成网格摘要和详情模型。

- Files to change：[src/types.ts](E:/Code/Codex/photo-organizer/src/types.ts)、[src-tauri/src/models.rs](E:/Code/Codex/photo-organizer/src-tauri/src/models.rs)、[src-tauri/src/db.rs](E:/Code/Codex/photo-organizer/src-tauri/src/db.rs)。
- DB/schema impact：grid 查询只投影 thumbnail、文件名、尺寸、Effective 分类和状态；detail 查询再加载 EXIF、路径、Auto、Manual。
- API impact：fetchAssets 返回 AssetGridItem；新增 getAssetDetail。
- React state impact：assets 数组不再承担完整 Detail metadata；DetailPanel 按 activeAssetId 请求。
- Rust/domain impact：保持 page size 120 时不读取每个 Asset 的完整 semantic labels。
- Tests to add/update：DTO projection、detail completeness、120-item query performance、manual marker。
- Completion condition：Grid 不再为每一张卡片重复加载完整 Detail。
- Dependency：B3 完成。

### B5 — Single Asset Classification Editor

Goal：在 DetailPanel 中支持单 Asset 编辑。

- Files to change：[src/components/DetailPanel.tsx](E:/Code/Codex/photo-organizer/src/components/DetailPanel.tsx)、[src/App.tsx](E:/Code/Codex/photo-organizer/src/App.tsx)、[src/api.ts](E:/Code/Codex/photo-organizer/src/api.ts)。
- DB/schema impact：使用 B2 override schema。
- API impact：调用 updateClassificationOverride 和 updateTagOverride。
- React state impact：编辑草稿、提交中、错误和成功刷新状态；不能修改 Objective Numeric Feature。
- Rust/domain impact：校验 Registry field、enum value、tag state 和 revision。
- Tests to add/update：编辑 Primary、Tone、Color、Saturation、ADD/REMOVE tag、错误提交。
- Completion condition：用户能编辑所有 Registry 字段，且页面立即显示 Effective 和 provenance。
- Dependency：B3、B4 完成。

### B6 — Restore Auto

Goal：实现按字段恢复自动分类。

- Files to change：[src/components/DetailPanel.tsx](E:/Code/Codex/photo-organizer/src/components/DetailPanel.tsx)、[src/api.ts](E:/Code/Codex/photo-organizer/src/api.ts)、[src-tauri/src/ipc.rs](E:/Code/Codex/photo-organizer/src-tauri/src/ipc.rs)、[src-tauri/src/db.rs](E:/Code/Codex/photo-organizer/src-tauri/src/db.rs)。
- DB/schema impact：删除指定 override row；不删除 Auto result。
- API impact：支持恢复单个字段和辅助标签。
- React state impact：恢复后重新请求 Detail 和当前 grid row。
- Rust/domain impact：恢复操作增加 revision，并在 transaction 中完成。
- Tests to add/update：restore primary、restore tone、restore tag ADD/REMOVE、重复 restore。
- Completion condition：恢复后 Effective 等于 Auto，Auto provenance 保留。
- Dependency：B5 完成。

### B7 — Batch Classification Editor

Goal：支持对多选 Asset 批量应用 Registry patch。

- Files to change：[src/App.tsx](E:/Code/Codex/photo-organizer/src/App.tsx)、[src/components/DetailPanel.tsx](E:/Code/Codex/photo-organizer/src/components/DetailPanel.tsx)、新增 batch editor component、[src/api.ts](E:/Code/Codex/photo-organizer/src/api.ts)。
- DB/schema impact：批量写入必须 transaction；失败时整批回滚或返回明确逐项结果，不能静默部分成功。
- API impact：新增 batchUpdateClassification。
- React state impact：使用现有 selectedAssetIds；批量编辑不改变 Asset ownership。
- Rust/domain impact：对每个 Asset 校验 Registry patch，统一增加 revision。
- Tests to add/update：multi-asset patch、partial validation、cancel、selection refresh。
- Completion condition：批量修改后所有 selected Asset 的 Effective 一致可验证。
- Dependency：B5、B6 完成。

### B8 — Effective Classification SQL

Goal：让 list、count、group、search 和 filter 在数据库中直接使用 Effective。

- Files to change：[src-tauri/src/db.rs](E:/Code/Codex/photo-organizer/src-tauri/src/db.rs)、[src-tauri/src/models.rs](E:/Code/Codex/photo-organizer/src-tauri/src/models.rs)。
- DB/schema impact：增加 override indexes、semantic auto indexes 和 Effective SQL CTE/view。
- API impact：AssetFilter 删除 folderPrefix；分类条件按 Registry ID 表达。
- React state impact：filter 不再由前端读取全部 Asset 后计算。
- Rust/domain impact：单值用 COALESCE；标签用 EXISTS / NOT EXISTS；Library scope 先于 Effective filter。
- Tests to add/update：Effective list/count equality、manual tag filter、parent recursive scope + filter、pagination。
- Completion condition：前端拿到的结果已经满足 Effective filter。
- Dependency：B3、Checkpoint A10 完成。

### B9 — FilterState Cleanup

Goal：删除 folderPrefix 和旧目录导航语义。

- Files to change：[src/types.ts](E:/Code/Codex/photo-organizer/src/types.ts)、[src/App.tsx](E:/Code/Codex/photo-organizer/src/App.tsx)、[src/api.ts](E:/Code/Codex/photo-organizer/src/api.ts)、[src-tauri/src/models.rs](E:/Code/Codex/photo-organizer/src-tauri/src/models.rs)、[src-tauri/src/db.rs](E:/Code/Codex/photo-organizer/src-tauri/src/db.rs)。
- DB/schema impact：不删除 Asset relative_path；只移除 folderPrefix SQL 和 API 过滤。
- API impact：semanticState 改为 analysis status 语义；不再提供 folder list 作为 filter。
- React state impact：emptyAssetFilter 删除 folderPrefix；countActiveFilters 不再计入目录。
- Rust/domain impact：OriginalDirectory 保留给未来 Export，不进入 AssetFilter。
- Tests to add/update：旧 folderPrefix 请求拒绝或忽略、relativePath 仍可读、analysis status filter。
- Completion condition：FilterState 只处理内容属性、状态、数值和日期，不承担磁盘导航。
- Dependency：B8 完成。

### B10 — Active Filter Chips

Goal：显示 Effective classification 和其他筛选 chip，并支持独立清除。

- Files to change：[src/App.tsx](E:/Code/Codex/photo-organizer/src/App.tsx)、[src/components/Sidebar.tsx](E:/Code/Codex/photo-organizer/src/components/Sidebar.tsx)、[src/types.ts](E:/Code/Codex/photo-organizer/src/types.ts)。
- DB/schema impact：无。
- API impact：无新数据库 API；使用已清理 FilterState。
- React state impact：每个 chip 对应一个 filter field；清除后 page 重置为 1。
- Rust/domain impact：确认 query 使用 Effective，而非 Auto display label。
- Tests to add/update：single chip、multi chip、clear one、clear all、Parent scope。
- Completion condition：chip 和查询结果保持一致。
- Dependency：B9 完成。

### B11 — Sidebar Classification UX

Goal：把分类入口和 analysis status 放入最终 Sidebar 结构。

- Files to change：[src/components/Sidebar.tsx](E:/Code/Codex/photo-organizer/src/components/Sidebar.tsx)、[src/App.tsx](E:/Code/Codex/photo-organizer/src/App.tsx)。
- DB/schema impact：semantic groups/count 使用 recursive Library scope。
- API impact：分类统计使用 Effective；analysis status 进入更多筛选。
- React state impact：删除快捷入口中的 not_analyzed/failed；加入 status filter。
- Rust/domain impact：group query 与 list query 使用相同 scope 和 Effective semantics。
- Tests to add/update：Sidebar sections、status filter、parent descendant groups、no Original Folder section。
- Completion condition：Sidebar 不显示原始目录，分类入口与 Effective 查询一致。
- Dependency：B8-B10 完成。

### B12 — Reanalysis Preservation Verification

Goal：确认自动重分析不会破坏 Manual Override。

- Files to change：[src/App.test.tsx](E:/Code/Codex/photo-organizer/src/App.test.tsx)、[src/components/OrganizationWorkspace.test.tsx](E:/Code/Codex/photo-organizer/src/components/OrganizationWorkspace.test.tsx)、[src-tauri/src/semantic_tasks.rs](E:/Code/Codex/photo-organizer/src-tauri/src/semantic_tasks.rs)、[src-tauri/src/db.rs](E:/Code/Codex/photo-organizer/src-tauri/src/db.rs)。
- DB/schema impact：确认 Auto update 和 Manual rows 分开。
- API impact：reanalyzeAsset 完成后返回新的 Auto revision，但保留 Manual。
- React state impact：分析进度刷新后 Detail 和 Grid 仍显示 Effective override。
- Rust/domain impact：自动保存不能 delete manual rows；失败状态不能写 UNKNOWN 代替。
- Tests to add/update：manual survives reanalysis、restore after reanalysis、failed analysis、source integrity。
- Completion condition：全部 Registry fields 的 preservation test 通过。
- Dependency：B1-B11 全部完成；D2 会进一步强化状态语义。

## 8. Migration Strategy

本阶段已应用 schema migration；迁移编号与 Checkpoint A 的 0007、0008 顺延，当前使用 0009。

### Applied 0009 — Manual Overrides

- 迁移前备份 SQLite。
- 创建 manual_classification_overrides。
- 创建 manual_tag_overrides。
- 增加 asset classification_revision。
- 为 asset_id、asset_id + tag_id、tag_id + state 建 indexes。
- 对旧 semantic_labels.is_manual / is_excluded 做兼容审计。
- 能安全映射的旧人工标签转换为 Manual Override。
- 不能确定语义的旧数据保留审计信息，不静默猜测。
- 旧字段在兼容期只读，不再参与业务查询。

本次实现已创建 `manual_classification_overrides`、`manual_tag_overrides`，并在所有手动写入、自动完成、源文件 fingerprint 变化时维护 `classification_revision`。旧 `semantic_labels.is_manual/is_excluded` 不被当作新的 Manual Override 业务状态。

### Taxonomy compatibility

- B 只登记稳定 Registry field 和 field IDs。
- Semantic model version、taxonomy version、imaging/color/tone algorithm version 的正式拆分由 D7 完成。
- classificationRevision 可以在 B 建立，但不能替代 D 的 pipeline-specific version。

迁移失败时：

- 回滚数据库事务。
- 保留迁移前备份。
- 不运行半迁移状态的业务查询。
- 不修改 SourceRoot。

## 9. Automated Tests

### Rust unit

- Registry completeness。
- Auto/manual/effective resolution。
- tag ADD/REMOVE。
- restore。
- OTHER/UNKNOWN/FAILED state handling。

### Rust integration

- override persistence。
- batch transaction。
- reanalysis preservation。
- parent recursive scope + Effective filter。
- Asset ID 和 source data preservation。

### Frontend

- DetailPanel editor。
- Restore Auto。
- batch editor。
- Effective provenance。
- chips。
- Sidebar status filters。

### DB migration

- override schema creation。
- old is_manual/is_excluded compatibility。
- unique constraints。
- revision increments。

### Path / source integrity

- manual operation 不触碰源文件。
- relativePath 仍保留并可被后续 Export 使用。

### Evaluation

本阶段不评估模型准确率；只确认人工修正和自动结果的生命周期正确。

## 10. Manual Verification

1. 打开已扫描的 fixture Library。
   - 预期：Grid 仍按 Parent recursive scope 显示。
2. 打开任意 Asset Detail。
   - 预期：显示 Auto、Manual 和 Effective 三层信息。
3. 修改 Primary Category。
   - 预期：Effective 立即变化，并显示人工修正标记。
4. 修改 Tone、Dominant Color Category 和 Saturation Level。
   - 预期：四类单值字段都可保存。
5. 对 Auxiliary Tag 执行 ADD 和 REMOVE。
   - 预期：Effective tags 按 ADD/REMOVE 合并。
6. 点击 Restore Auto。
   - 预期：override 被删除，Auto 结果恢复为 Effective。
7. 批量选择多个 Asset 修改分类。
   - 预期：所有选中 Asset 更新，未选中 Asset 不变。
8. 使用分类筛选。
   - 预期：结果使用 Effective，不因 Auto 值不同而错误显示。
9. 使用分析状态筛选。
   - 预期：FAILED 与 UNKNOWN 不混为一个分类入口。
10. 重新分析已有人工修正的 Asset。
    - 预期：Auto 更新，Manual 和 Effective override 保留。
11. 检查 Sidebar。
    - 预期：不存在 folderPrefix、原始文件夹或全部目录。
12. 检查 source fixture。
    - 预期：文件内容和 hash 不变。

## 11. Exit Criteria

- [ ] Registry 只包含五个 Derived Categorical Classification。
- [ ] Objective Numeric Feature 不在 Registry。
- [ ] 每个 Registry 字段支持 Auto、Manual、Effective、Restore Auto。
- [ ] Manual Override 持久化并支持事务。
- [ ] Effective Resolver 是唯一权威实现。
- [ ] Grid Summary 和 Detail DTO 分离。
- [ ] folderPrefix 从 FilterState、SQL 和 Sidebar 删除。
- [ ] 分类筛选使用 Effective。
- [ ] analysis status 位于更多筛选。
- [ ] Reanalysis 不删除 Manual Override。
- [ ] UNKNOWN、OTHER、FAILED 语义未混淆。
- [ ] Parent recursive scope 在 list/count/filter/group/pagination 一致。
- [ ] Rust、frontend、DB 和 source integrity 测试通过。
- [ ] Manual Verification 全部通过。
- [ ] 创建独立 Checkpoint B commit。

任一项失败，B 不得标记完成。

## 12. Expected Files To Change

- [src/types.ts](E:/Code/Codex/photo-organizer/src/types.ts)
- [src/App.tsx](E:/Code/Codex/photo-organizer/src/App.tsx)
- [src/api.ts](E:/Code/Codex/photo-organizer/src/api.ts)
- [src/components/DetailPanel.tsx](E:/Code/Codex/photo-organizer/src/components/DetailPanel.tsx)
- [src/components/AssetCard.tsx](E:/Code/Codex/photo-organizer/src/components/AssetCard.tsx)
- [src/components/Sidebar.tsx](E:/Code/Codex/photo-organizer/src/components/Sidebar.tsx)
- [src/App.test.tsx](E:/Code/Codex/photo-organizer/src/App.test.tsx)
- [src-tauri/src/models.rs](E:/Code/Codex/photo-organizer/src-tauri/src/models.rs)
- [src-tauri/src/db.rs](E:/Code/Codex/photo-organizer/src-tauri/src/db.rs)
- [src-tauri/src/ipc.rs](E:/Code/Codex/photo-organizer/src-tauri/src/ipc.rs)
- [src-tauri/src/semantic_tasks.rs](E:/Code/Codex/photo-organizer/src-tauri/src/semantic_tasks.rs)
- 新增 classification registry / effective resolver module。
- 已新增 0009 manual override migration；Checkpoint B 代码已应用该迁移。

## 13. Risks

- 旧 semantic_labels 的 is_manual/is_excluded 可能无法无损映射。
- Effective SQL 同时处理 recursive scope、manual tags 和分页，容易出现 list/count 不一致。
- 现有 AssetListItem 很宽，拆 DTO 可能影响 visual fixture 和 Organization Workspace。
- Batch update 的部分失败语义需要明确保持原子性。
- 重新分析和手工编辑可能并发写入 classificationRevision。
- 旧 tone 值 mid_tone 与前端 balanced 的不一致会在 Registry 接入时暴露。

## 14. Stop Condition

完成 B1-B12 后：

1. 运行全部 Rust、frontend、DB、Effective filter 和 source integrity 测试。
2. 完成 Manual Verification。
3. Review diff，确认没有实现 C-F。
4. 更新 IMPLEMENTATION_STATUS.md。
5. 创建 Checkpoint B commit。
6. 停止，等待审核。
