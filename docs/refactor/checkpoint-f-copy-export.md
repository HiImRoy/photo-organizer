# Checkpoint F — COPY Export

状态：NOT_STARTED

本阶段实现基于 Checkpoint E immutable Export Snapshot 的安全 COPY executor。正常 Export 只复制源文件，不移动、删除、重命名或覆盖源文件和已有目标。本阶段不实现 rollback。

## 1. Goal

交付以下能力：

- Export Job 和 Export Item 持久化。
- 严格按照 Export Snapshot 执行 COPY。
- 不重新计算分类、规则、OriginalDirectory 或 targetPath。
- 创建目标目录和目标文件时防止覆盖。
- 支持 collision-safe target creation。
- 提供进度事件。
- 支持取消。
- 保存 Export Log。
- 验证 source fingerprint 和 SourceRoot integrity。
- 支持桌面端完整 COPY 流程。

## 2. Non-goals

- 不实现 move。
- 不删除源文件。
- 不重命名源文件。
- 不覆盖已有目标。
- 不创建 hard link 或 symbolic link。
- 不实现 generated-copy rollback。
- 不实现永久删除。
- 不改变 Asset Ownership。
- 不把 Auxiliary Tag 作为默认目录维度。
- 不在 Execute 阶段重算 Export Rule。

## 3. Preconditions

- Checkpoint E 已完成并提交。
- Export Plan Snapshot 已通过 Preview 和 stale verification。
- SourceRoot、AppDataRoot、ExportRoot boundary 已可复用。
- collision 已在 Preview 阶段得到明确 targetPath。
- classificationRevision、sourceFingerprint 和 OriginalDirectory 已冻结在 Snapshot。
- 用户明确确认开始 COPY。

## 4. Architecture Invariants

- Execute 只能接收 planId 或 immutable snapshot ID。
- Execute 不接受当前 frontend rules、当前分类或 raw source path 作为替代输入。
- Execute 不重新查询并重算 targetPath。
- Execute 只做必要的安全复核：
  - source exists
  - source fingerprint unchanged
  - source root still valid
  - target boundary still valid
  - target absent
  - plan not stale
- 目标已经存在时不能 overwrite。
- 取消后已完成 COPY 保留，未执行项不继续执行。
- SourceRoot 永久只读。
- 所有状态和错误写入 AppData database/log。
- Rollback 不属于本阶段。

## 5. Current Implementation

- [src-tauri/src/organization.rs](E:/Code/Codex/photo-organizer/src-tauri/src/organization.rs)
  - 当前主要生成 read-only plan。
  - export_manifest 写出 JSON/CSV manifest，不是照片 COPY executor。
- [src-tauri/src/ipc.rs](E:/Code/Codex/photo-organizer/src-tauri/src/ipc.rs)
  - export_organization_manifest 可以写 manifest 文件。
  - 当前没有基于 immutable plan 的 COPY command。
- [src-tauri/src/db.rs](E:/Code/Codex/photo-organizer/src-tauri/src/db.rs)
  - 已有 organization plan 保存逻辑。
  - 需要新增 ExportJob、ExportItem 和执行状态持久化。
- [src-tauri/migrations/0001_initial.sql](E:/Code/Codex/photo-organizer/src-tauri/migrations/0001_initial.sql)
  - 已有 file_operations 相关历史结构，但不能直接假定它满足新的 Export Job 语义。
- [src-tauri/src/paths.rs](E:/Code/Codex/photo-organizer/src-tauri/src/paths.rs)
  - 已有 AppData log 目录。
- [src/types.ts](E:/Code/Codex/photo-organizer/src/types.ts)
  - 当前有 OrganizationPlan 类型，但没有 ExportJob/ExportItem 类型。
- [src/api.ts](E:/Code/Codex/photo-organizer/src/api.ts)
  - 当前没有 execute copy、progress stream 或 cancel export API。
