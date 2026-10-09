# 0079 句柄绑定的导出文件安全

状态：暂停；未验收。helper 与 Cargo feature 仅存在于本地 WIP，尚未注册到 lib.rs，未运行 Rust 检查或 Windows 安全测试。

## 目标与范围

关闭编辑导出回滚在核验目标哈希与图库边界后、按路径删除前遭遇外部替换的竞态。Windows 必须在一个不允许其他句柄写入或删除的文件句柄上读取内容哈希、卷与文件 ID、创建时间和最终解析路径，并通过该同一句柄调用 `SetFileInformationByHandle(FileDispositionInfo)` 删除。不得在核验后使用 `std::fs::remove_file` 重新解析目标路径。

本里程碑只拥有以下文件：

- `src-tauri/src/export_file_safety.rs`（新增）
- `src-tauri/Cargo.toml`
- `src-tauri/Cargo.lock`（仅由必要 Windows feature 解析器更新）
- 本计划
- `docs/decisions/0014-handle-bound-windows-export-rollback.md`（新增）

`workflow.rs` 与 `lib.rs` 由 workflow 集成方负责；本任务提供可直接注册的公开 API，不编辑这两个文件。不得清理构建产物或提交更改。不得访问个人照片目录；文件系统测试限于 `tempfile` 创建的测试目录或 `test-data/`。

## 调查结论与假设

- 现有回滚先按路径计算 BLAKE3、解析目标路径并检查保护根，之后使用 `remove_file`；这些独立路径操作允许检查对象与删除对象不同。
- 项目已有 `windows = 0.61.3`，可在原版本中启用必要的 `Win32_Storage_FileSystem` 与 `Win32_System_Threading` 等 feature，不增加生产 crate 或版本。
- `protected_roots` 使用项目 `source_identity::identity_key` 兼容的规范化根路径字符串；实际目标路径必须来自当前打开句柄，并先归一化再做同路径/子路径检查。
- Windows 的 `BY_HANDLE_FILE_INFORMATION` 组合身份采用 volume serial、file index high/low 和 creation FILETIME。路径只用于边界与展示，不充当物理文件身份。
- 句柄以 READ|DELETE（写入器另含 WRITE）打开，并只共享 READ，以阻止兼容的外部写句柄、删除和重命名；不支持所需句柄语义的文件系统必须返回错误，不可降级到路径删除。
- 非 Windows 仍须编译。可用平台本地元数据实现只读文件 ID/快照；任何无法按句柄删除同一对象的 `delete_checked`/`abort` 必须返回 `Unsupported`，留下文件而不按路径回退。
- 进程启动身份在 Windows 使用进程 creation FILETIME；非 Windows 的本进程可使用稳定 UUID stamp，但无法证明其他进程的生命周期时须返回 `Unsupported`。

## 实施步骤

1. 先提交此计划与 ADR，再实现模块边界、路径保护比较、流式 64 KiB 哈希、Windows 句柄打开/身份/最终路径/删除和进程实例查询。
2. 实现 `GeneratedTargetWriter`：`create_new` 创建并持有目标句柄；写入器公开 `file_mut` 和写入前身份查询；`finish` flush、sync、按句柄 hash 并返回快照；`abort` 只尝试删除该创建句柄自身代表的文件。若安全删除失败则保留并报告错误。
3. 实现 `GeneratedTargetLease`：在一个排他写入/删除共享策略下持有已生成目标；快照与 `delete_checked` 的 hash、身份、最终路径均从同一 handle 读取；删除仅使用同一 handle 的 FileDispositionInfo。
4. 为 Windows 文件信息、final path 和删除 API 启用现有 `windows 0.61.3` feature；确认 lockfile 只包含 feature 解析变化而无 crate/version 变化。
5. 添加仅使用测试自有临时目录的回归测试：图库原图根拒绝、Unicode 路径、相同字节不同身份、持有期间写/替换/重命名被拒绝或安全失败、按句柄删除不影响源/邻居、元数据检查不解码像素、writer abort 只删除自身文件、进程存活/不存在/身份不匹配，以及权限不确定时 fail-closed。通过仅测试 helper 对目标路径进行受控替换/移动，验证旧 handle 不会误删路径上后来出现的文件。
6. 完成本模块后通知 workflow/lib 集成方公开接口可用。待其注册模块后，对拥有文件运行 rustfmt 和定向 Rust 测试/静态检查；审阅 diff、lockfile 与测试目录状态。

## 验收

- Windows 回滚校验和删除均绑定到同一 OS 文件对象；危险删除不再重解析路径。
- 目标保护检查使用从 handle 获取的实际 final path 与项目 path identity 规则；身份是卷 ID、文件索引与创建时间，不使用路径冒充。
- hash 以 64 KiB 固定缓冲区流式读取，不创建图像像素缓冲。
- 输出创建只用 `create_new`，永不覆盖；abort 失败时不误删其他路径对象。
- Windows 文件系统语义或权限不能证明时明确报错。非 Windows CLI 编译，危险删除返回明确 `Unsupported` 而没有路径删除 fallback。
- 进程 PID 复用通过创建时间戳区分；权限不足或状态不确定时返回错误。
- 不增生产依赖/版本、不改 `workflow.rs`/`lib.rs`，无清理或提交操作。

## 验证记录

- 尚未实施或运行；待代码与 `lib.rs` 注册就绪后更新。
