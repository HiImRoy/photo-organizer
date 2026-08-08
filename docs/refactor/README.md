# PhotoOrganizer Refactor Implementation Runbook

状态：Architecture Plan 已冻结，Checkpoint A-F 尚未开始。

本目录把最终 Revised Architecture Plan 转换为可以逐阶段执行、测试、人工验证和提交的实施 Runbook。文档本身不代表实现已经完成；所有 Checkpoint 都必须按顺序执行，并在完成后停止等待审核。

## 冻结的核心架构原则

1. Source Filesystem 永久只读。扫描、分析、浏览和普通预览不得修改源目录。
2. 只有用户显式导入的目录才建立 Library。
3. 已显式导入的 Library 之间，根据 SourcePath 的最近已导入祖先自动建立 hierarchy。
4. Parent-first 与 Child-first 必须收敛到同一个 Library hierarchy 和 Asset ownership 结果。
5. 一个物理源文件只有一个逻辑 Asset。
6. Asset owner 是匹配源文件的 Most Specific Imported Library。
7. 点击 Parent Library 时，浏览范围包含 current Library 和全部 descendant Libraries。
8. Parent Browse Scope 与 Parent Scan Ownership Scope 完全分离。
9. Parent Scan 遇到显式 Child Library SourceRoot 时必须 prune subtree。
10. Auto、Manual、Effective Classification 三层分离。
11. 所有 Derived Categorical Classification 都可以人工修正。
12. 所有分类显示、筛选和 Export 都使用 Effective Classification。
13. activeAssetId 是当前图片状态的唯一来源；不得继续维护 previewAssetId。
14. Export 必须遵循 Preview → Immutable Snapshot → COPY Execute。
15. OriginalDirectory 是可选 Export Dimension，但绝不是 Sidebar、Filter 或 Library hierarchy。
16. Auxiliary Tag 默认不作为 Export Directory Dimension。

## 最终产品规则

### Library browse scope

点击任意 Library 的默认范围是：

    当前 Library + 所有 descendant Libraries

该范围通过 Library hierarchy 查询实现，不复制 Asset，也不修改 Asset 的最具体 owner。Sidebar 主数量同样使用 recursive count，保证节点显示数量与点击后的结果数量一致。

### Library scan scope

Parent Rescan 只维护 Parent 自己的 ownership scope。扫描遇到任何显式 descendant SourceRoot 时直接 prune，不继续遍历，也不隐式触发 Child Rescan。

### OriginalDirectory

OriginalDirectory 只属于 Export Rule Context。它使用 Asset 相对于当前 owner Library SourceRoot 的完整 relative parent path，可与分类、日期等 Export Dimension 组合，但不进入 Sidebar、FilterState 或 Library hierarchy。

## 执行顺序

```text
Checkpoint A
    ↓ 审核、测试、commit、停止
Checkpoint B
    ↓ 审核、测试、commit、停止
Checkpoint C
    ↓ 审核、测试、commit、停止
Checkpoint D
    ↓ 审核、测试、commit、停止
Checkpoint E
    ↓ 审核、测试、commit、停止
Checkpoint F
```

禁止一次执行多个 Checkpoint。每个 Checkpoint 内也必须按照对应文档中的步骤顺序执行。

## 冲突处理

如果真实代码与 Runbook 或冻结 Architecture Plan 冲突，必须停止实现并报告：

- 实际代码是什么。
- Runbook 假设是什么。
- 冲突会影响哪些 domain、schema、IPC 或 UI。
- 推荐修改方案和替代方案。

不得为了适配现有代码而自行改变核心产品规则。

## 安全和测试边界

- 文件系统测试只能使用仓库 test-data/ 下的 fixture 或其隔离运行目录。
- 不得操作用户真实照片目录。
- 不得修改、覆盖或删除源文件。
- 所有导出测试必须验证源文件 fingerprint 或 hash 未变化。
- 需要 schema 时，先备份 SQLite 数据库；迁移采用 forward-only 策略。
- 本 Runbook 阶段不创建任何 Migration 文件，除非对应 Checkpoint 已获批准并开始执行。

## Checkpoint 文档

- [Checkpoint A — Library Safety](E:/Code/Codex/photo-organizer/docs/refactor/checkpoint-a-library-safety.md)
- [Checkpoint B — Classification and Filter](E:/Code/Codex/photo-organizer/docs/refactor/checkpoint-b-classification-filter.md)
- [Checkpoint C — Preview](E:/Code/Codex/photo-organizer/docs/refactor/checkpoint-c-preview.md)
- [Checkpoint D — Semantic and Color](E:/Code/Codex/photo-organizer/docs/refactor/checkpoint-d-semantic-color.md)
- [Checkpoint E — Export Preview](E:/Code/Codex/photo-organizer/docs/refactor/checkpoint-e-export-preview.md)
- [Checkpoint F — Copy Export](E:/Code/Codex/photo-organizer/docs/refactor/checkpoint-f-copy-export.md)
- [Implementation Status](E:/Code/Codex/photo-organizer/docs/refactor/IMPLEMENTATION_STATUS.md)

## 当前范围外的事项

以下仍不是本轮实现范围：

- Auxiliary Tag 多目录 Export Policy。
- Arbitrary Library Group / Collection。
- 外部移动或重命名 Source Folder 后的自动追踪。
- 生成副本 rollback。
- 云同步、账号、远程后端、视频、专业 RAW、人脸身份识别和模型训练。
