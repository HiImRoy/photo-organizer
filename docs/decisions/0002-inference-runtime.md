# ADR-0002：推理运行时

- 状态：已接受（接口已定，具体 runtime 延后）
- 日期：2026-08-06

## 决策

语义层使用可替换 `SemanticClassifier` 接口、结构化 `ModelMetadata` 和 execution backend 枚举。当前默认实现是 `UnavailableClassifier`，返回明确的“模型未安装或未启用”，绝不生成随机或硬编码标签。

未来首选直接集成 ONNX Runtime 的 Rust 适配层。后端自动探测顺序由发布构建决定：可验证的 GPU/NPU provider、通用硬件 provider、CPU。UI 不要求用户理解 provider，也不会因可选模型缺失而禁用图库、缩略图或色彩分析。

## 理由

- 纯 CPU 基础功能不依赖推理 runtime。
- 接口先行允许候选模型共享任务状态、取消、基准和结果 schema。
- 推迟链接 ONNX 避免在尚无合规模型时增加体积与原生 DLL 发布风险。
- 后台任务持久化并限制并发，为暂停、恢复、取消和版本失效预留明确边界。

## 门槛

任何 runtime/provider 成为默认前必须在受支持 Windows 环境验证：冷启动、P50/P95、吞吐、峰值内存、CPU 占用、失败恢复、包体和许可证。不得把模型相似度称为可靠概率。
