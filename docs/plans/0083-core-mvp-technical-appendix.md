# 核心 MVP 技术附录：实现合同与验证细节

- 状态：ADR-0015 设计已接受；主要实现已集成，最终验证与桌面验收未完成
- 普通产品讨论请先读：[主方案](0083-core-mvp-filter-preview-folder-export.md)
- 决策摘要：[ADR-0015](../decisions/0015-unified-query-and-safe-copy-export.md)

本附录保留详细合同，供实现和安全评审使用。

## 详细合同（原计划技术章节）

- 状态：设计已接受；主要代码已集成，最终 Rust 检查、指定回归与桌面验收待完成
- 日期：2026-10-20
- 决策：[ADR-0015：统一查询、预览与安全原文件复制](../decisions/0015-unified-query-and-safe-copy-export.md)
- 范围：只写计划和技术合同，不改变应用或视觉风格

## 1. 目标与范围

完成可依赖的基础组合筛选、已有照片预览和按规则安全复制原文件到新文件夹。用户能清楚指定范围、查看所有匹配与目录映射、在执行前核对每个目标路径，并在失败、取消或恢复时知道哪些项已复制。原始来源在整个流程中只读。

本轮冻结 UI 外观和 AI 新功能。保留现有浏览 IA 和视觉系统，不新增字体、框架、sidecar、云服务或生产依赖。

包含：

- 统一现有 AssetQuery、AssetScope、count、分页与 group 总量合同。
- 可靠网格、单图、胶片栏、多图比较、编辑预览和导出目录预览。
- 从“当前完整 query”或“明确选择的资产”生成持久、完整、不可重算计划。
- 在新建目标子目录中安全复制原文件字节；严格 no-overwrite。
- 复制进度、取消、失败隔离、重启后人工 reconcile，以及只针对本 job 生成副本的预览回滚。
- Safe Copy 前最小 Catalog Recovery Gate；完整 N3 用户备份/恢复产品后置。

不包含：

- 改 UI 外观、AI/分类/特征扩展、视频/RAW、云同步、移动、永久删除、原地重命名、覆盖、任意文件删除或复用已有输出目录。
- 通用回滚或整个输出目录递归删除。
- 完整用户备份/恢复 UI、跨设备 relinking；这些须保留现存风险并在 roadmap 中单独跟踪。
- 不得声明当前实现已完成；各阶段须按主方案和路线图逐项验收。

## 2. 现状和用户影响

| 能力               | 当前实现                                                                                                                                                                                                        | 待完成的验收                                                                                                                          |
| ------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| 查询与基础筛选     | 全量 query/scope、计数、分页、分组、membership 与 neighbor 查询已接入；批量入口显式选择当前 query 或资产选择。                                                                                                  | 最终前端 175 tests、lint/typecheck/build 通过；all-target Rust 与桌面状态连续性仍待验证。                                             |
| 预览与重试         | 网格、单图、胶片栏、多图比较、编辑预览和目录预览具备各自加载/错误/重试状态。                                                                                                                                    | 深浅主题、窄宽度及 action group 几何的桌面验收；原图像素不得进入 import/analysis/model 路径。                                         |
| 编辑预览           | Rust EditRecipe 驱动编辑预览并保留 recipe 保存；旧全尺寸编辑派生导出及 legacy rollback 入口已封闭，UI 提供禁用说明。                                                                                            | 按高分辨率 fixture 验证各 decode boundary 只产生有界缩略图；编辑预览不得绕开受控 original viewer 例外。                               |
| Organization       | 完整不可变 plan 已持久化，IPC 创建返回 header、reader 分页还原 items/tree；运行时进度采用轻量 polling。                                                                                                         | service targeted tests 通过；all-target Rust 检查及桌面计划关闭/重开、分页与 stale 行为复核。                                         |
| Safe Copy/recovery | copy job、Catalog Recovery Gate、取消/重启核对及 job-owned rollback 已实现；双来源隔离 scan→plan→copy→rollback、source unchanged、high-res thumbnail/recovery 与 directory junction reparse 拒绝 tests 已通过。 | helper 12、runner 4、service 11 项 targeted tests 通过；all-target 216/217，唯一失败为 Windows file symlink OS 1314；桌面验收未完成。 |
| 桌面交互           | 前端组件及业务入口已集成；规范启动器在隔离 AppData/WebView 配置下启动成功，`-CheckOnly` 通过。                                                                                                                  | 原生交互与窄桌面仍未验收；headless 截图已检查组织空态及 SettingsDialog 的深浅主题和混合按钮几何，透明无边框 rail 仍需桌面确认。       |

现状证据见 [architecture.md](../architecture.md)、[roadmap.md](../roadmap.md)、[Checkpoint E](../refactor/checkpoint-e-export-preview.md)、[Checkpoint F](../refactor/checkpoint-f-copy-export.md)、[Checkpoint G](../refactor/checkpoint-g-product-architecture-consolidation.md) 和 [ADR-0014](../decisions/0014-handle-bound-windows-export-rollback.md)。旧 compact Organization plan 只允许只读查看/导出 manifest，不可直接升级为新 Safe Copy snapshot；必须按新版本重新解析并生成。旧 checkpoint 是历史记录；不能把其中的未实现 patch 当作当前代码。

