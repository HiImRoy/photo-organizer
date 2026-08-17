# Refactor Implementation Status

本文件记录旧 Checkpoint A-F 的实施状态。2026-08-10 起，Checkpoint G 将真实代码、旧计划和产品架构审查合并为新的继续工作基线；未完成的阶段不得因“已有部分代码”填写为完成。

跨 Checkpoint 的图像处理不变量：导入、图像解码、基础特征、题材/环境/主体分析和模型推理只使用应用私有缩略图或有界缩略图派生输入；原文件仅用于元数据、指纹、EXIF/内嵌预览元数据和受控的目标尺寸缩略图提取。显式 `original` 只属于用户主动查看的预览例外。

## Checkpoint A — Source Boundary + Nested Library

Status: BLOCKED_FOR_REVIEW

Commit: checkpoint A implementation snapshot plus verification fixes (see `git log`)

Migration: 0005 Library Source Identity and Hierarchy; 0006 Global Asset Identity and Ownership

Automated tests:

- Passed: `npm run format:check`
- Passed: `npm run lint`
- Passed: `npm run typecheck`
- Passed: `npm test` (10 tests)
- Passed: `npm run build`
- Passed: `npm run test:rust` (38 tests; Rust toolchain on PATH for the verification shell)
- Passed: `npm run clippy` (`-D warnings`)
- Passed: `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
- Passed: `npm run tauri -- info` (WebView2, MSVC, Rust, Cargo, rustup)
- Passed: `npm run tauri -- build --debug` (EXE, MSI, and NSIS bundles)
- Passed: import performance regression tests (shared 640px cache, bounded resize,
  full fingerprint reuse for small sources)
- Passed: import responsiveness safeguards (coalesced scan progress, batched job
  progress persistence, rebuildable thumbnail cache writes, throttled frontend refresh)

Manual verification: Not completed; the interactive Parent/Child/Grandchild desktop flow has not been run. The repository currently contains no tracked synthetic image fixtures beyond `test-data/README.md`; automated Rust integration tests cover the source-boundary and Parent/Child ownership flow.

Known issues:

- Checkpoint A remains blocked for review until the Manual Verification list in `checkpoint-a-library-safety.md` is completed in the desktop application.
- Import performance optimization Phases 1 and 2 are implemented in
  `docs/plans/0005-import-performance.md`; Release import speed was manually
  accepted, and the temporary profiling logs were removed after diagnosis.
- Preview generation now runs off the UI command path. Single-image preview requests
  the original source first and falls back to a bounded screen preview if the source
  cannot be read; the fallback uses a 1920x1200 default bound, a 15-second UI timeout,
  and a fixed centered canvas. The old floating zoom bar is replaced by a Lightroom-style
  Navigator in the right Information panel with a larger viewport frame, click/drag navigation,
  and a read-only current zoom ratio; the main canvas handles zooming and only displays the image.
- The post-clarification Library import/hierarchy milestone is implemented behind
  migration 0007: import can expose direct image-containing subfolders as
  independent Libraries, and drag/drop or the root-drop action persists a manual
  `parentLibraryId` with self/descendant cycle validation. This milestone does not
  change Checkpoint A's manual-review status or start Checkpoint B.
- The installed Rust toolchain is available at `C:\Users\666\.cargo\bin`; a new terminal should be opened if an existing shell does not yet include it in PATH.
- Checkpoint B 已实现并等待桌面端人工验证；Checkpoint C 的新预览资源方案因加载和缩放体验回退到旧逻辑，当前增量仅包含有限邻图预取、当前图片标记和 fit/100% 双击交互；Checkpoint D–F 仍需后续阶段处理。

## Checkpoint B — Manual Classification + Effective Filter

Status: IMPLEMENTED_PENDING_MANUAL

Commit: Checkpoint B implementation snapshot plus automated verification (see `git log`)

Migration: 0009 Manual Classification Overrides

Automated tests:

- Passed: `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
- Passed: `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings`
- Passed: `cargo test --manifest-path src-tauri/Cargo.toml --all-targets --all-features` (45 library tests + semantic benchmark/evaluation tests)
- Passed: `npm run format:check`
- Passed: `npm run lint`
- Passed: `npm run typecheck -- --pretty false`
- Passed: `npm test -- --run` (19 tests)
- Passed: `npm run build`

Manual verification:

Pending: DetailPanel single-asset editor, Auxiliary Tag ADD/REMOVE, Restore Auto, batch editor, Effective filters, FAILED versus UNKNOWN, reanalysis preservation, and source-file integrity.

Known issues:

