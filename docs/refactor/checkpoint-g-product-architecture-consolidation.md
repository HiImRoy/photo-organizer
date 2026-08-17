# Checkpoint G — Product Architecture Consolidation

状态：IMPLEMENTED_PENDING_MANUAL；G-UI 已实现，N1 仍暂停等待桌面验收

日期：2026-08-10

本 Checkpoint 将旧 Checkpoint A–F、Plan 0018/0019、当前真实代码和本次 Product Architecture Review 合并为继续工作基线。它不是第七个顺序开发阶段，也不关闭 A–F。前一轮 N1 只保留查询契约的技术价值；把 LAP-derived 功能改名后继续放在独立工作台的 IA 结果不接受，必须先执行 [Plan 0021](../plans/0021-lap-ui-integration-remediation.md)。

## 1. Source baseline

- Git HEAD：`3fb17ed feat: complete photo organizer workflow refinements`
- 评审时工作区已有尚未提交的导入/语义吞吐修正：`docs/architecture.md`、`docs/plans/0019-import-and-analysis-throughput.md`、`docs/testing.md`、`src-tauri/src/db.rs`、`scanner.rs`、`semantic_tasks.rs`、`workflow.rs`。
- 本 Checkpoint 不回滚或改写这些既有改动。
- LAP 对照：`julyx10/lap` commit `4d0960f`；仅作产品/工作流/性能概念研究，未复制 GPL 实现。

## 2. Review scope

本轮审查、旧状态复核与 UI 纠偏基线完成：

- 真实 capability inventory；
- current/recommended domain model；
- Query/Scope architecture；
- user-authored vs derived data classification；
- workflow/IA review；
- LAP comparison；
- thumbnail/vector benchmark strategy；
- dependency/evidence-gated roadmap；
- 唯一 Next Milestone 选择；
- `AssetQueryV1`、当前 scope 描述的技术切片保留，但旧工作台 IA 被判定为未通过；
- 明确 LAP-derived 功能必须回到主界面的 source/query/review/action 上下文。

本轮没有实现 Smart Album、Pick/Reject、Backup、HNSW、HEIC、RAW、Video、GPS、Face、Organization 新功能或 Safe Copy。

随后按本 Checkpoint 选定的唯一里程碑执行了 Plan 0021；G-UI 的主界面整合已实现，当前仅剩桌面端人工验收，不改变 A–F 的独立状态。

## 3. A–F reconciled status

| Checkpoint                                   | 旧状态记录                                       | 当前真实代码                                                                                                                          | G 的结论                                                                                      |
| -------------------------------------------- | ------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------- |
| A — Source Boundary + Nested Library         | `BLOCKED_FOR_REVIEW`；文件头仍误写 `NOT_STARTED` | 0005–0008 和自动化已实现；人工 Parent/Child 流程未完成。后续又加入 manual Library parent 和 virtual asset assignment。                | 保留安全基础，但物理 Library 与虚拟组织边界需要重新审查；不能标 Completed。                   |
| B — Manual Classification + Effective Filter | `IMPLEMENTED_PENDING_MANUAL`                     | 0009、manual override、Effective resolver、UI/SQL 已存在。                                                                            | 保持 Pending Manual；作为 Query predicate 复用。                                              |
| C — Preview                                  | `NOT_STARTED`                                    | 已有 screen/original preview、filmstrip、zoom/Navigator 和 stale guard；仍同时维护 `activeAssetId`/`previewAssetId`。                 | `PARTIAL_REQUIRES_RECONCILIATION`，不按旧状态假装未实现或已完成。                             |
| D — Semantic + Dominant Color                | `NOT_STARTED`                                    | 历史 TinyCLIP 与当前 SigLIP 2 均有过缩略图 batch 记录；Tone/Color、manual override 已存在，完整质量评测/版本 Exit Criteria 未全验收。 | `PARTIAL_REQUIRES_RECONCILIATION`。                                                           |
| E — Export Preview                           | `NOT_STARTED`                                    | Organization Dry-run、rules、mapping、issues、manifest 已存在。DB snapshot 不含完整执行事实，plan retrieval 不返回 items。            | `PARTIAL_REQUIRES_RECONCILIATION`；未来重命名为 immutable Organization Dry-run。              |
| F — COPY Export                              | `NOT_STARTED`                                    | Organization COPY 未实现；但 Edit 已独立实现派生 copy 和 rollback。                                                                   | Organization F 仍未开始；现有 Edit file mutation 必须在未来统一 FileOperationService 中收编。 |

旧 Checkpoint 文档继续作为历史约束和详细测试清单，不再作为机械的唯一执行顺序。任何旧 Exit Criteria 未通过的阶段都不能因功能“看起来存在”而标 Completed。

