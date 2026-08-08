# Checkpoint E — Export Preview

状态：NOT_STARTED

本阶段把现有 Organization dry-run 转换为 Export Preview，并建立 OriginalDirectory、Effective Classification 和不可变 Export Snapshot。此阶段绝不能真正 COPY。

## 1. Goal

交付以下能力：

- Organization UI 和业务语义迁移为 Export Preview。
- Export Rule 支持分类、时间和可选 OriginalDirectory Dimension。
- OriginalDirectory 相对于 Asset 当前 owner Library SourceRoot。
- Export Preview 使用 Effective Classification。
- Preview 生成 immutable Export Plan Snapshot。
- Snapshot 冻结 source、fingerprint、classification、rule 和 targetPath。
- classificationRevision 变化使旧计划 stale。
- SourceRoot、AppDataRoot、ExportRoot 边界得到统一验证。
- 预览阶段解决 collision、非法路径和目标安全问题。
- 本阶段不执行 COPY。

## 2. Non-goals

- 不实现真正 COPY；属于 Checkpoint F。
- 不实现 move、delete、rename、overwrite、hard link 或 symbolic link。
- 不实现 generated-copy rollback。
- 不把 Auxiliary Tag 作为默认 Export Directory Dimension。
- 不恢复 Original Folder Sidebar、Folder Tree 或 folderPrefix。
- 不修改 Asset Ownership。
- 不在 Execute 阶段重新计算规则或分类。

## 3. Preconditions

- Checkpoint D 已完成并提交。
- Checkpoint A 的 SourceRoot boundary、Asset identity 和 recursive Library Scope 可用。
- Checkpoint B 的 Effective Classification Resolver 和 classificationRevision 可用。
- OriginalDirectory 已确认为可选 Export Dimension。
- Checkpoint C 的 activeAssetId 和 Asset Detail 可用。

## 4. Architecture Invariants

- Export Preview 是只读规划操作。
- Preview → Immutable Snapshot → COPY Execute。
- Execute 不在本阶段实现。
- Snapshot 中的 targetPath 不得在 Execute 时重新计算。
- Snapshot 冻结 sourcePath、sourceFingerprint、Effective Classification、ruleVersion 和 classificationRevision。
- OriginalDirectory 只来自 Asset owner Library SourceRoot 下的 relative parent path。
- 未选择 OriginalDirectory 时，Export 不保留原始目录结构。
- OriginalDirectory 不进入 Sidebar、FilterState 或 Library hierarchy。
- Parent Library Export scope 使用 current + all descendants。
- Auxiliary Tag 默认只作为筛选和展示，不自动产生多目录副本。
- Export target 不得与任何 SourceRoot 或 AppDataRoot overlap。

## 5. Current Implementation

- [src/types.ts](E:/Code/Codex/photo-organizer/src/types.ts)
  - OrganizationRules、OrganizationLevelKind、OrganizationPlanRequest、OrganizationPlanItem 和 OrganizationPlan。
  - OrganizationLevelKind 已包含 OriginalDirectory、PrimarySemantic、Tone、DominantColor 和 Saturation。
- [src/components/OrganizationWorkspace.tsx](E:/Code/Codex/photo-organizer/src/components/OrganizationWorkspace.tsx)
  - 当前展示 Organization dry-run、规则和只读导出控制。
- [src-tauri/src/organization.rs](E:/Code/Codex/photo-organizer/src-tauri/src/organization.rs)
  - build_plan 生成只读计划。
  - render_context 读取 semantic labels、tone、color、saturation 和 relative_path。
  - OriginalDirectory 当前从 Asset relative_path 生成。
  - validate_target_boundary 目前主要检查当前 source root，尚未覆盖全部 SourceRoots、AppDataRoot 和 canonical identity。
- [src-tauri/src/ipc.rs](E:/Code/Codex/photo-organizer/src-tauri/src/ipc.rs)
  - preview_organization_plan 调用 list_assets_for_organization 和 build_plan。
  - export_organization_manifest 可以写出 JSON/CSV manifest，但不是 COPY executor。
- [src-tauri/src/db.rs](E:/Code/Codex/photo-organizer/src-tauri/src/db.rs)
  - organization_plans、organization_plan_items 和 organization_plan_issues 已有持久化逻辑。
  - list_assets_for_organization 目前依赖 list_assets，后续必须使用 A 的 recursive scope。
- [src-tauri/migrations/0003_organization_dry_run.sql](E:/Code/Codex/photo-organizer/src-tauri/migrations/0003_organization_dry_run.sql)
  - 当前 Organization plan schema。