Places365 categories resource 的哈希差异已诊断：固定上游版本、HEAD Git blob 与工作树均是规范 LF 字节，SHA-256 为 `2affba635eb657e7ca95f4e6cc69bd9fac29ef4c32aeb83cafdfcd06ec6a1ea6`；旧 `6cc3…` 刚好对应同一内容转换成 CRLF 后的字节。资源校验脚本已改为固定 LF 字节哈希，不做归一化；本次全量 `verify-release-resources.ps1` 检查通过，但这不代表功能验收或 CI 发布通过。完整证据见[本机接收记录](../handoff/2026-10-19.md)。

## 3. 选型和执行顺序

技术选型与取舍在 ADR-0015 冻结：

- 复用 Tauri/Rust/React/SQLite；不加生产依赖或 Python sidecar。
- 复用 AssetQuery v2 与 SQLite 查询构建器，以完整 query 在数据库一次解析；UI page 不是范围。
- SQLite 是计划、job、item 与审计的唯一业务权威。manifest 是从持久计划导出的报告。
- organization 与 edit copy 共用唯一 FileOperationService 及 ADR-0014 handle-bound writer/rollback。
- 每个计划 item 执行原文件 byte copy；不重新解码或编码。
- 以 SQLite VACUUM INTO 实施最小 Catalog Recovery Gate；不直接复制活动 DB、不启用新 Rust backup feature。

实施状态与剩余顺序（2026-10-20）：

1. Query/Scope、全量分组与各类预览重试已实现；最终同步依赖后的前端 19 个测试文件、175 项测试，以及 lint、typecheck、build 均通过；JobPanel/API 定向 12 项回归通过。后端 all-target Rust 当前 216/217；唯一未通过用例待解决。
2. 持久化完整 plan、分页 reader、job/recovery、Safe Copy 与 job-owned rollback 已集成。计划创建 IPC 返回 header，明细通过分页 reader 获取；运行时进度由轻量轮询更新，而非 push event。
3. 双来源隔离 fixture 的真实扫描→plan→copy→rollback 流程已通过并确认来源未变；helper 12、runner 4、service 11 项 targeted Rust tests 通过，包含预览前后目标修改均拒绝执行；high-res thumbnail/recovery tests 通过。directory junction 测试使用 Windows fixture 创建真实 junction 并依 handle identity 验证 grant/open-existing-root 拒绝。all-target 为 216/217，唯一 `semantic_tasks` file symlink 用例因 Windows OS 1314 创建权限错误失败，未 skip。`cargo fmt --all -- --check`、desktop clippy 与 `cargo build --features desktop` 通过。
4. 规范 `启动 PhotoOrganizer.cmd` 已使用 `test-data/manual-launch-validation-2026-10-20` 下隔离 AppData/WebView 配置成功构建并启动；关闭本次进程后 `-CheckOnly` 通过，测试目录已清理且 tracked fixtures 未改。Edge headless 检查了组织空态 1280×900 深浅主题，以及 900px SettingsDialog 深浅主题共 6 张截图；未见横向溢出，混合 primary/secondary 几何一致（32px 高、8px 圆角、1px 边框、11px 行高、12px 横向 padding、8px 间距）。这不覆盖窄桌面布局或原生交互。Native/CUA 初始化受 `setup refresh had errors` 阻断；桌面交互、过滤至恢复完整工作流仍未验收。

旧 roadmap 的完整 N3 前置门禁已被接受的 ADR-0015 缩小为最小 Catalog Recovery Gate；只有该门槛实测通过后才可开放 Safe Copy。完整 N3 产品 UI 后置不表示 N3 已完成。

## 4. 查询、范围与快照合同

### 查询和筛选

不同筛选字段按 AND 组合，同字段多选默认 OR；题材/标签明确保存 any/all。缺失值不参与数值范围或拍摄日期区间；只有显式“为空”谓词匹配缺失值。拍摄日期缺失默认不命中，不回退到 mtime；mtime 是独立可选择条件。日期范围采用 start-inclusive、end-next-day-exclusive。日期筛选使用 EXIF 原始日历日期 YYYY-MM-DD，带 offset 时保留用于显示和时刻排序但不按当前系统时区转日；无 offset 不猜时区。

用户排序后追加 Source ID 和 Asset ID 升序，确保稳定分页。UI 沿用有上限 page/offset；任何 query 或结果 revision 变化都废弃旧页并从头加载。切 root 或改变 query 清空 selection，分页追加保留 selection；current asset 若仍命中则保留，否则切到首项，空结果则清空。工具栏明确分开“选中 N”和“全部匹配 M”，全 query 不在 React 生成百万个 Asset ID。查询 count、page 与 group aggregate 共享相同 root/filter。Group contract 包含 unique groupCount、base query 的 unique asset globalTotal、每组 unique asset count；group list 自己分页，展开组内 assets 再独立分页。多标签 asset 可以在多个组各出现一次，组 count 非互斥，禁止求和得出 globalTotal。Query 计划解析在一个数据库读取快照内查询，不从前端页循环拼接。

