# 系统架构

## 总览

PhotoOrganizer 使用 Tauri 2 承载 React/TypeScript UI，扫描、图像处理、SQLite 和 ONNX 推理均在 Rust 单进程内本地执行。前后端通过类型化 command/response 与进度事件通信，不开放网络端口。

```text
React 专业工作区
  ├─ 顶部工具栏：视图、搜索、排序、导入、语义任务
  ├─ 左侧：图库、目录、多维筛选
  ├─ 中央：网格 / 单图画布 / 胶片栏
  └─ 右侧：文件、EXIF、影调色彩、语义结果
  └─ 增强工作台：收藏/集合、搜索、重复/相似、比较、编辑、人物门禁
          │ Tauri IPC + scan-progress / semantic-progress
Rust application core
  ├─ read-only scanner + EXIF + thumbnails
  ├─ traditional image analyzer
  ├─ Places365 scene classifier + TinyCLIP search encoder
  ├─ PicoDet subject detector + YuNet portrait helper
  ├─ workflow service：虚拟集合、向量检索、聚类、编辑配方与安全另存
  ├─ scan / semantic background coordinators
  └─ SQLite repository + parameterized filters + migrations
          │
Application data directory             Tauri resources
  ├─ photo-organizer.sqlite3              ├─ model-int8.onnx
  ├─ thumbnails/                          ├─ tokenizer/config/licenses
  ├─ previews/                             └─ onnxruntime.dll + notices
  └─ logs/

User-selected source directory: read-only during scan and analysis
```

## 模块边界

- `src/`：React 工作区、类型化 Tauri client、确定性视觉夹具和组件测试。
- `src-tauri/src/db.rs`：连接、事务、迁移、任务持久化、参数化筛选和分组统计。
- `scanner.rs`：只读目录遍历、fingerprint、增量判定、基础分析和缺失标记。
- `imaging.rs`：解码、方向处理、应用私有缩略图、连续影调/色彩特征；优先使用 JPEG EXIF 内嵌预览，已有当前缓存时只从缩略图重算特征；主色使用有彩色像素加权，单独保存中性色比例和主色覆盖率。
- `semantic.rs`：Places365 场景分类、TinyCLIP 预处理与向量检索、ONNX Runtime、catalog 与 benchmark。
- `subject.rs`：PicoDet/COCO 主体检测、YuNet 多尺度输出解码、中文主体标签聚合与模型状态。
- `semantic_tasks.rs`：单 worker 的有界批处理、缩略图路径门禁、场景/主体双模型持久化、单项失败隔离、进度和终态。
- `workflow.rs`：收藏/集合、精确重复、本地文本与以图检索、相似聚类、人物隐私门禁，以及编辑预览/另存副本的安全执行器。
- `tasks.rs`：扫描取消 token 与语义暂停/继续/取消控制器。
- `ipc.rs`：唯一暴露给 UI 的命令；模型状态只报告实际启用的 provider。
- 高清预览通过 `get_preview_data_url(asset_id, tier)` 受控读取：screen tier 以 EXIF 方向解码并缓存约 2560px JPEG，original tier 只为当前查看器临时读取原图；两者都不写入源目录。
- `remove_library(library_id)` 先取消该图库的活动任务，再用 SQLite 外键事务清理索引、分析、任务和计划，并只清理没有其他资产引用的应用 cache；它不调用源目录删除、移动或重命名。
- `migrations/`：只增不改、随二进制嵌入的 SQLite schema。

## 增强工作流数据流

1. 收藏直接更新 `assets.is_favorite`；集合通过 `collections` / `collection_assets` 保存多对多虚拟成员，不改变 `library_id`、路径或文件。
2. 精确重复查询复用扫描生成的完整 BLAKE3，不重新读取源图；结果只用于审阅或生成虚拟“重复待处理”集合。
3. 文本查询调用 TinyCLIP 的文本输出，图片搜索和聚类只读取与当前模型、分析版本和源 fingerprint 一致的 `semantic_embeddings`。
4. 相似聚类用向量主维度建立有界候选窗口，再做精确余弦与 complete-link 拆分；当前 5000 个 embedding 上限会明确报告截断。
5. 编辑预览与导出都调用同一个 Rust `EditRecipe`。导出先持久化 `edit_export_plans`，确认时重验源 fingerprint 和目标边界（目标父目录先 canonicalize，避免 junction/symlink 绕过），并在写文件前记录 `file_operation_jobs/file_operations`；回滚同样先预览并重验生成副本哈希，只删除未变化的应用生成副本。
6. 旧的人脸表和身份 clear-all 边界仍然独立保留；新的主体工作流只写 `subject_analysis_runs` / `subject_labels`，不写检测框、身份向量或聚类。

