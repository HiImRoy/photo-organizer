# 0053 统一本地来源与收藏夹模型重构

状态：方案修订版；Phase 1、Phase 2 已实现，Phase 3 统一图库导航第一轮已实现，后续阶段按 [0053 实施路线图](./0053-implementation-roadmap.md) 继续推进。

## 1. 背景与目标

当前项目同时使用 libraries、assets.library_id、asset_library_assignments、collections、collection_assets、assets.is_favorite，以及 UI 中的“图库”“来源”“收藏”“集合”等概念表达图片归属，职责发生重叠。

本次重构只解决两件事：

1. 图片真实来自哪个本地目录；
2. 用户希望把图片虚拟归入哪些收藏夹。

本次不把整理、导出和真实文件复制执行器一起重写，避免来源、收藏、查询、OrganizationPlan 和文件执行五套核心模型同时变化。

## 2. 本次范围

本次 0053 包含：

- 本地来源 Source 模型；
- 收藏夹 Collection 模型；
- 默认收藏与爱心同步；
- 收藏夹树及层级；
- 加入、移出、移动收藏夹；
- 左侧导航重构；
- BrowseRoot 与 AssetQuery 查询模型；
- 旧 asset_library_assignments 数据迁移；
- 旧 favorite、collection、library 查询兼容；
- 收藏夹计数、缺失文件和重启恢复。

本次不重构：

- 多来源 OrganizationPlan；
- 查询结果快照；
- sourceRoots[]；
- 目录树生成；
- 文件冲突检查；
- 批量复制执行器；
- 导出目录创建和校验；
- 非空目录导出、覆盖文件、源目录写入。

上述内容后续建立 0054：多来源 BrowseScope、整理方案与导出执行器重构。

## 3. 统一产品概念

| 内部概念          | 用户名称     | 含义                                     | 是否修改源文件 |
| ----------------- | ------------ | ---------------------------------------- | -------------- |
| Source            | 本地来源     | 图片实际从哪个磁盘目录扫描进入系统       | 否             |
| Collection        | 收藏夹       | 用户希望把哪些图片虚拟归在一起           | 否             |
| AssetQuery        | 当前浏览结果 | 来源或收藏夹与筛选条件组合得到的动态结果 | 否             |
| OrganizationPlan  | 整理方案     | 根据图片范围生成目标目录映射             | 本次不改       |
| ExportDestination | 导出目录     | 后续真正写入图片副本的位置               | 本次不改       |

产品心智固定为：

- 本地来源：这些照片实际在哪里；
- 收藏夹：我想把哪些照片放在一起；
- 筛选：我现在想看哪些照片；
- 整理：未来想把这些照片复制成什么结构。

图库只作为左侧 UI 模块名称，不作为同时指代 Source 和 Collection 的数据库实体。

## 4. 本地来源 Source

### 4.1 唯一语义

Source 只表示图片实际从哪个本地目录被扫描进入系统。

assets.library_id 只承担真实来源关系。任何用户的收藏、拖拽、移动和归档操作都不能改变 assets.library_id。

禁止出现：

图片实际位于成都来源，但因为用户操作而被 UI 显示为来自重庆来源。

### 4.2 来源与收藏夹分离

用户将成都来源中的 A.jpg 放入旅行精选时，只建立 collection_assets 关系：

- 真实来源：assets.library_id；
- 虚拟归档：collection_assets；
- Source 查询不读取用户归档关系；
- Collection 查询通过成员关系返回图片。

两套关系完全独立。

### 4.3 Source 层级

Source 树只能表达真实磁盘路径或明确导入关系：

本地来源
照片
成都
重庆

用户不能通过 UI 任意拖拽 Source 改变真实父子关系。

如果存在旧的 manual Source relationship：

- 能根据真实路径恢复的，按真实路径恢复；
- 无法映射到真实路径的，保留为根级 Source；
- 不把旧手工关系转换为新的物理层级；
- 后续不再生成新的手工 Source relationship。

