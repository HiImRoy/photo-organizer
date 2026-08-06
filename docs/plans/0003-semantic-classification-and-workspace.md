# PhotoOrganizer 语义分类与专业工作区里程碑计划

- 状态：实现完成；正式 Windows 安装包验证受本机 MSVC 工具链阻塞
- 创建日期：2026-08-07
- 最后更新：2026-08-07
- 范围：深色三栏工作区、真实本地语义推理、后台任务、SQLite 组合筛选与分组、发布资源和验证

## 目标

在不修改用户原图的前提下，把 PhotoOrganizer 扩展为 Lightroom 类三栏桌面工作区；随应用分发一个许可清晰、CPU 可运行的真实图文模型；持久化多标签语义结果、embedding 和可恢复任务；让语义、影调、主色、亮度、饱和度、时间与原始文件夹组合筛选真正进入 SQLite 查询层。

## 已确认基线

- 当前应用为 Tauri 2 + React/TypeScript + Rust 单进程；扫描、缩略图、基础特征和 SQLite 已可用。
- `semantic_labels` 与 `analysis_jobs` 只有起步结构；运行时仍是 `UnavailableClassifier`，不会生成标签。
- `list_assets` 只支持排序和分页，没有筛选；资产列表类型没有完整 EXIF、影调、语义标签或主标签。
- 前端当前是浅色两栏布局，右侧详情是临时浮层；不存在单图模式、胶片栏或可折叠检查器。
- 仓库尚无 Git 提交，所有文件均为未跟踪状态；无法使用传统基线 diff，必须结合文件清单、静态搜索、测试和截图自审。
- 本机 Rust 工具链存在于 `%USERPROFILE%\.cargo\bin`，但不在默认 PATH；MSVC linker/Build Tools 仍需在构建阶段复核。

## 模型候选与初步决策

| 候选                                    | 许可证                                                                         | 体积/能力                                                             | Windows CPU 与分发判断                                                                 |
| --------------------------------------- | ------------------------------------------------------------------------------ | --------------------------------------------------------------------- | -------------------------------------------------------------------------------------- |
| TinyCLIP ViT-8M/16 + Text-3M，INT8 ONNX | Microsoft TinyCLIP、HF 原权重与 ONNX conversion 标记 MIT；最终归档许可证与哈希 | 完整 INT8 ONNX 约 24.3 MB，零样本图文匹配，可覆盖本里程碑全部稳定标签 | 选择；使用 ONNX Runtime CPU，随安装包分发模型和 tokenizer，不要求 Python/CUDA/环境变量 |
| ONNX Model Zoo SqueezeNet               | Model Zoo Apache-2.0                                                           | 约 5 MB，但仅 ImageNet-1K 单标签对象分类                              | 体积优，但无法可靠覆盖室内、街道、夜景、截图、文档等多标签场景，不选                   |
| OpenAI CLIP ViT-B/32                    | 代码与公开权重 MIT                                                             | 零样本能力成熟，但权重超过 300 MB                                     | CPU 可用但安装包和内存成本过高，不选                                                   |
| Apple MobileCLIP                        | 代码与模型分别有 Apple 许可文件                                                | 小型图文模型，移动端性能好                                            | 需要额外 ONNX 导出、许可归档和 Windows 基准；当前分发链不如 TinyCLIP 直接，不选        |

最终目标模型：`TinyCLIP-ViT-8M-16-Text-3M-YFCC15M` 的完整 INT8 ONNX conversion。模型输出是图文相似度，不在 UI 中称为概率或准确率。第一版只启用实际验证的 CPU execution provider；DirectML/GPU/NPU 只保留架构扩展点，不显示为已启用。

## 关键假设与边界

