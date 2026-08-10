# Checkpoint C — Preview

状态：PARTIAL_REQUIRES_RECONCILIATION

本阶段原计划统一当前图片状态、预览资源层级和异步加载安全。当前按用户体验要求保留旧预览链路，仅保留 `activeAssetId` 单一当前图片状态和“双击回到适应屏幕”的交互调整；新的 DPR/tier 资源方案不作为已完成内容。

## 1. Goal

交付以下能力：

- activeAssetId 成为唯一当前图片状态。
- 删除 previewAssetId。
- Grid、DetailPanel、Single Preview 和 Filmstrip 共享同一当前 Asset。
- 支持 thumbnail、screen、original 三层资源。
- 支持 DPR-aware screen preview。
- 支持 zoom、pan、fit 和键盘导航。
- 取消或保护过期的预览请求。
- 原图传输不会造成无界内存增长。
- Preview 全程只读 SourceRoot。

## 2. Non-goals

- 不改变 Library hierarchy、Asset ownership 或 Browse Scope。
- 不增加 Manual Classification。
- 不升级 Semantic 或 Color 算法。
- 不实现 Export Preview 或 COPY。
- 不写入源图片 metadata。
- 不实现视频或 RAW preview。

## 3. Preconditions

- Checkpoint B 已完成并提交。
- AssetGridItem、AssetDetail 和 activeLibrary scope 已稳定。
- SourcePath、fingerprint 和 AppData cache boundary 已由 A 建立。
- 当前桌面端可以读取 fixture 图片。

## 4. Architecture Invariants

- activeAssetId 是唯一当前图片状态源。
- selectedAssetIds 是批量选择状态，不能替代 activeAssetId。
- 预览资源必须绑定 Asset ID、fingerprint 和尺寸；tier/DPR 资源契约仍待重新规划。
- 过期响应不能覆盖当前 Asset。
- Original 读取只读源文件。
- thumbnail、screen、preview cache 只能写入 AppData。
- missing 或 fingerprint changed 时必须显示明确状态。
- Preview 不重新计算 classification，也不改变 Asset ownership。

## 5. Current Implementation

- [src/App.tsx](D:/Code/Codex/photo-manager/src/App.tsx)
  - 只维护 activeAssetId；activeAsset 同时驱动 DetailPanel、Single Preview、Filmstrip 和导航。
  - openSinglePreview、selectPreview、navigatePreview 只更新 activeAssetId。
  - SinglePreview、ZoomablePreview 和预览请求逻辑位于同一文件。
  - 预览控制器恢复为原图优先、原图失败后回退 screen、generation guard、bounded timeout 的旧逻辑；双击在适应屏幕和 100% 之间切换。
  - 网格当前图或单图当前图进入稳定状态后，延迟预取前后相邻最多两张原图；缓存有数量和估算内存上限，并按源版本复用。
  - 胶片栏以跟随当前 Asset 的环形描边标记预览图片；左右切换时不强制居中，只有当前缩略图离开可视区域时才做最小滚动。fit 测量会忽略零尺寸容器并在图片加载/窗口变化时重算。
- [src/api.ts](D:/Code/Codex/photo-manager/src/api.ts)
  - `fetchPreview(assetId, tier, maxWidth, maxHeight)` 仅保留旧的 screen/original 请求兼容参数，不按 viewport/DPR 重请求。
- [src/components/Thumbnail.tsx](D:/Code/Codex/photo-manager/src/components/Thumbnail.tsx)
  - 负责网格 thumbnail 显示和加载状态。
- [src-tauri/src/ipc.rs](D:/Code/Codex/photo-manager/src-tauri/src/ipc.rs)
  - preview IPC 读取 Asset source，生成或读取 AppData preview cache。
  - 恢复旧的 screen/original 路由；screen cache key 绑定 asset、fingerprint 和尺寸，original data URL 受 96 MiB 上限。
- [src-tauri/src/imaging.rs](D:/Code/Codex/photo-manager/src-tauri/src/imaging.rs)
  - process_image 写 thumbnail。
  - load_oriented_image 读取并应用 EXIF orientation。
  - SCREEN_PREVIEW_SPEC 定义当前 screen 规格；screen preview 仍只写 AppData cache。