## 5. 旧 asset_library_assignments 迁移

当前 asset_library_assignments 能覆盖真实 assets.library_id，这与新的 Source 语义冲突。

迁移原则：

### 没有旧 assignment

保持原状：

assets.library_id = 真实 Source。

### 存在旧 assignment

例如 A.jpg 的真实 Source 是成都，旧 assignment 指向旅行精选：

1. 保留 assets.library_id = 成都；
2. 根据旧 assignment 的目标图库建立一个普通收藏夹；
3. 将 A.jpg 写入该收藏夹；
4. 删除或废弃旧 assignment；
5. assignment 不再参与任何 Source 查询。

迁移后 asset_library_assignments 不再参与：

- 图片浏览；
- 来源统计；
- Source 筛选；
- Source 树计数；
- 整理来源识别。

旧手工图库即使与 Source 同名也允许迁移为同名收藏夹，因为 Source 和 Collection 属于不同命名空间。同一父收藏夹下的 Collection 名称必须唯一，但不要求与 Source 全局唯一。

## 6. 收藏夹 Collection

收藏夹是完全虚拟的数据结构：

Collection
↓
collection_assets
↓
Asset

收藏夹不得：

- 创建磁盘目录；
- 移动源文件；
- 修改源文件；
- 修改 Source；
- 修改 assets.library_id。

一张图片可以同时属于任意多个收藏夹。

### 6.1 收藏夹层级

普通收藏夹允许：

- 创建子收藏夹；
- 重命名；
- 调整同级顺序；
- 移动到其他普通收藏夹；
- 删除；
- 添加图片；
- 移除图片。

收藏夹禁止形成循环。后端必须校验目标父节点不能是当前节点、当前节点的任意后代或默认收藏。

### 6.2 默认收藏

默认收藏是系统收藏夹，固定使用：

- system_key = default_favorites；
- collection_kind = system_favorites。

默认收藏是系统叶节点：

- 固定置顶；
- 不可删除；
- 不可改名；
- 不可移动；
- 不允许作为其他收藏夹的子节点；
- 不允许创建子收藏夹。

### 6.3 默认收藏与爱心

默认收藏成员关系是爱心状态的最终数据真源：

Asset 属于 default_favorites
等价于
Asset heart = active。

点击爱心：

- 插入 default_favorites 的 collection_assets 关系；
- 同步更新兼容字段 assets.is_favorite。

取消爱心：

- 删除 default_favorites 的 collection_assets 关系；
- 同步更新 assets.is_favorite。

迁移阶段 assets.is_favorite 只作为兼容镜像。所有操作在同一事务中完成，避免旧接口短暂读取到不一致状态。旧接口清理后再评估是否删除该字段。

### 6.4 父收藏夹查询与计数

普通收藏夹父节点默认聚合自身和所有后代成员，按 asset_id 去重。

后端至少返回：

- directAssetCount：当前节点直接成员数；
- aggregateAssetCount：当前节点及后代成员去重后的数量；
- directPresentCount：直接成员中源文件可访问的数量；
- aggregatePresentCount：聚合成员中源文件可访问的数量；
- missingCount：收藏关系仍存在但源文件缺失的数量。

树中默认显示 aggregatePresentCount。详细信息中可同时显示直接成员、聚合成员和缺失文件数量。

默认收藏没有后代，因此默认收藏浏览结果只包含其直接成员，也就是所有爱心图片。

### 6.5 缺失文件

源文件离线或移动硬盘拔出时：

- 保留 Asset；
- 标记 file_status = missing；
- 不自动删除 collection_assets；
- 收藏夹计数和详情仍能识别缺失成员。

Source 重新上线并扫描恢复后，原收藏关系自动恢复可见。普通浏览默认只显示 present 成员，详情仍可显示 missing 数量。

## 7. 左侧导航与添加入口

左侧模块标题仍为“图库”，内部明确分组：

