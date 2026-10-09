# 0080 编辑导出审计留存与恢复身份

状态：暂停；未验收。db.rs 与 migration 0018 已有本地 WIP 草稿，尚未完成迁移验证或测试；不得对私人数据库启动/迁移。

## 目标与范围

资产或图库从索引中移除后，编辑导出计划及其审计历史仍须保留。数据库迁移需原子、安全且可通过重复初始化保持幂等；新增字段为后续导出恢复提供文件与进程身份记录。本任务只改 `src-tauri/src/db.rs`、新增 `src-tauri/migrations/0018_edit_export_audit_recovery.sql` 与本计划，不改 workflow 实现，不访问照片目录，不增加生产依赖。

## 调查结论与假设

- 当前 schema 版本为 17。初始化逐版本开启 SQLite 事务，执行迁移 SQL、可选 Rust post-step、写入版本标记后一起提交；迁移 18 的根快照填充将留在此事务内。
- `edit_export_plans.asset_id` 当前为 `NOT NULL ... ON DELETE CASCADE`。移除图库会删除未能重分配的 assets，故现有 FK 会连带删除计划；未发现直接删除计划的 SQL。新的 `ON DELETE SET NULL` 应保留计划，同时将资产关联置空。
- 计划表重建保留原九列及其数据，并新增 `source_path`、`source_file_identity`、`protected_roots_json`。历史 `source_path` 从关联 asset 的 `absolute_path` 快照；旧记录的文件身份不可从当前文件推测，因此保持 NULL。
- `protected_roots_json` 为迁移时已知 library root identity 的稳定 JSON 数组。优先用已存 `source_identity_key`；为空时用 `source_path`，再回退 `root_path`，通过项目 `identity_key` 纯字符串规范化。该函数不访问文件系统。
- `file_operations` 只增加目标身份和执行/回滚进程身份的 nullable 列。既有操作（包括 running 状态）保持不变；不调整 `analysis_jobs` 恢复逻辑。
- 应用按 schema version 跳过已应用迁移，因此重复 `initialize()` 不重复执行非幂等 ALTER；迁移 SQL 与版本标记在同一事务，失败时必须整体回滚。

## 实施步骤

1. 增加迁移 18 SQL：在事务内重建 `edit_export_plans`，复制全部旧列及状态/recipe/错误/时间；关联资产路径填入 `source_path`，未知文件身份保持 NULL；新增 nullable `file_operations` 身份列。
2. 在初始化迁移序列中注册版本 18，并在同一事务 post-step 中以纯字符串构造所有已有计划的 library-root identity 快照；更新 schema 版本断言。
3. 审核资产去重、移除 asset、移除 library 路径，确保计划只因真实资产去重而重关联，不因删除资产或图库而显式清理；保持 file-operation job 历史逻辑原样。
4. 加入 17→18 回归 fixture，验证所有计划字段、source/root 快照、asset 删除和整库删除的 `SET NULL` 留存、重复初始化、迁移中途失败的事务回滚，以及含中文/西里尔文的源路径只作字符串快照而不触碰源文件。
5. 仅对本任务拥有文件做格式/静态检查，先运行定向 DB 测试；集成检查交主代理安排。检查最终 diff 并在此记录准确结果和未运行项；不提交、不清理 target。

## 验收与安全边界

- 迁移保留既有导出计划的 ID、资产引用（除资产删除后置空）、fingerprint、目标、recipe、状态及创建/执行时间和错误信息。
- 删除资产或整个 library 后，计划仍存在且 `asset_id IS NULL`；file-operation 行、job 行和 running 状态不由本迁移或删除流程清理/改终态。
- 迁移不根据文件当前内容、存在性或元数据推导历史 `source_file_identity`；不打开、创建、修改、移动或删除照片路径指向的任何文件。
- 失败迁移不留下部分 schema、部分数据转换或版本 18 标记；成功后重复初始化不改变历史记录。

## 执行与验证记录

- 初始草案记录时尚未实施或运行测试。
- 2026-10-10 交接：db.rs 与 migration 0018 已有本地 WIP 草稿，迁移和测试尚未验证；不得用于私人数据库。
