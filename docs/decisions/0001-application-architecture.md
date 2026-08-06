# ADR-0001：应用架构

- 状态：已接受
- 日期：2026-08-06

## 背景

应用必须在普通 Windows 电脑上安装即用、离线工作，并保证扫描与分析阶段不修改原图。候选方案必须兼顾发布稳定性、图像算法扩展和语义推理。

## 候选方案

| 维度               | A：Tauri + Rust + Python sidecar    | B：Tauri + Rust + 直接 ONNX           | C：Electron + Node 原生模块      |
| ------------------ | ----------------------------------- | ------------------------------------- | -------------------------------- |
| 用户安装           | 可单包，但需捆绑 Python 与依赖      | 单包，组件最少                        | 单包但 Chromium 较大             |
| Windows 打包稳定性 | sidecar、DLL 搜索和杀软误报风险较高 | Rust 单进程最简单                     | Node 原生模块 ABI/签名需管理     |
| 体积/启动          | 通常最大、sidecar 有冷启动          | 基础切片最小、启动快                  | Chromium 基线体积较大            |
| 图像生态           | Python/OpenCV/Pillow 最丰富         | Rust 基础能力足够，高级算法较少       | JS 加原生模块，生态中等          |
| AI 集成            | Python 迭代最快                     | ONNX 适配与打包工作较多               | ONNX Node 绑定可用但原生发布复杂 |
| CPU/GPU/NPU        | Python runtime provider 灵活        | 可按平台打包 provider，需逐一验证     | 受 Node binding 支持限制         |
| 调试/迭代          | 双运行时、IPC 成本高；算法快        | 单运行时简单；算法迁移成本中等        | 前后端语言统一，原生边界仍复杂   |
| 崩溃隔离           | sidecar 崩溃隔离最佳                | 推理崩溃会影响主进程，可后续拆 worker | worker 可隔离，内存开销更大      |
| 许可证/更新        | Python 包与 DLL 清单更长            | Rust crate + ONNX + 模型分别审核      | Electron/Node/原生模块清单较长   |
| 模型热更新         | 容易                                | 可行，需稳定 adapter/schema           | 可行                             |

## 决策

采用方案 B 的渐进版本：Tauri 2 + React/TypeScript，Rust 负责 SQLite、只读扫描、缩略图、基础分析、任务调度和安全文件操作；定义语义分类器与 execution provider 抽象，只有通过许可和基准门槛后才接入 ONNX Runtime。

当前纵向切片不引入 Python、远程后端或本地 HTTP 服务。若未来算法确实需要 Python 生态，必须另写 ADR，量化包体、启动、杀软误报、崩溃隔离和迁移影响。

## 结果

- 安装包运行时组件少，基础功能可纯 CPU、纯离线运行。
- 基础分析由 Rust 图像库实现，算法复杂度受控。
- 语义推理的 provider 和模型分发仍需按硬件/许可证分别验证。
- 若未来直接链接 ONNX 导致稳定性问题，可在保持 IPC contract 的前提下把推理迁入单独 Rust worker，而不改变图库核心。