- [src-tauri/src/models.rs](E:/Code/Codex/photo-organizer/src-tauri/src/models.rs)
  - OrganizationLevelKind::OriginalDirectory 已存在。

## 6. Target State

### Domain Model

    ExportRule
    ExportDimension
    OriginalDirectoryContext
    EffectiveClassificationExportContext
    ExportPlanSnapshot
    ExportPlanItemSnapshot

OriginalDirectoryContext：

    relative parent directory
    relative to current Asset owner Library SourceRoot

### DB Model

规划使用新的 export plan snapshot 语义；旧 organization 表可以作为兼容读取，但不能继续动态重算已保存计划。

Snapshot item 至少冻结：

    assetId
    sourcePath
    sourceFingerprint
    effectiveClassificationSnapshot
    originalDirectory
    targetPath
    ruleVersion
    classificationRevision
    createdAt

### React State

- ExportRule draft。
- Export Preview result。
- immutable planId。
- stale / ready / blocked 状态。
- 不在 Execute 时把当前 FilterState 或当前 classification 当作新的规则输入。

### IPC

目标 API：

- previewExportPlan(request)
- getExportPlanSnapshot(planId)
- verifyExportPlanSnapshot(planId)

本阶段不提供 executeCopy。

### Rust Data Flow

    activeLibraryId
      ↓
    recursive Library Scope
      ↓
    Effective Classification query
      ↓
    owner-relative OriginalDirectory
      ↓
    target path planning
      ↓
    destination safety and collision checks
      ↓
    immutable snapshot

### UI Behavior

- UI 名称从 Organization 改为 Export Preview。
- 用户可以显式选择 OriginalDirectory 作为目录维度。
- 不选择时不保留原始目录层级。
- Preview 页面清楚显示 plan status、issues、target tree 和 stale 状态。
- 不出现 COPY 进度或执行按钮；执行属于 F。

## 7. Detailed Implementation Steps

### E1 — Export Rule Model

Goal：建立最终 Export Rule 和 Dimension 模型。

- Files to change：[src/types.ts](E:/Code/Codex/photo-organizer/src/types.ts)、[src-tauri/src/models.rs](E:/Code/Codex/photo-organizer/src-tauri/src/models.rs)、[src/components/OrganizationWorkspace.tsx](E:/Code/Codex/photo-organizer/src/components/OrganizationWorkspace.tsx)。
- DB/schema impact：兼容旧 Organization rules；未来 snapshot schema 保存 ruleVersion。
- API impact：Organization 请求逐步改名为 Export Preview request，保留兼容读取。
- React state impact：rules 使用 Export Dimension 顺序和 fallback；默认不加入 OriginalDirectory。
- Rust/domain impact：明确 Primary Category、Tone、Color、Saturation、Capture Date 和 OriginalDirectory 的 dimension semantics。
- Tests to add/update：规则序列化、空 rules、dimension order、tag 不作为默认维度。
- Completion condition：规则模型能表达所有已确认 Export Dimension，且不混入 Sidebar/Filter。
- Dependency：A-D 完成。

### E2 — OriginalDirectory Optional Dimension

Goal：实现 owner-relative OriginalDirectory，并将其限制在 Export Context。

- Files to change：[src-tauri/src/organization.rs](E:/Code/Codex/photo-organizer/src-tauri/src/organization.rs)、[src-tauri/src/models.rs](E:/Code/Codex/photo-organizer/src-tauri/src/models.rs)、[src/types.ts](E:/Code/Codex/photo-organizer/src/types.ts)。
- DB/schema impact：Snapshot item 保存 originalDirectory；不删除 Asset relative_path。
- API impact：plan item 返回 owner-relative originalDirectory。
- React state impact：OriginalDirectory 是可选 dimension checkbox/select，不进入 filter state。
- Rust/domain impact：从 Asset owner Library SourceRoot 计算完整 relative parent path；owner reassignment 后使用新 relative_path。
- Tests to add/update：root file、DAY1、DAY1/subfolder、nested Library owner、no dimension。
- Completion condition：OriginalDirectory 不会生成 owner SourceRoot 名称前缀，也不会恢复 Folder Tree。
- Dependency：E1、A ownership reconciliation 完成。

### E3 — Effective Classification Export Context

Goal：Export planning 使用 B 的 Effective Classification，不直接使用 Auto 或 raw fields。