- [src/components/OrganizationWorkspace.tsx](E:/Code/Codex/photo-organizer/src/components/OrganizationWorkspace.tsx)
  - 当前是只读 Organization workspace，需要在 E 的命名和 Snapshot 基础上增加 F 的执行状态。

## 6. Target State

### Domain Model

    ExportJob {
        id
        planId
        status
        totalItems
        completedItems
        failedItems
        skippedItems
        cancelledItems
        bytesCopied
        createdAt
        startedAt
        completedAt
    }

    ExportJobItem {
        jobId
        planItemId
        sourcePath
        sourceFingerprint
        targetPath
        status
        bytesCopied
        error
        startedAt
        completedAt
    }

### DB Model

Export Job 记录：

- source path、source fingerprint、target path。
- plan item reference。
- status、error、progress 和 timestamps。
- 不保存 rollback 操作，不生成 rollback API。

### React State

- exportJobId。
- exportStatus。
- completed、failed、skipped、cancelled counts。
- bytesCopied。
- currentPath。
- cancellation state。
- immutable plan status。

### IPC

目标 API：

- startCopyExport(planId)
- cancelCopyExport(jobId)
- getExportProgress(jobId)
- subscribeExportProgress(jobId)
- getExportLog(jobId)

startCopyExport 不接受可重新计算规则的 frontend payload。

### Rust Data Flow

    planId
      ↓
    load immutable snapshot
      ↓
    verify stale/source/boundary/target
      ↓
    create ExportJob
      ↓
    COPY each exact snapshot item
      ↓
    persist item status and log
      ↓
    emit progress

### UI Behavior

- 用户从 Export Preview 明确点击 COPY。
- 执行前显示 Snapshot summary 和安全复核结果。
- 执行过程中显示 progress、current item、failed/skipped。
- Cancel 后停止取下一个 item。
- 不显示 rollback 操作。
- 源目录没有任何写入按钮。

## 7. Detailed Implementation Steps

### F1 — Export Job Schema

Goal：建立 COPY Job 和 Item 的持久化模型。

- Files to change：[src-tauri/src/models.rs](E:/Code/Codex/photo-organizer/src-tauri/src/models.rs)、[src-tauri/src/db.rs](E:/Code/Codex/photo-organizer/src-tauri/src/db.rs)；未来新增 0010 migration。
- DB/schema impact：创建 export_jobs、export_job_items 和必要的 log/status fields；不创建 rollback tables。
- API impact：增加 Job response、progress response 和 status enums。
- React state impact：增加 ExportJob state，但不改变 immutable plan state。
- Rust/domain impact：Job 只引用 plan snapshot，Item 复制 source/target/fingerprint 作为执行审计。
- Tests to add/update：schema round-trip、job status transition、plan reference、no rollback schema。
- Completion condition：创建 Job 后可以恢复其状态，数据库记录在 AppData。
- Dependency：Checkpoint E 完成。

### F2 — Safe COPY Executor

Goal：严格按照 Snapshot 执行单项 COPY。

- Files to change：新增 export executor module；修改 [src-tauri/src/ipc.rs](E:/Code/Codex/photo-organizer/src-tauri/src/ipc.rs)、[src-tauri/src/db.rs](E:/Code/Codex/photo-organizer/src-tauri/src/db.rs)、[src-tauri/src/paths.rs](E:/Code/Codex/photo-organizer/src-tauri/src/paths.rs)。
- DB/schema impact：每个 item 写入 started/completed/failed 状态。
- API impact：startCopyExport 只接收 planId。
- React state impact：启动后切换为 Export Job progress state。
- Rust/domain impact：读取 Snapshot sourcePath，复核 fingerprint，创建安全目标文件；目标存在时拒绝覆盖。
- Tests to add/update：copy fixture、missing source、changed source、existing target、Unicode path、source read-only。
- Completion condition：单项 COPY 成功或产生明确失败，不会写入 SourceRoot。
- Dependency：F1 完成。

