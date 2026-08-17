# PhotoOrganizer Roadmap

> 更新日期：2026-08-17<br>当前原则：停止按竞品功能列表扩张；先把现有能力收敛为完整工作流。<br>详细评审：[Product Architecture Review](product-architecture.md)<br>执行策略：[Next-stage Product Strategy](plans/next-stage-product-strategy.md)

## Current baseline

当前代码已经具备：本地 Folder-oriented 扫描、SQLite catalog、应用私有 thumbnail/preview、EXIF、Tone/Color、SigLIP 2 摄影题材候选与 embedding、组合筛选、Favorite/Rating/Color Label、manual override、Collection、文本/相似搜索、相似聚类、重复审阅、四图比较、编辑派生副本，以及 Organization Dry-run。

这不等于所有能力均已产品化。主要缺口是：

- Library、真实 Folder 和虚拟归属边界混用；
- Grid/Search/Collection/Review 没有统一 AssetQuery；
- 批量入口没有统一 AssetScope；
- LAP-derived 工具的历史渲染器仍需继续拆分，但主界面已改为持久 Grid/Preview + 上下文工具区；
- Saved View 只有 schema；
- Organization plan 尚不是可供 Safe Copy 消费的完整不可变快照；
- Catalog 中的用户自产数据没有 Backup/Restore；
- thumbnail/vector 的大规模瓶颈尚未用 benchmark 定位。

## G-UI — LAP-derived UI Integration Remediation（当前里程碑）

### Why now

历史 `WorkflowWorkspace` 把 Browse Source、Query、Review Set 和 Selection Action 混在一个独立页面中，导致主 Grid/Preview 上下文被替换。G-UI 已将其改为主界面下方的上下文工具区；仍需桌面人工验收后才能恢复 N1 的性能基线和后续功能。

### Deliverables

- Favorite、Collection、Search 回到主界面的 source/query 入口。
- Similar、Duplicate 回到统一 Grid/Preview 的 Review Set 或右侧审阅面板。
- Compare、Batch Edit 从 Selection Action 启动。
- Edit 保留单图焦点模式；Organization 保留独立 Dry-run 工作区。
- Browse → Review → Compare/Edit → Back 保持 query、page、selection、active asset 和 scope。

### Explicit exclusions

不实现新业务能力、Saved View、Culling、Backup、Safe Copy、媒体格式扩展、性能优化或数据库 migration。

### Gate

G-UI 通过主界面状态连续性和桌面脚本验收后，才恢复 N1。

当前状态：自动化实现已通过，等待桌面人工验收；A–F 不因 G-UI 改造而自动完成。

## N1 — Workflow Foundation Consolidation（G-UI 之后）

### Why now

Saved View、Culling、Metadata Browser 和 Organization 都依赖统一查询/范围。继续加功能会扩大现有分叉。

### Deliverables

- 冻结 LibrarySource、FolderRef、Asset、legacy PhysicalFile、Collection、SavedView 术语。
- 从现有 `AssetFilter + list_assets + asset_filter_sql` 演进 `AssetQueryV1`。
- 定义 `AssetScopeInputV1` 和 `ResolvedAssetScopeV1`。
- 重新组织已有入口：Browse source、Search、Review、selection action、asset action、Organize。
- 移除产品层面的 generic “智能工作台”；不删除已有业务能力。
- 建立 thumbnail 1k/10k/50k 和 vector 1k/10k/50k/100k baseline。

### Explicit exclusions

不实现 Smart Album、Pick/Reject、Backup、HNSW、HEIC、RAW、Video、GPS、Face、Organization 新规则或 Safe Copy；不新增 migration 和生产依赖。

### Gate

两个工作流必须连续完成且上下文不丢失：

1. Browse -> Find Similar -> Compare -> mark -> back。
2. Current Query/Collection -> Organization，显示明确 scope 名称、数量和 snapshot 语义。

## N2 — Daily Photography Review

Dependencies：N1 的 AssetQuery、AssetScope 和统一 browse/review surface。

候选内容：

- 独立 Culling State：Unflagged / Pick / Reject；
- Saved View = versioned Saved AssetQuery，不建立第二套 Album；
- Metadata Browser：Camera、Lens、File Type、Resolution、Aspect Ratio、ISO、Aperture、Shutter、Focal Length、Capture Date；
- 完成现有 Compare、Similar、Duplicate、Collection 在统一 Review 流程中的整合。

Evidence gates：P/X 键盘筛片脚本、Saved Query 兼容性测试、EXIF completeness 与查询延迟。

## N3 — Catalog Protection

Dependencies：稳定的 user-data manifest、Saved Query format 和 asset relinking key。

