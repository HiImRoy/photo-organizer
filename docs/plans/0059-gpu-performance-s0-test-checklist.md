# 0059 GPU 性能优化：S0 测试清单

**测试范围：** GPU 能力检测、状态展示、CPU 回退和启动稳定性。

**不属于本清单：** DirectML/CUDA 实际推理、GPU Batch Size 自动调节、I/O Binding，以及 GPU 图像渲染优化。本阶段只验证 Batch Size 门控；真正的 Provider 能力必须在 S1 门禁中单独验收。

## 完成度判断

| 能力                    | 状态             | 说明                                                                                  |
| ----------------------- | ---------------- | ------------------------------------------------------------------------------------- |
| Windows DXGI 适配器枚举 | 已完成           | Rust 侧枚举名称、设备 ID、专用显存、共享内存和软件适配器标志。                        |
| 独立 GPU 候选判断       | 已完成（启发式） | 排除软件适配器，并以 256 MiB 专用显存作为第一阶段候选信号；不是 Provider 可用性结论。 |
| GPU 状态 IPC/API        | 已完成           | `get_gpu_capabilities` 已接入 Tauri invoke、前端 API 和应用状态。                     |
| 设置页状态展示          | 已完成           | 无 GPU/集成 GPU 显示不可用；检测到独立 GPU 但 DirectML 未接入时显示待接入。           |
| CPU 回退                | 已完成           | 当前模型分析仍使用 CPU，GPU 检测失败不会阻塞图库。                                    |
| 设置与批大小门控        | 已完成           | 配置范围为 1–32；Provider 未就绪时实际批大小限制为 8。                                |
| DirectML 实际加速       | 已由 S1 完成      | Provider、模型会话、自检、实际后端记录和 CPU 回退已落地；详见 0061。                  |
| GPU 性能基线            | S2 已完成首轮     | 使用应用缩略图完成 CPU/DirectML 对照和 warm-up/批次统计；长图库与实时显存仍后置。     |

## 自动化测试清单

### 前端

- [x] TypeScript 类型检查：`npm.cmd run typecheck`
- [x] ESLint：`npm.cmd run lint`
- [x] Vitest 全量回归：`npm.cmd run test -- --run`
- [x] 设置页验证“独立 GPU 加速”状态和 CPU 回退文案
- [x] 设置页验证 GPU 开关禁用态和超过 CPU 上限的批大小选项
- [x] Vite 生产构建：`npm.cmd run build`
- [x] Prettier 检查本轮涉及的源码、文档和配置

### Rust / Tauri

- [x] Rustfmt：`cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check`
- [x] Rust 全目标测试：`cargo test --manifest-path src-tauri/Cargo.toml --all-targets`
- [x] Clippy：`cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings`
- [x] 纯逻辑边界：软件适配器、低专用显存、正常硬件显存
- [x] Windows DXGI 实际枚举契约：状态值、选中适配器一致性、DirectML 未配置状态
- [x] 缩略图专用处理相关既有测试未回归

### 启动和环境

- [x] `scripts/manual-build-start.ps1 -CheckOnly -SkipDependencySync`
- [ ] 双击 `启动 PhotoOrganizer.cmd` 后人工确认设置页状态（需要用户桌面验收）
- [ ] 在无独立 GPU 电脑上确认状态为“不可用”（需要对应硬件环境）
- [ ] 在独立 GPU 电脑上确认状态为“待接入”，且分析仍明确使用 CPU（需要对应硬件环境）

## 已执行结果

| 检查项               | 结果                                                                  |
| -------------------- | --------------------------------------------------------------------- |
| 前端测试             | 通过：13 个测试文件，84/84 个测试                                     |
| Rust 测试            | 通过：103 个库测试、1 个 benchmark 测试、3 个评估测试                 |
| 前端构建             | 通过：Vite production build                                           |
| Clippy / Rustfmt     | 通过：warnings denied，格式检查通过                                   |
| 手动启动入口环境检查 | 通过：Node/npm/Rust/依赖检查；Node 22.12 低于推荐的 22.13，构建仍通过 |

## S1 进入条件

以下条件全部满足后，才开始 DirectML Provider 实现：

1. 确定 Provider 许可、运行库体积、Windows 版本和安装包资源策略，并新增 ADR。
2. Provider 能在目标设备上初始化，并能在初始化失败时自动回退 CPU。
3. 使用同一批缩略图对比 CPU/GPU 输出，确认分类、特征和错误处理一致。
4. 测量显存峰值、内存峰值、Batch Size、取消/恢复和长时间运行稳定性。
5. 通过真实安装包 smoke，确认无 GPU 设备不会启动失败或卡死。
