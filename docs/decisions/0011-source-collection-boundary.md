# ADR 0011：本地来源与收藏夹分离

状态：已接受，实施从 0053 Phase 1 开始。

## 背景

当前系统同时使用 `libraries`、`assets.library_id`、`asset_library_assignments`、`assets.is_favorite` 和 `collections` 表达图片归属。旧 assignment 会在查询时覆盖真实 `library_id`，收藏状态又与虚拟集合关系分开保存，导致“图片实际在哪里”和“用户想把图片放在哪里”无法区分。

## 决策

1. 现有 `libraries` 表继续作为物理本地来源 Source 的存储，不新增平行 `sources` 表。
2. `assets.library_id` 永远表示图片实际被扫描进入的 Source。收藏、拖拽、虚拟移动和筛选不能修改它。
3. `collections` / `collection_assets` 表示虚拟收藏夹。Collection 可以形成树，一张 Asset 可以属于多个 Collection，但不会创建目录或修改源文件。
4. 默认收藏使用唯一的 `system_key = 'default_favorites'`，是固定置顶的系统叶节点。默认收藏成员关系是真实爱心状态，`assets.is_favorite` 在迁移阶段只作为兼容镜像。
5. `asset_library_assignments` 只作为旧数据迁移和短期兼容数据保留。新查询不能使用它覆盖 Source，Phase 5 再评估清理旧表和旧接口。
6. 0053 只稳定 Source、Collection 和统一查询边界，不改整理计划、目标目录树、冲突检查、复制执行器和导出目录规则；这些内容留给 0054。

## 迁移策略

- 新增 0016 迁移，重建 `collections` 以增加父节点、系统类型、系统键和排序字段。
- 旧 Collection 原样迁移为根级普通 Collection。
- 旧 `is_favorite` 迁移为默认收藏成员关系。
- 旧 assignment 按目标图库迁移为普通 Collection；名称冲突使用带“（迁移）”后缀的确定性名称，避免把旧关系静默合并到用户已有收藏夹。
- 旧手工 Source 父子关系按真实 Source 路径恢复；无法恢复的关系提升为根级。
- 全部迁移在 SQLite 事务内执行，失败整体回滚，重复启动不创建第二个默认收藏。

## 后果

正面影响：

- Source 树重新忠实表达磁盘来源；
- 收藏夹可以跨 Source、多对多和分层；
- 收藏操作不再污染物理来源；
- 后续 AssetQuery 和 0054 整理快照可以共享同一个浏览范围。

代价与限制：

- 需要重建旧的全局唯一 `collections` 表；
- 旧查询和整理接口需要兼容适配；
- 跨 Source 收藏夹在 0053 中只能浏览、筛选和搜索，不能进入当前单 Source 整理流程；
- 迁移完成前必须保留旧 assignment 数据以便审计和回退。

## 验证

0053 Phase 1 使用 v15 SQLite fixture 验证：真实 `library_id` 保持不变、默认收藏与爱心一致、旧 assignment 转成 Collection、Source 查询不再受 assignment 影响、旧手工层级按真实路径恢复、迁移失败可回滚、重复初始化幂等。
