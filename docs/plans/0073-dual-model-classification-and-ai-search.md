# 0073 筛选分类与 AI 搜索双模型流水线执行计划

状态：ADR-0013 已接受；Conditional GO 仅限隔离、离线的 caption + FTS 可行性 spike。原型待验证、尚未实现。生产接入与打包为 NO-GO，直到 spike 全部门槛通过且另行获得明确产品批准。本轮仅更新文档，不改变产品范围。

## 目标与范围

本计划当前仅授权按 [ADR-0013](../decisions/0013-caption-search-feasibility.md) 进行隔离、离线可行性 spike，不授权实现产品功能。若未来另行取得产品批准，分类与 AI 搜索才可按下列原则设计为两套独立流水线：

1. **筛选分类流水线**继续使用当前模型：SigLIP2 负责摄影题材候选，Places365 提供场景证据，PicoDet 识别主体，YuNet 辅助人像判断。保持现有题材/主体标签、筛选字段及分类任务契约；搜索描述不得创建、修正或覆盖分类结果。
2. spike 评估预训练 Florence-2-base-ft 与本地 SQLite FTS5 的可行性；假设描述只服务 AI 搜索，不参与筛选分类。英文 caption 与双语 query mapping 仅是假设，中文检索尚未得到证明。
3. spike 使用应用生成的 `grid-640-v1` 缩略图：自动技术与导入回归仅使用 `test-data/` fixture；真实质量试验须由用户明确一次性发起，可临时读取现有图库中已生成的应用缩略图。流水线、任务、状态与数据隔离是未来设计目标，不代表现有实现。
4. 未来生产任务必须在导入完成后低优先级运行、支持恢复且永不进入导入热路径；当前 spike 不接入导入/分类调度器。
5. spike 全部门槛通过也只允许提交后续产品决策；不训练模型，不持久化用户照片/训练集，不在本轮打包或启用模型。

## 现状与依据

