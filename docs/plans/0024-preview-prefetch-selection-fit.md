# Preview Prefetch, Current Mark, and Fit Interaction

状态：IMPLEMENTED_PENDING_MANUAL

边界说明：本计划只描述用户主动查看单图预览时的 viewer-only 资源缓存。它不得被复用到导入、基础分析、语义分析或模型推理；这些流程统一遵循 `docs/decisions/0010-thumbnail-only-import-analysis.md` 和计划 0036 的缩略图-only 契约。

## 目标

在不改变 SourceRoot、IPC 原图大小边界和既有预览架构的前提下，改善单图预览的连续浏览体验：

1. 当前图片加载原图后，在后台有限预取前后相邻图片的原图。
2. 在内存中复用有限数量的原图 data URL，避免重复点击同一图片时再次读取。
3. 修复预览容器尚未完成布局时适应比例被锁定的问题。
4. 在胶片栏用跟随当前 Asset 的环形描边标记正在预览的图片，并保留批量选择状态的独立语义。
5. 双击交互改为：适应屏幕状态 → 100%，放大状态 → 适应屏幕。

## 范围与假设

- 预取范围只包含当前图片前后相邻的最多两张图片，不加载整页或整库原图。
- 原图缓存只存在当前桌面会话的前端内存中，最多保留当前图和两个邻近图，并限制估算内存占用；过大的响应不进入缓存。
- 预取请求失败只静默结束，不影响当前预览；用户实际点击时仍会重新请求。
- 缓存键使用 Asset ID、源路径、文件大小和修改时间，避免同一 Asset 被重新扫描后复用旧 data URL。
- 不写入 SourceRoot，不改变数据库 schema，不增加生产依赖。

## 实施内容

- `usePreviewController`：增加去重、有限 LRU 原图缓存和延迟预取；当前请求与邻近请求共享 in-flight Promise。
- `usePreviewController`：适配计算忽略零尺寸测量，并在图片加载、双击和窗口变化时重新测量。
- `App.tsx`：将当前图片前后邻居传入预览控制器；在胶片栏用环形描边和 `aria-current` 标记当前图片，只有缩略图离开可视区时才做最小滚动，不强制居中。
- `App.test.tsx`：覆盖邻图预取复用、当前图片标识和双击两段式缩放。
- `docs/refactor/checkpoint-c-preview.md`：记录本次体验修复仍属于 C 的部分实现，不恢复已回退的 DPR/tier 方案。

## 安全与性能边界

- 预取不会绕过现有 `get_preview_data_url` 的原图大小限制。
- 缓存不会暴露原始路径，也不会修改、复制或删除原始文件。
- 预取延迟启动，让当前图片请求优先；切换图片时只取消尚未启动的邻图预取计时器，已开始的 IPC 请求由 generation guard 和缓存上限约束。
- 运行时只保留有限 data URL，不能因连续浏览无限增长。

## 验收

- 前端测试覆盖当前图邻接预取、缓存复用、当前图片可见标识、fit/100% 双击交互。
- 通过 format、lint、typecheck、frontend tests、build、Rust fmt/clippy/tests 和 `git diff --check`。
- 桌面人工确认：首次打开当前图、点击下一张、返回上一张、快速连续切换、窗口调整、当前标识和双击交互。

## 当前验证结果

- 前端 37 个测试通过。
- `format:check`、`lint`、`typecheck` 和 `build` 通过。
- Rust fmt、Clippy、57 个库测试及 3 个二进制测试通过。
- 桌面人工验收仍待用户确认，开发应用已重新加载本次改动。

## 未解决风险

- 原图仍通过受控 data URL 传给前端；如果真实照片普遍接近 IPC 上限，应另行评估临时文件句柄或流式资源协议，不能在本计划中扩大内存缓存。
- 当前预览导航仍限于已加载的 browse scope；跨页预取不在本计划内。