- Files to change：[src-tauri/src/organization.rs](E:/Code/Codex/photo-organizer/src-tauri/src/organization.rs)、[src-tauri/src/db.rs](E:/Code/Codex/photo-organizer/src-tauri/src/db.rs)、[src-tauri/src/models.rs](E:/Code/Codex/photo-organizer/src-tauri/src/models.rs)。
- DB/schema impact：plan item 保存 Effective snapshot，不只保存 label 字符串。
- API impact：Export preview response 同时返回 effective context 和 provenance。
- React state impact：Preview tree 使用 snapshot context，不从当前卡片临时拼接路径。
- Rust/domain impact：Library scope 先解析 current + descendants，再应用 Effective filter 和 Export Rule。
- Tests to add/update：manual override affects target path、auto vs effective、tag not default dimension、parent scope。
- Completion condition：修改 Manual 后重新 Preview 能得到新 target；旧 snapshot 不被悄悄修改。
- Dependency：B3、A10、E1 完成。

### E4 — Destination Safety

Goal：统一验证 SourceRoot、AppDataRoot 和 ExportRoot 的边界。

- Files to change：[src-tauri/src/organization.rs](E:/Code/Codex/photo-organizer/src-tauri/src/organization.rs)、[src-tauri/src/ipc.rs](E:/Code/Codex/photo-organizer/src-tauri/src/ipc.rs)、A1 path identity module。
- DB/schema impact：可在 plan issues 中保存 boundary issue；不写 source。
- API impact：Preview 返回明确 safety issue 和 blocked status。
- React state impact：安全错误阻止 Preview ready；不让用户进入 COPY 语义。
- Rust/domain impact：对所有 SourceRoots、AppDataRoot、目标祖先/后代、junction/symlink、Unicode 和 non-existing target 做 canonical boundary check。
- Tests to add/update：target equals source、target inside source、target ancestor of source、target inside AppData、reparse escape。
- Completion condition：所有非法目标在 Preview 阶段被阻止。
- Dependency：A1 完成。

### E5 — Collision Planning

Goal：在 Preview 阶段完整解决目标冲突，而不是 Execute 时临时决定。

- Files to change：[src-tauri/src/organization.rs](E:/Code/Codex/photo-organizer/src-tauri/src/organization.rs)、[src-tauri/src/models.rs](E:/Code/Codex/photo-organizer/src/models.rs)、[src/components/OrganizationWorkspace.tsx](E:/Code/Codex/photo-organizer/src/components/OrganizationWorkspace.tsx)。
- DB/schema impact：Plan item 保存最终 targetPath 和 collision strategy。
- API impact：支持 skip、sequence、short hash 等已确认策略。
- React state impact：显示每项 ready、warning、error、skipped_conflict。
- Rust/domain impact：检测现有目标、计划内部重复目标、非法文件名、路径长度和 duplicate source。
- Tests to add/update：existing target、same target from two assets、sequence、hash、skip。
- Completion condition：每个 Plan Item 都有明确最终 targetPath 或明确阻止原因。
- Dependency：E1-E4 完成。

### E6 — Immutable Export Snapshot

Goal：生成不可变 Preview Snapshot。

- Files to change：[src-tauri/src/db.rs](E:/Code/Codex/photo-organizer/src-tauri/src/db.rs)、[src-tauri/src/models.rs](E:/Code/Codex/photo-organizer/src-tauri/src/models.rs)、[src-tauri/src/ipc.rs](E:/Code/Codex/photo-organizer/src-tauri/src/ipc.rs)；未来新增 0009 migration。
- DB/schema impact：保存 plan、item、issues、Effective snapshot、OriginalDirectory、targetPath、ruleVersion、classificationRevision。
- API impact：preview 返回 planId；读取 snapshot 不重新构建计划。
- React state impact：Preview 页面绑定 planId 和 immutable response。
- Rust/domain impact：创建 snapshot 时一次性完成 scope、filter、effective、original directory、target path 和 safety checks。
- Tests to add/update：snapshot round-trip、reload after restart、source/classification changes do not mutate old plan。
- Completion condition：同一个 planId 的 item targetPath 不因当前状态变化而改变。
- Dependency：E2-E5 完成。

### E7 — classificationRevision Stale Detection

Goal：检测 Preview 后分类或 Auto 结果变化。

