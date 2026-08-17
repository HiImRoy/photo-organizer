# 0031 标签 taxonomy 收敛

## 目标

把面向个人摄影师的主体标签和拍摄题材收敛成少量、互补、可筛选的中文标签，避免同一语义在自动分析结果中重复出现。

## 目标标签

主体标签保留：

- 单人
- 多人
- 动物
- 车辆
- 食品
- 植物

主体模型不再输出“人物”“人像”“宠物”；宠物检测并入动物，单个人体检测或高置信人脸检测归为单人，多个人体检测归为多人。

拍摄题材保留现有稳定 ID，并更新展示名称：

| 稳定 ID              | 展示名称 |
| -------------------- | -------- |
| `photo_portrait`     | 人像     |
| `photo_landscape`    | 风光自然 |
| `photo_street`       | 街拍纪实 |
| `photo_architecture` | 建筑     |
| `photo_still_life`   | 静物产品 |
| `photo_food`         | 美食     |
| `photo_wildlife`     | 动物     |
| `photo_macro`        | 植物     |
| `photo_activity`     | 运动     |
| `photo_vehicle`      | 交通工具 |
| `photo_document`     | 文档截图 |
| `photo_abstract`     | 抽象艺术 |

`photo_documentary` 从活动题材 taxonomy 中移除；没有被当前 taxonomy 接受的题材和旧的 `unknown` 统一解析为 `photo_abstract`。保留稳定 ID 的原因是已有数据库和 Places365 映射可以平滑迁移，不改变用户已有的组织规则接口。

## 实施范围

1. 更新 Rust 题材/主体 taxonomy、模型输出聚合和 SigLIP 2/Places365 融合映射。
2. 提升 taxonomy 版本，隔离不兼容的旧自动结果；读取旧兼容值时归并到新展示名称。
3. 更新数据库的有效分类、主类筛选和语义分组，使 `unknown`/已删除的纪实题材不再生成独立分组。
4. 更新前端筛选选项、标签文案和可视化测试夹具。
5. 增加主体互斥/归并、题材目录和旧值兼容测试，并运行 Rust、TypeScript、构建检查。

## 验收标准

- 主体筛选区不出现人物、人像、宠物，只出现单人、多人、动物、车辆、食品、植物。
- 新一轮主体分析对一人只产生单人，对多人只产生多人，宠物只产生动物。
- 拍摄题材筛选区不出现纪实与工业、未知；题材名称使用上表中文名称。
- 低置信或旧 unknown 结果在有效分类和分组中显示/筛选为抽象艺术。
- 旧数据库不会因为标签改名导致读取崩溃，手动筛选接口仍能工作。

## 实施结果

- 主体 taxonomy 已升级为 `photo-organizer-subject-tags-v2`，PicoDet/YuNet 聚合输出单人、多人、动物、车辆、食品、植物；单人和多人互斥，宠物归入动物。
- 摄影题材 taxonomy 已升级为 `photo-organizer-photography-topics-v3`，删除纪实与工业，统一名称，并把拒识结果归入抽象艺术。
- Rust 数据读取、有效分类、主类筛选、语义分组和前端选项均已使用归并后的 ID；旧值在展示/读取边界兼容。
- 验证结果：Rust 81 个测试、前端 43 个测试通过；TypeScript、ESLint、Prettier、Clippy、生产构建和 release resource 校验通过。