## 扫描与语义数据流

1. 用户通过系统目录选择器明确选择根目录。
2. 扫描任务只读遍历 JPEG/PNG/WebP，生成 fingerprint、应用私有缩略图、EXIF 和传统特征；有当前缓存时基础特征重算只解码缩略图，单文件失败不终止任务。
3. UI 可在扫描时继续读取已提交资产。自然完成后才把本轮未见资产标为 missing；取消不会误标未遍历资产。
4. 语义任务只选取基础分析已完成且当前 fingerprint/模型/分析版本没有有效结果的图片；主体结果缺失时，即使场景结果已存在，也会重新进入同一任务。
5. 单 worker 在 CPU 上以不超过 32 张的批次执行场景与主体模型；两者都只能读取当前 `grid-640-v1` 缩略图。PicoDet 使用 320 输入并补传 `scale_factor`，YuNet 使用 640 输入并在 Rust 中解码 12 个多尺度输出。
6. 场景标签写入 `semantic_labels`，主体标签写入独立的 `subject_labels`；查询层将二者合并为显示和辅助标签，但主体标签永远不参与主类别竞争。
7. 应用退出时已完成结果保持；重启把运行中的任务项恢复为 queued 并自动继续。用户暂停的任务保留 paused，可手动继续；取消将剩余资产还原为未分析。
8. 图片 fingerprint 变化后旧标签、主体运行记录和 embedding 仍可审计，但查询通过 fingerprint 与版本约束排除；重新分析写入当前结果。

## 查询边界

`Repository::list_assets` 构造参数化 SQLite `WHERE`，计数查询和分页查询共享同一条件：搜索、语义标签 any/all、影调、主色、亮度、饱和度、拍摄时间、原始目录和语义状态。排序、总数和分页均在数据库层完成，React 不对当前页做替代性过滤。

主要语义标签分组只读取场景模型的 `is_primary=1` 结果。主体模型提供可并存的 `人物`、`多人`、`人像`、`动物`、`宠物`、`车辆`、`食品`、`植物`，全部作为辅助标签；`人物` 与 `人像` 可以同时出现，`多人` 由人物检测数量派生。主体标签不会把图片强行归入某个主类别，也不会因“未知”分数高而伪造置信度。原始目录由图库根目录和完整相对路径祖先构成，父目录过滤覆盖所有后代。

## 安全与故障边界

- 没有删除、移动、重命名、覆盖或写回 EXIF/XMP 的命令；编辑器唯一的写操作是用户二次确认后的全新派生副本。
- 缩略图、SQLite、日志、模型和 embedding 均不写入源图库。
- 缩略图读取必须来自数据库登记并位于应用 cache root 的规范路径。
- 预览读取只能通过 asset id 找到数据库登记的源路径；screen 预览缓存位于应用数据目录，图片切换用 generation token 忽略旧请求。
- “从图库移除”只删除应用索引和 cache，源目录、原始图片和 EXIF/XMP 保持不变。
- 场景模型和主体模型分别报告状态；任一模型缺失、哈希失败或 session 初始化失败只禁用对应标签层，不产生占位标签，也不回退到原图、云端或身份识别。
- 扫描和语义任务使用独立注册器；语义保持单 worker 和有界批次，避免阻塞浏览和占满 CPU。
- 测试只使用仓库夹具或临时目录，并对源文件做前后哈希验证。

## 架构变更规则

引入 Python sidecar、云端服务、GPU/NPU provider、其他桌面框架或新的生产 native runtime 都必须增加 ADR，说明许可、包体、失败回退、发布和迁移影响。
