# ADR-0008：用独立主体模型层补充 Places365 场景层

- 状态：已接受
- 日期：2026-08-10
- 关联计划：`docs/plans/0027-subject-tags-and-model-workflow.md`

## 背景

Places365 的训练目标是场景分类，不能可靠回答“画面中是否有人”“是否多人”“是否有人脸”“是否有车或动物”。把这些结果继续塞入拍摄题材会重新产生“所有图片都有车辆/街道”的系统性错误。

当前数据库已经有语义标签、缩略图门禁和可恢复的分析任务，但预留的人脸表属于未来隐私功能，当前没有合规模型，也不能直接用于主体标签。

## 决策

增加独立的主体模型 provider，并把它接入现有缩略图分析任务，但保持数据和模型边界独立：

1. PicoDet-S-COCO 只负责通用目标检测；通过稳定的本地映射生成 `person/group/animal/pet/vehicle/food/plant` 标签。
2. YuNet 只负责轻量人脸检测；仅在检测到可信人脸时生成 `portrait`，不保存人脸框、embedding 或身份信息。
3. 主体结果写入 `subject_labels`，空结果写入 `subject_analysis_runs`，从而区分“已分析但无标签”和“尚未分析”。
4. Places365 仍写入 `semantic_labels` 并决定唯一主拍摄题材；主体 provider 不拥有 `is_primary`。
5. 当前“分析”任务对同一张缩略图顺序运行两个 provider；任何 provider 的失败只影响自身结果，数据库写入按 provider 分开并重验 fingerprint。
6. 模型状态单独暴露；主体模型未安装时不阻塞场景分析，也不静默生成主体标签。

## 备选方案

- **继续使用 TinyCLIP 提示词识别主体**：提示词相似度未校准，且容易复现原有类别偏置；不采用。
- **让 Places365 输出主体标签**：训练目标不匹配；不采用。
- **只用一个通用大视觉语言模型**：本地 CPU 体积、速度和许可证审查成本更高；不作为 MVP。
- **用 Ultralytics YOLO 权重**：部署成熟，但当前项目没有接受其模型/代码许可与发行影响；第一版选择官方 PaddleDetection Apache-2.0 代码体系的 PicoDet，并单独归档模型来源。
- **把 YuNet 人脸框写进已有 face_detections**：会把普通主体标签和未来身份/聚类隐私数据耦合；第一版只保存非身份化的 portrait label，face 表继续保持 opt-in。

## 影响

- 新增主体模型资源、主体 taxonomy 版本和 SQLite 派生表；不会修改已有 Places365 taxonomy。
- 旧的手动 `vehicle`、`food`、`group` 等标签继续可读；新自动主体标签使用相同稳定 ID，手动覆盖通过现有 tag override 机制生效。
- 主体检测仍可能漏检或误检，UI 必须把分数称为模型置信分数，不称为准确率；人工标签可以覆盖自动结果。
- 所有输入是应用私有缩略图，原始照片保持只读；清除主体数据只影响数据库派生结果。