- B cannot be marked `COMPLETED` until the manual checklist in `checkpoint-b-classification-filter.md` is run in the desktop app.
- The classification editor is collapsed by default, uses Chinese labels and selection-only controls, uses color swatches for main-color choices, and no longer accepts raw classification IDs as text input.
- Checkpoint A remains `BLOCKED_FOR_REVIEW`; this implementation does not silently change A's status.
- Objective Numeric Feature remains read-only and outside the Derived Classification Registry.

## Checkpoint C — Preview

Status: PARTIAL_REQUIRES_RECONCILIATION

Commit:

Migration:

Automated tests:

- Passed: `npm run format:check`
- Passed: `npm run lint`
- Passed: `npm run typecheck`
- Passed: `npm test -- --run` (37 tests)
- Passed: `npm run build`
- Passed: `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
- Passed: `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings`
- Passed: `cargo test --manifest-path src-tauri/Cargo.toml --all-targets --all-features` (57 library tests + 3 binary tests)
- Preview regression coverage now follows the previous request path: original-first,
  screen fallback, generation guard, zoom/pan/fit, keyboard/Escape, and double-click fit.
- The DPR-aware screen tier/cache-key tests from the superseded implementation were removed.

Manual verification:

Pending: Parent/Child scope preview, screen resource quality after window/DPR changes, missing
source/error presentation, AppData-only cache writes, source hash integrity, and real desktop
memory/latency behavior.

Known issues:

- `activeAssetId` 现在是 Grid、DetailPanel、Single Preview、Filmstrip 和键盘导航的唯一当前图片状态。
- 本节记录的是 Checkpoint C 历史实现；其中“原图优先”和原图预取路径已被后续缩略图-only 规范取代，不得恢复到导入或分析流程。
- 当前 screen 预览使用有界缩略图派生输出；generation guard、组件清理和 15 秒超时仍保护旧响应。Tauri 的底层 decode 没有可移植的硬 AbortSignal。
- `original` data URL 仅保留为用户主动查看的显式预览例外，不能被导入、分析或模型推理复用。
- 双击预览现在在“适应屏幕”和“100%”之间切换；其余缩放、平移和 Navigator 逻辑保持旧实现。
- 0024 的历史预取描述不再代表当前导入/分析契约；胶片栏以环形描边跟随当前图并在左右切换后自动居中，仍等待桌面端验证真实加载收益。
- 不能在完成重新规划和桌面人工清单前标记 `COMPLETED`；当前工作树还包含前序用户修改，因此没有创建独立 C commit。

## Checkpoint D — Semantic + Dominant Color

Status: PARTIAL_REQUIRES_RECONCILIATION

Commit:

Migration: 0012 Semantic Taxonomy (`category_group`, `taxonomy_version`)

Automated tests:

- Passed: `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
- Passed: `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings`
- Passed: `cargo test --manifest-path src-tauri/Cargo.toml --all-targets --all-features` (79 library tests + 3 binary tests)
- Passed: targeted Prettier check for the changed frontend/plan files; repository-wide `npm run format:check` still reports pre-existing warnings in `docs/plans/0026-places365-scene-clustering.md` and `src-tauri/resources/models/places365-resnet18/MODEL-SOURCE.md`.
- Passed: `npm run typecheck -- --pretty false`
- Passed: `npm test -- --run` (38 tests)
- Passed: `npm run lint` and `npm run build`

Manual verification:

Pending: Semantic quality review on representative local fixtures, unknown/other boundary review, taxonomy filter counts, manual scene override, auxiliary multi-label display, and source integrity.

Known issues:

- 历史 TinyCLIP、当前 SigLIP 2、缩略图输入、批处理、Tone/Color 和人工 override 已存在过对应实现；当前 MVP 只保留 SigLIP 2 题材模型。
- D1-D3 taxonomy/拒识/分组基线已实现；`unknown` 不再是模型 prompt，成功空结果由 Effective Resolver 生成虚拟拒识状态。
- 0029 题材候选层已接入：SigLIP 2 使用多 prompt 匹配 logits 作为可审计候选证据，Places365 映射证据负责可观察场景题材的保守主类，主体任务可补充明确的单人/多人/动物/车辆/食品/植物标签；当前 taxonomy 已收敛为 `photo-organizer-photography-topics-v3`，主体 taxonomy 为 `photo-organizer-subject-tags-v2`。低置信题材统一归入抽象艺术，纪实与工业不再作为摄影师筛选类别。MVP 已移除 TinyCLIP/MobileCLIP 的随包资源和装载入口。
- D8 多强调色第一版已实现：缩略图-only 的 OKLab 加权聚类、感知近色合并、显著性/面积/空间连续性排序、`coveragePalette`/`prominentPalette` 多值返回、数据库 Effective 筛选和中文 UI 展示均已接入。
- SigLIP 2 评测协议、批量评测 CLI、逐类阈值候选和 margin 扫描已经实现；仓库没有授权的真实摄影评测集，因此尚未完成逐类 quality calibration、混淆矩阵人工复核、Color evaluation、before/after 报告和 manual visual review Exit Criteria，也没有把候选阈值写回运行时。D 仍保持 PARTIAL_REQUIRES_RECONCILIATION。