### F3 — Collision-safe Target Creation

Goal：执行时处理 Preview 后发生的目标竞争，不改变 Snapshot targetPath。

- Files to change：export executor module、[src-tauri/src/organization.rs](E:/Code/Codex/photo-organizer/src-tauri/src/organization.rs)、[src-tauri/src/db.rs](E:/Code/Codex/photo-organizer/src-tauri/src/db.rs)。
- DB/schema impact：Item 保存 skipped_conflict 或 failed，而不是改写 targetPath。
- API impact：返回 target already exists、boundary changed 等明确 error。
- React state impact：UI 显示 skipped/failed，不隐式选择新名称。
- Rust/domain impact：使用 create_new 或等价 no-overwrite 原子创建；Execute 不调用 collision planner 生成新路径。
- Tests to add/update：race target creation、existing target、same plan target、partial target creation。
- Completion condition：Execute 不覆盖目标，不重新计算 collision strategy。
- Dependency：F2 完成。

### F4 — Progress Events

Goal：提供可恢复、可观察的 COPY 进度。

- Files to change：[src-tauri/src/models.rs](E:/Code/Codex/photo-organizer/src-tauri/src/models.rs)、[src-tauri/src/ipc.rs](E:/Code/Codex/photo-organizer/src-tauri/src/ipc.rs)、新增 progress event 逻辑、[src/api.ts](E:/Code/Codex/photo-organizer/src/api.ts)。
- DB/schema impact：周期性保存 item/job progress；避免每个字节写一次 DB。
- API impact：subscribeExportProgress 或等价事件。
- React state impact：更新 total、completed、failed、skipped、bytes 和 currentPath。
- Rust/domain impact：进度事件只能描述 Snapshot item，不产生新 target path。
- Tests to add/update：event ordering、job restart read、zero item、large item progress。
- Completion condition：UI 进度与 Job DB 状态最终一致。
- Dependency：F2、F3 完成。

### F5 — Cancellation

Goal：支持用户取消未完成 COPY。

- Files to change：[src-tauri/src/ipc.rs](E:/Code/Codex/photo-organizer/src-tauri/src/ipc.rs)、export executor、[src/api.ts](E:/Code/Codex/photo-organizer/src/api.ts)、[src/components/OrganizationWorkspace.tsx](E:/Code/Codex/photo-organizer/src/components/OrganizationWorkspace.tsx)。
- DB/schema impact：Job 状态增加 cancelling/cancelled；Item 未执行项保持 pending/cancelled。
- API impact：cancelCopyExport(jobId)。
- React state impact：显示 cancelling，禁用重复启动，完成后显示 partial result。
- Rust/domain impact：在 item 边界和可安全检查的位置响应取消；不删除已完成目标，不继续取下一个 item。
- Tests to add/update：cancel before start、cancel between items、cancel during large copy、idempotent cancel。
- Completion condition：取消后不会继续复制新 item，已完成输出保留，源文件不变。
- Dependency：F4 完成。

### F6 — Export Log

Goal：记录每个 Job 和 Item 的执行审计。

- Files to change：[src-tauri/src/db.rs](E:/Code/Codex/photo-organizer/src-tauri/src/db.rs)、export executor、[src-tauri/src/paths.rs](E:/Code/Codex/photo-organizer/src-tauri/src/paths.rs)、[src-tauri/src/ipc.rs](E:/Code/Codex/photo-organizer/src-tauri/src/ipc.rs)。
- DB/schema impact：保存 source、target、fingerprint、status、error、bytes 和 timestamps。
- API impact：getExportLog(jobId)。
- React state impact：完成后显示可读的成功、失败、跳过和取消日志。
- Rust/domain impact：日志写入 AppData，不写 SourceRoot；错误不吞掉。
- Tests to add/update：success log、failure log、cancel log、restart recovery、Unicode paths。
- Completion condition：每个 Snapshot item 都有最终执行状态。
- Dependency：F2-F5 完成。