图库
本地来源
2025 夏季照片
青岛旅行
海边
城市
收藏夹
默认收藏
人像精选
待打印
建筑灵感

Source 与 Collection 使用不同图标，但共享树行、展开、选中和计数样式。

入口不再使用“新建图库”，改为：

＋ 添加

点击后选择：

- 导入本地来源；
- 新建收藏夹。

这两个入口必须明确说明它们属于不同类型，但不重复堆叠解释文字。

### 7.1 导入本地来源

流程：

1. 点击“添加”；
2. 选择“导入本地来源”；
3. 选择本地文件夹；
4. 配置现有扫描选项；
5. 创建 Source；
6. 启动扫描任务。

如果路径已存在，提示“该文件夹已作为本地来源导入”，提供“打开已有来源”，不重复建立 Source。

### 7.2 新建收藏夹

流程：

1. 输入名称；
2. 选择父收藏夹；
3. 默认父节点为收藏夹根；
4. 可选是否立即加入当前图片范围。

父收藏夹选择器：

- 只显示普通收藏夹；
- 不显示 Source；
- 不允许选择默认收藏。

从当前浏览结果创建时必须显示当前范围、筛选条件和匹配图片总数，不得让用户误以为只加入当前已加载的缩略图。

后端必须根据 AssetQuery 直接执行批量事务，不允许前端加载全部 asset IDs 后逐个发送，也不加载原图、不重新 AI 分析、不强制生成缩略图。

## 8. 统一浏览模型

逐步废弃 libraryId、favoriteOnly、collectionId 等并行来源状态，统一使用：

BrowseRoot：

- source：本地来源；
- collection：收藏夹；
- all：全部来源。

AssetQuery 包含：

- root；
- filter；
- includeDescendants。

Source 的 includeDescendants 表示包含物理子来源；Collection 的 includeDescendants 表示包含后代收藏夹成员。MVP 默认使用 true，暂不暴露开关，但后端类型保留该能力。

示例：

成都来源 + 人像 + 五星，表示在成都 Source 及其物理后代中叠加筛选。

旅行收藏夹 + 横向照片，表示在旅行 Collection 及其后代成员中叠加筛选。

旧查询通过兼容适配器转换：

- libraryId 转换为 BrowseRoot.source；
- favoriteOnly 转换为默认收藏 Collection；
- collectionId 转换为 BrowseRoot.collection。

所有新的图片列表、图片计数、批量加入收藏、搜索结果和 AI 搜索范围逐步接收 AssetQuery。

## 9. 图片归档操作

统一保留三种操作：

- 加入收藏；
- 从当前收藏夹移除；
- 移动到其他收藏夹。

### 9.1 加入收藏

选择图片后点击“加入收藏”，打开收藏夹多选器：

- 可以一次选择多个普通收藏夹；
- 可以选择默认收藏，等价于点亮爱心；
- 加入普通收藏夹不自动点亮爱心；
- 重复关系幂等；
- 操作只写 collection_assets，不修改 Source 和磁盘文件。

### 9.2 从当前收藏夹移除

只在收藏夹上下文中提供。

只有当前节点的 direct member 才能直接移除。如果图片只是通过后代收藏夹聚合显示，不能从父收藏夹含糊移除。界面应提示图片实际来自哪个子收藏夹，用户进入直接所属收藏夹后再移除。

### 9.3 移动到其他收藏夹

移动定义为：

加入目标收藏夹
加上
从当前直接所属收藏夹移除。

只允许当前节点 direct member 执行。

不允许在以下范围直接执行移动：

- Source 浏览结果；
- 全局搜索结果；
- 动态筛选结果；
- 父收藏夹聚合成员。

这些场景使用“加入收藏”。

默认收藏特殊处理：

- 从默认收藏移出等价于取消爱心；
- 移动到默认收藏等价于点亮爱心；
- 默认收藏不是普通收藏夹移动来源或父节点。

### 9.4 拖拽规则

图片拖到普通收藏夹：加入收藏夹。
图片拖到默认收藏：点亮爱心。
图片拖到 Source：禁止。