## Checkpoint E — Export Preview

Status: PARTIAL_REQUIRES_RECONCILIATION

Commit:

Migration:

Automated tests:

Manual verification:

Known issues:

- Organization Dry-run、rules、mapping、issues 和 manifest 已存在。
- 当前 0003 plan 是 compact audit snapshot，不能作为 Safe Copy 的 immutable execution source。
- `get_organization_plan` 不恢复 items，manifest 仍接收前端传回的 plan object。

## Checkpoint F — COPY Export

Status: NOT_STARTED_FOR_ORGANIZATION

Commit:

Migration:

Automated tests:

Manual verification:

Known issues:

- 没有消费 confirmed OrganizationPlan 的 COPY executor。
- Edit workflow 已独立实现派生 copy 和 rollback；这不是 Checkpoint F 完成，未来需要统一 FileOperationService 边界。

## Checkpoint G — Product Architecture Consolidation

Status: IMPLEMENTED_PENDING_MANUAL

Commit: current working tree; G-UI remediation implemented, pending desktop manual acceptance

Migration: None

Review outputs:

- `docs/product-architecture.md`
- `docs/roadmap.md`
- `docs/plans/next-stage-product-strategy.md`
- `docs/refactor/checkpoint-g-product-architecture-consolidation.md`
- `docs/plans/0020-workflow-foundation-consolidation.md`
- `docs/plans/0021-lap-ui-integration-remediation.md`

Automated verification:

- Passed: `npm run format:check`
- Passed: `npm run lint`
- Passed: `npm run typecheck`
- Passed: `npm test -- --run` (53 tests)
- Passed: `npm run build`
- Passed: `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check`
- Passed: `cargo test --manifest-path src-tauri/Cargo.toml --no-default-features --all-targets --quiet` (88 library tests + 1 + 3 binary tests)
- Added source predicates `favoriteOnly` / `collectionId` without a migration; SQLite query tests remain covered by the existing database suite.

Manual verification:

- Product/architecture review completed against current code and LAP product concepts.
- Embedded G-UI now renders one context tool at a time; the old seven-tab workflow navigation is not rendered inside the main browse surface.
- Frontend regression coverage now verifies explicit-selection and current-query scope continuity through Organization Dry-run and back, plus Search/Collection/Duplicate/Similar/Compare/Edit result return and focus behavior.
- Pending: desktop G-UI scripts for Browse → Similar/Compare → mark → Back, Search/Collection → Duplicate/Similar → Back, and Selection → Organization → Back.
- Pending: real desktop layout/scroll check with the embedded tool area at the user's window size.
- `cargo` is installed at `C:\Users\13002\.cargo\bin\cargo.exe`; invoke it by absolute path when the shell PATH is not refreshed.

Known issues:

- Checkpoint G reconciles status; it does not complete A-F Exit Criteria.
- N1 is paused until `G-UI — LAP-derived UI Integration Remediation` is completed.
- The embedded tool area still reuses the historical workflow renderers internally; it is no longer a top-level replacement surface, but the tool renderer should be split further only if manual usability exposes a concrete issue.

## Status Rules

- `IMPLEMENTED_PENDING_MANUAL` 表示自动化 Exit Criteria 已完成，但桌面端人工验证尚未完成；不能等同于 `COMPLETED`。
- `PARTIAL_REQUIRES_RECONCILIATION` 表示能力已存在，但实现顺序或架构已经偏离旧 Checkpoint，必须先按 G 重新收敛，不能标记为完成。
- `REVIEW_COMPLETE` 只表示审查和规划文档完成，不表示下一实施里程碑已开始。
- `N1_PARTIAL` 表示 N1 已开始且部分 gate 已通过；未完成的 benchmark、人工验收或工具链验证必须保持未完成。
- `UI_REMEDIATION_REQUIRED` 表示现有能力可以保留，但主界面整合方式未通过产品验收；不得用改名或单独页面继续扩展。
- 只有在对应文档的全部 Exit Criteria 通过后，Status 才能改为 COMPLETED。
- 如果实现中发现 Architecture Plan 与真实代码冲突，Status 保持 BLOCKED_FOR_REVIEW，并记录冲突，不得自行绕过。
- 每个 Checkpoint 完成后必须创建独立 commit，然后停止。
- 不得在同一个 commit 中混入下一个 Checkpoint 的业务代码。
- Rollback 不属于 Checkpoint F。