### F7 — Source Integrity Verification

Goal：在 COPY 前后验证 SourceRoot 完整性。

- Files to change：export executor、[src-tauri/src/db.rs](E:/Code/Codex/photo-organizer/src-tauri/src/db.rs)、source identity/path module、Rust integration tests。
- DB/schema impact：Item 保存验证结果和 fingerprint mismatch error。
- API impact：Job summary 显示 source integrity failure。
- React state impact：源 fingerprint 变化时阻止该 item，并明确提示重新 Preview。
- Rust/domain impact：执行前检查 source exists、boundary 和 fingerprint；COPY 后不修改 source，必要时再次读取 metadata/hash。
- Tests to add/update：source changed after preview、source missing、source mtime change、content hash mismatch。
- Completion condition：任何 source 变化不会被静默复制。
- Dependency：F2-F6 完成。

### F8 — Desktop End-to-End Test

Goal：验证从 Preview 到 COPY、Progress、Cancel 和 Log 的完整桌面流程。

- Files to change：[src/components/OrganizationWorkspace.tsx](E:/Code/Codex/photo-organizer/src/components/OrganizationWorkspace.tsx)、[src/App.tsx](E:/Code/Codex/photo-organizer/src/App.tsx)、[src/api.ts](E:/Code/Codex/photo-organizer/src/api.ts)、desktop/Rust tests。
- DB/schema impact：使用 test-data/ fixture DB 和 ExportRoot。
- API impact：验证 start/cancel/progress/log。
- React state impact：验证 plan stale、job progress、partial cancellation 和 final log。
- Rust/domain impact：验证 COPY-only、no overwrite、source integrity 和 AppData log。
- Tests to add/update：complete copy、collision、cancel、failure、Unicode path、parent recursive export。
- Completion condition：所有 Exit Criteria 和 Manual Verification 通过。
- Dependency：F1-F7 全部完成。

## 8. Migration Strategy

本阶段规划未来 Export Job migration，但当前不创建文件。

### Planned 0010 — Export Jobs

- 迁移前备份 SQLite。
- 创建 export_jobs。
- 创建 export_job_items。
- 建立 plan_id、plan_item_id、job status、item status indexes。
- 保留 sourceFingerprint、targetPath 和 error audit。
- 不创建 rollback tables、rollback commands 或 rollback UI。
- 旧 file_operations 表不直接解释为新的 Export Job，除非经过字段和语义审计。
- migration 失败回滚事务，恢复数据库备份。
- schema 采用 forward-only。

### Execution failure policy

- 单项失败记录 item failed，Job 继续或按明确策略停止；策略必须在 Job 中可见。
- 目标已存在不能覆盖。
- 取消不删除任何已生成目标。
- 源文件失败不改变源文件状态。
- 不提供 rollback。

## 9. Automated Tests

### Rust unit

- Snapshot item exact execution。
- source and target boundary。
- no-overwrite creation。
- fingerprint verification。
- cancellation state machine。
- progress aggregation。

### Rust integration

- COPY fixture。
- Parent recursive scope plan。
- Unicode source and target。
- existing target。
- source changed after Preview。
- cancel partial job。
- restart and log recovery。

### Frontend

- start COPY from planId。
- progress display。
- cancel interaction。
- final log。
- stale plan blocked。
- no rollback controls。

### DB migration

- Export Job schema。
- job/item status transitions。
- plan snapshot reference。
- failure recovery。
- no rollback tables。

### Source integrity

- source hash before/after identical。
- source mtime and directory entries unchanged。
- target writes only inside validated ExportRoot。

### Evaluation

本阶段不评估模型质量；评估 COPY correctness、failure visibility、throughput 和 cancellation latency。

## 10. Manual Verification

1. 在 Export Preview 创建无错误 Snapshot。
   - 预期：plan status ready。
2. 点击 COPY。
   - 预期：创建 Job，按照 Snapshot targetPath 复制。
