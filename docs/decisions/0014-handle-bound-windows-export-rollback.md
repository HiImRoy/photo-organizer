# ADR-0014：Windows 导出回滚绑定文件句柄

- 状态：已接受
- 实现状态：暂停；设计方向已接受，但 helper 尚未集成或验收。
- 日期：2026-10-05
- 关联计划：`docs/plans/0079-handle-bound-export-file-safety.md`

## 背景

当前编辑导出回滚在几个独立步骤中按路径解析目标、读取 BLAKE3、解析规范路径以排除图库根，最后调用 `std::fs::remove_file`。外部进程可在最后一次检查后替换目标路径，因此最终被删除的对象不一定是刚刚校验哈希与保护边界的对象。把核验和删除都改为路径调用仍会保留这个竞态。

Windows 提供以文件 handle 执行 `SetFileInformationByHandle` 的 `FileDispositionInfo` 删除能力；API 要求 handle 带有 DELETE 权限。`GetFileInformationByHandle` 可取得卷序列号和文件索引；`GetFinalPathNameByHandleW` 可返回该 handle 所指对象解析后的最终路径。项目当前依赖 `windows 0.61.3`，无需新增生产 crate 或版本。Windows 是现有发布 CI 平台。

## 决策

1. Windows lease 以 READ|DELETE 权限打开目标，并将共享模式限于 READ，使兼容的外部写入、删除和重命名无法在 lease 期间取得冲突 handle。每次快照和最终删除都在该 handle 上流式读取 64 KiB 缓冲的 BLAKE3、读取物理身份，并查询最终路径。`delete_checked` 验证预期 hash、预期物理身份和保护根后，在同一 handle 上调用 `SetFileInformationByHandle(FileDispositionInfo)`，再关闭 handle；禁止 `std::fs::remove_file` fallback。
2. 物理身份编码为 volume serial、file index high/low 和 creation FILETIME。路径只参与保护根判定，不充当文件 ID。Unicode 路径通过 Win32 宽字符 API 和 `PathBuf` 保留。
3. 创建器使用 `create_new`，取得读、写、删除权限且只允许其他 handle 共享读。它在创建后立即通过 handle 验证最终路径不在任何保护根内；打开或边界核验失败时，仅可尝试对仍持有的创建 handle 设置删除标记。`finish` flush 与 sync 后在该 handle 上计算 hash 和快照；`abort` 只对它自己的 handle 执行同对象删除。若删除能力不可证或操作失败，保留文件并返回错误。
4. `inspect_file_identity` 只读取 filesystem metadata/handle 信息，不解码源图像像素。hash 只读取字节，不调用图片库。导出图像编码仍由调用方向 `writer.file_mut()` 写入有界的已处理图像。
5. 非 Windows 保留 CLI 编译和可用的 metadata/process 能力；无法保证按同一个已验证 handle 原子删除时，`delete_checked` 与 `abort` 明确返回 `std::io::ErrorKind::Unsupported`，不以按路径删除伪装成等价保证。非 Windows 不支持查询任意进程实例：本进程返回稳定 UUID stamp，其他 PID 的活性返回 Unsupported。
6. Windows 进程实例身份采用 creation FILETIME。活性检查以受限查询权限打开进程、检查退出码与 `GetProcessTimes`；PID 不存在或进程已退出返回 `Ok(false)`，创建时间不匹配表示 PID 已复用并返回 `Ok(false)`，时间匹配且仍运行返回 `Ok(true)`。访问被拒绝或其他无法判定情况返回错误，禁止把未知解释为死亡。
7. 使用现有 `windows 0.61.3` 增加所需 `Win32_Storage_FileSystem`、`Win32_System_Threading` 等 features；不改依赖版本，不引入新生产依赖。

## 方案比较

| 方案                                                    | 取舍                                                                                                                                   | 决定                     |
| ------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------- | ------------------------ |
| 路径 hash/边界检查后 `remove_file`                      | 检查与删除分别解析路径，可删除被替换的新对象，正是待关闭竞态。                                                                         | 拒绝                     |
| 重新打开路径后检查，再路径删除                          | 仍有最终检查到 unlink 之间的竞态；新 handle 也可能已指向替换对象。                                                                     | 拒绝                     |
| 同一 Windows handle 校验并 `SetFileInformationByHandle` | 将被检查的字节、身份、final path 与删除绑定到同一 handle；占用 handle 时拒绝常见外部写/删除/重命名。需要 Windows 专用 API 与 feature。 | 采用                     |
| 跨平台按路径模拟                                        | 无法提供同等的 handle-bound delete 保证，静默 fallback 会重新引入漏洞。                                                                | 危险操作明确 Unsupported |

## 影响与限制

- 采用方案依赖文件系统/驱动正确实现 Windows share 与 handle disposition 语义。遇到拒绝访问、无法取得最终路径、文件 ID 不完整或 disposition 失败时操作失败关闭；不得退化为路径删除。
- 同一目标 handle 阻止目标文件在打开期间被常规写入、删除或重命名。保护根检查读取该 handle 的 final path。外部对祖先目录进行并发移动不属于这里实现的目录树锁；如需抵御恶意祖先目录重挂载，需要另外的目录 handle/安全边界设计。
- 内容 hash 仍是流式字节核验，不做图片解码。源路径的物理身份检查只访问元数据/handle，不读入源图像像素。
- 非 Windows 用户仍可构建与使用 CLI；编辑回滚的危险删除与 writer abort 会给出 Unsupported 并保留目标，不能假定路径删除等价。
- 进程权限错误会让状态保持未知并阻止基于该结果的清理或恢复；这是 fail-closed 行为。

## 官方 API 参考

- Microsoft Learn: [SetFileInformationByHandle](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-setfileinformationbyhandle)（`FileDispositionInfo` 需要 DELETE access，删除通过目标 handle 提交）
- Microsoft Learn: [GetFinalPathNameByHandleW](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getfinalpathnamebyhandlew)（返回指定 handle 的最终路径）
- Microsoft Learn: [GetFileInformationByHandle](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getfileinformationbyhandle)（文件信息结构包含可用于对象比对的卷序列号和文件索引）
- Microsoft Learn: [OpenProcess](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-openprocess)（按进程对象权限打开本地进程；权限错误不能解释成已退出）
