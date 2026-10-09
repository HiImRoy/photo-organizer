# 0078 Durable Editor Export Workflow

状态：暂停；未验收。migration 0018 与 handle-bound helper 仅为本地 WIP；workflow DTO/API 尚缺，helper 未注册到 lib.rs，IPC 引用未落地类型；不得声称可编译。

## 目标

在资产被删除、图库被移除、应用崩溃或导出/回滚只完成一半时，仍能查看分页审计记录，并通过明确的预览与二次确认恢复数据库状态。回滚必须把目标哈希、物理文件身份、保护根与最终删除绑定到同一文件 handle；不得按路径猜测 legacy 操作归属、自动补造文件身份或自动重做文件/模型工作。

## 文件所有权与依赖门槛

本计划的实现范围仅为：

- `src-tauri/src/workflow.rs`
- `docs/plans/0078-durable-editor-export-workflow.md`

集成依赖由并行任务提供，不能在本任务中重造：

- 0080/migration 0018：计划源路径、源文件身份、冻结保护根、目标文件身份和进程 owner 列。
- 0079/ADR-0014：`crate::export_file_safety` 同句柄 writer/lease 与进程实例查询 API。
- 0081：IPC 二步确认 token 仅引用服务端保存的 workflow snapshot。

若任何签名或字段与冻结合同不一致，先报告主代理，不做兼容性猜测，不修改他人所有权文件。不得新增生产 crate、修改 schema/API 合同、提交或清理构建产物。

## 实施顺序

1. 先新增并审阅本计划；在 schema/helper 可用前，只做不依赖其实现细节的 workflow 设计与测试编排，不启动会因缺模块/列而必然失败的 Cargo 编译。
2. 建立以 canonical database identity、plan ID、操作 kind 为键的进程内 RAII registry。export、rollback、recovery 对同一 plan 串行；owner PID/creation stamp 与 registry 状态共同判定恢复资格。当前进程 SQL 中断只在 registry 已空时可恢复；其他进程只有 OS 明确证明实例已退出/身份不匹配才可恢复；NULL owner、权限错误、未知状态均拒绝，并给出改选新目标路径的提示。不得仅凭 DB `running` 推断 owner 已死亡，也不得启动时批量改状态。
3. 扩展 workflow DTO 与只读历史查询：固定合同字段、分页上限 50、按创建时间和 ID 稳定排序、数据库端筛选/计数，不逐项 hash。损坏 recipe 留作 `None` 并附 issue；`asset_id` 可空，历史不因资产消失而丢失。`can_rollback`/`can_recover` 只表示可请求 action preview，不替代 action 时的安全核验。
4. 导出执行使用 preview 时冻结的源路径、源 fingerprint、源物理身份和保护根。资产已变为 NULL 或身份/fingerprint 不符时 fail closed，不从路径重新解析资产。先用短事务 durable-claim plan/job/operation 为 running，再以 create-new writer 持有新文件；写像素前提交其物理身份。完成 hash、job/operation/plan 状态在短事务内一起落终态。任何错误只 abort 本次持有的创建对象；abort 失败就保留副本及可见中间日志。不得扩展现有源编码或模型流程。
5. 回滚 preview 对 plan-owned operation、已记录的 target FID、当前 handle snapshot、冻结根与当前已知根的并集、所有同身份路径运行任务及后续成功操作做核验。legacy 缺少明确 owned operation 或目标 FID 时只读拒绝，不从当前文件推断。failed export 仅在 FID 已持久记录且 handle 仍证实同一生成对象时允许预览清理，并明确提示这是不完整副本。execute 接受已预览 DTO，重新核对快照后 durable-claim owner/状态，再经同一 lease 的 `delete_checked` 删除；不持有全库事务跨文件 I/O。
6. 增加显式 recovery preview/execute。preview 的 fingerprint 覆盖 plan/job/operation 的状态、哈希、FID、owners 与保护根等相关记录；execute 要求再次确认、重新生成并比较当前 preview/事务快照，禁止覆盖新运行状态。`finalize_export` 只承认已存 hash+FID 与目标 handle 一致；`mark_export_failed` 保留文件并释放可证实中断的运行记录；`finalize_rollback` 只在目标已不存在时更新日志且不声称本进程完成删除；`reset_rollback` 保留目标并使记录转为可重新预览的失败/非运行状态。恢复本身不触碰文件；部分副本删除必须再走正常 rollback preview/execute。
7. 以临时数据库和测试自有临时目录加入回归，覆盖审计分页/重启/资产删除、身份写入后中断与最终事务失败、owner 活性与 PID 复用/未知、过期恢复 preview、partial-failed 清理、冻结根保护、同字节异身份替换拒绝及源文件哈希不变。用受控数据库 trigger/注入点模拟持久化失败；禁止访问个人图库。
8. 协作依赖落盘后先检查 helper/schema/API 实际接线，再对 `workflow.rs` 格式化并运行所有 `workflow::tests` 与相关静态检查；需要的默认 all-target 验收由主代理统一安排。审阅只属于本任务的最终 diff，并记录确切命令、结果、协作阻塞和剩余安全边界；不提交。

## 验收边界

- History 分页不会依赖资产行存在，也不会对整表加载或逐文件读 hash。
- 文件创建/删除只由 handle-bound helper 完成；路径从不作为物理身份或删除授权。
- 崩溃留下 durable、带 owner 的 running 日志；不确定 owner 一律拒绝自动恢复。
- 预览不是删除授权本身：rollback execute 必须重核 owner、文件身份、哈希、保护根与竞争状态；IPC 二步确认只证明 UI 确认了服务端 snapshot。
- 规范化/哈希核验与按路径定位仍不能约束所有外部目录树并发变化；真正目标对象的检查与删除由同一 handle 绑定。任何 helper 无法证明的情形都失败关闭。
- 本计划不声明消除文件系统与 SQLite 间的崩溃原子性：如果同句柄删除成功后 DB 终态提交失败，durable running owner/状态必须保留，可由显式 recovery 识别“目标不存在”后仅修正日志。

## 验证记录

- 初始检查：migration 0018 与 `export_file_safety.rs` 尚未落盘，DTO/API 接线不可用；按范围暂不编译。协作依赖落盘后补记定向验证结果。