- Files to change：[src-tauri/src/db.rs](E:/Code/Codex/photo-organizer/src-tauri/src/db.rs)、[src-tauri/src/organization.rs](E:/Code/Codex/photo-organizer/src-tauri/src/organization.rs)、[src-tauri/src/ipc.rs](E:/Code/Codex/photo-organizer/src-tauri/src/ipc.rs)。
- DB/schema impact：plan item 保存 classificationRevision 和 sourceFingerprint。
- API impact：verifyExportPlanSnapshot 返回 ready/stale/source_changed/missing/blocked。
- React state impact：stale plan 显示重新 Preview 提示；不提供隐式更新。
- Rust/domain impact：分类 revision、source fingerprint、owner-relative path 或 rule version 变化都能使计划失效。
- Tests to add/update：manual edit after preview、semantic reanalysis、color version change、source file change、owner reassignment。
- Completion condition：旧计划不会执行或被静默改写。
- Dependency：E6、D versioning 完成。

### E8 — Export Preview UI

Goal：把 OrganizationWorkspace 转换为可理解的 Export Preview 界面。

- Files to change：[src/components/OrganizationWorkspace.tsx](E:/Code/Codex/photo-organizer/src/components/OrganizationWorkspace.tsx)、[src/App.tsx](E:/Code/Codex/photo-organizer/src/App.tsx)、[src/api.ts](E:/Code/Codex/photo-organizer/src/api.ts)。
- DB/schema impact：读取 snapshot，不在 frontend 持久化 source paths。
- API impact：调用 preview/read/verify snapshot；不调用 COPY。
- React state impact：规则草稿、preview result、plan status、issues 和 target tree。
- Rust/domain impact：所有 target path 来源于 snapshot。
- Tests to add/update：OriginalDirectory toggle、dimension order、stale status、issue display、no copy action。
- Completion condition：用户可以完整审阅目标树和风险，但不能在 E 阶段写入目标文件。
- Dependency：E1-E7 完成。

### E9 — Snapshot Verification Tests

Goal：验证 Preview 的不可变和安全属性。

- Files to change：[src/components/OrganizationWorkspace.test.tsx](E:/Code/Codex/photo-organizer/src/components/OrganizationWorkspace.test.tsx)、[src-tauri/src/organization.rs](E:/Code/Codex/photo-organizer/src-tauri/src/organization.rs) tests、[src-tauri/src/db.rs](E:/Code/Codex/photo-organizer/src-tauri/src/db.rs) tests。
- DB/schema impact：验证 snapshot schema 和 backward compatibility。
- API impact：验证 verify 不重算 target path。
- React state impact：验证 stale plan UI。
- Rust/domain impact：验证 source safety、Effective、OriginalDirectory 和 recursive scope。
- Tests to add/update：完整 E test matrix。
- Completion condition：所有 Exit Criteria 和 Manual Verification 通过。
- Dependency：E1-E8 全部完成。

## 8. Migration Strategy

本阶段规划未来 Export Snapshot migration，但当前不创建文件。

### Planned 0009 — Export Plan Snapshot

- 迁移前备份 SQLite。
- 建立 export_plans、export_plan_items、export_plan_issues 或等价新表。
- 保存 ruleVersion、scope、Effective snapshot、OriginalDirectory、targetPath、sourceFingerprint、classificationRevision。
- 旧 organization_plans 保留兼容读取，但新业务不动态修改旧 plan。
- 不创建 Export Job 或 COPY output table；Job 属于 F。
- 迁移失败回滚事务，恢复数据库备份，不接触 SourceRoot。
- schema 采用 forward-only。

## 9. Automated Tests

### Rust unit

- OriginalDirectory owner-relative path。
- dimension order。
- target boundary。
- collision strategy。
- immutable snapshot serialization。
- stale revision。

### Rust integration

- Parent recursive scope。
- Effective classification。
- nested owner relative path。
- source/appdata/export overlap。
- source fingerprint changed。
- no write during preview。

### Frontend

- Export Preview naming。
- dimension selection。
- OriginalDirectory optional behavior。
- target tree rendering。
- issue and stale rendering。
- no COPY invocation。

### DB migration

- snapshot schema。
- plan/item round-trip。
- old organization plan compatibility。
- revision and fingerprint persistence。

### Source integrity

- Preview 前后源 hash、mtime 和目录内容不变。
- target directory 不产生文件。

### Evaluation

本阶段评估目标路径确定性、collision 解决率和 stale detection，不评估 COPY throughput。

## 10. Manual Verification

1. 选择 Parent Library。
   - 预期：Preview scope 包含 Parent 和全部 descendants。