- [src-tauri/src/paths.rs](D:/Code/Codex/photo-manager/src-tauri/src/paths.rs)
  - 创建 thumbnail、preview 和 database 目录。
- [src/App.test.tsx](D:/Code/Codex/photo-manager/src/App.test.tsx)
  - 当前测试覆盖旧 screen/original 请求、选择、预览、导航、缩放/平移/适配和 Escape；不再把 DPR/tier 方案视为当前契约。

## 6. Target State

以下是待重新规划的未来目标，不代表本轮回退后的当前实现；在重新确认加载性能和缩放体验前，不继续实施其中的 DPR/tier 扩展。

### Domain Model

    PreviewResourceRequest {
        assetId
        fingerprint
        tier
        requestedWidth
        requestedHeight
        devicePixelRatio
    }

    PreviewResource {
        assetId
        fingerprint
        tier
        cacheKey
        dimensions
        payload
    }

### DB Model

本阶段不强制新增业务表。Cache 文件使用 AppData key；如果后续持久化资源 metadata，必须与 Asset fingerprint 绑定。

### React State

- activeAssetId：唯一当前 Asset。
- activePreviewResource：当前 tier 的资源状态。
- previewRequestGeneration：过期保护。
- previewAbortController：取消未完成请求。
- selectedAssetIds：批量选择，独立保留。

### IPC

目标 API：

- fetchPreview(assetId, tier, requestedSize, devicePixelRatio)
- fetchPreviewResource metadata with fingerprint/cache key
- 失败时返回 missing、stale、unsupported 或 decode error。

### Rust Data Flow

    assetId
      ↓
    database sourcePath + fingerprint
      ↓
    source boundary validation
      ↓
    AppData cache lookup
      ↓
    read / orient / resize
      ↓
    bounded preview response

### UI Behavior

- 点击卡片、Enter、双击和 Filmstrip 都只改变 activeAssetId。
- Preview 和 DetailPanel 始终显示同一个 Asset。
- Escape 返回 Grid，但不产生第二个 preview selection。
- Original 只为当前单图查看器和前后相邻最多两张图片按需加载；不预加载整组原图。

## 7. Detailed Implementation Steps

### C1 — Remove previewAssetId

Goal：删除重复的 previewAssetId 状态和派生对象。

- Files to change：[src/App.tsx](E:/Code/Codex/photo-organizer/src/App.tsx)、[src/App.test.tsx](E:/Code/Codex/photo-organizer/src/App.test.tsx)。
- DB/schema impact：无。
- API impact：无。
- React state impact：删除 previewAssetId、previewAsset；所有预览入口只写 activeAssetId。
- Rust/domain impact：无。
- Tests to add/update：搜索源码确保不存在 previewAssetId；更新 preview selection assertions。
- Completion condition：应用中只有 activeAssetId 作为当前图片 ID。
- Dependency：Checkpoint B 完成。

### C2 — activeAssetId Single Source

Goal：统一 Grid、Detail、Single Preview 和 selection anchor 的当前图片语义。

- Files to change：[src/App.tsx](E:/Code/Codex/photo-organizer/src/App.tsx)、[src/components/DetailPanel.tsx](E:/Code/Codex/photo-organizer/src/components/DetailPanel.tsx)、[src/components/AssetCard.tsx](E:/Code/Codex/photo-organizer/src/components/AssetCard.tsx)。
- DB/schema impact：无。
- API impact：getAssetDetail 和 fetchPreview 都以 activeAssetId 为入口。
- React state impact：分页或过滤后如果 activeAsset 不在结果中，统一清理 activeAssetId 或选择明确 fallback。
- Rust/domain impact：无 ownership 变化。
- Tests to add/update：select/open/close/refresh 后三块 UI 显示同一 Asset。
- Completion condition：没有一个 UI 区域保留独立的 preview selection。
- Dependency：C1 完成。

### C3 — Filmstrip Navigation

Goal：基于当前 Library Browse Scope 和当前分页结果实现 Filmstrip。

