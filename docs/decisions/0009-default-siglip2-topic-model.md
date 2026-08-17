# ADR-0009：默认摄影题材模型切换为 SigLIP 2 Base

- 状态：已接受
- 日期：2026-08-12
- 关联计划：`docs/plans/0029-siglip2-topic-candidate-layer.md`

## 背景

PhotoOrganizer 的摄影题材候选层曾经保留 TinyCLIP、SigLIP 2 Base 和
MobileCLIP-S0 三套适配器。多套模型让顶部模型选择器、任务 provenance、资源
说明和实际加载路径承担了不必要的分支，也增加了桌面包体和 CPU 内存成本。

## 决策

将 `SigLIP2-Base-Patch16-224` INT8 设为默认摄影题材模型：

- 前端首次选择、API 默认参数、IPC 缺省值和 Rust 组合分类器默认值统一为
  `siglip2-base`；
- 任务创建的默认模型 metadata 使用 SigLIP 2 的名称、版本、分析版本、许可
  和 SHA-256，确保标签、embedding 与任务记录可追溯；
- TinyCLIP 与 MobileCLIP-S0 不再随包分发，也不再出现在选择器和 IPC 装载入口；
- 模型加载继续执行资源契约和 SHA-256 校验，SigLIP 2 失败时显示不可用，
  不静默回退到 TinyCLIP；
- AI 搜索和相似聚类使用当前已装载题材模型产生的 embedding，历史 TinyCLIP
  基准和评测入口继续保留。

## 选择理由

SigLIP 2 的图文匹配 logits 更适合将摄影师向的多提示词候选作为独立证据，
同时现有适配器已经处理了 224 输入、64 token、768 维 embedding 和资源完整性
校验。它的模型体积明显大于轻量替代模型，因此在桌面应用中按需装载，避免
启动阶段创建 session 导致黑屏。

## 影响与后续

- 首次运行需要加载约 360.5 MB 的 SigLIP 2 INT8 权重，模型准备仍由用户显式
  触发。
- 历史 TinyCLIP 标签和 embedding 不会被原地改写；新版本查询只读取当前
  SigLIP 2 provenance，旧记录仍可用于迁移审计。
- 仍需使用获得授权的摄影评测集完成逐类 threshold/margin 校准、质量对比和
  长时间 CPU/内存验证，之后再决定是否调整模型选择或默认阈值。