2. 选择 Primary Category → Color。
   - 预期：目标树只包含分类和颜色维度。
3. 选择 Primary Category → OriginalDirectory。
   - 预期：OriginalDirectory 相对于 Asset owner SourceRoot。
4. 对 nested Library Asset 检查路径。
   - 预期：不会把 Child Library 名称重复加入 OriginalDirectory。
5. 不选择 OriginalDirectory。
   - 预期：Export 不保留原始目录层级。
6. 创建 Preview。
   - 预期：生成 planId 和 immutable target paths。
7. 修改一个 Asset 的 Manual Classification。
   - 预期：旧计划显示 stale，不能被静默更新。
8. 修改规则后重新 Preview。
   - 预期：得到新的 planId 和新的 target paths。
9. 选择 SourceRoot、AppDataRoot 或其重叠目标。
   - 预期：Preview 被阻止并显示安全 issue。
10. 制造已有目标和计划内部冲突。
    - 预期：每项显示明确 collision strategy 或 blocked 状态。
11. 检查目标目录。
    - 预期：E 阶段没有创建任何 COPY 输出。
12. 检查源目录。
    - 预期：源文件 hash 和 mtime 不变。

## 11. Exit Criteria

- [ ] Organization 语义改为 Export Preview。
- [ ] Export Rule 支持已确认的 dimensions。
- [ ] OriginalDirectory 可选，且相对于当前 owner SourceRoot。
- [ ] 未选择 OriginalDirectory 时不保留原目录层级。
- [ ] Export 使用 Effective Classification。
- [ ] Parent scope 包含全部 descendants。
- [ ] boundary 检查覆盖所有 SourceRoots、AppDataRoot 和 ExportRoot。
- [ ] collision 在 Preview 阶段解决。
- [ ] Snapshot 冻结 source、fingerprint、classification、rule、OriginalDirectory 和 targetPath。
- [ ] classificationRevision 或 source fingerprint 变化会使计划 stale。
- [ ] Preview 阶段不创建任何目标文件。
- [ ] OriginalDirectory 没有进入 Sidebar、FilterState 或 Library hierarchy。
- [ ] Rust、frontend、DB、source integrity 和 snapshot tests 通过。
- [ ] Manual Verification 全部通过。
- [ ] 创建独立 Checkpoint E commit。

## 12. Expected Files To Change

- [src/types.ts](E:/Code/Codex/photo-organizer/src/types.ts)
- [src/api.ts](E:/Code/Codex/photo-organizer/src/api.ts)
- [src/App.tsx](E:/Code/Codex/photo-organizer/src/App.tsx)
- [src/components/OrganizationWorkspace.tsx](E:/Code/Codex/photo-organizer/src/components/OrganizationWorkspace.tsx)
- [src/components/OrganizationWorkspace.test.tsx](E:/Code/Codex/photo-organizer/src/components/OrganizationWorkspace.test.tsx)
- [src-tauri/src/models.rs](E:/Code/Codex/photo-organizer/src-tauri/src/models.rs)
- [src-tauri/src/db.rs](E:/Code/Codex/photo-organizer/src-tauri/src/db.rs)
- [src-tauri/src/ipc.rs](E:/Code/Codex/photo-organizer/src-tauri/src/ipc.rs)
- [src-tauri/src/organization.rs](E:/Code/Codex/photo-organizer/src-tauri/src/organization.rs)
- [src-tauri/src/paths.rs](E:/Code/Codex/photo-organizer/src-tauri/src/paths.rs)
- 未来新增 0009 Export Snapshot migration；本次不创建。

## 13. Risks

- 旧 Organization plan schema 不包含完整 Effective 和 OriginalDirectory snapshot。
- recursive scope、Effective filter 和 collision planning 可能导致大图库 Preview 变慢。
- owner reassignment 会改变 relativePath 和 OriginalDirectory，需要使旧 plan stale。
- Windows target boundary 对不存在目录和 reparse point 的处理复杂。
- Export Rule dimension 顺序变化会改变所有 targetPath，必须通过 ruleVersion 检测。
- 现有 manifest 输出逻辑不能被误认为 COPY executor。

## 14. Stop Condition

完成 E1-E9 后：

1. 运行全部 Rust、frontend、DB、snapshot、path 和 source integrity 测试。
2. 完成 Manual Verification。
3. Review diff，确认没有实现 F 的 COPY。
4. 更新 IMPLEMENTATION_STATUS.md。
5. 创建 Checkpoint E commit。
6. 停止，等待审核。
