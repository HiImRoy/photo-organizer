# 0061 交互审计与三组修复验收记录

> 文档状态：三组目标修复及最终验收已完成。
> 更新时间：2026-09-14。
> 本记录替代原来的“仅审计、不改源码”交接边界，记录实际修复、证据、验收结果与尚未完成的限制。

## 1. 范围与证据边界

本任务使用仓库视觉夹具、独立 Vite 服务和 Playwright CLI 验收浏览器交互；未访问个人照片目录、真实用户数据库或不可逆文件操作。Rust 验收使用项目测试和临时测试数据，不依赖用户库。

修复按三组组织：

1. 第一组：图库键盘上下文、弹层 Escape 传播和 SettingsDialog 焦点隔离。
2. 第二组：语义任务同一 job 的控制、worker 进度、终态提交顺序化。
3. 第三组：整理预览与前端 stale fetch/request 生命周期；由 Darwin 负责，当前尚未完成最终验收。

除本记录和 docs/current-functionality.md 外，本次文档工作不修改其他文件。浏览器报告和 trace 是验收产物，不作为源码或用户数据。

## 2. 三组修复总览

| 组别   | 范围                                                                 | 状态   | 主要证据                                                                              |
| ------ | -------------------------------------------------------------------- | ------ | ------------------------------------------------------------------------------------- |
| 第一组 | 全局键盘快捷键、输入/编辑上下文、弹窗隔离、filter Escape、设置焦点   | 已完成 | output/playwright/interaction-audit/interaction-audit-report.md；前端回归与浏览器复验 |
| 第二组 | 语义任务 pause/resume/cancel、worker 批次尾、finish 与持久化终态     | 已完成 | Rust 单测、cargo test/check、cargo fmt -- --check                                     |
| 第三组 | Organization snapshot、旧 plan/export 禁用、旧请求和卸载后的结果忽略 | 已完成 | Organization 8/8、semantic revision 1/1、前端全量验收通过                             |

## 3. 第一组：键盘上下文与弹层焦点

### 3.1 原始问题

浏览器审计在 visual-fixture=library 中复现了四项用户可见问题：

- 搜索框聚焦时按 Enter 仍进入单图预览；
- 单图预览中的搜索框按 ArrowLeft/ArrowRight 仍切换图片；
- 设置按钮保持焦点时按数字会给背后的选中图片评级；
- SettingsDialog 只有 aria-modal，没有初始焦点、Tab/Shift+Tab 圈定和关闭后的触发器焦点恢复。

随后补充确认 filterPopover 的 Escape 只关闭自身、不消费事件，会继续触发 window 全局 Escape；该传播问题也纳入本组回归。

### 3.2 修复与验收

全局 keydown 先尊重 event.defaultPrevented 和输入法合成状态，再判断文本输入/编辑元素、交互控件、弹窗上下文和图库卡片上下文。单键评级与色标只接受无 Ctrl、Meta、Alt 修饰的裸按键；原有 Ctrl/Meta+, 设置快捷键保持不变。图片卡片正常点击取得焦点后，数字评级和视图快捷键仍可用，不因控件过滤而失去图库导航能力。

浏览器复验结果：

- 搜索框 Enter 留在网格，未打开 single-workspace；单图预览中搜索框 ArrowRight 不导航；
- 设置弹窗打开后初始焦点进入“深色”单选项，数字 3 不会评级背景图片；
- 从“完成”按 Tab 循环到“关闭设置”，Shift+Tab 可反向循环；Escape 关闭后焦点恢复到“打开设置”；
- Control+3、Meta+3、Alt+3 不评级；带 Ctrl/Meta/Alt 的色标按键不设置颜色；
- filter Escape 关闭筛选弹层但保留选择集合和网格视图；
- 正常图库卡片点击后，数字评级和视图快捷键仍按图库上下文工作。

证据保存在：

- output/playwright/interaction-audit/interaction-audit-report.md；
- output/playwright/interaction-audit/.playwright-cli/traces/trace-1789325211964.trace；
- 报告中列出的搜索、设置和筛选 snapshot/screenshot。

