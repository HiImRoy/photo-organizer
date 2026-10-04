# 0076 筛选与规范化分类 ID 一致性

状态：已实现，定向验证及默认特性 Rust 集成验收通过

## 目标与边界

使图库的题材和主体筛选命中与列表/详情展示的有效分类一致。修复范围限定在 `src-tauri/src/db.rs` 和本执行计划；不更改 taxonomy、旧数据、迁移、前端控件或模型版本，不重新解释未知标签或已过期来源。

## 调查结论与决策

- 有效语义读取先按源指纹、当前模型/分析版本和 taxonomy 版本筛选，再由 `canonical_stored_label_id`、`canonical_topic_id` 或 `canonical_subject_id` 规范化显示值。题材 SQL 读取原始 `semantic_labels.label`；主体 SQL 只把请求 ID 规范化为 canonical ID，却直接比较存储的题材/subject label 和人工覆盖 ID。因而当前有效的 `landscape` 可显示为 `photo_landscape`，当前有效的 `person` 可显示为 `single_person`，但 canonical 筛选会漏掉它们。
- 保留每个自动来源既有的 provenance 条件：asset fingerprint、SigLIP2 或 PicoDet 的模型与分析版本、各自 taxonomy 版本。别名扩展只作用于已知 canonical ID，不降低这些有效性门槛；未知 ID 不获得新的自动语义映射。
- 在 SQL 绑定中为已知 canonical 题材/主体生成对应的存储 ID 集合。相同集合用于自动标签、subject_labels 和手动 add/remove，以保持规范化后的有效标签语义。与 `resolve_tags` 一致，先应用 remove、再应用 add；同一 canonical 标签同时存在 add/remove 时最终由 add 保留。仅 remove 会屏蔽自动标签，仅 add 即使没有自动结果也能命中。
- 保持辅助标签 Any/All 的逐标签组合方式，以及列表结果与 count 共用同一筛选谓词。题材多选继续采用现有 OR/IN 语义。
- 主分类读取会先丢弃 `canonical_topic_id` 不支持的 raw ID，再按相似度取首个有效题材；筛选 SQL 也必须在排序/限行前过滤无效 ID。筛选候选集合由现有 canonical topic 列表和别名表构造，并再次经 `canonical_topic_id` 验证；它覆盖全部合法题材，而非仅覆盖当前查询目标，以保持多个合法题材之间原有的最高分选择行为。

## 实施步骤

1. 增加数据库 fixture，使用当前模型、分析、taxonomy 版本和匹配 asset fingerprint 写入 raw `landscape` 题材及 raw `person`/其他主体记录；确认返回的有效分类为 canonical ID，并先证明 canonical 筛选当前失败。
2. 扩展 fixture 覆盖过期模型/分析版本、过期 taxonomy、指纹不匹配等记录；确认它们既不展示也不因别名扩展而命中。覆盖手动 add/remove 与自动标签的显示/筛选一致性，以及辅助标签 Any/All、count 和分页。
3. 在 `asset_filter_sql` 中添加窄范围的已知 ID 别名展开和相应参数绑定；保持未知筛选值及现有 provenance 条件不变。
4. 增加主分类排序边界 fixture：当前 provenance 的高分拒绝 ID、较低分合法题材及更低分的第二合法题材；验证展示与筛选都选中最高分合法题材，且拒绝 ID 保留原值。
5. 更新本计划的实施与验证记录，审阅最终 diff，确认没有越过文件所有权。

## 验证

- 先运行新增回归测试，观察其在修复前失败、修复后通过。
- 运行相关 `db` 单测、Rust 格式检查与 Clippy；如共享 Rust 静态边界要求扩大验证，再由主代理统一执行 all-target 检查。
- 所有文件相关 fixture 使用测试自建临时目录；不访问个人图库。遵照并行构建协调，不运行 `cargo clean`，不清理或删除任何 target 内容；若默认 Cargo 构建锁持续占用，记录证据并交由主代理统一验证。
- 运行 `git diff --check` 并审阅仅限本任务文件的差异。

## 执行与验证记录

- 并行工作者报告：SQL 修复前新增回归测试已编译并按预期红灯；本任务在修复后使用默认特性及 `--no-default-features` 分别运行该定向用例，均为 1/1 通过。
- `cargo test --manifest-path src-tauri/Cargo.toml --no-default-features --lib db::tests::`：39/39 通过。
- `cargo clippy --manifest-path src-tauri/Cargo.toml --no-default-features --lib -- -D warnings`：通过。
- 仅对 `src-tauri/src/db.rs` 运行 Rustfmt 后，`rustfmt --edition 2024 --check src-tauri/src/db.rs` 通过；所属文件 `git diff --check` 通过。
- 新增 `primary_filter_selects_highest_scored_canonical_topic` fixture 使用当前 SigLIP2 模型/分析/taxonomy 与匹配源指纹，验证高分 `document` 不入选、低分 `landscape` 与展示一致、再低分合法 `photo_abstract` 不会因筛选目标而被错误选中，且数据库中的 raw `document` 未被改写。该用例在下述 all-target 运行中通过。
- workflow 最终路径断言修复落盘前的一次默认特性 `cargo test --manifest-path src-tauri/Cargo.toml --all-targets` 为 181/182；唯一失败是 `workflow::tests::edit_export_requires_preview_and_never_overwrites` 比较扩展路径文本与普通路径文本（实际含 `\\\\?\\`）。workflow agent 随后将比较改为 `identity_key`；当前源码下隔离复验 1/1 通过，最终 all-target 运行也通过。因此这是旧断言与路径表示不一致，非当前源码回归或环境失败。
- 最终默认特性 `cargo test --manifest-path src-tauri/Cargo.toml --all-targets`：lib 182/182、large-library-benchmark 2/2、semantic-benchmark 1/1、semantic-evaluate 3/3 通过；其他列出的 binary targets 为 0 tests，命令退出码 0。
- 最终默认特性 `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`：通过。 `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check`：通过；源码修复性格式化仅运行于本任务拥有的 `db.rs`。
- 未运行 `cargo clean`，未清理 target，未提交。

## 验收

- 当前 provenance 的 raw legacy ID 在界面显示与 canonical 题材/主体筛选间保持一致。
- 当前 provenance 的未知/拒绝主分类 ID 不参与最高分候选排序，也不被改写；筛选从全部合法题材中选择最高分项。
- 旧版本、旧 taxonomy 或不同源指纹的自动结果不进入显示或筛选；未知标签不会因本修复获得新的语义解释。
- 手动添加和移除、辅助标签 Any/All、列表总数及分页都与有效分类一致。
- 不修改其他 agent 所有的文件，不提交 commit；记录实际运行与未运行的检查及阻碍。
