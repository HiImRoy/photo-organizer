# Plan 0021 — LAP-derived UI Integration Remediation

- 状态：IMPLEMENTED_PENDING_MANUAL — 主界面整合已落地，等待桌面端脚本验收
- 日期：2026-08-10
- 前置：[Checkpoint G](../refactor/checkpoint-g-product-architecture-consolidation.md)
- 取舍依据：[Product Architecture Review](../product-architecture.md)

## Goal

修复从 LAP 引入后被错误打包成独立“工作台”的界面逻辑。保留已有能力和本地安全边界，但把功能重新放回主图库的来源、查询、审阅和选择动作中。

本计划借鉴 LAP 的业务组织方式，不复制其实现代码或 GPL 实现：主窗口保持稳定，左侧切换来源，中心保持统一 Grid/Preview，右侧和底部承载上下文动作。LAP 的 Home、Content 和 sidebar search 结构是本计划的参考证据。

## Implementation note (2026-08-17)

本轮已完成 G-UI 的最小完整切片：

- `App` 不再用 `workspaceMode="workflows"` 替换三栏主界面；Grid/Single Preview、DetailPanel 和选择状态始终保留。
- 收藏与虚拟集合进入左侧“来源”。`AssetFilter` 新增 `favoriteOnly` / `collectionId` 查询谓词，SQLite 直接使用 `assets.is_favorite` 和 `collection_assets`，不新增 migration。
- 选择栏提供“加入集合 / 比较 / 找相似 / 重复审阅 / 编辑副本”，并将现有能力打开到主图库下方的上下文工具区；工具结果点击图片不会自动退出工具区，右侧 DetailPanel 会跟随焦点资产。
- 集合上下文中选择目标集合不会改变主图库的 query/page/sort，也不会清除显式选择；“加入已选 N 张”继续使用同一批选中资产，侧栏主动切换集合仍保持清空选择的浏览语义。
- 顶部“查找与审阅”现在只是打开上下文工具，不再是独立一级页面；组织整理仍是唯一保留的独立 Dry-run 工作区。
- 新增了主 Grid、DetailPanel、Search/Collection/Duplicate/Similar/Compare/Edit 返回、收藏来源筛选和 Organization scope continuity 的前端回归测试；工作流结果点击只改变焦点资产，不会覆盖显式选择；没有增加生产依赖或文件系统写操作。

本轮没有把 G-UI 标记为人工验收完成。桌面端仍需按 G-UI.6 脚本验证主界面尺寸、滚动和状态连续性。

## Diagnosis

当前 `WorkflowWorkspace` 同时承载四种不同产品语义：

| 语义                     | 当前功能             |
| ------------------------ | -------------------- |
| Browse Source            | Favorite、Collection |
| Query                    | AI Search            |
| Review Set               | Duplicate、Similar   |
| Selection / Asset Action | Compare、Edit        |

它们不应成为同一个一级导航容器中的平级 Tab。仅将“智能工作台”改名为“查找与审阅”不能解决以下问题：

- 打开后替换主图库三栏，用户看不到原始 Grid/Preview 上下文。
- 工作台拥有独立结果列表和专用查询，结果不能自然回到统一 Grid。
- Search、Similar、Duplicate、Collection 使用不同的范围语义。
- Compare、Edit 与 Browse Source 混在同一个导航轴上。
- 点击结果后退出工作台，selection、active asset、page 和返回位置容易丢失。

## Product decisions

1. Main shell remains persistent：左侧 Sidebar、中心 Grid/Single Preview、右侧 DetailPanel 不因普通查找或审阅动作被替换。
2. `AssetQueryV1` 表达当前结果成员关系；`AssetScopeInputV1` 表达批量动作范围；二者不由各功能重新解释。
3. Browse Source、Query、Review Set、Selection Action 四个轴分离。
4. Search/Collection/Favorite 是来源或查询入口；Similar/Duplicate 是 Review Set；Compare/Batch Edit/Organize 是 Selection Action。
5. Organization Dry-run 可以保留独立工作区，因为它是 Scope → Rule → Plan → Review 的安全规划流程；它不是普通浏览入口。
6. 单图编辑可以进入独立焦点模式，但返回时必须恢复 query、selection、active asset 和 Grid 位置。
7. 不新增 Smart Album、Culling、Backup、COPY、媒体格式、数据库 migration 或生产依赖。

## Surface mapping

