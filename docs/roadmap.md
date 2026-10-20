# PhotoOrganizer Roadmap

> 更新日期：2026-10-20<br>当前原则：停止按竞品功能列表扩张；先把现有能力收敛为完整工作流。<br>详细评审：[Product Architecture Review](product-architecture.md)<br>执行策略：[Next-stage Product Strategy](plans/next-stage-product-strategy.md)

## 2026-10 核心 MVP 闭环实施（主要实现已集成；最终验证中）

ADR-0015 的设计于 2026-10-19 接受。主要前后端实现已集成，隔离双来源 scan→plan→copy→rollback 与 recovery 定向回归已运行；后端 all-target 仍有 1 项 Windows symlink OS 1314 失败，完整桌面交互未验收。规范启动器与 `-CheckOnly` 已在隔离测试路径通过；自动化或 headless 检查通过不代表阶段验收、Recovery Gate 或完整 MVP 已通过。详见[主方案](plans/0083-core-mvp-filter-preview-folder-export.md)、[ADR-0015](decisions/0015-unified-query-and-safe-copy-export.md)、[技术附录](plans/0083-core-mvp-technical-appendix.md)与[本机实施记录](handoff/2026-10-20.md)。

| 阶段              | 当前状态                                                               | 验收状态                                                                                                                                                  |
| ----------------- | ---------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1. 筛选与看图     | 已实现；最终前端 175 项测试及 lint/typecheck/build 通过。              | all-target Rust 尚有 1 项 Windows symlink 环境失败；原生桌面流程待验收。                                                                                  |
| 2. 导出计划       | 已实现持久完整 plan 与分页 reader；隔离端到端流程已验证。              | scan→plan→copy→rollback 隔离流程通过；all-target Rust 尚有 1 项环境失败，桌面复核待完成。                                                                 |
| 3. 安全复制与恢复 | 已实现 job/recovery/copy；双来源隔离 scan→plan→copy 通过且来源未改变。 | helper 12 / runner 4 / service 11 项 targeted tests 通过；directory junction reparse 拒绝测试通过；all-target Rust 216/217，file symlink OS 1314 待解决。 |
| 4. 撤销与桌面验收 | 受限 rollback 与预览后完整性检查已实现；对应 targeted tests 通过。     | rollback integrity targeted tests 通过；headless 共享控件几何已检查，原生桌面工作流尚未验收。                                                             |

旧 roadmap 的“完成完整 N3 产品前不开放 Safe Copy”历史门禁，已由接受的 ADR-0015 缩小为通过最小 Catalog Recovery Gate 后才可开放 Safe Copy。门槛实现已接入，但 all-target 有一项 Windows symlink OS 1314 失败，最终门槛验收待完成；桌面交互也未通过。完整 N3 用户备份/恢复 UI 与跨设备 relinking 仍后置；这不代表 N3 整体完成，并保留磁盘/AppData/设备损坏时缺少自助恢复的风险。完整阶段定义见 0083 主方案。

## Current baseline

当前代码已经具备：本地 Folder-oriented 扫描、SQLite catalog、应用私有 thumbnail/preview、EXIF、Tone/Color、SigLIP 2 摄影题材候选与 embedding、组合筛选、Favorite/Rating/Color Label、manual override、Collection、文本/相似搜索、相似聚类、重复审阅、四图比较、编辑预览与 recipe 保存、完整 query/group/scope、不可变组织计划及 Safe Copy job/recovery/受限 rollback 实现。

这不等于所有能力均已产品化。主要缺口是：

- Library、真实 Folder 和虚拟归属边界混用；
- 完整 UI/浏览交互尚未通过 Windows 桌面人工验收；
- LAP-derived 工具的历史渲染器仍需继续拆分，但主界面已改为持久 Grid/Preview + 上下文工具区；
- Saved View 只有 schema；
- Catalog 中的用户自产数据没有 Backup/Restore；
- thumbnail/vector 的大规模瓶颈尚未用 benchmark 定位。

编辑预览与 recipe 保存仍可用；旧的全尺寸编辑派生导出和 legacy rollback 路径已封闭，UI 显示禁用原因。导入、分析及模型路径必须保持 thumbnail-only；唯一允许的原图像素读取是用户在当前 viewer 明确请求 original preview，且该结果不得复用于其他处理。

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

当前状态：自动化前端检查已通过，等待桌面人工验收；导航 rail 外壳透明无边框、共享圆角控件几何、混合按钮组与窄宽度等最新界面契约尚未完成视觉验收；A–F 不因 G-UI 改造而自动完成。

## N1 — Workflow Foundation Consolidation（G-UI 之后）

### Why now

Saved View、Culling、Metadata Browser 和 Organization 都依赖统一查询/范围。继续加功能会扩大现有分叉。

### Deliverables

- 冻结 LibrarySource、FolderRef、Asset、legacy PhysicalFile、Collection、SavedView 术语。
- 0083 已实现统一 query/scope 的核心消费路径，包括全量计数、分组、membership 与 neighbor 查询；更广的术语和来源模型整理仍需单独评估。
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

历史门禁（须完成完整 N3 产品后才开放 Safe Copy）已被 ADR-0015 接受后的范围调整替代。Safe Copy 需先通过最小 Catalog Recovery Gate；门槛已有实现，但最终 all-target Rust 检查与验收待完成，不能因隔离复制流程运行成功就把它标为已通过。完整 N3 继续按本阶段另行跟踪。

## N4 — Immutable Organization Dry-run

状态：0083 已实现供后续 Safe Copy 使用的不可变计划与分页 reader；桌面及后端 all-target 检查待完成。以下仍是 broader N4 的产品契约。

Dependencies：AssetQuery + AssetScope + Metadata/User/Derived value contract。

- Scope 可来自 Current Query、Selection、Collection、Saved View 和明确的 Review set。
- OrganizationPlan 冻结 source path/fingerprint、resolved asset IDs、Effective values/revisions、rule version、target path 和 issues。
- Plan 生成后不随 Rating、Culling、Filter 或 AI 重跑自动变化。
- 数据变化使计划 stale；Safe Copy 只能消费 confirmed plan，不能重算路径。
- 本阶段仍不 mkdir/copy/move/rename/delete。

## N5 — Safe Copy

状态：0083 已集成 Safe Copy job/recovery/copy 实现；最终后端检查、Recovery Gate 验收与桌面检查待完成。

Dependencies：ADR-0015 最小 Catalog Recovery Gate 已通过 + N4 confirmed immutable plan；完整 N3 产品仍为后续独立能力。

- 建立唯一 FileOperationService：Plan -> Validate -> Journal -> Execute -> Verify -> Commit。
- 第一阶段只允许 COPY，目标必须不存在，禁止覆盖。
- 支持进度、取消/恢复、source fingerprint、target hash 和完整日志。
- Execute 只接收 confirmed plan ID，不接收前端 rules 或 raw paths。

## N6 — Rollback

状态：0083 已集成 job-owned rollback；目标在 preview 后被改变时拒绝执行的 targeted regression 已通过，桌面验收待完成。

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
