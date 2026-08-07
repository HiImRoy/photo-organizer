# 数据模型

## 原则

- SQLite 是本地事实来源，启用 foreign keys 与 WAL。
- 核心筛选/排序连续值使用独立列和索引，不塞入不透明 JSON。
- 资产记录不会因源文件暂时缺失而删除，只更新 `file_status`。
- 数据库迁移按单调版本执行并可重复启动。

## 起步 schema

### `libraries`

`id`、唯一 `root_path`、`created_at`、`last_scan_at`、`status`、`last_error`。

### `assets`

`id`、`library_id`、唯一的库内 `absolute_path`、`relative_path`、`file_name`、`extension`、`file_size`、`modified_at`、`fingerprint`、`width`、`height`、`orientation`、`capture_time`、相机/镜头/曝光可空字段、`file_status`、`scan_status`、`analysis_status`、`error_message`、`first_seen_at`、`last_seen_at`，以及独立的 `semantic_status`、`semantic_error`、`semantic_analyzed_at`。

索引覆盖图库+文件状态、文件名、拍摄/修改时间、尺寸和相对路径。

### `thumbnails`

`asset_id`、`cache_path`、`spec`、`source_modified_at`、`source_size`、`status`、`error_message`、`updated_at`。`asset_id + spec` 唯一。

### `tone_features`

`asset_id`、亮度均值/中位数/低高分位、暗部/高光比例、对比度、动态范围、影调/曝光/对比度标签、`algorithm_version`、`analyzed_at`。

### `color_features`

`asset_id`、饱和度均值/中位数、主色 RGB/类别、主色列表 JSON、色相统计 JSON、冷暖评分、中性色比例、色彩丰富度、近黑白概率、饱和度标签、`algorithm_version`、`analyzed_at`。起步 UI 使用均值和主色；其他列为后续算法保留且保持可空。

### `semantic_labels`

`asset_id`、稳定英文 `label`、中文 `display_name`、原始 `similarity`、使用的 `threshold`、`model_name`、`model_version`、`analysis_version`、`source_fingerprint`、`generated_at`、`is_manual`、`is_excluded`、`is_primary`。相似度不命名为 probability/accuracy。

### `semantic_models`

记录模型/分析版本、许可证、来源、模型/tokenizer SHA-256 与路径、实际 execution backend、安装时间和 active 状态。UI backend 来自实际加载状态，不从枚举推断。

### `semantic_embeddings`

按 `asset_id + model_name + model_version + analysis_version + source_fingerprint` 唯一保存 little-endian `f32` blob、维度和生成时间。当前只用于分类缓存，不对 UI 提供相似搜索。

### `analysis_jobs`

`id`、`library_id`、`job_type`、`status`、`progress_current`、`progress_total`、`completed_count`、`failed_count`、`skipped_count`、`execution_backend`、模型/分析版本、`created_at`、`updated_at`、`error_message`。支持 queued/running/paused/cancelling/completed/cancelled/failed。

### `analysis_job_items`

逐资产保存 `job_id`、`asset_id`、任务创建时的 `source_fingerprint`、状态、尝试次数、错误和更新时间。该表使单张失败后继续、协作式取消和重启恢复不依赖内存进度。

### `file_operation_jobs` / `file_operations`

为后续安全复制预留：任务状态、dry-run 标志、源/目标、操作类型、计划/执行状态、冲突策略、源/目标哈希、错误和撤销状态。起步版本不暴露执行 command。

## 增量规则

- 稳定身份由 `library_id + absolute_path` 保证；相同路径的大小或修改时间变化会更新 fingerprint 并使缩略图/分析失效。
- fingerprint 起步使用内容摘要；语义查询只有在 `source_fingerprint = assets.fingerprint` 且模型/分析版本为当前值时才采用结果。
- 每次扫描生成 `scan_started_at`，成功发现的记录更新 `last_seen_at`；扫描自然完成后，本轮未见记录标为 missing。取消扫描不批量标 missing，避免把未遍历部分误判为缺失。
- 重启时 semantic running/cancelling 任务和 running item 恢复为 queued 并自动继续；paused 保持暂停，已完成 item 不重复执行。

### `organization_plans` / `organization_plan_items` / `organization_plan_issues`

`0003_organization_dry_run.sql` 保存只读整理预览的图库、目标根目录、范围快照、版本化规则、摘要和更新时间。计划项保存源 asset、源快照 fingerprint、相对目标路径、字节数、稳定顺序和预览状态；问题表保存稳定代码、严重度、源/目标路径和说明。它们不是复制队列，不包含 executed/copying/moving/deleting 状态。完整映射可从当前 SQLite 资产重新计算，导出清单不会写入源图库，也不会创建目标目录。

## 迁移

迁移文件随 Rust 二进制嵌入，在打开数据库时事务执行。`0002_semantic_workspace.sql` 和 `0003_organization_dry_run.sql` 只新增表、列和索引，不修改已发布的旧 migration。`schema_migrations(version, applied_at)` 保证重复初始化安全。测试覆盖空库、重复初始化、版本顺序、组合筛选、组织计划表和唯一约束。