### AssetScope 和 UI 当前状态

批量输入仅有：

- Query：使用 query 的全部匹配 Asset，不受 page、已加载窗口或 current asset 影响。
- ExplicitAssets：明确 Asset ID 列表，可跨页和跨 Source；去重但不静默丢失失效项，失效项作为 issue 呈现。

Favorite、Collection、All、Source 均通过 AssetQuery.root 表达，不添加另一套 scope。active/current asset、selection、page 和 query 是独立状态。当前图片不自动纳入批量操作；用户必须选择“当前 query 全部”或“所选项目”。

### 计划持久事实

计划头保存版本、创建时间、完整 scope/query、count、rules 与 rule version、TargetGrant、目标父目录 identity、normalized child name、目标预期不存在标记、状态和 plan fingerprint。新 child root 在规划时不存在，不能保存其 identity；目录创建后在 job 记录 outputRoot identity。每个 item 保存顺序、asset/source IDs、owner SourceRoot、源路径/owner-relative 路径、文件物理 identity、大小、mtime、完整 fingerprint、规则使用的 effective 值与 revision、最终 target relative/absolute path、命名序号、冲突键与 issues。

计划 reader 从 SQLite 还原所有 item、target tree 和统计；不依赖 React state、查询后续结果或 manifest。执行不重算 query/filter/rules/目标路径。初次 start 与显式 resume 前，若源身份、目标父目录、pending 项所用 effective/rule revision 或计划规则不再匹配，必须重新 preview；resume 仅重规划 pending items，旧 job 的 completed history 保留。进入 running 后使用已确认冻结的分类/规则值，不因用户改标签重算目标；每个 item 从同一源只读 handle 核验 identity/content fingerprint，变化只令该 item failed，不使 completed 项回退。

计划状态：ready、blocked、stale、consumed、legacy_read_only。阻塞问题必须列到具体 item/路径。旧 compact plan 为 legacy_read_only，只能查看/导出 manifest，不能授权执行。confirmation token 沿用 0081：process-local、最多 64 条、最长 5 分钟、单次消费；绑定规范化数据库身份、operationKind、plan ID/revision、完整服务端 snapshot，以及首次执行的 TargetGrant/父目录 identity 或后续操作的已记录 outputRoot identity。同 plan/operation 的新 preview 撤销旧 token；任何执行尝试先消费 token；重启后 token 全部失效。

## 5. 预览矩阵

| 场景                      | 数据与行为                                                                                                   | 失败/安全要求                                                                                          |
| ------------------------- | ------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------ |
| 网格缩略图                | 初次使用应用私有 640px thumb；视口需要时可升级至现有有界 screen tiers（最多 1280px 网格档）。EXIF 方向一致。 | 不为整库预取；只访问登记缓存和受控 decoder。失败槽显示文件级错误与单项 retry。                         |
| 单图 Fit                  | 使用 screen preview，最长边不超过现有 2560px viewer 上限，适配画布。                                         | 切换 asset 递增 generation/cancel token；旧响应不得盖新图。                                            |
| 单图 100%                 | 在 viewer 里显式请求高分辨率；如必须读取 original，只有用户当前 viewer 的直接动作可走 original tier。        | 不把 original 像素或缓存传给分析、模型、EditRecipe preview 或文件 COPY。失败留在 viewer 并可单项重试。 |
| 胶片栏/快速切换           | 继续使用受控 thumbnails；当前图和 selection 分开。                                                           | 每个槽位有 loading/ready/error；新一代请求取代旧响应。                                                 |
| 多图比较                  | 当前四图；每个槽独立加载 state/request ID 与 retry。                                                         | 一个 asset 失败不阻塞其它槽、不让整体状态永久 loading；单项错误有文件名和 retry。并发上限为四。        |
| Edit preview              | EditRecipe 应用到有界应用预览 derivative；UI 显示当前 asset/recipe 状态。                                    | 与原文件组织 COPY 分开。预览不得将原图路径传入导入/分析/模型链路；recipe 改变即使预览 stale。          |
| Organization tree preview | 从冻结 plan items 显示来源、相对源目录、规则层级、命名、最终目标、count、冲突和修正结果。                    | 纯只读；生成 preview 不创建 root/subfolders/files，不以 JSON/CSV 替代 DB plan。                        |

单图 viewer 的 original tier 是唯一明确的完整原图查看例外；import、analysis 和 model 只能消费应用缩略图或有界 derivative。原图 COPY 是 byte stream，不解码。

## 6. Folder rule、目标及冲突合同

默认选系统目录 picker 返回的现有父目录，并指定一个不存在的新 child folder。TargetGrant 冻结 parent identity；plan 冻结 normalized child name 与预期不存在。MVP 不复用输出目录、不自动创建多级父目录。创建前再次检查 child 未出现；创建后记录新 outputRoot identity。后续 resume/rollback 允许同一已存在的 job root，但必须 identity 与 job 记录完全匹配；未知或替换 root 阻塞。