主代理回归记录为 App 与 BackgroundTaskStatus 相关 69 tests、typecheck 和 lint 通过；浏览器报告还记录了审计基线 13 个文件、92 个测试通过以及本组修复后的逐项复验。

末轮浏览器复验使用已保留的 interaction-audit session 和 visual-fixture=library：

- 搜索框获得焦点后按 Enter，页面仍为 http://localhost:5173/?visual-fixture=library 网格，未出现 single-workspace；
- 打开设置后初始焦点位于“深色”单选项，按数字 3 后首张图片仍显示“未评级”；关闭后“打开设置”触发器恢复焦点；
- 证据快照为 .playwright-cli/page-2026-09-13T20-37-19-640Z.yml、.playwright-cli/page-2026-09-13T20-37-36-010Z.yml 和 .playwright-cli/page-2026-09-13T20-38-00-393Z.yml，均位于 output/playwright/interaction-audit；
- 随后以 npx.cmd Playwright CLI 关闭 interaction-audit 浏览器；5173 已无监听，本任务启动的 Vite 父子进程已退出，报告和快照均保留。

## 4. 第二组：语义任务同 job 状态顺序化

### 4.1 原始问题

原审计确认三类竞态：

- pause IPC 先写 paused，但 worker 批次尾无条件写回 running；
- worker 提交 completed 到 registry remove 之间，late pause/cancel 可重新打开已终态 job；
- registry 中没有 control 时，仅凭持久化 job 存在就允许 cancel，终态 job 也可能被改成 cancelled。

### 4.2 实际修复

- tasks.rs 增加每个语义 job 的共享控制 guard 和不可逆 terminal 标记；pause、resume、cancel 在同一 job 锁下拒绝终态后的控制。
- ipc.rs 在 control 存在时持有同一 job guard，按顺序完成状态持久化和控制信号更新；control 缺失时走持久化的 active-only cancel 契约。library 删除路径继续调用同一取消契约。
- semantic_tasks.rs 将批次尾、暂停/取消发布和最终 completed 放入同一 guard；publish 的 paused/cancelled/completed 状态不会被后续 worker 进度倒退，终态后再移除 registry。持久化失败会记录日志并发布 failed/error 终态，不静默移除为“成功”。
- db.rs 在事务内先读取并检查 job 状态，再以非终态条件更新 job/items；已经是 cancelled 的终态仍允许对应终态发布，不会被 SQL guard 静默挡掉。

### 4.3 确定性测试与结果

针对性单测包括：

- tasks.rs：semantic_task_controls_reject_late_controls_after_terminal；
- semantic_tasks.rs：batch_tail_persists_pause_before_final_finish；
- ipc.rs：persisted_terminal_semantic_job_rejects_registry_absent_cancel。

完整验收已通过：

- C:\Users\13002\.cargo\bin\cargo.exe test --manifest-path src-tauri/Cargo.toml：全量 131 passed、0 failed（汇总为 127 + 0 + 0 + 1 + 3 + 0）；
- C:\Users\13002\.cargo\bin\cargo.exe check --manifest-path src-tauri/Cargo.toml：通过；
- C:\Users\13002\.cargo\bin\cargo.exe fmt --manifest-path src-tauri/Cargo.toml -- --check：通过。

## 5. 第三组：Organization 与 stale fetch（已完成）

Organization 和 App 同 job stale fetch 修复已完成；Organization 目标回归 8/8 通过，semantic revision regression 1/1 通过。

请求 snapshot 必须完整包含：

- libraryId；
- targetRoot；
- scope；
- filter；
- selectedAssetIds；
- rules。

已确认的目标回归：

- 目标目录、范围、筛选、选择或规则变化立即使旧 plan 失效，并禁用旧 plan 的 export；
- 旧请求返回不能覆盖当前输入或最新计划；
- 输入变化后发起的新请求只接受最新 request identity；
- 组件卸载后忽略异步结果；

其中 request snapshot 已覆盖 libraryId、targetRoot、scope、filter、selectedAssetIds、rules；A-B-A 不会复活旧计划，StrictMode 和卸载 guard 已覆盖，preview/export 版本也已绑定。上述 Organization 目标回归由 Darwin 报告为 8/8 通过。

最终验收：