| 功能                  | 新的主界面位置                                           | 返回语义                        |
| --------------------- | -------------------------------------------------------- | ------------------------------- |
| Favorite / Collection | 左侧 Browse Source、卡片菜单、底部 Collection Tray       | 回到原 query 和 selection       |
| AI Search             | 顶部 query bar 或左侧 Search source                      | 结果仍显示在统一 Grid           |
| Similar               | 卡片菜单“查找相似”或右侧 Review Inspector                | 保留原 query，Review Set 可关闭 |
| Duplicate             | 顶部/选择栏“重复审阅”或右侧 Review Inspector             | 继续在统一 Grid 中审阅          |
| Compare               | 选择栏中的 Compare，打开轻量 Compare Overlay/Light Table | 关闭后恢复 selection            |
| Edit                  | 单图 Detail/Editor focus mode                            | 返回当前 asset 和原 Grid        |
| Organize              | 选择栏中的“整理预览”，进入独立 Dry-run                   | 明确显示 Scope 和返回图库       |

## Execution steps

### G-UI.1 Inventory and freeze

- 列出 `WorkflowWorkspace`、`App.workspaceMode`、Selection、DetailPanel、Grid、Collection 和 Review API 的现有入口。
- 保留 `AssetQueryV1` 和 `AssetScopeInputV1` 作为技术契约，但暂停继续扩展旧工作台。
- 不以“改名按钮”作为 IA 完成条件。
- [x] 已完成入口盘点，并冻结旧独立工作台继续扩展。

### G-UI.2 Source integration

- 将 Favorite、Collection、Search 变成主界面的 source/query 入口。
- Source 切换只改变 query/source 描述，不替换中心 Grid/Preview。
- Collection 的固定成员与 Query 的动态结果必须在 UI 上明确区分。
- [x] 收藏 / 集合已进入左侧“来源”；集合成员通过 `collectionId` 进入统一查询。

### G-UI.3 Review integration

- Similar、Duplicate 生成带来源和数量的 `ReviewSet`，在统一 Grid 上显示，或打开右侧审阅 Inspector。
- Review 结果点击资产时不自动退出到另一个工作区。
- Compare 从显式 selection 启动，最多保留当前产品已有的比较上限。
- [x] Similar / Duplicate / Compare 从选择栏或上下文工具区启动，主 Grid 不被替换。

### G-UI.4 Action integration

- Favorite、Rating、Color Label、Batch Classification 在卡片、DetailPanel 或 selection action bar 中完成。
- Edit 保持源文件只读；另存副本仍需显式预览。
- Organization 继续使用独立 Dry-run，但从当前 `AssetScope` 启动。
- [x] 卡片收藏、选择栏批量修正、集合、比较和编辑副本入口已接回主界面。

### G-UI.5 State continuity

- Browse → Search/Similar/Duplicate → Compare/Edit → Back 必须保留 query、page、sort、selection、active asset 和返回位置。
- Review/Source/Action 不能各自维护第二套 current result contract。
- 所有跨页批量操作必须显示“查询范围”或“显式选择范围”。
- [x] 上下文工具复用当前 `AssetQueryV1` / `AssetScopeInputV1`；点击工具结果不再强制返回或重置主图库。

### G-UI.6 Verification

- 自动化测试覆盖 source 切换、Review Set 返回、selection continuity、Compare/Edit 返回和 Organization scope。
- 桌面验收脚本至少覆盖：
  - Browse → Find Similar → Compare → mark → Back；
  - Search/Collection → Duplicate/Similar → Back；
  - Query/Selection → Organization scope preview → Back。
- [x] 前端行为测试覆盖主 Grid/DetailPanel 连续可见、Search/Collection/Duplicate/Similar/Compare/Edit 返回、显式选择连续性和收藏来源查询。
- [x] 嵌入上下文工具不再显示 Favorite/Collection/Search/Duplicate/Similar/Compare/Edit 的通用平级 Tab；来源、查询和动作分别回到左栏、顶栏、选择栏或详情上下文。
- [ ] 桌面端按上述脚本完成人工验收。
- 只有主界面集成验收通过后，才恢复 N1 的 benchmark 和后续功能规划。

## Out of scope

- 新增 Pick/Reject、Smart Album/Saved View CRUD、Metadata 全量筛选。
- Backup/Restore、Organization immutable snapshot、Safe Copy、Move/Delete。
- HEIC、RAW、Video、GPS、Face identity。
- HNSW、virtualization、batch thumbnail IPC、asset protocol 等性能实现。

## Acceptance criteria

- 不再存在一个承载 Favorite、Collection、Search、Duplicate、Similar、Compare、Edit 的通用一级工作台。
- 普通 Search、Collection、Similar、Duplicate 操作不替换主 Grid/Preview。
- Compare、Edit、Organization 的入口来自明确 selection/asset/scope 上下文。
- 返回后 query、page、selection、active asset 和 scope 描述保持一致。
- Organization 是唯一保留的独立安全规划工作区；Edit 只在单图焦点模式下例外。
- 不新增 migration、生产依赖或真实源文件写操作。
- 相关 frontend tests、typecheck、lint、format、build 通过；桌面脚本完成后才能关闭 G-UI。

## Stop condition

G-UI 未完成前，不继续实现 N2、Saved View、Backup、Safe Copy，也不把 N1 标记为完成。