默认规则保留 owner-relative 原目录和原文件名；用户可选按拍摄年月归档。Query 或 ExplicitAssets 解析结果只有在 Source IDs 多于一个时，才在 owner-relative 目录前增加“清理后来源名 + 稳定短 Source ID”层；单来源不增来源层。AI 分类与连续影调/颜色都只是可选目录维度，模型未就绪不阻塞不使用这些维度的基础筛选、预览和复制。不得恢复旧审计中未由当前实现与本轮需求支持的 folder-prefix 扩展。所有实际使用的分类/特征连同 revision 冻结进 item。

拍摄年月缺失时不改用 mtime。用户必须明确选择“拍摄日期未知”目录、回到原目录层级或阻塞该 item；最终路径和 fallback 在逐项 preview 中可见。该目录 fallback 不改变 query 日期筛选语义。AI 字段仅在用户显式加入规则时生效；字段/模型不可用时显示逐项 fallback 或阻塞，不影响其他基础能力。

命名默认原 basename；显式 template 可以引用 original name/extension、拍摄日期、按冻结排序计算的序号与短 Asset ID。缺失值必须选择明确 fallback；不静默用 mtime。所有最终文件名/目录名在 preview 显示。路径组件执行 Windows 非法字符、保留名、尾随空格/点与长度检查。

冲突规则：

1. 默认保留原名；重复目标默认 block。用户可在 preview 明确选择 block、skip、add sequence 或 add short_hash，再生成一个新冻结计划。
2. skip 项逐条列出，明确显示 included/excluded counts，并在确认前再次展示排除项；sequence/short_hash 结果逐项可见。执行时的新冲突只失败该项，不能临时改名、临时跳过或覆盖。
3. 用户可调整目录维度、missing-field fallback 或 conflict policy 后重新生成计划，重新查看所有路径。
4. Query 或 ExplicitAssets 解析结果跨来源时，namespace 解决不同 SourceRoot 的相同原目录名；同来源不自动添加来源目录。剩余冲突照常按所选策略处理。
5. 计划比较按 Windows 绝对路径、大小写不敏感、Unicode NFC 与分隔符规范化做保守预检；执行时 exclusive create_new 是最终 no-overwrite 仲裁。

SourceRoot、AppDataRoot 与目标 root 双向保护：目标不得重叠任意已注册 SourceRoot 或 AppDataRoot，包含 equal/inside/ancestor。执行前检查所有 source 与 target identity；使用 reparse-aware 目录 handle lease、reparse 点校验和最终 handle path/identity 复核；不能证明安全时拒绝。遵守 ADR-0014 的威胁边界；不能宣称解决了任意恶意祖先目录重挂载。非 Windows 不支持无法证明安全的危险写入/删除。

业务示例（以下路径仅为说明，不访问或创建这些目录）：

| 用户操作                                                                                                                                                                                    | 预览与结果                                                                                                                                                                                                                 |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Query 命中 4 张图：来源“北岸”的 Trip/DSC_001.jpg（2024-05-04）和 Archive/DSC_001.jpg（2024-05-24）；来源“南岸”的 Trip/DSC_001.jpg（2025-06-08）；北岸的 Unknown/DSC_002.jpg（无拍摄日期）。 | 标题显示 matched=4。由于跨两个来源，目标前缀分别为“北岸-稳定短ID”和“南岸-稳定短ID”；源相对目录和不同年月均显示。                                                                                                           |
| 用户选择现有父目录 E:/整理，新建 child “旅行归档”；将结构设为来源/拍摄年月（显式去掉原目录层），同名规则选 add short_hash；无日期 fallback 选择 skip。                                      | 预览树列出三个待复制文件，例如 旅行归档/北岸-…/2024/05/DSC_001-短hashA.jpg、…/DSC_001-短hashB.jpg 和 旅行归档/南岸-…/2025/06/DSC_001.jpg。无日期项逐条列在排除栏。计数为 matched=4、eligible=3、ruleSkipped=1、blocked=0。 |
| 若用户改选默认 block，同一来源、同一年月、同一目标 basename 的冲突使相应项 blocked；不可开始。若改选 skip，则被排除项和新的 included/excluded counts 必须再次确认，不能执行时临时处理。     | 用户能在执行前核对完整目标树、文件名和被排除资产。                                                                                                                                                                         |
| 确认并复制后。                                                                                                                                                                              | 每个目标的 bytes/hash 与源快照一致，内嵌 EXIF 随字节保留；文件系统时间/ACL 不保证复制。回滚预览逐项核对 target root/file identity 与 hash，确认后只删除可证明由该 job 生成且未变化的副本，保留源图和空目录。               |

## 7. 导出确认、job 与恢复

用户 flow：

1. 明确选择 Query 全部结果或 ExplicitAssets，并看到准确 count。
2. 选目标现有父目录和一个新的 child 名称；编辑目录规则/命名。
3. Rust 一次解析 scope 并将完整 immutable plan/item/tree/issue 保存到 SQLite。
4. 用户审阅 item 数、目录树、来源层、逐项目标和阻塞冲突；blocked/stale 不显示可执行确认。
5. 用户确认 exact plan；Rust 生成一次性 5 分钟 token。
6. 初次 start 只传 planId + token；Rust 消费 token 后重验源 identity/fingerprint、parent identity、child 预期不存在、保护边界和 plan revision。建 root 后持久化 outputRoot identity。
7. 在目标根新建成功后逐项串行写入、hash/identity 验证、持久化 progress；展示已完成、失败、取消、剩余数量。
8. 结束页支持逐项重试完整文件、隔离项 reconcile 和仅针对 job-owned 完成项的 rollback preview。

