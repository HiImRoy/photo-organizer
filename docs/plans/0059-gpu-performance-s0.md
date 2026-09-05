# 0059 GPU 性能优化：S0 硬件能力检测与性能基线

**状态：已实现，待人工验收。**

本阶段只建立“能否尝试 GPU 加速”的可信检测闭环，不改变当前 CPU 推理路径，也不把“检测到显卡”误报成“模型已经在 GPU 上运行”。

## 目标

- 在 Windows 上枚举图形适配器，记录名称、厂商/设备 ID、显存和软件适配器状态。
- 在设置页明确显示检测状态：检测中、已检测到独立 GPU、仅集成 GPU、未检测到、检测失败。
- 没有通过 Provider 初始化和自检时，分析继续使用 CPU；GPU 状态不能阻塞主界面启动。
- 为后续 DirectML、CUDA 或其他 Provider 的接入提供稳定的能力数据和验收边界。

## 本阶段实现

1. `src-tauri/src/gpu.rs` 使用 Windows DXGI 枚举适配器。独立 GPU 判定目前是保守的第一阶段启发式：排除软件适配器，并要求专用显存达到 256 MiB；该结果只叫作 `is_discrete_candidate`，不是 Provider 可用性结论。
2. 新增 `get_gpu_capabilities` IPC 和前端类型，浏览器预览返回诚实的“不检测本地 GPU”状态。
3. 设置页处理区显示 GPU 诊断状态。DirectML 当前为 `not_configured`，因此即使检测到独立 GPU，也显示“待接入”，不提供虚假的启用开关。
4. 新增 Rust 单元测试覆盖软件适配器、低专用显存和正常硬件适配器三种边界。

## 明确不在 S0 内

- 不加载 DirectML/CUDA Provider。
- 不修改分析 batch size、worker 数量或任务调度策略。
- 不把原图交给推理、特征提取或预览分析；缩略图专用处理规范保持不变。
- 不因为检测到 GPU 就默认改变用户现有的 CPU 结果或耗电行为。

## 验收标准

- `npm.cmd run typecheck`、`npm.cmd run lint`、`npm.cmd run test -- --run` 和 `npm.cmd run build` 通过。
- `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check`、`cargo test --manifest-path src-tauri/Cargo.toml --all-targets` 和 Clippy `-D warnings` 通过。
- 无 GPU、仅集成 GPU、检测失败和独立 GPU 四种状态均能在设置页得到明确且不误导的文案；检测失败不能阻塞图库浏览。
- 现有 CPU 分析任务和缩略图边界测试不受影响。

## 后续阶段门禁

S1 才能评估 DirectML Provider：需要单独 ADR，说明 Provider 许可、运行库体积、设备选择、失败回退、安装包资源和 Windows 版本兼容性。只有 Provider 初始化、缩略图批处理正确性、CPU/GPU 输出一致性、显存/内存上限、取消恢复和安装包 smoke 全部通过后，设置页才可以出现可操作的 GPU 开关。

S2 继续评估 I/O binding、批大小自动调节和缩略图解码/渲染的 GPU 优化；这些优化必须以实际基线数据为依据，不能把 WebView2 的界面合成 GPU 与 ONNX 推理 Provider 混为一谈。