- 模型、tokenizer 和许可证放在 Tauri resource 中；安装后复制到应用资源目录，由 Rust 只读加载。
- 固定英文标签 ID、中文显示名、英文提示词与每类阈值进入版本化 label catalog；`unknown` 只在没有标签达到阈值时产生。
- 主标签为达到阈值的非 `unknown` 标签中相似度最高者；相同分数按 catalog 稳定顺序选择。此规则记录到数据和模型文档。
- 单张图片保存原始余弦相似度、阈值、模型/分析版本、源 fingerprint、生成时间、人工标记和主标签标记；embedding 与语义标签分表保存。
- 后台语义任务逐项持久化，单张失败不终止；暂停、继续、取消使用内存控制器和数据库状态协同。重启后运行中任务恢复为可继续的 queued/paused 状态。
- 扫描和语义分析使用不同任务注册器；模型推理最多单 worker，避免与缩略图扫描争抢全部 CPU。
- 不实现模型训练、人脸身份、云端请求、描述生成、相似搜索或文件整理。

## 实施阶段

### 1. 决策、资源与迁移

- [x] 新增 ADR，确认 TinyCLIP、ONNX Runtime、CPU provider 和随包分发方案。
- [x] 下载并归档模型、tokenizer、许可证、来源和 SHA-256；更新第三方声明与 Tauri resource 配置。
- [x] 新增 `0002` migration，不修改 `0001`；增加模型注册、embedding、任务项、语义状态、阈值、主标签和版本/指纹字段。
- [x] 更新迁移执行器和数据模型文档，测试迁移幂等与升级顺序。

### 2. 真实语义运行时

- [x] 集成 `ort` CPU runtime 和 Hugging Face tokenizer；实现 CLIP 图像预处理、提示词 tokenization、真实推理、相似度和 embedding 输出。
- [x] 建立 21 个稳定标签 ID、中文名、提示词和阈值；实现 `unknown` 与主标签规则。
- [x] 保留 `UnavailableClassifier`，模型资源缺失或校验失败时基础功能继续可用且不产生假标签。
- [x] 更新 benchmark CLI，使其真正加载 TinyCLIP 并输出 CPU 延迟、吞吐、失败数、模型哈希、样例预测与实际 backend。

### 3. 持久后台任务

- [x] 实现语义任务创建、查询、进度事件、暂停、继续、取消与失败隔离。
- [x] 应用重启时把中断任务恢复为可继续状态；完成结果保留，未变化且版本匹配的图片跳过。
- [x] 图片 fingerprint、模型或分析版本变化后使旧结果不参与筛选并重新分析。
- [x] 添加任务状态与安全测试，验证源文件哈希不变。

### 4. SQLite 组合筛选与分组

- [x] 定义类型化筛选请求：语义标签 any/all、影调、主色、亮度、饱和度、时间、原始文件夹、未分析和失败。
- [x] 在 repository 构造参数化 SQL，使计数、分页、排序与筛选使用同一 WHERE 语义。
- [x] 返回语义标签、主标签、完整 EXIF/传统特征和语义分析元数据。
- [x] 实现主要语义标签分组统计和稳定主标签规则测试。

### 5. Lightroom 类深色工作区

- [x] 重构为顶部工具栏、可折叠左筛选面板、中央网格/单图工作区、可折叠右检查器。
- [x] 单图模式增加底部胶片栏，支持快速切换；保持分页和键盘焦点。
- [x] 接入搜索、组合筛选、排序、语义任务控制、模型状态和真实标签展示。
- [x] 建立深色 token 与克制的新拟物层级，更新响应式规则和 `docs/ui-guidelines.md`。

### 6. 验证与发布

- [x] 更新 Vitest、Rust repository/runtime/task 测试及 Unicode/临时夹具安全测试。
- [x] 运行 Prettier、ESLint、TypeScript、Vitest、rustfmt、Clippy、Rust tests 和 production build。
- [x] 用仓库图标测试资源执行真实 TinyCLIP CPU 分类与 benchmark，记录结果、样例预测与模型哈希。
- [x] 实际尝试 Tauri build 和 Windows 安装包；前端 production build 通过，Rust MSVC 编译因缺少 `link.exe` 停止，故未生成本里程碑安装包、未执行打包 WebView2 smoke。
- [x] 实际检查 1920×1080、1440×900、1366×768 和 960×720，保存网格、单图/胶片栏、折叠面板和组合筛选截图。
- [x] 审查最终文件清单、production 资源、许可证和未解决风险，更新本计划验收记录。