普通收藏夹拖到普通收藏夹：修改 parent_collection_id。
普通收藏夹拖到默认收藏：禁止。
默认收藏拖动：禁止。
收藏夹拖到 Source：禁止。

Source 拖到任何节点默认禁止，Source 层级由真实路径和扫描关系确定。

### 9.5 删除收藏夹

普通收藏夹有子节点时，确认窗口提供：

- 删除整个收藏夹树；
- 仅删除当前收藏夹，子收藏夹提升到原父节点。

两种操作都只删除虚拟节点和成员关系，不删除 Asset、不修改 Source、不修改磁盘文件。默认收藏不可删除，只允许清空其成员关系。

## 10. 数据库设计

继续复用 collections 和 collection_assets，不新建平行的 favorite folders 表。

collections v2 增加：

- parent_collection_id；
- collection_kind，允许 manual 和 system_favorites；
- system_key；
- display_order；
- created_at；
- updated_at。

约束：

- system_key = default_favorites 全局最多一个；
- 同一父节点下 Collection 名称按 NOCASE 唯一；
- 根级名称也必须唯一；
- parent_collection_id 不能形成循环；
- 默认收藏不能作为父节点；
- collection_assets 以 collection_id、asset_id 为主键保证幂等；
- 保留 asset_id、collection_id 反向索引；
- 增加 parent_collection_id、display_order 树查询索引。

如果现有 collections.name 存在全局 UNIQUE 约束，迁移时重建 collections 表，不只执行简单的 ALTER TABLE ADD COLUMN。

## 11. Migration 策略

所有步骤在一个事务中完成：

1. 创建 collections_v2；
2. 复制旧 collections 数据为 manual 根级收藏夹；
3. 创建唯一的默认收藏系统叶节点；
4. 将 assets.is_favorite 写入默认收藏；
5. 将旧 asset_library_assignments 转换为 Collection 和 collection_assets；
6. 处理旧 manual Source relationship，能按真实路径恢复则恢复，否则转根级 Source；
7. 替换旧表；
8. 建立索引和约束；
9. 提交事务。

任何一步失败都必须回滚，不得破坏已有扫描索引、源路径和原图。

迁移必须可重复执行，不得创建多个默认收藏，不得静默丢失旧 assignment 信息。

## 12. Organization 兼容策略

0053 不重构 OrganizationPlan 和真实导出执行器，只做必要兼容。

- Source 范围继续允许进入现有整理系统；
- 单一 Source 内的筛选结果继续允许使用现有能力；
- 跨 Source 收藏夹本次只允许浏览、筛选、收藏和搜索；
- 如果跨 Source 收藏夹进入当前整理系统，明确提示“当前收藏夹包含多个本地来源，现版本整理功能暂不支持”；
- 禁止选第一个 Source、错误合并路径、修改 library_id 或静默丢弃其他 Source。

0054 单独处理：

- 多来源 BrowseScope；
- AssetQuery 结果快照；
- 多来源 OrganizationPlan；
- sourceRoots[]；
- 目标目录树；
- 冲突检查；
- 空导出目录校验；
- 批量复制执行。

## 13. 实现阶段

### Phase 1：语义和 Migration

完成 Source 唯一语义、collections v2、默认收藏、favorite 迁移、assignment 迁移、Collection hierarchy、同级名称唯一、循环校验和 migration rollback 测试。

此阶段不改大规模 UI。

### Phase 2：AssetQuery

完成 BrowseRoot 和 AssetQuery，支持 Source、Source 后代、Collection、Collection 后代、All 和已有 AssetFilter，并增加旧查询适配器。

### Phase 3：统一左侧导航

实现“图库 / 本地来源 / 收藏夹”结构，完成默认收藏置顶、不同图标、展开、选中、计数、删除旧来源筛选模块和“＋ 添加”入口。

### Phase 4：收藏交互