最小 IPC 合同（名称暂定、字段语义冻结）：

| 请求/响应                                         | 最小字段                                                                                                                                                             |
| ------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| pickTargetParent                                  | 返回 opaque TargetGrant 与可显示的目录名；原始路径仅在 Rust 内部绑定 grant。                                                                                         |
| createPlan(request)                               | Query 或 ExplicitAssets、orderedRules、missingValuePolicy、conflictPolicy、TargetGrant、normalizedChildName；response 为 planId/revision/status/counts/tree/issues。 |
| readPlan(planId, cursor)                          | 完整 scope 摘要、plan status、counts 与分页 items/tree；旧计划标记 legacy_read_only。                                                                                |
| preview/authorize(operationKind, planId 或 jobId) | 完整冻结 preview 和一次性 token/expiry；新 preview 撤销同 operation 旧 token。                                                                                       |
| startSafeCopy(planId, token)                      | 只传 planId + token；响应 jobId/state/outputRoot identity 创建结果。                                                                                                 |
| cancelJob(jobId) / resumeJob(jobId, token)        | 只传 jobId 或 jobId+token；resume 使用 job 已记录 root identity，不传新路径/rules。                                                                                  |
| subscribe/readJob(jobId)                          | 持久 job/item 状态、attempt、bytes、计数、错误和隔离原因。                                                                                                           |
| previewRollback(jobId) / rollback(jobId, token)   | preview 冻结 outputRoot/file identities、hash 和逐项结果；执行只传 jobId+token。                                                                                     |

任何执行 IPC 不接收 raw target path、规则、当前 React assets 或 frontend 重算路径。

安全执行次序：提交 item intent → 创建 child root 并记录 outputRoot identity → exclusive create_new 新目标 → 记录目标 handle identity → 64 KiB 流式 copy/hash/cancel check → flush/sync → 验证目标字节、size、hash、最终 handle path 与 identity → 提交完成状态。源使用同一只读 handle 核验身份/fingerprint 并读取 copy stream，目标使用同一 writer handle 写入和验证。一次只有一个 worker/一个文件；文件 IO 不持有长事务；内存占用不随源文件大小增长。复制只承诺文件 bytes 和内嵌 EXIF 不变，不承诺目标文件系统时间/ACL 等 metadata。

取消在 64 KiB 边界检查。已完成文件保留，pending 项停止。正在写入项只可对仍持有的同一 writer handle 调 ADR-0014 abort；失败就保留 partial 并记录，禁止按路径清除。Crash/restart 不自动复制、不自动删残留、不做 byte-offset resume；job/item 标 reconcile-required。用户看到剩余项、已完成项和隔离目标，核验已记录 outputRoot identity，重新预览并签发新 token。重试是完整单文件，不是断点续传。

### Job 与 item 状态转换

| 对象/当前状态                      | 触发                                                              | 下一状态与保护                                                                                                                                                                                                                                                                |
| ---------------------------------- | ----------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Job queued                         | 新 start 或 resume token 消费且对应 preflight 成功                | running；首次 start 创建新 child root 并持久记录 identity，resume 只复核已记录的 outputRoot identity，不再次创建，也不要求 root 不存在。创建后若 crash 且身份未记录，目标 root 隔离，不接管。                                                                                 |
| Job running                        | 用户取消                                                          | cancelling；不再启动下一 item，在当前 64 KiB 边界停止。                                                                                                                                                                                                                       |
| Job cancelling                     | 当前无 writer 或同 handle abort 成功                              | cancelled；已完成 items 保留，未执行项为 pending。                                                                                                                                                                                                                            |
| Job cancelling                     | 同 handle abort 失败                                              | partial_failed；当前 item 为 held_partial，仍持有 handle 时可预览同 handle 清理。                                                                                                                                                                                             |
| Job running                        | 所有 eligible items completed                                     | completed。source 变化只令单 item failed，其余仍匹配快照的 pending 项可继续；有失败项则 job 为 partial_failed。                                                                                                                                                               |
| 非终态 Job                         | app crash/restart                                                 | interrupted 后进入 reconcile_required；不自动 replay/delete/resume。                                                                                                                                                                                                          |
| Job interrupted/reconcile_required | 用户核对 items/root；pending revision 仍有效；签发新 resume token | queued；已完成历史不变，outputRoot identity 必须匹配。pending 规则/effective revision 已变化时为剩余项生成新 plan，不改旧完成记录。                                                                                                                                           |
| Job partial_failed                 | 用户在单项面板显式 retry，源 fingerprint 仍匹配，目标不存在       | queued；只重试这个完整文件，attempt 增加，旧失败记录保留。                                                                                                                                                                                                                    |
| Item pending                       | 写入 intent 成功                                                  | intent_recorded；持久审计先于任何文件变更。                                                                                                                                                                                                                                   |
| Item intent_recorded               | create_new 成功并记录 target identity                             | target_created，再进入 copying。create 前存在目标则 failed，不改名。                                                                                                                                                                                                          |
| Item copying                       | 源/目标同 handle hash、size、identity、路径均验证                 | verified → completed。                                                                                                                                                                                                                                                        |
| Item copying                       | cancel 且 abort 成功                                              | pending；目标不留。                                                                                                                                                                                                                                                           |
| Item copying                       | cancel 且 abort 失败、当前进程仍持有 writer                       | held_partial；UI 可 preview“按该 handle 删除已记录 partial 并重试”；删除前再核验 target/root identity 与 hash。                                                                                                                                                               |
| Item 未完成                        | app restart 或 handle 已失去                                      | reconcile_required；target 不存在且 root/source 合法时可显式新 preview 后重试；若完整 target 的 root identity、target identity 和 source expected hash 全匹配，用户可显式 reconcile 为 completed（仅认领状态，不写不删）。不完整文件一律保留隔离；handle 已丢失时无删除按钮。 |
| Item failed/source changed         | 新 preview 发现源 identity/fingerprint 已变                       | 不复用旧 item；保留旧失败审计，为新源生成新 plan。                                                                                                                                                                                                                            |