- Files to change：[src/App.tsx](E:/Code/Codex/photo-organizer/src/App.tsx)、新增或调整 Filmstrip component、[src/types.ts](E:/Code/Codex/photo-organizer/src/types.ts)。
- DB/schema impact：无。
- API impact：需要使用当前 scope 的 summary list，不重新构造 direct-only list。
- React state impact：Filmstrip 点击只更新 activeAssetId；跨页导航使用 page/pageSize 和 stale guard。
- Rust/domain impact：导航顺序必须与当前 sort/filter query 一致。
- Tests to add/update：上一张、下一张、跨页、Parent descendants、过滤后导航。
- Completion condition：Filmstrip 不跳出当前 Library Scope 和 FilterState。
- Dependency：C2 完成。

### C4 — Preview Resource Tier Model

Goal：明确 thumbnail、screen、original 的请求和缓存契约。

- Files to change：[src/types.ts](E:/Code/Codex/photo-organizer/src/types.ts)、[src/api.ts](E:/Code/Codex/photo-organizer/src/api.ts)、[src-tauri/src/models.rs](E:/Code/Codex/photo-organizer/src-tauri/src/models.rs)、[src-tauri/src/ipc.rs](E:/Code/Codex/photo-organizer/src-tauri/src/ipc.rs)。
- DB/schema impact：无强制 migration；cache key 必须包含 fingerprint、tier 和尺寸。
- API impact：fetchPreview 扩展为带 tier、requested size、DPR 的 typed request。
- React state impact：按 tier 管理 loading、ready、error 和 stale。
- Rust/domain impact：thumbnail 使用既有 cache；screen/original 采用明确上限和响应格式。
- Tests to add/update：tier routing、cache key、fingerprint invalidation、unsupported tier。
- Completion condition：不同 tier 不会复用错误尺寸或旧 fingerprint。
- Dependency：C2 完成。

### C5 — DPR-aware Screen Preview

Goal：根据 viewport 和 devicePixelRatio 请求合适的 screen resource。

- Files to change：[src/App.tsx](E:/Code/Codex/photo-organizer/src/App.tsx)、预览 component、[src/api.ts](E:/Code/Codex/photo-organizer/src/api.ts)、[src-tauri/src/ipc.rs](E:/Code/Codex/photo-organizer/src-tauri/src/ipc.rs)。
- DB/schema impact：cache metadata 或 file key 增加 requested dimensions/DPR。
- API impact：screen preview request 携带 DPR 和最大尺寸。
- React state impact：viewport resize 使当前 screen resource stale 并可重新请求。
- Rust/domain impact：限制最大 decode/output size，避免无界内存。
- Tests to add/update：DPR 1/2、窗口 resize、cache reuse、large image bounds。
- Completion condition：screen 资源清晰度和内存上限符合请求。
- Dependency：C4 完成。

### C6 — Original Image Transport

Goal：提供受控 Original 读取，避免长期持有大 base64。

- Files to change：[src-tauri/src/ipc.rs](E:/Code/Codex/photo-organizer/src-tauri/src/ipc.rs)、[src-tauri/src/imaging.rs](E:/Code/Codex/photo-organizer/src-tauri/src/imaging.rs)、[src/api.ts](E:/Code/Codex/photo-organizer/src/api.ts)、预览 component。
- DB/schema impact：无；如建立临时 cache，必须绑定 fingerprint 并写入 AppData。
- API impact：选择 bounded bytes、temporary resource handle 或受控 data URL；禁止将任意 raw path 暴露给 frontend。
- React state impact：Original 加载独立于 screen，完成后可释放。
- Rust/domain impact：读取前重新验证 Asset source 和 fingerprint；decode failure 明确返回。
- Tests to add/update：大文件、missing source、changed fingerprint、memory bound、Unicode path。
- Completion condition：Original 只读，源文件不变，传输不会无限累积。
- Dependency：C4、C5 完成。

### C7 — Async Cancellation / Stale Guard

Goal：取消无用请求并阻止旧响应覆盖当前图片。