- `src-tauri/src/topics.rs` 使用 SigLIP2 进行题材候选判断；`src-tauri/src/semantic_tasks.rs` 协调题材和主体分析；`src-tauri/src/subject.rs` 使用 PicoDet/YuNet；Places365 保持场景证据角色。模型资源、来源和许可分别记录在 `src-tauri/resources/models/` 下的 `MODEL-SOURCE.md`。
- 当前分类任务已有 `analysis_jobs` / `analysis_job_items`、暂停/恢复/取消和重启恢复能力。分类结果按模型、分析版本、taxonomy 版本和源指纹保存；资产上的 `semantic_status` 属于分类状态，不应用作描述任务状态。
- 当前 `search_local_images` 在 `src-tauri/src/workflow.rs` 中对 SigLIP2 文本/图片 embedding 做余弦排序；相关 IPC 在 `src-tauri/src/ipc.rs`，前端调用在 `src/api.ts`。现有向量搜索继续保留为当前产品行为；未来经批准的描述/FTS 搜索须独立，spike 失败时保持当前方案。
- 当前未发现图片描述专用结果表、描述任务契约或 FTS5 检索表。`src-tauri/Cargo.toml` 当前为 `ort = 2.0.0-rc.12`、启用 DirectML，`rusqlite = 0.37` 使用 bundled SQLite；发布构建中的 FTS5 可用性仍须验证。
- `docs/requirements.md` 现将大模型描述列为排除项，`docs/roadmap.md` 尚无描述索引里程碑。后续进入实现前，ADR 通过后需在对应实现任务中同步修订用户需求、路线图、发布资源与第三方许可文档；本计划不修改这些文件。
- Florence-2-base-ft 官方模型卡标示 MIT、0.23B 并列出 caption 任务，但示例包含 `trust_remote_code=True`；ONNX Community 页面展示的是 Transformers.js 导出，不证明其兼容当前 Rust `ort` / DirectML。revision 固定、远程代码审计和逐文件许可/hash 检查都是 spike 门槛。[Microsoft 模型卡](https://huggingface.co/microsoft/Florence-2-base-ft)；[ONNX Community 模型页](https://huggingface.co/onnx-community/Florence-2-base-ft)
- Transformers.js dtype 指南的 Florence 示例对 encoder 量化敏感：`embed_tokens`、`vision_encoder` 用 fp16，而 `encoder_model`、`decoder_model_merged` 用 q4，且目标为 WebGPU；不能用全子图 q4 估计部署体积或质量，也不能当作 DirectML 实测。[dtype 指南](https://huggingface.co/docs/transformers.js/guides/dtypes)；[DirectML 文档](https://onnxruntime.ai/docs/execution-providers/DirectML-ExecutionProvider.html)
- FTS5 是词法检索，不提供语义翻译；trigram 需要三字符片段，中文短查询及有效 tokenizer 粒度下的短词必须单独测量，不得宣称中文检索已经证实。[SQLite FTS5 文档](https://www.sqlite.org/fts5.html)

## 不可破坏的边界

- 所有导入、分类及描述推理仅消费应用私有缩略图或其有界派生图。原图只可用于项目既有的文件元数据、指纹、EXIF/嵌入预览和受控缩略图生成；不得把原图路径传给 Florence、预处理、重试、恢复任务或模型。
- 描述任务只在当前图库已提交资产记录且有效缩略图可用时入队。缩略图缺失、路径越界、尺寸超限或源指纹过期时必须跳过/重新排队并给出独立状态，不得回退到完整解码原图。
- 描述数据及其索引只保存在应用本地数据库；不上传图片、描述、搜索词或遥测数据，不增加账号、网络服务、API Key 或云端推理。
- 任何模型权重、ONNX 导出、tokenizer、转换代码和新增运行时依赖都需要独立核对来源、版本、哈希、许可与再分发条件。模型卡的许可声明不能自动覆盖第三方转换产物和依赖。
- 描述文本是检索线索，不是校准事实或分类标签。界面不得把模型描述包装成已核验的照片事实。

## 分阶段实施

### 阶段 0：架构与模型可行性 ADR（已完成；不等于产品批准）

已由 [ADR-0013](../decisions/0013-caption-search-feasibility.md) 完成方案比较与决策边界：

- 决定仅对隔离离线 spike 为 conditional GO，生产接入/打包当前 NO-GO。
- Rust `ort` 手写 encoder-decoder、Python/Transformers sidecar、Transformers.js worker/browser 与保留 SigLIP2 向量搜索均已比较；没有选定生产推理形态或新增依赖。
- 生产授权仍需通过 ADR-0013 的 revision/hash/licensing、CPU offline、精确包体、FTS5、双语检索质量、latency/memory、导入回归及隐私门槛，并另行取得产品批准。

**当前结论：**原型待验证、未实现。门槛未通过前，不改数据库/API/UI、模型资源清单或生产依赖；若 spike 失败，留在当前 SigLIP2 搜索方案，不自动改用备选 runtime。

### 阶段 1：条件准许的隔离离线可行性 spike（待验证、未实现）

采用两条严格分开的数据路径：自动技术验证与导入回归只使用 `test-data/` fixture 经应用有界流程生成的 `grid-640-v1` 缩略图；caption/中文检索质量验证仅可在用户日后明确发起并选择样本后，临时读取所选资产已经生成的 `grid-640-v1` 缩略图。后者绝不访问/遍历个人原图目录、不复制或保存图片、不持久化 caption、查询或人工标注，仅保留不含照片内容的聚合指标。此为一次性抽样而非数据集。本轮仅编辑文档，不实际操作任何图库。整个 spike 与生产应用隔离，断网运行，不接入当前数据库/任务调度/产品 UI；只保留配置、revision/hash 与错误类别等非内容信息。

- 仅在真实质量评估时，由用户明确选择当前图库中约 100 张有代表性的现有资产，并只读取已生成的 `grid-640-v1` 应用缩略图；覆盖常见题材、人像/多人、弱光、静物、动植物、建筑/街拍和易混场景，混入不依赖现有分类标签的抽样以降低偏差。此人工质量试验不能由 `test-data/` fixture 结果代替。
- 评审 caption 是否准确描述主要主体、动作和场景；记录关键漏写、明显幻觉、英文/中文输出比例、单张生成耗时、P50/P95、吞吐、峰值进程内存与可获取的 GPU 显存。
- 使用至少 20 组中英文等价查询，覆盖主体、动作、颜色、环境、时间和组合描述。英文 caption + 双语 query mapping 只是待证假设；人工审阅 top-10、计算 Precision@10，并记录语言失配与漏召回。
- 对 FTS5 `unicode61` / `trigram` 做查询对照，重点测一至两个汉字、短于 tokenizer 有效粒度的中文、trigram 少于三个字符、连续中文、混合中英、空格/标点及同义词。FTS5 不做语义翻译。
- 若启用现有 SigLIP2 向量搜索作为对照，只在评测中并排记录；不得将其设为图片描述、FTS 查询或新功能可用性的必需依赖。
- 必须完整离线生成全部固定样本；CPU 路径必需，DirectML 仅在当前 `ort` + 实际目标机器上实测兼容和收益后才作为可选结果。精确文件子集、immutable revision、逐文件 SHA-256 与总包体须一并记录。Transformers.js 的 WebGPU dtype 示例不是 DirectML 证据；全子图 q4 不是可接受的尺寸估算。
- 预先声明参考机、CPU P50/P95 latency 与峰值 RAM 上限、可选 DirectML RAM/VRAM 上限和包体限额；未声明预算即未通过 gate，不得按结果事后放宽。

**质量门槛（ADR-0013 固化）：**100 条 caption 中至少 90% 的主要主体/场景描述基本正确，明显关键事实幻觉不超过 5%；20 组等价查询 median Precision@10 不低于 0.70，中英文 median 差异不超过 0.15，并单列短中文结果。延迟/内存/包体遵循预注册设备预算。测试输入只用于一次性可行性判断，不用于训练、微调或自动调参。

**阶段 2–5 生产门禁：** 以下数据库、推理任务、API/UI、测试与发布条目仅保留为未来候选需求。只有阶段 1 所有门槛通过，且产品明确批准并先行更新 `docs/requirements.md` 与 `docs/roadmap.md` 后，才可建立独立实现任务和重新审阅此设计；不代表本轮可以开始任何生产接入。

### 阶段 2：数据库与版本化结果契约

新增独立 migration；先确认 bundled SQLite 的 FTS5 编译能力及目标版本，不破坏既有语义数据。具体表名由 ADR 决定，最低数据契约如下：

- 图片描述记录独立于 `semantic_labels`、`subject_labels` 和 `semantic_embeddings`，至少有 asset ID、描述文本/语言、模型名与模型版本、权重哈希、caption/预处理分析版本、源指纹、实际执行后端、生成时间、状态和错误信息。
- 描述任务有独立的队列项、重试次数、已处理/失败/跳过计数、当前资产、暂停/取消/恢复状态。可以复用通用任务存储代码，但所有读写必须按独立 task type 隔离，不得读写分类的 `semantic_status` 或覆盖分类任务状态。
- FTS5 索引独立维护，至少能查找有效当前版本的描述；是否一并索引文件名/标签须由 ADR 定义。索引更新与描述结果提交保持事务一致，删除/替换/失效描述时索引同步删除或重建。
- 新模型或 caption 规范变更只使描述记录过期并重建描述索引；SigLIP2/Places365/PicoDet/YuNet 的版本或分类 taxonomy 变更不应自动触发图片描述重算。源指纹变化仅使相关资产各自的派生结果失效。
- 记录 tokenizer、索引 schema 和查询规范化版本，确保索引迁移、重建和故障恢复可诊断；FTS 内容不可与过期模型版本混合排名。

### 阶段 3：独立描述推理与低优先级调度

- 新增独立描述模型接口、运行状态和错误报告；模型加载与分类器状态分开，缺少描述模型时筛选分类、浏览和现有搜索不受影响。
- 描述任务在图库导入完成后排队；导入/重扫正在进行、现有高优先级分类任务积压或用户前台操作需要模型资源时，描述 worker 不争用推理资源。每次只处理有界小批次（建议从单张开始），在图片间检查暂停、取消、扫描状态与优先级。
- 为 ONNX Runtime session 与设备建立应用级资源仲裁：同一 GPU 上的高占用模型不得无界并发；记录所选及实际后端。DirectML 不可用或推理失败时，按明确策略回退 CPU 并报告；CPU 回退也失败时保存描述任务失败状态，不影响分类状态。后端切换和单张重试必须幂等。
- 资源不足时允许延迟或暂停描述模型加载/推理；不允许通过提高导入并发或突破当前有界图片解码来“追赶”描述队列。生成长度、单张超时和可中断边界在原型阶段实测后设定。
- 应用重启后恢复 queued/paused/interrupted 描述任务，已完成且源指纹/模型版本匹配的记录跳过；源已变化或缩略图不可用的任务明确重新排队/跳过。重试、恢复路径继续校验输入必须位于应用 thumbnail cache 并满足尺寸约束。

### 阶段 4：本地文字检索 API 与 UI

- 增加描述模型状态、描述任务进度/控制和 FTS 搜索的独立 IPC/API；不复用 `search_local_images` 的相似度阈值契约来假装 FTS 排名，也不让 AI 搜索依赖分类模型已装载。
- 结果限定当前图库，返回资产、FTS 排名/匹配摘要以及描述版本/可用性；过滤失效指纹和非当前描述版本。参数化构造安全查询，覆盖 FTS 特殊语法、空白、标点、Unicode 与恶意查询输入。
- 搜索界面准确显示描述索引覆盖量；尚未生成描述、任务进行中、部分失败、模型不可用及无匹配结果须可区分。新任务在后台任务面板有自己的进度、暂停/继续/取消和错误详情，不合并导入/分类计数。
- 搜索结果继续复用现有图库资产卡片、选择/星级/色标/定位行为；检索描述只影响命中和排序，不写入筛选器，不影响人工标记和筛选分类结果。
- 可提供“使用现有向量检索回退”的独立可选路径/偏好；其关闭、模型缺失或索引未完成时，FTS 描述搜索仍完整可用。是否自动合并两种排名属于未定决策，需有独立质量证据和 ADR 才能实施。

### 阶段 5：测试、文档与发布验收

实现过程中补充以下测试。文件测试只能使用 `test-data/` 或测试自建临时目录；自动测试不得访问个人照片目录。

- **边界与模型：**高分辨率 fixture 验证模型仅收到应用 `grid-640-v1` 缩略图或有界派生输入，绝不创建/传递完整原图像素缓冲；测试模型资源缺失、损坏、初始化失败和 CPU/GPU 后端报告。
- **迁移与结果：**升级旧数据库保留全部分类结果；描述表/FTS5 首次创建、重复初始化、重建和事务回滚正确；模型/分析版本切换只重排描述；源指纹变化、资产删除、失败重试和 FTS 清理一致。
- **任务：**覆盖导入期间不启动描述推理、低优先级让出 GPU/CPU、队列有界、暂停/继续/取消、进程重启续跑、断点幂等、资源不足和错误隔离；描述失败不能令分类失败，分类重试不能清除描述状态。
- **搜索：**使用生成的 fixture 描述验证中文/英文、混合词、同义词、短词、标点/引号、空查询、Unicode 文件名、无结果、部分覆盖、过期记录、排序稳定和图库隔离；验证 FTS5 缺失的受控错误/禁用体验。
- **API/UI：**分别测试两个任务状态及进度；确认现有筛选值/标签不受描述影响，分类与描述模型可分别 unavailable；搜索仍可用文件名/描述索引，不要求先加载 SigLIP2；结果继续使用既有卡片交互。
- **性能：**在 `test-data/` 生成固定 1,000 张测试图片，使用同一 release 配置至少三次对比当前导入基线与描述功能开启后的扫描过程。扫描期不得有 caption inference，导入 wall-time 中位数相对基线回退目标不超过 5%，失败数为 0；另记录 caption CPU/GPU 的 P50/P95、吞吐、峰值 RAM/VRAM、暂停响应及恢复时间。设备内存硬上限须在 ADR 根据目标机测量确定，不凭空承诺。
- **用户图库质量试验：**仅由用户明确发起，对所选图库一次性读取已生成 `grid-640-v1` 应用缩略图；不遍历原图目录、不复制/保存照片、不持久化 caption、查询或标注。自动技术/回归测试必须使用 `test-data/`。完成后只记录聚合指标与错误类别。未取得真实图库缩略图质量/速度试验结果前，不得声称已改善实际搜索效果或完成发布验收。
- **发布：**核验 ONNX/runtime 兼容、资源校验脚本、包体变化、SBOM、`THIRD_PARTY_NOTICES.md`、模型来源/许可证、干净离线环境启动与缺模型降级。任何新增生产依赖或资源格式必须在 ADR 与发布说明中明确。

## 未来生产接入完成验收标准（不适用于当前 spike）

以下标准属于未来生产任务，不表示当前功能已获批准。当前 spike 是否可申请后续产品决策，以 ADR-0013 的全部证据门槛为准。

- 筛选分类继续由 SigLIP2、Places365、PicoDet 和 YuNet 各司其职；分类标签与现有筛选接口/存储语义保持兼容。分类与描述的进度、错误、版本、重试和重跑互不改变。
- AI 搜索使用本地生成描述和 FTS5，描述只从应用缩略图产生；中文、英文及短中文匹配试验达到 ADR 确认的质量门槛，漏召回和未覆盖状态能被观察，而非静默误报“无结果”。
- 描述处理始终低于导入及现有分类任务，导入期间推理调用数为 0；重复 release 基准达到上述导入不回退目标。暂停/续跑、取消和重启恢复通过自动化测试。
- Florence 不可用或授权不通过时，应用可继续导入、分类、筛选和浏览；向量回退可以关闭，且新 AI 搜索不依赖它。
- 测试报告包括准确的模型 revision/hash、设备与后端、样本选择方式、CPU/GPU 性能、质量指标、未通过项和未执行项；没有以训练或自动调参方式使用评测样本。
- 更新需求、路线图、模型来源/许可、发布资源及用户界面说明；完成相关 Rust/SQLite、IPC、前端测试与质量风险要求的全量静态检查/构建；审阅最终 diff。实现任务完成后按用户既有要求提交本任务修改，提交范围仅包含已审阅且属于该任务的文件；是否推送按用户另行要求执行。

## 生产批准前仍未定的实现问题

1. Florence-2-base-ft 是否能以项目允许的方式导出并在当前 ONNX Runtime CPU/DirectML 组合中可靠运行；若需 Python、Transformers、自定义远程代码或另一推理栈，是否接受相应架构/发布成本。
2. 描述模型按需下载、可选手动放置还是随安装包分发；用户离线体验、安装体积及权重再分发许可的取舍。
3. 使用 `<CAPTION>`、`<DETAILED_CAPTION>` 或其他固定任务；输出英文、中文还是双语；最大输出长度/解码参数及描述如何提示为“模型生成”。
4. 描述任务是否默认自动启用；是否仅在扫描与分类队列完全空闲时运行；用户交互或电池/省电状态是否进一步降速。
5. 通用 `analysis_jobs` 中新增独立类型，还是建立独立 caption job/task schema；如何共享调度原语而不共享结果/状态生命周期。
6. FTS tokenizer、中文规范化和别名词典；文件名/人工标签是否纳入文本检索；短词及中英同义词如何处理，是否接受轻量词典维护成本。
7. 默认只按 FTS5/BM25 排名，还是允许用户可选地合并现有 SigLIP2 向量结果；需要何种实验结果才能启用融合。
8. 目标 Windows 设备的最大额外 RAM/VRAM、描述吞吐门槛、可接受队列完成时长与质量指标是否需按 CPU/GPU 档位区分。

## 非目标

- 不训练、微调或量化训练模型，不建立、发布或持续维护图像训练/评测数据集。
- 不改变筛选分类的 taxonomy、提示词、阈值、标签 API 或现有模型权重；不让描述参与分类决策。
- 不要求向量检索、Chinese-CLIP、向量数据库、ANN/HNSW、云端模型、在线翻译、账号或远程服务。
- 不做图片内容编辑、EXIF 写入、人脸身份识别、基于原图的全分辨率分析、OCR 专项索引或生成式问答；这些如需加入必须另立计划与许可/隐私审查。
- 本计划目前只准许离线 feasibility spike，不包含业务实现或生产接入。本计划与 ADR-0013 定义本轮文档范围；`requirements.md` 与 `roadmap.md` 保持原状，未来生产实现仍须先取得明确产品批准并同步更新这两份文件。

## 参考资料与项目边界

- Florence-2-base-ft 官方模型卡（任务、参数量、当前许可证标记与推理示例）：https://huggingface.co/microsoft/Florence-2-base-ft
- ONNX Community Florence-2-base-ft（Transformers.js 导出示例）：https://huggingface.co/onnx-community/Florence-2-base-ft
- Transformers.js custom/offline 使用文档：https://huggingface.co/docs/transformers.js/custom_usage
- Transformers.js dtype/量化指南：https://huggingface.co/docs/transformers.js/guides/dtypes
- ONNX Runtime DirectML Provider 文档：https://onnxruntime.ai/docs/execution-providers/DirectML-ExecutionProvider.html
- SQLite FTS5 官方文档（编译、tokenizer、短 trigram 查询、索引维护）：https://www.sqlite.org/fts5.html
- 项目现有约束：`AGENTS.md`、`docs/requirements.md`、`docs/roadmap.md`、`docs/release.md`、`docs/testing.md`。
- 相关实施依据：`docs/plans/0036-thumbnail-only-decode.md`、`docs/plans/0063-photography-label-taxonomy-v4.md`、`docs/plans/0065-startup-model-lifecycle-and-fallback.md`、`docs/plans/0068-import-performance-optimization.md`、`docs/plans/0052-search-result-gallery-bridge.md`。
- 当前实现入口：`src-tauri/src/topics.rs`、`src-tauri/src/semantic_tasks.rs`、`src-tauri/src/subject.rs`、`src-tauri/src/workflow.rs`、`src-tauri/src/ipc.rs`、`src-tauri/src/db.rs`、`src/api.ts`、`src/components/WorkflowWorkspace.tsx`、`src/components/BackgroundTaskStatus.tsx`。