单项失败面板显示源路径、目标路径、错误、attempt、已写 bytes、当前 target/root identity 与最近校验 hash。动作只有“重试此文件”（目标不存在且源仍匹配）、“仍持有创建 handle 时预览并安全 abort partial”或“重新规划该项”。handle 丢失后不提供 partial 删除；若完整目标的 root/target identity 和 source expected hash 均吻合，只能在用户确认后 reconcile 为 completed，不写不删。未知/替换对象永不认领。任何 retry/resume/rollback 都重新 preview 并发新 token。

### 进度与记录保留

Plan counts 满足 matched = eligible + ruleSkipped + blocked；blocked > 0 时不可 start。skip 是规则明确排除，不算 failure。Job total 等于冻结的 eligible；completed 按不同 item 只计一次，retry attempt 不重复加总。Job/item/plan/audit 保留 source、root、path、identity、hash 和归属快照，不随 Library、Asset 或 Collection 删除而级联清理。移除来源时如有 copy job，在记录所有权安全处理前阻止移除；未执行项需先取消或显式隔离，不能让已生成副本失去审计归属。

Catalog gate：safe copy 前以 VACUUM INTO 到唯一 AppData snapshot、sync、独立只读验证 integrity/schema/foreign keys；迁移前同样备份。SQLite 写 intent/审计在任何目标文件变更前持久提交。快照/磁盘空间/权限/检查失败则不开 Copy。恢复旧 Catalog 不自动重放 file job；目标与记录不一致时保留隔离并需人工 reconcile。门槛实现已接入，但 all-target Rust 与最终门槛验收待完成；完整 N3 UI 仍后置，有磁盘/AppData 丢失风险。

回滚只删除本 job 的已完成副本，并要求目标根身份、目标物理身份和 content hash 同时匹配，先 preview 再 confirm，同一个 verified handle 删除。任何改变或无法证明归属都保留。只回滚文件，不删 source、不递归删目录、不删除空 target root。

## 8. 分阶段实施与依赖

| 阶段                          | 目标                                                                                         | 当前状态与剩余验收                                                                                        |
| ----------------------------- | -------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------- |
| P0：Gate & design             | 接受 ADR-0015 的范围调整；按最新 AGENTS 维护 UI/thumbnail 约束。                             | 设计已接受；不代表 G-UI 或本 MVP 的桌面验收已通过。                                                       |
| P1：Query/Scope               | 整合 AssetQuery v2 consumer；统一 count/page/group 与 Query/ExplicitAssets 解析。            | 已实现，前端套件已通过；all-target Rust 与桌面验收待完成。                                                |
| P2：Preview reliability       | grid/single/filmstrip/compare/edit/tree error and retry states。                             | 已实现；前端检查已通过；需完成最新 UI 几何与 thumbnail-only boundary 桌面/fixture 验收。                  |
| P3：Immutable plan            | SQLite 持久化 header/items/tree/issues；stale/blocked/ready reader。                         | 已实现 header IPC + paged reader；all-target Rust 与桌面 round-trip 复核待完成。                          |
| P4：Recovery gate + Safe Copy | VACUUM INTO snapshots、验证/恢复演练、唯一 FileOperationService、copy/cancel/log/reconcile。 | 已实现并跑过隔离 scan→plan→copy→rollback 与 high-res thumbnail/recovery tests；all-target 仍有 1 项未过。 |
| P5：Owned rollback            | 对 job-owned copy preview and same-handle delete。                                           | 已实现；targeted tests 验证 preview 后目标变更会拒绝执行；modified/unknown targets 必须保留。             |
| P6：桌面验收                  | 从选择 query 到 copy、cancel/restart、rollback preview 的完整产品验收。                      | 未执行；正式 launcher 与隔离 AppData/WebView 桌面流程通过前不标记 MVP/桌面验收完成。                      |

## 9. 验收矩阵

所有文件系统测试只用仓库 test-data fixture 或测试创建的临时目录，绝不遍历用户照片目录。每项测试对源文件前后 fingerprint，断言源文件未写入/移动/改名/删除。

