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

`asset_id`、饱和度均值/中位数、平均色度、兼容主色 RGB/类别、`dominant_colors_json`、色相统计 JSON、冷暖评分、中性色比例、彩色主色覆盖率、色彩丰富度、近黑白概率、饱和度标签、`algorithm_version`、`analyzed_at`。当前 `dominant_colors_json` 保存 `ColorPalette` 对象：包括 `algorithmVersion`、最多 5 个按面积排序的 `coveragePalette` 候选和最多 3 个按视觉显著性排序的 `prominentPalette` 候选；每个候选包含中文映射所需的稳定颜色类别、RGB 十六进制值、面积/显著性占比、局部对比度、色度和空间连续性。兼容主色字段由面积主色候选回填，自动颜色筛选也只消费达到主色覆盖率阈值的面积候选；旧数据仍可读取。所有候选从应用私有缩略图提取，不读取原图。

### `semantic_labels`

`asset_id`、稳定英文 `label`、中文 `display_name`、原始 `similarity`、使用的 `threshold`、`model_name`、`model_version`、`analysis_version`、`source_fingerprint`、`generated_at`、`is_manual`、`is_excluded`、`is_primary`。相似度不命名为 probability/accuracy。

当前自动主标签来自摄影题材候选层：人像、风光自然、街拍纪实、建筑、静物产品、美食、动物、植物、运动、交通工具、文档截图、抽象艺术。每个题材由多条提示词得到候选分数，并经过独立阈值和候选间隔拒识；当前唯一候选模型 SigLIP 2 使用匹配 logits；拒识结果在有效分类层归入抽象艺术；Places365 只作为环境/场景证据，不直接成为新的摄影题材。

`semantic_evidence` 还会保存当前题材模型候选、主体融合证据和 Places365 叶子场景的原始排名。每条记录通过模型名称、版本、分析版本和来源 fingerprint 区分，运行状态必须显示实际使用的模型组合。

### `subject_analysis_runs`

按 `asset_id + source_fingerprint + model_name + model_version + analysis_version + taxonomy_version` 唯一记录主体模型是否完成。保存 `completed/failed` 状态、错误和时间；空结果也写入 `completed`，避免每次浏览都重复检测。该表只保存派生分析状态，不保存原图路径副本、人脸框或身份信息。

### `subject_labels`

保存主体模型聚合出的稳定英文 `label`、中文 `display_name`、检测分数 `similarity`、阈值、模型/分析/分类法版本、来源 fingerprint 和生成时间。主体标签与 `semantic_labels` 分表，查询时只在读取层合并；表中没有 `is_primary`，因此主体标签不能成为主类别。

当前主体模型链为 PicoDet-S COCO 80 类检测器和 YuNet 人脸辅助检测器。应用只保存聚合后的 `单人`、`多人`、`动物`、`车辆`、`食品`、`植物`；单人和多人互斥，宠物归入动物，不保存检测框、关键点、脸部裁剪、embedding 或身份簇。

### `semantic_models`

记录模型/分析版本、许可证、来源、模型/tokenizer SHA-256 与路径、实际 execution backend、安装时间和 active 状态。active 记录还是成功装载过模型的持久化信号：下次启动会在后台自动恢复当前随包模型，不要求再次点击装载；历史已移除模型记录会迁移到当前 SigLIP 2 版本。UI backend 来自实际加载状态，不从枚举推断。语义运行状态另外报告题材候选模型；候选模型缺失不会伪造题材主标签。

### `semantic_embeddings`

按 `asset_id + model_name + model_version + analysis_version + source_fingerprint` 唯一保存 little-endian `f32` blob、维度和生成时间。智能工作台只读取与当前模型、分析版本和源 fingerprint 一致的向量，用于本地文本搜索、以图搜图和相似聚类。

### `analysis_jobs`

`id`、`library_id`、`job_type`、`status`、`progress_current`、`progress_total`、`completed_count`、`failed_count`、`skipped_count`、`execution_backend`、模型/分析版本、`created_at`、`updated_at`、`error_message`。支持 queued/running/paused/cancelling/completed/cancelled/failed。

### `analysis_job_items`

逐资产保存 `job_id`、`asset_id`、任务创建时的 `source_fingerprint`、状态、尝试次数、错误和更新时间。该表使单张失败后继续、协作式取消和重启恢复不依赖内存进度。

### `file_operation_jobs` / `file_operations`