- Files to change：[src/App.tsx](E:/Code/Codex/photo-organizer/src/App.tsx)、[src/api.ts](E:/Code/Codex/photo-organizer/src/api.ts)、[src-tauri/src/ipc.rs](E:/Code/Codex/photo-organizer/src-tauri/src/ipc.rs)。
- DB/schema impact：无。
- API impact：支持 request cancellation 或可安全忽略的 request token。
- React state impact：AbortController、generation token、assetId/fingerprint guard。
- Rust/domain impact：长 decode/read 可响应 cancellation；缓存写入使用 create_new 或原子 rename。
- Tests to add/update：快速切换 Asset、切换 tier、关闭 preview、旧响应延迟返回。
- Completion condition：旧请求永远不能更新当前图片状态。
- Dependency：C4-C6 完成。

### C8 — Zoom / Pan / Fit

Goal：实现可预测的缩放、平移和适配窗口行为。

- Files to change：预览 component、[src/App.tsx](E:/Code/Codex/photo-organizer/src/App.tsx)、相关 styles。
- DB/schema impact：无。
- API impact：必要时在 zoom threshold 触发 Original tier，但不重新计算分类。
- React state impact：zoom、pan、fit 状态绑定 activeAssetId；切换 Asset 时重置或明确保留策略。
- Rust/domain impact：无。
- Tests to add/update：fit、zoom in/out、pan bounds、switch Asset reset、large image。
- Completion condition：缩放不会导致页面滚动或请求风暴。
- Dependency：C5-C7 完成。

### C9 — Keyboard / Double Click / Esc

Goal：统一键盘和鼠标交互。

- Files to change：[src/App.tsx](E:/Code/Codex/photo-organizer/src/App.tsx)、[src/components/AssetCard.tsx](E:/Code/Codex/photo-organizer/src/components/AssetCard.tsx)、preview component。
- DB/schema impact：无。
- API impact：无。
- React state impact：Enter、双击和键盘左右键只更新 activeAssetId；Esc 关闭单图视图或清除选择。
- Rust/domain impact：无。
- Tests to add/update：Enter、double click、ArrowLeft/Right、Escape、input focus suppression。
- Completion condition：交互不会产生第二个 preview state。
- Dependency：C2、C3、C8 完成。

### C10 — Desktop Manual Verification

Goal：验证预览在真实桌面窗口中的性能和交互。

- Files to change：[src/App.test.tsx](E:/Code/Codex/photo-organizer/src/App.test.tsx)、visual fixture 和必要的 preview component。
- DB/schema impact：无。
- API impact：验证三层 preview IPC。
- React state impact：验证分页、FilterState、activeAssetId 和 Filmstrip。
- Rust/domain impact：验证 cache、source read、cancel 和 memory bounds。
- Tests to add/update：frontend preview tests、Rust preview tests、desktop smoke。
- Completion condition：所有 Exit Criteria 和 Manual Verification 通过。
- Dependency：C1-C9 全部完成。

## 8. Migration Strategy

本阶段预计不需要业务 schema migration。

- 现有 thumbnail/preview cache 文件如果继续使用，必须加入新的 tier、尺寸、DPR 和 fingerprint key。
- 旧 cache 无法证明 fingerprint 或尺寸一致时，允许懒惰失效并重新生成。
- 不扫描 SourceRoot 清理旧 cache。
- 如果未来需要 preview metadata 表，必须另立 migration，不在 C 中隐式增加。
- cache 写入失败只影响 preview，不得改变 Asset 或 source data。

## 9. Automated Tests

### Rust unit

- oriented image read。
- source boundary check。
- 旧 preview cache 和 source boundary 行为。

### Rust integration

- thumbnail/screen/original read。
- missing source。
- changed fingerprint。
- cancellation。
- AppData-only cache writes。

### Frontend

- activeAssetId single source。
- no previewAssetId。
- Filmstrip navigation。
- stale request guard。
- zoom/pan/fit。
- keyboard and Escape。

### DB migration

本阶段无 DB migration；如果检测到需要 schema，必须停止并重新审核范围。

### Source integrity

- preview、zoom、screen 和 original 前后源 hash 不变。
- source mtime 不被改变。

### Evaluation

本阶段不评估模型质量；只评估 preview latency、failure rate 和资源上限。

## 10. Manual Verification

1. 在 Parent Library 页面打开一张 Asset。
   - 预期：DetailPanel 和 Preview 显示同一 Asset。
2. 从 Grid 双击 Asset。
   - 预期：进入 Single Preview，activeAssetId 只有一个。
