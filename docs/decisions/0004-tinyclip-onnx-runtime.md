# ADR-0004：TinyCLIP 本地语义分类与随包分发

- 状态：已接受
- 日期：2026-08-07
- 替代：细化 ADR-0002，并替代 ADR-0003 的“起步版本不分发模型”结论

## 背景

PhotoOrganizer 需要在 Windows 桌面端离线提供多标签语义分类，同时不得上传图片、调用付费 API、要求 Python/CUDA 或伪造标签。目标标签同时包含对象、场景和视觉类型，固定 ImageNet 单标签分类器不能覆盖夜景、室内、街道、截图和文档等类别。

## 决策

集成 `TinyCLIP-ViT-8M-16-Text-3M-YFCC15M` 的完整 INT8 ONNX conversion，并使用 ONNX Runtime 1.24.1 CPU execution provider。模型、tokenizer、预处理配置、许可证和 `onnxruntime.dll` 都作为 Tauri resource 随安装包分发。

- TinyCLIP 代码与原权重：MIT。
- onnx-community conversion：MIT 模型仓库，保留原模型来源。
- ONNX Runtime：MIT，连同上游许可证和第三方声明归档。
- 模型文件：24,281,512 bytes，SHA-256 `10921310ddef06557ec1598d1260470a0a4db53f70ffe0deb60b946dcad6d27a`。
- ONNX Runtime DLL：14,131,232 bytes，SHA-256 `8a1aad8d59d02a5337d4e3f5bbd1158c3f7bf84fe3b3f0052f957dd3e75a91cb`。

运行时在建图前验证模型和 tokenizer SHA-256，通过动态加载应用资源目录中的 DLL 建立 Level 3 优化 session。当前发行只启用并显示实际成功验证的 CPU provider；DirectML、CUDA 和 NPU 不声明为可用。

模型输出 512 维图像/文本 embedding。应用以英文提示词计算余弦相似度，按版本化阈值选出最多四个标签；UI 仅称为“相似度”。主标签是达到阈值的非 `unknown` 标签中相似度最高者，同分按 catalog 顺序；没有标签达到阈值时才写入 `unknown`。

## 比较

- OpenAI CLIP ViT-B/32：零样本能力成熟，但权重超过 300 MB，CPU 与安装包成本明显更高。
- ONNX Model Zoo SqueezeNet：约 5 MB，但仅适合 ImageNet 对象单标签，目标类别覆盖不足。
- Apple MobileCLIP：体积与移动性能有优势，但需要额外 ONNX 导出、Windows 验证和更复杂的许可归档。
- TinyCLIP INT8：24.3 MB、现成 ONNX、多场景零样本匹配、CPU 可运行，当前里程碑综合成本最低。

## 结果与限制

- 用户不需要网络、Python、CUDA、环境变量或模型目录配置。
- 包体增加约 38.4 MB（未计压缩和第三方通知），但获得离线开箱即用路径。
- 模型缺失或校验失败时继续使用 `UnavailableClassifier`；图库、扫描、缩略图和传统分析不受影响，也不会出现假标签。
- INT8 零样本分类的提示词和阈值仍需更大的授权图片集逐类校准；本里程碑不宣称准确率。
- 未来启用硬件 provider 必须新增基准、发布资源和真实启用探测，不能只增加枚举。

## 来源

- TinyCLIP 论文：<https://arxiv.org/abs/2309.12314>
- Microsoft Cream/TinyCLIP：<https://github.com/microsoft/Cream>
- 原权重：<https://huggingface.co/wkcn/TinyCLIP-ViT-8M-16-Text-3M-YFCC15M>
- ONNX conversion：<https://huggingface.co/onnx-community/TinyCLIP-ViT-8M-16-Text-3M-YFCC15M-ONNX>
- ONNX Runtime：<https://github.com/microsoft/onnxruntime>