实现爱心、批量爱心、加入收藏、多目标加入、当前收藏夹直接成员移除、移动到其他收藏夹、收藏夹创建、重命名、排序、层级移动、删除和图片/收藏夹拖拽。

### Phase 5：旧逻辑清理

逐步移除 favoriteOnly、collectionId、asset_library_assignments、moveAssetsToLibrary 和 assignAssetToLibrary 作为用户归档手段的依赖。保留必要读取兼容，直到旧持久化状态和工作流完成迁移。

### Phase 6：回归和验收

覆盖大 Source、多 Source、深层收藏夹、多对多收藏、默认收藏、missing 文件、移动硬盘离线/恢复、Unicode、同级重名、循环、重启、旧数据迁移、rollback、窄桌面和深浅色主题。

## 14. 验收标准

### Source

- 本地来源只表示真实磁盘来源；
- 收藏、拖拽和移动不能改变 Source；
- asset_library_assignments 不再影响 Source 查询；
- Source 层级不因 UI 手工拖拽改变；
- 重新扫描不破坏收藏关系。

### 收藏夹

- 一个 Asset 可以属于多个收藏夹；
- 收藏夹支持正常层级；
- 收藏夹不能循环；
- 同级名称唯一；
- 父收藏夹默认包含后代成员；
- 聚合结果按 asset_id 去重；
- 收藏关系不影响源文件。

### 默认收藏

- 始终存在；
- 固定置顶；
- 不可删除、改名、移动；
- 不允许子收藏夹；
- 成员与爱心严格一致；
- 重启后状态一致。

### 查询

- Source 和 Collection 都通过 AssetQuery 浏览；
- AssetFilter 可以叠加到两种范围；
- Collection 可以跨多个 Source；
- 查询不再假设结果只有一个 library_id；
- 旧查询通过兼容适配器工作。

### 成员操作

- “加入收藏”可以选择多个目标；
- 重复加入幂等；
- 加入普通收藏夹不点亮爱心；
- 加入默认收藏等价于点亮爱心；
- 从当前收藏夹移除只影响 direct membership；
- 父收藏夹聚合成员不能被含糊移动；
- 收藏操作只修改数据库关系。

### Missing

- 源文件离线不会删除收藏关系；
- UI 能识别 missing 收藏成员；
- Source 恢复后收藏自动恢复可见。

### Migration

- 原 collections 全部保留；
- 原 favorite 全部迁移；
- 原 assignment 信息不会静默丢失；
- migration 失败整体回滚；
- 不改变源文件和源路径；
- 不重新分析图片。

## 15. 本次明确不做

- 多来源 OrganizationPlan；
- Collection 查询结果快照；
- 真正文件复制导出器；
- sourceRoots[]；
- 导出目录自动创建；
- 导出到非空目录；
- 文件覆盖；
- 源目录写入；
- 自动修改源目录结构；
- 智能收藏夹；
- 保存动态筛选；
- 云同步；
- Collection 自动规则；
- Collection 转换成真实文件夹；
- Source 与 Collection 相互嵌套。

## 16. 后续 0054 接口预留

0053 不实现整理重构，但 AssetQuery 必须可以被 0054 直接复用：

AssetQuery
↓
Resolve asset IDs
↓
Create immutable snapshot
↓
OrganizationPlan v2
↓
Preview
↓
Execute copy

本次不要创建 favorite-specific query、collection-specific query 或 organization-specific source query。所有新的图片范围逻辑都围绕 AssetQuery 收敛。

## 17. 最终产品心智

完整路径：

导入本地来源
↓
扫描和建立索引
↓
浏览 / 搜索 / 筛选
↓
加入一个或多个收藏夹
↓
继续筛选和管理收藏
↓
后续进入整理系统

必须始终保持：

Source ≠ Collection
Collection ≠ Folder
Browse Result ≠ Collection
Organization Plan ≠ Source

本次只稳定真实来源和虚拟归档两套模型，整理快照、多来源整理和真实导出留到 0054。
