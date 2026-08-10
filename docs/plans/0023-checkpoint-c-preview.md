# Checkpoint C — Preview Execution Plan

状态：PARTIAL_REQUIRES_RECONCILIATION（按用户请求回退）

## 目标

按旧 Checkpoint A→B→C 的顺序收敛现有预览实现：让 `activeAssetId` 成为唯一当前图片状态，明确 thumbnail/screen/original 资源层级，补齐 screen 尺寸/DPR 请求和异步过期保护，并验证缩放、平移、适配、胶片栏和键盘导航。本计划中的新 DPR/tier 资源方案因加载速度和缩放手感回退，不再作为当前实现目标。

## 当前审计结论

- 当前 `src/App.tsx` 已统一使用 `activeAssetId`，DetailPanel 和 Single Preview 共用当前图片。
- 单图浏览恢复为胶片栏、原图优先、screen 回退、Navigator、zoom/pan/fit 和 generation guard 的旧逻辑；双击现在回到适应屏幕。
- Rust preview IPC 保持旧 screen cache 和原图大小边界；cache key 不包含新方案要求的 tier/DPR。
- 当前单图资源只属于当前 active asset；不会预加载整组原图。分页/过滤顺序由当前 `AssetQueryV1` 决定。

## 实施范围

1. 删除 `previewAssetId`，统一 Grid、DetailPanel、Single Preview、Filmstrip 和键盘导航到 `activeAssetId`。
2. 为 preview 请求增加 typed tier 和 requested size/DPR 参数；screen cache key 必须包含 fingerprint、tier、尺寸和 DPR。
3. 保持 source 只读，限制 preview 输入/输出和原图 IPC 大小；使用 generation/abort-safe cleanup 防止旧响应覆盖当前图片。
4. 保留现有 Lightroom-style zoom/pan/fit/Navigator，并补充输入焦点抑制、资源切换重置和测试。
5. 更新 Checkpoint C 状态与验证记录；不实现 D、E、F，也不在本计划内改动数据库 schema。

## 假设与风险

- 继续使用当前 Tauri command + data URL 传输边界，不引入新生产依赖或远程服务。
- 原图仅为当前单图查看器按需读取，受固定 IPC 字节上限保护；screen/thumbnail 仍优先使用 AppData cache。
- Tauri IPC 的底层调用目前没有可移植的 AbortSignal 语义，因此以 generation guard、组件清理和 bounded request 保护 UI；如果桌面验证发现后台 decode 仍需硬取消，再单独立 ADR。
- 当前共享工作树包含此前用户修改，本轮不提交或重置无关 diff；正式 Checkpoint commit 需在用户确认整棵工作树的提交边界后执行。

## 验收

- 前端 preview tests 覆盖 single-source、tier 参数、stale response、zoom/pan/fit、keyboard/Escape。
- Rust preview tests 覆盖尺寸/DPR clamp、cache key、tier 路由和 source/cache boundary。
- 运行 format、lint、typecheck、frontend tests、build、Rust fmt/clippy/tests，并复核 `git diff --check`。
- 桌面人工验收仍由用户在当前开发应用中执行；在此之前不得将 Checkpoint C 标记为完整通过。

## 当前结果

- 已撤销本计划引入的 viewport/DPR screen 请求、tier/cache-key 扩展以及对应的专用测试。
- 当前仅保留 `activeAssetId` 单一状态和双击回到适应屏幕；回退后的前端 37 个测试、Rust 57 个库测试及 3 个二进制测试已通过。
- 本计划不产生独立 Checkpoint C commit；共享工作树中的前序修改未被提交或重置。