3. 观察 Progress。
   - 预期：completed、bytes、currentPath 和失败数实时更新。
4. 检查目标目录。
   - 预期：只产生 Snapshot 中的目标文件。
5. 检查源目录。
   - 预期：源文件仍存在，hash 和 mtime 不变。
6. 制造已有目标文件。
   - 预期：不覆盖，Item 记录 skipped 或 failed。
7. Preview 后修改 Asset 分类。
   - 预期：旧 plan stale，COPY 被阻止。
8. Preview 后修改源文件或删除源文件。
   - 预期：fingerprint/missing 检查阻止对应 Item。
9. 开始一个包含多个大文件的 Job 后点击 Cancel。
   - 预期：已完成文件保留，未执行文件停止，Job 显示 cancelled/partial。
10. 查看 Export Log。
    - 预期：每个 Item 都有明确状态和错误信息。
11. 检查 UI。
    - 预期：没有 rollback、move、delete 或 overwrite 操作。

## 11. Exit Criteria

- [ ] Export Job 和 Export Item 可持久化。
- [ ] Execute 只接收 immutable planId。
- [ ] Execute 不重新计算分类、OriginalDirectory、规则或 targetPath。
- [ ] COPY 不修改 SourceRoot。
- [ ] COPY 不覆盖已有目标。
- [ ] boundary、source exists、fingerprint 和 stale checks 全部生效。
- [ ] collision race 有明确失败或跳过状态。
- [ ] Progress events 和 DB 状态最终一致。
- [ ] Cancel 能停止未执行项目且不删除已完成输出。
- [ ] Export Log 覆盖每个 Item。
- [ ] 不存在 rollback 实现或 UI。
- [ ] Rust、frontend、DB、source integrity 和 desktop tests 通过。
- [ ] Manual Verification 全部通过。
- [ ] 创建独立 Checkpoint F commit。

## 12. Expected Files To Change

- [src/types.ts](E:/Code/Codex/photo-organizer/src/types.ts)
- [src/api.ts](E:/Code/Codex/photo-organizer/src/api.ts)
- [src/App.tsx](E:/Code/Codex/photo-organizer/src/App.tsx)
- [src/components/OrganizationWorkspace.tsx](E:/Code/Codex/photo-organizer/src/components/OrganizationWorkspace.tsx)
- [src-tauri/src/models.rs](E:/Code/Codex/photo-organizer/src-tauri/src/models.rs)
- [src-tauri/src/db.rs](E:/Code/Codex/photo-organizer/src-tauri/src/db.rs)
- [src-tauri/src/ipc.rs](E:/Code/Codex/photo-organizer/src-tauri/src/ipc.rs)
- [src-tauri/src/organization.rs](E:/Code/Codex/photo-organizer/src-tauri/src/organization.rs)
- [src-tauri/src/paths.rs](E:/Code/Codex/photo-organizer/src-tauri/src/paths.rs)
- 新增 COPY/export executor module。
- 未来新增 0010 Export Job migration；本次不创建。

## 13. Risks

- COPY executor 的目标创建与 Windows 文件系统竞争可能产生 race。
- 大文件 COPY 的 cancellation 只能在安全边界响应，不能保证任意字节级中断。
- source fingerprint 计算成本可能影响大量导出。
- 旧 file_operations 语义可能与新 Export Job 不兼容。
- 部分完成后用户可能误以为全部成功，UI 必须显示精确统计。
- 不实现 rollback 后，用户需要通过目标目录和日志自行处理生成副本。

## 14. Stop Condition

完成 F1-F8 后：

1. 运行全部 Rust、frontend、DB、COPY、progress、cancel 和 source integrity 测试。
2. 完成 Manual Verification。
3. Review diff，确认没有加入 rollback。
4. 更新 IMPLEMENTATION_STATUS.md。
5. 创建 Checkpoint F commit。
6. 停止，等待审核。