## 风险与缓解

- ONNX community conversion 虽标记 MIT，仍需同时归档 Microsoft TinyCLIP 原仓库、原权重仓库和 conversion 来源，不能只依赖页面标签。
- INT8 conversion 的类别质量和阈值未经过专业图库大规模校准；本里程碑只报告原始相似度与测试夹具行为，不宣称准确率。
- `ort` 会增加 native runtime 和安装包体积；必须验证 DLL 随包位置、离线启动和 CPU fallback。
- 当前模型 inference 与扫描都在同一 Rust 进程；通过单 worker、暂停/取消点和错误隔离降低资源竞争，崩溃隔离 sidecar 留给后续 ADR。
- Git 无跟踪基线；不覆盖现有用户文件，所有编辑使用明确文件清单并在末尾静态搜索和完整测试。

## 进度记录

### 2026-08-07 基线审计

- 已完整阅读项目说明、需求、架构、数据模型、UI 规范、模型评估、前后端代码、`0001` migration、现有 ADR、测试与 Git 状态。
- 已确认模型和任务表是预留结构，不足以满足阈值、embedding、主标签、源指纹、单项失败和重启恢复。
- 已确认真实组合筛选必须从 `Repository::list_assets` 开始重构，不能在 React 当前页数组上过滤。

### 2026-08-07 实施进展

- TinyCLIP INT8 与 ONNX Runtime CPU 已完成 SHA-256 校验、真实推理和仓库资源基准；模型缺失 fallback 不生成标签。
- `0002` migration、后台任务恢复、SQLite any/all 与多维组合筛选、主标签统计已接入；Rust 18/18 测试通过。
- 深色三栏、双侧折叠、网格/单图、胶片栏、右侧 EXIF/语义检查器、语义任务条与筛选分组已完成。
- 浏览器确定性夹具已验证 1920×1080、1440×900、1366×768 和 960×720 均无页面横向溢出，并保存截图。

## 验收记录

- 前端最终验证：Prettier、ESLint、TypeScript、Vitest 6/6 和 Vite production build 均通过；production bundle 不含视觉验收 fixture 文案。
- Rust core 最终验证：`rustfmt --check`、gnullvm `cargo test --no-default-features --all-targets` 18/18、Clippy warnings denied 均通过。完整 Tauri desktop 编译未能越过本机缺少 MSVC linker 的外部工具链门槛。
- 真实 TinyCLIP release CPU 基准：48 张仓库 PNG，失败 0，平均 23.8269 ms，P50 23.1080 ms，P95 30.6964 ms，吞吐 41.9687 张/秒；实际 backend 为 CPU，报告包含模型 SHA-256 与真实样例预测。
- SQLite repository 测试覆盖语义 any/all 与影调、主色、亮度、饱和度联合条件，筛选总数和分页共用相同条件；安全测试确认临时源夹具字节与哈希不变。
- 浏览器 smoke 覆盖网格、单图、胶片栏、左右折叠、语义 AND 组合筛选和主标签分组；1920×1080、1440×900、1366×768 与 960×720 均无页面溢出。修复确定性 fixture 的重复标签键后，新页面控制台 error/warn 为 0。
- `npm.cmd run tauri build` 的前端 production 阶段通过，随后 Rust MSVC 编译明确失败：`linker link.exe not found`。2026-08-06 的 GNU/LLVM NSIS 是旧里程碑产物，不包含本次 TinyCLIP/工作区变更，发布闭环审计已将其删除。
- 未完成项：正式 MSVC 安装包、打包 WebView2 smoke、干净 Windows VM、签名/MSI、GPU/NPU provider、峰值内存与授权真实摄影集分类质量评测。
