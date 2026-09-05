# ADR 0012：Windows DirectML 执行 Provider

状态：已接受（S1 代码已实施；本机 DirectML 缩略图冒烟通过，长图库性能校准继续在 S2）

## 背景

S0 只完成了 Windows 图形适配器诊断和设置页门禁。仅检测到独立 GPU 不能证明 ONNX Runtime 能够加载模型；模型会话、运行库和 Provider 必须在同一台机器上完成真实初始化。

## 决策

1. Windows 桌面包使用 Microsoft.ML.OnnxRuntime.DirectML 1.24.1 的 x64 native 运行库。
2. onnxruntime.dll 与 onnxruntime_providers_shared.dll 一起放入 Tauri runtime 资源目录，并在启动/模型加载边界做 SHA-256 校验。
3. Rust ort 开启 directml feature。模型会话只有在用户启用 GPU 且 DirectML 状态为 ready 时，才注册 DirectML Provider，并优先选择高性能 GPU。
4. Places365、SigLIP 2/MobileCLIP 和 PicoDet/YuNet 共用 backend-aware session builder。每个 classifier 的运行时状态记录实际选中的 backend。
5. DirectML 会话创建或模型自检失败时，当前模型自动回退 CPU，并把 Provider 标记为 error；不会把“检测到显卡”伪装成 GPU 已启用。
6. CPU 保持默认路径，CPU 任务批大小上限为 8；DirectML 的有效上限按选定适配器专用显存分级，最高 32。worker 仍是单任务、有限批处理；批次执行失败时在相同后端递归减半，最终隔离到单图重试。
7. CUDA、cuDNN、Python sidecar、远程推理和 WebView2 GPU 合成不在本 ADR 范围内。WebView2 的渲染开关与 ONNX 模型 Provider 独立。

## 影响

- 安装包增加 DirectML 版 ONNX Runtime 主 DLL 和 shared provider DLL；包体变化由发布检查脚本记录。
- DirectML 是 Windows 系统组件，不随应用重新分发；旧版 Windows/驱动或不支持的模型会进入 CPU 回退。
- 前端只根据 Provider 自检结果启用 GPU 开关；模型状态仍显示实际 backend，便于诊断。
- DirectML 支持动态输入时可能受 shape 影响；S2 使用应用缩略图、warm-up 和批延迟统计建立保守显存分级，并在运行时失败后按同一后端递归降批。实时显存预算和模型级动态 shape 诊断继续后置，不把容量启发式当作实时占用。

## 验证

- 运行库主 DLL 与 shared DLL 哈希由 scripts/verify-release-resources.ps1 校验。
- Rust 单元/全目标编译验证 backend 传递和 CPU fallback。
- 已用应用生成的 \`grid-640-v1\` 缩略图在本机执行 Places365 DirectML 冒烟：3 张、失败 0、状态 completed；原图路径被缩略图边界拒绝，符合项目规范。
- 无独立 GPU、Provider 不可用、模型会话失败三种状态都必须保持 CPU 可用。
- 有独立 GPU 的机器还需手动 acceptance：设置页显示 DirectML ready，装载模型状态为 direct_ml，分析进度记录 direct_ml；关闭 GPU 后重新装载恢复 cpu。