- semantic同job旧查询不会把旧响应写回当前状态；
- npm test：14 files、110 tests 全部通过；
- typecheck、lint、build 全部通过；
- App/Organization 四个文件 prettier --check 通过；
- 全仓 git diff --check 通过；
- scripts/manual-build-start.ps1 -CheckOnly exit 0。Node 22.12.0 低于建议的 22.13 版本并给出 warning，但 build 成功。

上述命令和回归均未访问个人照片目录或用户数据库。

## 6. 覆盖范围与限制

浏览器覆盖了 fixture 网格/单图详情、批量选择、搜索、筛选、搜索 Enter、单图左右键、设置焦点/评级/Tab 圈定和 Escape。按要求未扩展截图矩阵，未在本轮新增主题切换、窄宽布局或排序控件的浏览器检查。

视觉 fixture 没有真实 Tauri IPC、SQLite 持久化、真实扫描延迟、文件冲突或原图访问；fixture 中的评级是 optimistic UI 状态。因此 fixture 能证明浏览器事件上下文和可见交互，不能单独证明桌面后端数据落盘。

Rust 测试覆盖同 job 锁、批次暂停、终态 late control 和 registry 缺失时的持久化终态取消；本组没有扩展恢复架构，也没有把真实用户数据库纳入测试。持久化故障路径有 failed/error 终态和日志，但未新增独立恢复机制。

当前 interaction-audit 浏览器会话和本任务启动的 Vite 服务已完成末轮快速复验；关闭时仅处理本任务启动的浏览器和 Vite，不清理其他服务、用户文件或既有证据。

## 7. 后续更新点

三组修复及验收已收束；后续仅在发现新的可复现问题时追加证据，不扩大到整理执行/回滚等当前产品尚未开放的能力。

## 8. 2026-09-15 提交前归档记录

- 本次归档只读核对了 git status --short、git diff --stat、git log -5 --oneline 以及 0060/0061 记录；没有 stage、commit、push、amend，也没有自行处理其他代理的改动。
- 当前未提交列表正好 21 条，与本归档上下文提供的预期路径逐项对应：17 个已跟踪且相对 HEAD 有修改的路径为 docs/current-functionality.md、docs/photo-evaluation.md、eslint.config.js、src-tauri/src/db.rs、src-tauri/src/ipc.rs、src-tauri/src/semantic.rs、src-tauri/src/semantic_tasks.rs、src-tauri/src/subject.rs、src-tauri/src/tasks.rs、src/App.test.tsx、src/App.tsx、src/components/BackgroundTaskStatus.test.tsx、src/components/BackgroundTaskStatus.tsx、src/components/OrganizationWorkspace.test.tsx、src/components/OrganizationWorkspace.tsx、src/components/SettingsDialog.tsx、src/components/backgroundTaskStatus.css；4 个未跟踪路径为 docs/plans/0060-classification-repair-verification.md、docs/plans/0061-interaction-audit-and-repair.md、src/components/SettingsDialog.test.tsx、src/components/organizationWorkspace.css。
- 未发现预期路径缺失、额外不明路径或越界修改。已跟踪路径均保留其 HEAD 已提交基线；4 个未跟踪路径尚未进入提交历史。git diff --stat 只统计已跟踪差异：17 files changed、3159 insertions、473 deletions，不包含未跟踪文件。
- 归档时最近五个提交为：e3970a3 chore: checkpoint desktop development and document cross-machine handoff；39cffcb docs: prepare public repository；fd2d7bb fix: align asset query IPC field names；40a2945 feat: unify source and collection browsing；ea1d219 chore: checkpoint before UI redesign。
- 前一轮已完成验证均属于 2026-09-14 的既有结果，本次没有重跑：前端 npm test 为 14 files、110 tests 全部通过；Rust 全量为 131 tests 通过；typecheck、lint、build、Prettier、scripts/manual-build-start.ps1 -CheckOnly 和 git diff --check 均通过。Node 22.12.0 低于建议 22.13 版本并产生 warning，但 build 成功。
- 前一轮浏览器证据仍保留在 output/playwright/interaction-audit；interaction-audit 浏览器已关闭，5173 无监听，本任务启动的 Vite 已退出。此归档段只记录提交前状态，不改变任何源码或用户文件。