保存文件操作任务状态、dry-run 标志、源/目标、操作类型、计划/执行状态、冲突策略、源/目标哈希、错误和撤销状态。整理工作区仍只做 dry-run；编辑器在独立计划二次确认后以 `edit_copy` 写入日志并创建一个不存在的新副本。

### `assets.is_favorite` / `collections` / `collection_assets`

0053 将 `collections` 扩展为虚拟收藏夹树：除了名称、说明和时间，还保存 `parent_collection_id`、`collection_kind`、`system_key` 和 `display_order`。`collection_assets` 保存多对多成员及加入时间；删除集合或成员关系只改变 SQLite，不改变资产路径、真实来源或源文件。

`system_key = 'default_favorites'` 的系统叶节点是默认收藏。它的成员关系是爱心状态的真实来源；迁移阶段 `assets.is_favorite` 只作为兼容镜像，爱心和默认收藏的加入/移除必须在同一事务中同步。普通收藏夹不会自动点亮爱心，一张 Asset 可以属于多个普通收藏夹。

`assets.library_id` 始终表示图片真实扫描来源。旧 `asset_library_assignments` 仅作为 0053 迁移和短期兼容数据保留，不得覆盖 Source 查询；旧 assignment 会转换为普通 Collection 关系。

### `saved_views`

保存命名查询的 `library_id` 和版本化 `query_json`。schema 已预留，当前智能工作台 MVP 尚未暴露保存视图 UI。

### `AssetQuery v2`

查询层使用版本化的 `root + includeDescendants + filter + sort + page` 契约。`root` 可为 `all`、物理 `source`、虚拟 `collection` 或系统 `favorites`；Source 只沿 `parent_relation='source'` 递归，Collection 沿 `parent_collection_id` 递归并通过 `EXISTS` 去重，因此一张 Asset 同时属于父子收藏夹时不会重复出现在结果中。旧 `libraryId`、`favoriteOnly`、`collectionId` 只在兼容适配器中使用。

### `edit_export_plans`

保存编辑导出 plan id、asset id、计划时源 fingerprint、目标路径、完整 `EditRecipe` JSON、状态、时间和错误。计划确认后仍会重验 fingerprint、目标不存在且位于所有图库根目录之外。

### `face_detections` / `face_clusters` / `face_cluster_members`

仅用于未来显式 opt-in 的本地人物功能。当前迁移建立可级联清理的派生表和 `workflow_preferences.face_analysis_enabled=false`；没有合规模型时不会写入这些表。clear-all 会先删除成员、聚类和检测，再关闭开关，不触碰原图。

## 增量规则

- 稳定身份由 `library_id + absolute_path` 保证；相同路径的大小或修改时间变化会更新 fingerprint 并使缩略图/分析失效。
- fingerprint 起步使用内容摘要；语义和主体查询只有在 `source_fingerprint = assets.fingerprint` 且模型/分析/分类法版本为当前值时才采用结果。
- 每次扫描生成 `scan_started_at`，成功发现的记录更新 `last_seen_at`；扫描自然完成后，本轮未见记录标为 missing。取消扫描不批量标 missing，避免把未遍历部分误判为缺失。
- 重启时 semantic running/cancelling 任务和 running item 恢复为 queued 并自动继续；paused 保持暂停，已完成 item 不重复执行。

### `organization_plans` / `organization_plan_items` / `organization_plan_issues`

`0003_organization_dry_run.sql` 保存只读整理预览的图库、目标根目录、范围快照、版本化规则、摘要和更新时间。计划项保存源 asset、源快照 fingerprint、相对目标路径、字节数、稳定顺序和预览状态；问题表保存稳定代码、严重度、源/目标路径和说明。它们不是复制队列，不包含 executed/copying/moving/deleting 状态。完整映射可从当前 SQLite 资产重新计算，导出清单不会写入源图库，也不会创建目标目录。

## 迁移

迁移文件随 Rust 二进制嵌入，在打开数据库时事务执行；旧 migration 文件只增不改。常规迁移新增表、列和索引，`0016_unified_source_collection.sql` 还会在同一事务中重建需要收敛的集合表，并由 Rust 迁移助手完成旧归属关系转换，因此失败会整体回滚。`schema_migrations(version, applied_at)` 保证重复初始化安全。测试覆盖空库、重复初始化、版本顺序、组合筛选、组织计划表、收藏/集合、重复分组和唯一约束。
