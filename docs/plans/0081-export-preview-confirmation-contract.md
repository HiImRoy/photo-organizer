# 0081 编辑导出历史与回滚/恢复确认契约

状态：暂停；未验收。IPC 有未验证 WIP；分页常量 200/500 与计划约定 20/50 不一致，history/recovery workflow DTO 与函数尚缺，命令和 helper 未接入 lib.rs。

## 目标与范围

将回滚与恢复执行收敛到服务端持有、短期有效、一次性消费的预览快照，并将编辑导出历史/预览相关的 SQLite 与文件系统工作移出 Tauri UI 线程。本任务仅修改 `src-tauri/src/ipc.rs`、`src-tauri/src/lib.rs` 与本计划；workflow DTO 与核心校验、schema 迁移、文件安全 helper、前端 API 由各自所有者维护。

不增加依赖，不触碰用户照片目录，不调整启动、模型准备或数据路径，不提交、不清理工作区。

## 契约与假设

- workflow 提供 `EditExportHistoryPage`、`EditRecoveryPlan` 和增强后的 `EditRollbackPlan`，以及 `list_edit_exports`、`preview_edit_recovery`、接收完整服务端计划快照的 rollback/recovery execute 函数；IPC 直接使用这些类型，不定义重复 DTO。
- IPC 唯一新增传输类型是 rollback 预览 wrapper，使用 `serde(flatten)` 保留原快照字段并增加 `confirmationToken`。
- `AppState` 每个进程持有独立授权 store。授权绑定规范化数据库身份、操作种类、plan ID 及 workflow 返回的完整快照；同 plan/操作的新预览撤销旧授权。UUID v4 token 最长有效 5 分钟，store 最多保留 64 条；成功或失败的执行尝试都先消费 token。
- 执行参数中的 plan ID 只用于与已消费 token 绑定值比对；核心始终接收 store 中的快照。授权检查失败时不得调用 workflow execute。核心 execute 负责再次检查数据库记录与目标文件的当前身份/内容。
- 历史、编辑导出预览、回滚预览、恢复预览与执行中的阻塞 IO 通过 `spawn_blocking` 调度。编辑导出 execute 保持原有持久化 preview 与参数契约。
- `export_file_safety` 由其文件所有者提供；仅当 helper 文件存在时，在 `lib.rs` 中注册模块。

## 实施步骤

1. 增加本计划并确认当前 IPC/state 与 Tauri handler 注册点。
2. 在 AppState 增加内存授权 store，实现 TTL、容量上限、同 plan/操作失效、原子取出与校验；将其纳入所有 `AppState` 构造路径。
3. 增加编辑导出历史分页、恢复预览/执行 IPC；将 rollback preview 包装授权 token，并改为消费 token 后把服务端保存的完整快照交给 workflow execute。
4. 将历史/预览/执行相关阻塞 IO 放入 `spawn_blocking`，注册三个新增 IPC 命令，并按 helper 文件是否存在注册安全模块。
5. 增加 store、wrapper serde 与参数绑定/不派发测试；只格式化所属 Rust 文件，随后按共享 workflow、迁移和 helper 状态完成针对性检查，并把桌面 feature 全量测试留给集成阶段。
6. 审核仅本任务文件的最终 diff，记录测试结果、未运行检查与协作契约限制。

## 验收边界

- 过期、重复、未知、错操作、错 plan 或错数据库身份的 token 均拒绝；新进程的空 store 不接受旧 token；无效授权不进入核心执行。
- 回滚和恢复只能执行服务端保存的、与 token 所绑定的完整预览快照；执行失败也必须重新预览。
- 新增三个 IPC 命令均注册；分页参数保持可选并由 IPC 提供安全默认值/范围约束。
- 文件系统 fixture 仅使用 `test-data/` 或测试拥有的临时目录；不访问个人照片目录。
- 不改 workflow 或数据库实现，不加入生产依赖，不修改启动与模型准备流程。

## 执行与验证记录

- 初始检查：`AGENTS.md`、当前 IPC/state、Tauri handler、Repository 数据库路径、workflow 当前签名及现有计划已检查。
- 实现与检查结果待补。
