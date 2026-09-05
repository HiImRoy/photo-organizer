# 0061 GPU 性能 S1：DirectML Provider 与模型会话

状态：S1 代码实现完成；S2 批处理校准与运行时自适应已实现；真实长图库吞吐和实时显存诊断待继续验收

## 目标

把 S0 的 GPU 检测开关变成可验证的 Windows DirectML 推理路径，同时保留可观测、可恢复的 CPU fallback。

## 已完成

- 纳入 Microsoft.ML.OnnxRuntime.DirectML 1.24.1 x64 runtime 与 shared provider DLL。
- 开启 ort directml feature，统一构建 CPU/DirectML session。
- Places365、SigLIP 2、MobileCLIP、PicoDet、YuNet 记录实际 selected backend。
- DirectML provider 探测先经过运行库校验；模型准备失败自动回退 CPU。
- worker 按实际 backend 传递到批量/单图重试；CPU 上限 8，DirectML 初始上限按专用显存分级（最高 32），批次失败在同一后端递归减半。
- 任务进度记录分类器实际执行后端，界面显示 \`DirectML GPU\` 或 \`CPU\`，不再用“本地计算”掩盖后端。
- 活动模型保存实际后端，重启后台恢复时优先恢复 DirectML；会话失败安全回退 CPU 并显示原因。
- benchmark/evaluate 支持真实 DirectML 模型加载。
- 设置页 backend 选择与分析有效批大小保持一致。

## 未完成

- 在带独立 GPU 的真实 Windows 机器上继续测量 SigLIP 2、Places365、PicoDet/YuNet 的显存峰值与吞吐；当前已用应用生成的 \`grid-640-v1\` 缩略图完成 Places365 DirectML 3 张冒烟，失败 0，吞吐约 1.79 张/秒。
- S2 已基于 DXGI 专用显存建立保守初始 batch 上限，并增加同后端递归降批；实时显存预算、长图库吞吐和模型级稳定性仍属于后续 S2.3 校准。
- 对 DirectML 不支持的算子和动态 shape 增加更细的诊断。
- 比较 GPU 分析吞吐与 WebView2 渲染/预览的独立优化空间。

## 验收清单

1. 无独立 GPU：GPU 开关置灰，分析仍以 CPU 完成。
2. 有独立 GPU但 Provider 初始化失败：显示 error，不误报可用；模型装载回退 CPU。
3. DirectML ready：模型状态 selectedBackend=direct_ml，任务 progress executionBackend=direct_ml。
4. 关闭 GPU：重新装载 selectedBackend=cpu，batch size 上限回到 8。
5. DirectML 任务失败：单项重试仍只使用应用缩略图，不打开原图。
6. 资源校验：主 DLL、shared DLL、license 和 notices 均随包存在且哈希通过。
7. 启动 PhotoOrganizer.cmd 的 CheckOnly、frontend build、Rust tests/check/clippy 通过。
8. 本机冒烟命令（只使用仓库测试缩略图）：
   \`cargo run --bin import-benchmark -- --images "..\\test-data\\manual-verification-20260808\\Parent 库" --data-dir "target\\gpu-smoke-import"\`
   \`cargo run --bin semantic-benchmark -- --images "target\\gpu-smoke-import\\app-data\\thumbnails" --backend directml --batch-size 1\`
   结果：\`backend=direct_ml\`、\`failureCount=0\`、\`status=completed\`。