- Backup user-authored data：Library 配置、Favorite、Rating、Color Label、Collection、Saved View、manual override、future Culling、preferences。
- 保存必要的 stable locator/fingerprint 和 confirmed operational records。
- 默认不备份 thumbnail、preview、embedding、AI label 等可再生大数据。
- Restore 必须先 preview、报告冲突、备份当前 DB，并事务执行。

完成 N3 前不开放 Organization Safe Copy。

## N4 — Immutable Organization Dry-run

Dependencies：AssetQuery + AssetScope + Metadata/User/Derived value contract。

- Scope 可来自 Current Query、Selection、Collection、Saved View 和明确的 Review set。
- OrganizationPlan 冻结 source path/fingerprint、resolved asset IDs、Effective values/revisions、rule version、target path 和 issues。
- Plan 生成后不随 Rating、Culling、Filter 或 AI 重跑自动变化。
- 数据变化使计划 stale；Safe Copy 只能消费 confirmed plan，不能重算路径。
- 本阶段仍不 mkdir/copy/move/rename/delete。

## N5 — Safe Copy

Dependencies：N3 Catalog Protection + N4 confirmed immutable plan。

- 建立唯一 FileOperationService：Plan -> Validate -> Journal -> Execute -> Verify -> Commit。
- 第一阶段只允许 COPY，目标必须不存在，禁止覆盖。
- 支持进度、取消/恢复、source fingerprint、target hash 和完整日志。
- Execute 只接收 confirmed plan ID，不接收前端 rules 或 raw paths。

## N6 — Rollback

Dependencies：N5 可审计 COPY。

- 只删除由应用生成、位于允许输出根且 hash 未变化的副本。
- 必须先 preview rollback。
- Move、Delete originals、overwrite 和原地 rename 仍不在范围。

## Evidence-gated performance lane

性能工作按证据穿插，不作为固定功能阶段。

| 领域                | 必需证据                                                                  | 可能决策                                                                                 |
| ------------------- | ------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------- |
| Thumbnail           | 1k/10k/50k：首屏、P95 frame、IPC/SQLite/decode/payload/cache/stale/memory | virtualization、batch IPC、asset protocol、cache/prefetch 中只选择被证据支持的方案       |
| Text/Similar Search | 1k/10k/50k/100k：cold/warm、load/deserialize/score/sort、P50/P95、memory  | 先 resident cache、contiguous matrix、pre-normalization、optimized dot product           |
| ANN/HNSW            | 简单优化后 100k 仍不满足交互预算，并有 index lifecycle ADR                | persistence、rebuild、insert/delete、model version、corruption recovery 全部成立后才考虑 |

## Media expansion lane

核心工作流稳定后再评估：

1. HEIC：消费者格式，独立 decoder/license/package milestone。
2. RAW：先解决 Asset -> PhysicalFile[1..n]；第一阶段只 metadata、embedded preview、viewer 和 RAW+JPEG pairing。
3. Video：独立媒体管线，不与“支持更多格式”合并。

## Capability decision summary

| 能力                            | User Value        | Dependency                       | Architecture Risk | Maintenance Cost | Evidence Required               | Roadmap     |
| ------------------------------- | ----------------- | -------------------------------- | ----------------- | ---------------- | ------------------------------- | ----------- |
| Workflow/IA consolidation       | High              | current code                     | Medium            | Medium           | usability scripts               | N1          |
| AssetQuery / AssetScope         | High              | low                              | Medium            | Medium           | call-site audit                 | N1          |
| Culling / Saved View / Metadata | High              | N1                               | Medium            | Medium           | workflow/query evidence         | N2          |
| Backup/Restore                  | High              | stable user-data contract        | High              | Medium           | restore/corruption tests        | N3          |
| Immutable Organization          | Very High         | N1/N2                            | High              | Medium           | snapshot/stale tests            | N4          |
| Safe Copy                       | Very High         | N3/N4                            | Very High         | High             | fault/no-overwrite/resume tests | N5          |
| Rollback                        | High              | N5                               | Very High         | High             | target-integrity tests          | N6          |
| HNSW                            | Unknown           | vector evidence                  | Very High         | High             | 100k benchmark                  | Unscheduled |
| HEIC/RAW/Video                  | Medium to low now | stable core + media-specific ADR | High              | High             | format fixture matrix           | Later lanes |

## Not scheduled

账号、云同步、远程后端、订阅、模型训练、大模型描述、人脸身份识别、地图、永久删除、默认移动、覆盖、原地批量重命名和 Managed Import 当前均不进入里程碑。任何加入都需要新的产品决策、依赖评审和安全边界。