3. 点击 Filmstrip 另一张图片。
   - 预期：Detail、Preview、当前高亮同步变化。
4. 使用左右方向键。
   - 预期：按照当前 sort/filter 和 recursive scope 导航。
5. 打开单图并缩放到不同级别。
   - 预期：当前图片优先请求 original；原图读取失败时回退 screen，只有限预取相邻图片。
6. 快速连续切换多张图片。
   - 预期：旧请求不会覆盖最后选中的图片。
7. 等待当前图打开后切换下一张再返回。
   - 预期：邻图会在后台预取，已缓存图片不重复发起原图请求。
8. 在适应屏幕状态双击，再次双击回到适应屏幕。
   - 预期：第一次为 100%，第二次恢复适应屏幕。
9. 调整窗口尺寸。
   - 预期：预览继续适配窗口，不触发连续的 DPR/screen 资源请求。
10. 使用 zoom、pan、fit。

- 预期：图片不会跳出边界，切换 Asset 后状态符合定义。

11. 按 Escape。

- 预期：关闭 Single Preview 或清除选择，不产生隐藏 preview state。

12. 断开或删除 fixture source。
    - 预期：显示 missing/error，不修改数据库外的源目录。
13. 检查 cache。
    - 预期：只写入 AppData，源文件 hash 不变。

## 11. Exit Criteria

- [x] previewAssetId 已删除。
- [x] activeAssetId 是唯一当前图片状态。
- [x] Grid、Detail、Preview、Filmstrip 状态一致。
- [ ] thumbnail、screen、original 三层 tier 契约稳定（本轮未保留新 tier 方案）。
- [ ] cache key 包含 Asset fingerprint、tier、尺寸和 DPR（待重新规划）。
- [x] Original transport 有大小和内存边界。
- [x] 旧响应不会覆盖当前 Asset。
- [x] zoom、pan、fit、键盘和 Escape 通过自动化测试。
- [x] 当前图片标识、邻图有限预取和 fit/100% 双击行为通过自动化测试。
- [x] Preview 不写 SourceRoot（自动化 source-boundary 测试通过）。
- [x] Rust、frontend 和现有 source-integrity 自动化测试通过。
- [ ] Manual Verification 全部通过。
- [ ] 创建独立 Checkpoint C commit。

## 12. Expected Files To Change

- [src/App.tsx](D:/Code/Codex/photo-manager/src/App.tsx)
- [src/types.ts](D:/Code/Codex/photo-manager/src/types.ts)
- [src/api.ts](D:/Code/Codex/photo-manager/src/api.ts)
- [src/components/AssetCard.tsx](D:/Code/Codex/photo-manager/src/components/AssetCard.tsx)
- [src/components/Thumbnail.tsx](D:/Code/Codex/photo-manager/src/components/Thumbnail.tsx)
- 预览相关 component 和 styles。
- [src/App.test.tsx](D:/Code/Codex/photo-manager/src/App.test.tsx)
- [src-tauri/src/models.rs](D:/Code/Codex/photo-manager/src-tauri/src/models.rs)
- [src-tauri/src/ipc.rs](D:/Code/Codex/photo-manager/src-tauri/src/ipc.rs)
- [src-tauri/src/imaging.rs](D:/Code/Codex/photo-manager/src-tauri/src/imaging.rs)
- [src-tauri/src/paths.rs](D:/Code/Codex/photo-manager/src-tauri/src/paths.rs)

## 13. Risks

- 原图 data URL 或 IPC payload 可能造成内存峰值。
- 多个 tier 和 DPR 会扩大 cache key 数量。
- 现有 generation guard 与真实 cancellation 的语义不同。
- 分页切换期间 activeAssetId 可能暂时不在当前 page。
- Tauri IPC 对大资源的传输方式可能需要 desktop-specific 验证。
- preview cache 的旧格式需要懒惰失效，不能影响数据库和源文件。

## 14. Stop Condition

完成 C1-C10 后：

1. 运行全部 Rust、frontend、preview、path 和 source integrity 测试。
2. 完成 Manual Verification。
3. Review diff，确认没有实现 D-F。
4. 更新 IMPLEMENTATION_STATUS.md。
5. 创建 Checkpoint C commit。
6. 停止，等待审核。