| 场景                     | 必须验证                                                                                                                                                              |
| ------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 组合筛选/日期            | 多字段 AND、同字段 OR、标签 any/all；空值与“为空”；缺拍摄日期不回退 mtime；日期日边界；stable tie-breaker。                                                           |
| Query / pagination       | 全量 count 与 page 条件完全一致；变更后旧 generation 不混页；organization Query 忽略 UI page。                                                                        |
| Group                    | 未加载后续 page 时 DB 全量 group 数及每组 count 正确；group page 与 asset page 不混淆。                                                                               |
| Explicit selection       | 跨页、跨 Source 去重；current image 不自动加入；失效 ID 作为阻塞 issue 而非静默丢弃。                                                                                 |
| Query snapshot           | 多 Source root+descendants；Favorite/Collection roots；数据库重启后可重建 plan items/tree。                                                                           |
| Export rule fallback     | 默认 original directory + 原 basename；拍摄年月为可选维度；缺失拍摄年月需显式选 fallback 并显示预览，不回退 mtime；模型不可用不阻塞基础规则。                         |
| 原始文件复制             | JPEG/PNG/WebP 原始字节 hash 相同、未重新编码；Unicode/中文/西里尔/空格路径；大文件内存恒定；源 fingerprint 变化即 stale。                                             |
| 目标冲突                 | 既存 root、既存 target、同计划多 item 冲突、大小写/NFC 等价键、保留名、超长路径；每一项都禁止 overwrite。                                                             |
| 多来源路径               | 相同 owner-relative 原目录仅在跨 source 时增加 source namespace；清理后来源名冲突由 stable ID 区分；单 source 无额外层。                                              |
| 保护边界                 | Target 等于/位于/包含任一 SourceRoot 或 AppDataRoot；nested Library；symlink/junction/reparse escaping；target ancestor 与 source ancestor 双向比较；无法验证时拒绝。 |
| Preview                  | 640 thumb→bounded upgrade；单图 generation stale 保护；Compare 单一槽失败和单项 retry；EditRecipe 变化使预览过期；无预览请求触发原图分析。                            |
| 取消/重启                | cancel before first item、在 64 KiB 中断、已完成保留、abort success/failure；kill/restart 后不自动复制或删除；旧 job 需要新的 preview/token。                         |
| 权限/磁盘                | 源读取拒绝、目标 create/write/sync 拒绝、snapshot 无空间、目标卷空间耗尽、只读目录；状态与逐项错误可恢复，源不变。                                                    |
| Catalog backup/migration | WAL 有已提交写入时 VACUUM INTO snapshot 包含它；不直接 copy live DB；integrity/foreign key/schema 检查；migration 前快照失败即停；临时 DB restore 演练。              |
| Rollback                 | 成功完成副本且 hash/identity/root unchanged 可删；改内容/替换对象/目标根 identity 变更/unknown partial 全部阻塞；目录和原图保留；非 Windows Unsupported。             |
| UI 控件几何              | mixed primary/secondary controls 使用共享 `--control-radius` 与几何；主题切换、窄桌面尺寸；navigation rail 外壳透明无边框。                                           |
| Thumbnail-only boundary  | 高分辨率 fixture 覆盖 import/analysis/model/decode 输入边界，不创建 full-resolution source pixel buffer；唯一例外是当前 viewer 的显式 original preview。              |

## 10. 风险与文档优先级

1. 旧 roadmap、F、N6 的阶段划分与本计划的 minimum recovery gate/受限 rollback 有冲突。ADR-0015 已接受并替代完整 N3 产品门禁；Recovery Gate 实现已接入，但最终后端检查与门槛验收待完成。旧 checkpoint 是历史，不改写成已实现。
2. 把完整用户备份/恢复后置会让磁盘或设备损坏时无法自助恢复，必须在 MVP 限制与发布说明中持续披露；N3 不得整体标完成。
3. 计划冻结后才允许复制，SQLite 快照、job record、目录身份或确认 token 任一校验失败都阻塞。确认后的 source edit、rename、目标 race 都会让项目 stale 或 item failed。
4. handle-bound 安全取决于 Windows/文件系统语义；ancestor directory reparenting 并非 ADR-0014 已解决的威胁。实现不得承诺超出句柄 lease/reparse/final identity 可证明的边界。
5. 文件系统与 Catalog 不能单事务提交。write-ahead item intent、目标身份和 reconcile 流程负责把无法原子化的状态显式呈现；不自动清理未知 target。
6. 具体 API 名称与 schema migration 字段可随实现微调，但只有保持本计划的语义和 ADR 安全边界，才算合同兼容。若取舍改变核心保护条件，先更新 ADR 并重新评审。

## 补充：N3 恢复门槛与备选方案

### 2. 将 N3 Safe Copy 门禁限定为 Catalog Recovery Gate

在 Safe Copy 开放前实现并通过以下最小门槛：

