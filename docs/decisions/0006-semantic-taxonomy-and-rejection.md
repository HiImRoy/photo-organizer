# ADR 0006：语义分类分组与拒识状态

状态：Accepted

日期：2026-08-10

## 背景

现有 TinyCLIP 代码把 `portrait`、`landscape`、`night`、`water` 和 `unknown` 放进同一个候选集合，再用全局 top-score window 和强制 primary fallback 生成结果。这样会产生三个问题：候选之间不是同一层级，`unknown` 会参与竞争并在低置信度时被强制写入，前端也无法区分“一个场景”与“多个属性”。

## 决策

采用分组 taxonomy，并保持数据库和前端的兼容字段：

| 组别      | 语义                   | 结果约束                             | 兼容字段                           |
| --------- | ---------------------- | ------------------------------------ | ---------------------------------- |
| `scene`   | 图片的主要场景/用途    | 组内最多一个；达不到阈值可以没有结果 | `is_primary=1`、`primary_category` |
| `subject` | 图片中可同时出现的主体 | 可有多个                             | `is_primary=0`、`auxiliary_tags`   |
| `context` | 环境和拍摄属性         | 可有多个                             | `is_primary=0`、`auxiliary_tags`   |

`unknown` 是解析后的拒识状态，不是 prompt、不参与相似度排序、不保存模型分数。`other` 是一个真正的 `scene` 候选，只有它自身达到阈值并通过组内间隔检查才被保存。`failed` 继续只表示运行失败。

采用 additive migration 增加 `category_group` 与 `taxonomy_version`。查询只读取当前模型、分析版本、源指纹和 taxonomy 版本；旧结果保留在库中但视为 stale，重新分析后才进入当前业务视图。

## 兼容性

- 不删除 `primary_category`、`auxiliary_tags`、`is_primary` 和现有稳定 label ID。
- `primary_category` 暂时是 legacy scene slot，而不是整个 taxonomy 的总称。
- 手动分类仍可覆盖 scene；手动 tag ADD/REMOVE 仍作用于辅助标签。
- 组织规则中的 `primary_semantic` 继续使用有效 scene 值。

## 取舍

本 ADR 不更换模型。TinyCLIP 的 prompt embedding 和阈值仍需在后续 D 评估集上校准；若未来需要真正的独立多标签概率模型，再单独提交模型替换 ADR。这个边界避免把 UI/数据契约问题误判成换模型即可解决的问题。

## 参考

- OpenAI CLIP zero-shot 分类与 prompt ensemble：<https://github.com/openai/CLIP>
- PhotoPrism 的模型标签到用户标签、阈值和类别映射：<https://docs.photoprism.app/developer-guide/vision/tensorflow/>
- Open-set recognition 将 unknown/rejection 与 closed-set 类别分离：<https://openaccess.thecvf.com/content_cvpr_2016/html/Bendale_Towards_Open_Set_CVPR_2016_paper.html>