## 4. Inherited invariants

以下承诺从 A–F 保留并提升为产品级不变量：

1. SourceRoot 在浏览、扫描、分析、查询和规划阶段永久只读。
2. 原图、真实 Folder、LibrarySource 与虚拟组织必须分层。
3. 一个逻辑 Asset 是 Favorite/Rating/Culling/Collection/override 的稳定锚点。
4. Auto、Manual、Effective classification 分离；User decision 优先。
5. Parent browse scope 与 physical scan ownership scope 分离。
6. 所有 query 的 count/page/result 必须语义一致。
7. 批量操作必须接收明确 AssetScope。
8. Organization 遵循 Scope -> Rule -> Immutable Plan -> Review -> Confirm。
9. Safe Copy 只能消费 confirmed plan；执行时不重算 query、classification、rule 或 target path。
10. 文件写操作遵循 Plan -> Validate -> Journal -> Execute -> Verify -> Commit。
11. 第一阶段文件输出只允许 copy/no-overwrite；Move/Delete/overwrite 更晚。
12. 测试只使用 `test-data/` 或隔离临时 fixture，并验证源 hash/mtime/目录项。
13. 导入、图像解码、基础特征、题材/环境/主体分析和模型推理只使用应用私有缩略图或有界缩略图派生输入；原文件仅允许参与元数据、指纹、EXIF/内嵌预览元数据和受控的目标尺寸缩略图提取。显式 `original` 只属于用户主动查看的预览例外。

## 5. New findings

### 5.1 Physical/Catalog boundary conflict

- `Library` 是 SourceRoot，但 `parentLibraryId` 可手工排列，不再必然对应真实父目录。
- `asset_library_assignments` 可把图片显示到另一 Library 而不移动物理文件。
- 真正的 Folder summary API 存在但没有进入主 UI/query。

决定：N1 先冻结术语和兼容行为；不立即 migration。停止把 asset-to-library assignment 作为未来 Folder-first 模式扩展。

### 5.2 Query fragmentation

- Grid/filters 走 `AssetFilter + asset_filter_sql`。
- Favorites/Collections/Duplicates 分别走 workflow SQL。
- Search/Similar 走 embedding BLOB 全量加载和内存排序。
- Saved View 仅有 schema。
- Organization 只懂 all/filtered/selected。

决定：从现有 SQL builder 演进 `AssetQueryV1`，允许 semantic ranking 使用专门 stage，但统一 result contract。

### 5.3 Scope fragmentation

Selection、current page、current filter、collection 和 review results 没有统一解析。决定引入 `AssetScopeInputV1` 与 `ResolvedAssetScopeV1`，Organization 在 plan 生成时冻结 resolved items/fingerprints。

### 5.4 IA fragmentation

`WorkflowWorkspace` 把 Favorite、Collection、Search、Duplicate、Similar、Compare、Edit、Faces 打包。它打开时替换 Library 三栏；点击结果又退出工作台。决定取消 generic “智能工作台”作为产品概念，把既有能力重组为 browse source、query、review、selection action、asset action 和 Organize。

之前只把入口改名为“查找与审阅”并没有解决问题，仍然是独立容器和七个平级 tab。因此 N1 的 IA 子步骤不通过；source/query/review/action 的拆分改由 G-UI 执行。主界面必须持续保留 Grid/Preview、Selection 和 DetailPanel；只有 Organization Dry-run，及单图编辑的焦点模式，可以保留独立工作区。

### 5.5 Snapshot/file-operation conflict

Organization DB 记录是 compact audit snapshot，migration 注释明确允许重算；这不满足未来 Safe Copy。Edit copy/rollback 则已经建立另一套真实文件操作。决定先修 Organization snapshot contract，再建立唯一 FileOperationService；本轮不改代码。

### 5.6 Performance evidence gap

- Import 有小型数据；TinyCLIP 基准仅为历史记录，当前 SigLIP 2 仍需独立性能证据。
- Thumbnail browsing 没有 1k/10k/50k 数据。
- Vector query 没有 1k/10k/50k/100k 分段数据，当前还存在 10,001/5,000 hard cap。

决定：benchmark 先于 virtualization、batch IPC、asset protocol、resident cache 或 HNSW。

## 6. User-data boundary

### Irreplaceable

Library 配置、manual hierarchy、asset assignments、Rating、Color Label、Favorite、Collections、Saved Views、manual classification/tag overrides、edit recipes/preferences、未来 Culling，以及 confirmed plan/file operation audit。

### Regenerable

Asset metadata projection、thumbnail、preview、Tone/Color、semantic labels/embeddings、similarity/duplicate candidates、face derived data、analysis jobs。

注意：虽然 Asset catalog 可重扫，恢复 user data 仍需要 source identity + relative path + fingerprint 等稳定 locator；只备份 user table 而不备份重连信息是不完整的。