1. 使用 SQLite 的 VACUUM INTO 在 AppData 中生成唯一、完整的 Catalog 快照；绝不直接复制正在使用的主数据库文件，以免遗漏 WAL 中已提交的数据。
2. 快照文件完成后调用文件同步；用独立只读连接打开快照并检查 PRAGMA integrity_check、PRAGMA foreign_key_check 和预期 schema version。任一检查失败，迁移与 Safe Copy 均失败关闭。
3. 每次 schema migration 前先生成并验证该版本的快照。快照失败时不启动 migration。
4. Safe Copy 前把完整计划、job、item 与 write-ahead 操作记录提交到 SQLite；确认记录可读且持久化后，才允许第一次目标文件系统写入。文件 IO 期间不持有长 SQLite 事务。
5. 用仓库 fixture 在隔离临时目录执行快照恢复演练，验证表结构、外键和必要的用户数据/操作记录可读。演练不得访问个人照片或真实用户目录。
6. 从较旧 Catalog 恢复时，非终态 job 一律进入 reconcile-required。恢复不会自动重放 COPY、删除目标或替用户重新授权；必须逐项检查目标身份与内容，并重新显示计划供用户授权。归属无法证明的目标保留并隔离。

实现选用 VACUUM INTO，因为当前 rusqlite 0.37 feature 只有 bundled/functions，没有启用 backup feature；它可经现有 SQLite 运行时生成一致快照而不增加依赖。SQLite Online Backup API 是替代方案，但需要启用/维护额外 Rust API feature；直接复制活动 DB 被拒绝。参考 SQLite 官方 [VACUUM INTO](https://www.sqlite.org/lang_vacuum.html) 与 [Online Backup API](https://www.sqlite.org/backup.html) 文档。

本门槛只开放 Safe Copy 所需的内部一致性备份、恢复证明、迁移前备份和可审计操作记录，不代表完成完整 N3 产品能力。面向用户的完整备份/恢复界面、跨设备迁移、路径 relinking、备份选择与冲突处理仍后置。代价是磁盘损坏、AppData 丢失或设备丢失时，用户仍缺少完整自助恢复路径；发布说明不得把该子集称为完整 Catalog Protection。ADR-0015 已接受并明确替代旧 roadmap 门禁，但只有门槛本身实现并验证通过后才能开放 Safe Copy。

## 方案比较

| 方案                                                                | 优点                                        | 代价/风险                                         | 决定                             |
| ------------------------------------------------------------------- | ------------------------------------------- | ------------------------------------------------- | -------------------------------- |
| 保持当前查询已加载页作为批量范围                                    | 改动最少                                    | 结果受滚动/分页影响，无法表达全查询或跨页明确选择 | 拒绝                             |
| 前端收集全部 Asset IDs 后提交                                       | 易于展示                                    | 大图库内存与 IPC 负担大；前端列表成为错误权威     | 拒绝                             |
| SQLite 内统一解析 AssetQuery/ExplicitAssets 并冻结完整 plan         | 与现有查询一致，范围、计数和路径均可审计    | 需要计划/item schema 和迁移                       | 采用                             |
| Organization 自行按路径写文件                                       | 初期实现直观                                | 与 Edit 安全重复，容易绕开 ADR-0014 与日志        | 拒绝                             |
| 复用统一 FileOperationService 与句柄绑定 writer                     | 只有一套 no-overwrite、验证、审计和撤销边界 | 需要适配既有 Edit writer 并建立共享测试           | 采用                             |
| Safe Copy 前要求完整 N3 用户备份/恢复产品                           | 完整用户恢复路径                            | 推迟 MVP；N3 产品工作大于安全复制最小门槛         | 由已接受 ADR-0015 的最小门槛替代 |
| N3 安全子集：一致快照、恢复演练、迁移前备份、持久审计；完整 UI 后置 | 以较小工作量证明复制前 Catalog 可恢复       | 仍有设备/磁盘损坏及自助恢复缺口                   | 采用；须先实现并通过门槛         |
| SQLite Online Backup API                                            | 适于活动库增量备份                          | 当前 rusqlite 未启用 backup feature               | 暂不采用                         |
| SQLite VACUUM INTO                                                  | 使用现有 SQLite、得到一致独立快照，不漏 WAL | 生成全量文件、需要临时磁盘空间                    | 采用                             |
| 直接复制活动 SQLite 文件                                            | 看似简单                                    | WAL/一致性风险                                    | 禁止                             |

## 影响与边界

- 过滤体验保留现有查询能力，但分组总数改为全量数据库统计；所有页面/视图必须展示同一 scope 的 total。
- 新计划只执行 SQLite 冻结事实。前端无法通过修改 manifest、当前页或 query 后续结果改变已确认计划。
- 对 I/O、锁、路径、目标创建、sync、数据库迁移和备份失败全部失败关闭；显示可行动的错误并保留证据。
- 这仍不是任意本地管理员、恶意文件系统驱动或祖先目录并发重挂载的完全防御；沿用 ADR-0014 的平台限制，不作超出其 threat model 的保证。
- full N3 备份/恢复界面、迁移工具、异机路径重新关联仍未包含在本 ADR 的最小 Safe Copy gate 中。
- 实现前如发现现有 schema/IPC 与此冻结合同冲突，必须更新此 ADR/计划后再实现；不得直接把 WIP patch 或旧 E/F 计划视作已落地。
