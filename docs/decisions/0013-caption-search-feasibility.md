# ADR-0013：图片描述与 FTS 搜索可行性

- 状态：已接受（仅隔离、离线可行性 spike 为 conditional GO；立即生产接入与打包为 NO-GO）
- 日期：2026-09-28
- 关联计划：`docs/plans/0073-dual-model-classification-and-ai-search.md`

## 背景

当前产品范围明确排除“大模型描述”，路线图也将其列为未排期事项（`docs/requirements.md:68`、`docs/roadmap.md:155`）。当前可用搜索继续基于 SigLIP2 向量；Rust 侧使用 `ort = 2.0.0-rc.12` 并启用 DirectML，SQLite 通过 `rusqlite` bundled。仓库尚无图片描述任务/结果表或 FTS5 索引；bundled SQLite 的 FTS5 能否用于实际发布构建仍未验证。

Florence-2-base-ft 的官方模型卡当前标示 MIT、0.23B 参数，并列出 caption 任务；模型卡示例使用 `trust_remote_code=True`，因此需固定不可变 revision、审计远程代码并分别核验权重、导出物、tokenizer 与相关代码的许可证。[Microsoft Florence-2-base-ft 模型卡](https://huggingface.co/microsoft/Florence-2-base-ft)

ONNX Community 提供面向 Transformers.js 的 Florence-2-base-ft 导出示例。它不是当前 Rust `ort` / DirectML 兼容性证据。[ONNX Community 模型页](https://huggingface.co/onnx-community/Florence-2-base-ft) Transformers.js 文档覆盖本地/自定义模型使用，但不证明项目 WebView2 上的性能或硬件兼容。[Transformers.js 自定义/离线用法](https://huggingface.co/docs/transformers.js/custom_usage)

Transformers.js dtype 指南特别指出 Florence encoder 对量化较敏感；示例保留 `embed_tokens` 与 `vision_encoder` 为 fp16，只将 `encoder_model` 与 `decoder_model_merged` 设为 q4，且示例设备是 WebGPU。不能据此假设全子图 q4 的体积或质量，也不能把 WebGPU 结果外推为本项目 DirectML 实测。[Transformers.js dtype 指南](https://huggingface.co/docs/transformers.js/guides/dtypes) 项目 DirectML Provider 的实际可用性必须在当前 runtime 和目标 Windows 环境验证。[ONNX Runtime DirectML 文档](https://onnxruntime.ai/docs/execution-providers/DirectML-ExecutionProvider.html)

SQLite FTS5 是词法全文检索，不提供语义翻译。其 trigram tokenizer 以三字符片段索引；少于三个字符的查询不能按普通 trigram 全文匹配预期工作，因此短中文查询必须单独验证。[SQLite FTS5 文档](https://www.sqlite.org/fts5.html)

## 决策

1. **Conditional GO 仅限隔离、离线的 caption + FTS 可行性 spike。** Spike 不接入当前产品流水线，不修改生产数据库、IPC、UI、模型资源清单或安装包，不新增生产依赖。仅在已固定并审计的文件全部本地就绪后断网运行。
2. **当前不批准任何生产接入或打包。** 所有下列证据门槛通过后，仍须另行取得明确产品批准；在生产实现开始前，必须同步更新 `docs/requirements.md` 与 `docs/roadmap.md`，反映功能范围、里程碑、依赖和发布资源。本 ADR 不是对现有产品范围的变更。
3. 分类与 AI 搜索保持彼此独立。现有 SigLIP2 向量搜索继续保留并可继续使用；caption/FTS spike 失败时，生产方案留在当前实现，不替换、不阻断或改写分类与向量搜索。
4. 所有模型输入必须是应用生成的 `grid-640-v1` 缩略图或经批准的有界派生图。不得把原图像素缓冲或原图路径传给模型；caption 不得进入导入关键路径。自动技术与导入回归测试只使用 `test-data/`。caption/中文检索质量 spike 仅可由用户日后明确发起，并只临时读取用户选定图库中已生成的 `grid-640-v1` 应用缩略图；不得访问/遍历个人原图目录、复制/保存照片或持久化 caption、标注或可复用数据集，只保留不含内容的聚合指标。本轮仅起草文档，不实际操作任何图库。
5. “英文 caption + 双语 query mapping”仅是待证假设：尚无证据证明中文搜索能稳定命中英文描述。FTS5 不会自动翻译或赋予查询语义；短于 tokenizer 有效粒度的中文词、尤其少于 trigram 三字符长度的查询，必须计入独立质量结果，不能默认为成功。

## Spike 证据门槛

以下条件须全部通过并归档，才可向产品申请下一阶段批准；缺项、无法复现或失败均保持生产 NO-GO。

1. **来源与完整性：** 固定每个上游仓库的不可变 revision；审计模型卡中的 `trust_remote_code` 及执行代码；逐文件核验所选权重、ONNX 导出、tokenizer、处理器配置和运行时的来源、许可、再分发条件，并记录本地 SHA-256。MIT 模型卡标记不自动许可第三方导出或依赖。
2. **CPU 离线端到端：** 在完全断网环境，以 CPU 对 `test-data/` 自动技术测试集完整生成 caption，输入由应用有界流程生成且仅为 `grid-640-v1` 缩略图，失败数为零。CPU 是必需能力，不得要求 DirectML 才能完成基础任务。DirectML 仅为可选项，须在项目当前 `ort` 版本及实际支持设备上完成真实兼容、质量、延迟与资源测量，并显示相对 CPU 的实际收益；仅有 Provider 检测或 WebGPU 数据不算通过。
3. **部署文件与体积：** 给出精确的文件子集、精度/量化配置、revision、逐文件 SHA-256 与总包体，并预先声明可接受的包体上限；只能按实际拟部署组合计算。2026-09-28 观察到的 ONNX Community [`main` 树页面](https://huggingface.co/onnx-community/Florence-2-base-ft/tree/main) 约 5.37 GB 是可变 `main` 树中所有变体/文件的合计观察值，不是一个部署组合大小，也不是可复现预算；实际方案必须 pin 不可变 revision 并逐文件校验。
4. **FTS5 可用性：** 通过项目发布所用 bundled SQLite 实际创建并查询 FTS5 虚拟表，记录 SQLite 版本与编译/运行验证。若该构建未启用 FTS5，spike 失败；不得静默替换为不同生产依赖。
5. **真实图库检索与描述质量：** 此门槛必须来自用户明确发起的一次性真实图库试验，不能用人工/合成 fixture 冒充产品效果。用户选定约 100 张代表性图片后，试验仅读取这些资产已生成的应用 `grid-640-v1` 缩略图；不遍历原图目录、不复制/保存图片、不持久化 caption、查询或人工标注。使用至少 20 组中英文等价查询和一次性人工判分：至少 90% 描述的主要主体/场景基本正确，明显关键事实幻觉不超过 5%；median Precision@10 不低于 0.70，中文与英文 median 差异不超过 0.15。单独报告一/二字中文、trigram 短于三字符、连续中文、混合中英、标点、同义表达与漏召回；短查询不得被静默解释为“无结果”。仅保留不含照片内容的聚合指标与错误类别。
6. **速度与内存：** 在试验前声明参考机器、CPU caption P50/P95 延迟上限、CPU 峰值 RAM 上限、可选 DirectML 的 RAM/VRAM 上限与测量方法；Spike 只有在满足预声明上限时通过。缺少目标设备或预算本身即表示此门槛未通过，不得事后按测得结果放宽。
7. **导入隔离与回归：** 使用固定的 1,000 张 `test-data/` fixture、相同 release 配置，至少三次对照当前导入基线与仅已安装 spike 组件的扫描。扫描期间 caption 推理调用数必须为零，失败数为零，导入墙钟 median 回退不得超过 5%。
8. **数据处理：** 不训练或微调模型，不建立、保留或发布用户照片/标注训练集。自动测试输入限于 `test-data/`；用户发起的质量评估只临时读取所选资产已有缩略图及内存中的一次性评分。不得持久化照片、caption、查询或标注；只保留不含照片内容的聚合指标、配置、revision/hash 与错误统计。不得联网传输图片、caption、查询或遥测。

## 方案比较

| 方案                             | 取舍与迁移影响                                                                                                                                                                                                                                                                          | 决定                                                               |
| -------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------ |
| Rust `ort` 手写 encoder-decoder  | 可留在现有 Rust/SQLite 单进程，复用 CPU 与 DirectML 接口且不增加生产 sidecar；但须手工实现图像处理、tokenizer、encoder/decoder 输入输出契约、自回归生成与停止条件，并审计导出图和输出后处理。ONNX Community 的 Transformers.js 导出不能证明此路径兼容。原型成本和 DirectML 风险尚未知。 | 仅可在 spike 中验证；不是已批准的生产实现。                        |
| Python + Transformers sidecar    | 最接近模型卡示例，可能更快确认原始 Transformers 输出；但会引入 Python 运行时/包体、进程监督和 IPC、版本/依赖/许可面及额外内存，并与 ADR-0005 的单进程 Rust 决策冲突。                                                                                                                   | 不选作本轮生产路径；若 spike 表明必须采用，需另行 ADR 和产品批准。 |
| Transformers.js worker / browser | ONNX Community 导出明确以 Transformers.js 为目标；custom/offline 文档可供隔离验证。代价是新增 JS runtime 依赖和 WebView worker 生命周期、内存及硬件条件；dtype 示例是 WebGPU，不证明当前 WebView2、Rust `ort` 或 DirectML 的兼容与收益。                                                | 可作为 spike 的独立比较对象；不进入当前应用或发布包。              |
| 保留现有 SigLIP2 向量搜索        | 零迁移、零新增依赖，现有搜索保持工作；它不提供可供 FTS 词法检索的 caption 文本，且其中文表现仍受现有检索能力约束。                                                                                                                                                                      | 继续作为当前生产行为；也是 spike 失败时的回退状态与可选比较基线。  |

## 影响与后续

- 本 ADR 不授权数据库 migration、caption worker、FTS schema、IPC/UI、模型下载或安装包变更。当前分类、现有 SigLIP2 向量搜索及应用数据不迁移。
- Spike 若未通过全部门槛，关闭该研究并保留当前产品方案；不自动改用 Python、浏览器模型或其他依赖。
- Spike 若通过，结果只满足“可以提交下一阶段产品决策”的条件，不等于产品批准。未来生产工作开始前另需明确批准并同步改写需求与路线图；之后再单独审议部署架构、数据迁移、许可与发布门槛。
- 不得因本 ADR 将“大模型描述”从当前排除/未排期项视为已纳入 MVP。