决定：Catalog Backup/Restore 排 N3/P1，晚于 N1 工作流基础，早于 Safe Copy。

## 7. Selected next milestone

当前唯一下一里程碑：`G-UI — LAP-derived UI Integration Remediation`。

包含：

- 拆分 Browse Source、Query、Review Set、Selection/Asset Action；
- 将 Search、Favorite、Collection、Similar、Duplicate、Compare、Edit 接回主 Grid/Preview 上下文；
- 保留并复用 `AssetQueryV1`、`AssetScopeInputV1`，不再扩展旧工作台；
- 完成主界面状态连续性和桌面用户脚本验收。

它不包含任何新的业务能力、benchmark 优化或 schema migration。详细范围、验收和风险见 [Plan 0021](../plans/0021-lap-ui-integration-remediation.md)。G-UI 通过后，才恢复 N1 的性能基线和后续工作。

## 8. N1 approval gate

用户已要求修复 LAP-derived 功能的界面整合；已创建 [Plan 0021](../plans/0021-lap-ui-integration-remediation.md)。此前对 N1 的批准不等于批准继续维护旧工作台。剩余 gate 为：

- 说明 `AssetQueryV1` 最小字段和版本兼容；
- 列出每个现有功能属于 source/query/review/action 哪一类；
- 完成主界面集成和返回上下文用户脚本，不只重命名按钮；
- 明确 Organization 和单图 Edit 为允许保留的独立焦点界面；
- 预期无 migration/production dependency，若发现需要则停止并提交 ADR。

## 9. Review artifacts

- [Product Architecture Review](../product-architecture.md)
- [Revised Roadmap](../roadmap.md)
- [Next-stage Product Strategy](../plans/next-stage-product-strategy.md)
- [Plan 0020 — Workflow Foundation Consolidation](../plans/0020-workflow-foundation-consolidation.md)
- [Plan 0021 — LAP-derived UI Integration Remediation](../plans/0021-lap-ui-integration-remediation.md)
- 旧 [Refactor Runbook](README.md) 和 A–F Checkpoints 作为历史材料保留。

## 10. Verification for this checkpoint

本轮已完成 G-UI 自动化实现切片；桌面端人工验收仍未完成：

- [x] `npm run format:check`
- [x] 旧 A–F 状态与真实代码人工复核
- [x] LAP 主界面集成方式对照复核
- [x] G-UI 计划、范围和验收条件建立
- [x] `git diff --check`
- [x] G-UI 前端实现
- [x] 嵌入工具区移除七个平级工作台 Tab，改为由当前来源、查询、选择或图片上下文单独打开
- [ ] G-UI 桌面验收
- [x] `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check`
- [x] `cargo test --manifest-path src-tauri/Cargo.toml --no-default-features --all-targets --quiet`
- [x] `cargo clippy --manifest-path src-tauri/Cargo.toml --no-default-features --all-targets -- -D warnings`
- [x] 链接和状态人工复核
- [x] 确认本次切片没有 migration、生产依赖或真实源文件写操作；工作区原有吞吐修正保持不动

因此本 Checkpoint 当前记为 `IMPLEMENTED_PENDING_MANUAL`；不得标记 N1_COMPLETE，也不得把 A–F 标记为完成。

## 11. Unresolved risks

- 旧 A/B 人工验证仍未完成。
- 旧 A 的 “Most Specific physical owner” 与后来 manual hierarchy/asset assignment 的产品语义冲突。
- `activeAssetId`/`previewAssetId`、多套 DTO 和多套 result list 会增加 N1 重构风险。
- Edit copy/rollback 已成为独立文件操作实现，未来统一边界需要迁移策略。
- `styles.css` 存在多层重复 selector/cascade，IA 拆分时容易出现视觉回归。
- Saved View 的 schema 没有 query version/checksum；开放写入前必须先确定 contract。
- 性能 baseline 尚未执行；当前不具备引入 thumbnail batch、virtualization、resident vector cache 或 HNSW 的证据。
- `WorkflowWorkspace` 的历史渲染器仍作为上下文工具内部复用；它不再替换主界面，但如果人工验收发现工具区仍过于拥挤，下一步只做拆分和布局修复，不恢复独立工作台。
- G-UI 的 collection source 使用 `AssetFilter.collectionId`，需要在桌面脚本中验证父图库/子图库 scope 与集合成员计数一致。

## 12. Stop condition

当前已完成 G-UI 的自动化实现切片，但尚未通过桌面人工验收。下一步只允许执行 G-UI 验收与回归修复；在 G-UI 验收前不恢复 N1 benchmark，不进入 N2、Backup、Immutable Organization 或 Safe Copy。
