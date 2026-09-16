# 0062 标签分层恢复与题材判定收敛

状态：已完成（代码与自动验证完成；仓库仍有两项既有格式检查告警）

## 目标

根据旧版主体/题材分离契约，恢复 Topic、Subject、Context 三层边界，修复当前把主体证据提升为题材、把风光派生为“风景”主体以及把未知结果吞入“抽象艺术”的问题。保持原图只读、缩略图门禁、既有 IPC 字段和整理流程不变。

## 现状证据

- `docs/plans/0027-subject-tags-and-model-workflow.md` 与 `docs/decisions/0008-subject-model-layer.md` 明确规定主体标签只写主体层，不参与 `primary_category`。
- 当前 `semantic_tasks.rs` 仍存在主体改写 primary 和 `photo_landscape -> scenery` 派生路径；当前 `semantic.rs` 与前端兼容映射还把 `unknown`、纪实、活动、文档归并为 `photo_abstract`。
- 当前题材模型是通用 SigLIP2 零样本模型，没有本项目授权的摄影训练/校准集，因此本次先修复标签契约与拒识边界，不声称已经完成真实数据 precision/recall 校准。

## 目标契约

### Topic：拍摄题材

保留且只允许一个 primary：`photo_portrait`、`photo_landscape`、`photo_street`、`photo_architecture`、`photo_still_life`、`photo_wildlife`、`photo_macro`、`photo_vehicle`、`photo_abstract`。

`photo_portrait` 只由题材 prompt 选择；检测到 `single_person` 或 `multiple_people` 不得改写题材。未达到题材阈值或 margin 时不写 primary，表示未分类，不再伪造 `photo_abstract`。

### Subject：画面主体

可并存且永远 `is_primary=false`：`single_person`、`multiple_people`、`animal`、`vehicle`、`food`、`plant`。`photo_vehicle` 表示交通工具摄影题材，`vehicle` 表示画面检测到车辆，二者保持独立。自动分析不再产生 `scenery`。

### Context：环境证据

Places365 的环境结果继续使用 `context` 分组，只辅助展示，不参与 Topic/Subject 统计和 primary 竞争。

## 实施范围

1. 在 `semantic_tasks.rs` 停用主体改写题材和风景派生；补充 street/landscape 与 subject 并存、无 scenery 自动派生测试。
2. 在 `topics.rs` 收紧人像摄影 prompts，保留九个题材并继续使用单一 primary 选择；版本提升以使 prompt/拒识契约变化触发重分析。
3. 在 `semantic.rs` 拆分按层级的 canonical 映射/显示/分组判断，保留 legacy 可读性但不把 `unknown`、纪实、活动、文档自动改成抽象艺术；版本提升。
4. 在 `subject.rs` 恢复 `vehicle` 主体目录，并保持主体输出非 primary；仅保留确定性的高重叠 person 框去重，未扩大为通用坐标/NMS、阈值或校准重写。
5. 在 `db.rs` 的当前自动读取/筛选边界使用新的 Topic/Subject 分层和版本，避免旧版本结果冒充当前分析；不新增 SQL 迁移，现有版本字段足以隔离新结果。
6. 在 `classificationLabels.ts` 删除自动 `scenery` 选项，增加 `vehicle`，按 primary/tag 层区分 legacy 兼容；必要时同步直接契约测试。

## 版本策略

- 题材 taxonomy：从 `photo-organizer-photography-topics-v4` 提升到 `photo-organizer-photography-topics-v5`。
- 主体 taxonomy：从 `photo-organizer-subject-tags-v3` 提升到 `photo-organizer-subject-tags-v4`。
- SigLIP2 题材分析版本提升到 `photo-organizer-semantic-topic-candidates-siglip2-v4`，Subject 分析版本提升到 `photo-organizer-subject-picodet-yunet-v2`，主体 taxonomy 提升到 `photo-organizer-subject-tags-v4`；确保 prompt、融合和目录变化会重新排队分析，不删除已有数据库记录。

## 验收与限制

- Rust 测试覆盖 street/landscape 题材与 single_person/animal 主体不互相改写、无 scenery 派生、主体目录含 vehicle 且全部非 primary、`night`/`sunset`/`street` 旧 Context 按原 id 可读、旧值按层级兼容和未分类不落入 abstract。
- 前端测试覆盖九个题材、六个主体、删除 scenery、区分 `photo_vehicle`/`vehicle`、按 kind 限定 catalog 查找以及未知 primary fallback 不伪造成抽象艺术。
- 运行相关 Rust tests、前端相关 tests、typecheck、lint、格式检查；不连接个人图库，不访问个人数据库，不读取原图。
- 本次不会证明通用 SigLIP2 对真实摄影集的最终准确率，也不做模型权重替换；真实阈值/召回校准和检测框 NMS 作为后续独立实验。

## 实施记录

已完成的实施与验证记录：

- 分层：移除主体改写 primary 与 `photo_landscape -> scenery` 派生；Topic 只写 `scene/is_primary=true`，Subject 只写 `subject/is_primary=false`；`context` 保持独立，legacy `night`、`sunset`、`street` 保留原 id。
- 题材：Topic 保留九个摄影题材，收紧 portrait prompts；未达到可靠题材阈值时不写 primary，`unknown`、纪实、活动、文档不再 canonical 为 `photo_abstract`。
- 主体：恢复六个主体（含 `vehicle`），普通车辆不会生成 person；多人仅由独立 person 框计数；保留缩略图门禁，仅做确定性的高重叠框去重。
- 前端：`classificationValueLabel` 先按 kind canonicalize，再按 scene-primary 或 subject-non-primary 查 catalog；primary 未知值显示“未分类”，tag 与 primary 的同名 id 不串层。
- 版本：题材 taxonomy v5、主体 taxonomy v4、SigLIP2 题材分析 v4、Subject 分析 v2；无 SQL 迁移，不删除历史记录。
- 自动验证：`cargo fmt --manifest-path src-tauri/Cargo.toml --all`；相关 Rust 单测与全量 lib 测试；`npm.cmd run test`、`npm.cmd run typecheck`、`npm.cmd run lint`、前端格式检查和 `npm.cmd run build`。
- 限制：通用 SigLIP2 零样本模型尚未使用用户真实照片集做 precision/recall 或阈值校准，本次未替换模型权重；全仓前端格式检查仍被既有 `docs/plans/0059-gpu-performance-s0-test-checklist.md` 与 `src/query.ts` 报告，未涉及本任务修改；未访问个人照片或数据库；工作区保持未提交。
