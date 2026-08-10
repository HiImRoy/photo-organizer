# Checkpoint D — Semantic + Dominant Color

状态：PARTIAL_REQUIRES_RECONCILIATION

本阶段升级 Semantic taxonomy、分类状态语义、模型版本追踪和 Dominant Color pipeline。当前工作树已完成 D1-D3 的 taxonomy/拒识/分组基线，但 D4-D10 的评估、阈值校准、Dominant Color 细化和人工视觉复核仍未完成；完成后必须提交并停止，不能自动进入 Checkpoint E。

### 2026-08-10 执行记录

- 新增执行计划 [0025-semantic-taxonomy-and-open-set](../plans/0025-semantic-taxonomy-and-open-set.md) 和 ADR 0006。
- `semantic_labels` 通过 migration 0012 增加 `category_group` 与 `taxonomy_version`；旧结果保留但不会混入当前 taxonomy。
- 当前自动标签分为 `scene`（互斥且最多一个）、`subject`（多选）和 `context`（多选）。旧 `primary_category` 继续作为 scene 的兼容槽位。
- `unknown` 已从 TinyCLIP prompt 和相似度榜单移除；完成但没有可靠 scene 的结果由 Effective Resolver 生成虚拟拒识状态，FAILED 仍不生成自动分类。
- 侧栏、详情、场景分组和筛选已消费同一分组元数据；未知筛选使用虚拟状态条件。
- Rust 58 个库测试 + 3 个二进制测试、clippy、fmt check，以及前端 38 个测试、typecheck、format check 已通过；尚未完成人工语义质量验收。

## 1. Goal

交付以下能力：

- 稳定的 Semantic taxonomy IDs。
- Primary Category 与 Auxiliary Tags 分离。
- OTHER、UNKNOWN、FAILED 三者严格分离。
- Semantic model version 和 taxonomy version 独立记录。
- TinyCLIP prompt ensemble、threshold、confidence 和 margin 可追踪。
- Imaging、Color、Tone pipeline 版本独立记录。
- Dominant Color 输出面积调色板和视觉显著调色板，不再依赖单一 RGB 主色。
- 视觉显著调色板使用显著性、环境对比度、色度、面积和空间连续性综合排序，并支持多个主色。
- Semantic 和 Color 都有固定 evaluation dataset。
- 输出 before/after 结果并完成人工视觉复核。

## 2. Non-goals

- 不重新设计 Asset Ownership。
- 不修改 Library Browse Scope 或 Scan Scope。
- 不增加 Manual Override 数据模型；B 已负责该层。
- 不重新设计 Preview resource transport。
- 不执行 Export Preview 或 COPY。
- 不实现模型训练。
- 不实现人脸身份识别、视频或完整 RAW 开发。

## 3. Preconditions

- Checkpoint B 已完成并提交。
- Effective Classification Resolver 已存在。
- Auto 和 Manual 数据已经分层。
- classificationRevision 可以检测 Effective 变化。
- Checkpoint C 可以提供稳定的预览用于人工复核。
- evaluation fixture 和模型资源在本地可用。

## 4. Architecture Invariants

### 状态语义

Semantic 分析成功但置信度不足：

    semanticAnalysisStatus = COMPLETED
    primaryCategory = UNKNOWN

Semantic 推理或执行失败：

    semanticAnalysisStatus = FAILED
    primaryCategory = no auto result

明确类别：

    semanticAnalysisStatus = COMPLETED
    primaryCategory = concrete category or OTHER

UNKNOWN 不能代表 FAILED。OTHER、UNKNOWN、FAILED 不能混淆。

### Pipeline Versions

Semantic：

    semanticModelVersion
    taxonomyVersion

Imaging / Color：

    imagingAnalysisVersion
    colorAlgorithmVersion
    toneAlgorithmVersion

不同 pipeline 独立判断过期，不使用一个通用 modelVersion 替代。

### Classification Layers

- D 只写 Auto Classification。
- 不删除 Manual Override。
- Effective 仍由 B 的唯一 Resolver 计算。
- Registry 仍只有 Primary Category、Auxiliary Tags、Tone、Dominant Color Category / Palette、Saturation Level。
- Dominant Color Category / Palette 是多值分类；调色板的 RGB、coverage、saliency 和 spatial coherence 属于 Imaging Auto detail，不进入 Registry。

### Dominant Color Interaction Contract

- 当前 `ColorSwatches` 只负责选择 `dominantColorCategories` 的稳定分类 ID，不直接编辑 RGB、coverage、saliency 或其他算法候选数据。
- D 升级 Color pipeline 时，不得把主色人工修正退回文本输入或单选控件；分类层仍使用可多选色块，且选择结果写入 Manual Override 的完整类别列表。
- `coveragePalette` 和 `prominentPalette` 可以有不同数量、排序和候选指标，但只能通过稳定分类 ID 进入 Registry；候选 RGB、面积占比和显著性占比作为只读 Auto detail 展示。
- Color reanalysis 只更新 Auto 结果和对应 pipeline 版本，不覆盖已有 Manual Override；Restore Auto 后才重新采用最新 Auto Effective 结果。
- 如果 D 调整颜色 taxonomy 或增加类别，必须同步更新稳定 ID、中文目录、色块映射、数据库兼容读取、筛选和 evaluation fixture，不能只修改某一个界面常量。
- 所有主色筛选继续使用 Effective category list；不能用 raw RGB 或仅 Auto 结果替代 Effective。

## 5. Current Implementation

- [src-tauri/src/semantic.rs](E:/Code/Codex/photo-organizer/src-tauri/src/semantic.rs)
  - LABELS 数组定义稳定 ID、displayName、category group 和 threshold；unknown 不再是模型候选。
  - select_predictions 按 scene/subject/context 分组选择；scene 只保留一个通过阈值和组内 margin 的结果。
  - 当前 inference error 由 SemanticError 返回。
- [src-tauri/src/semantic_tasks.rs](E:/Code/Codex/photo-organizer/src-tauri/src/semantic_tasks.rs)
  - 负责逐 Asset 执行 Semantic task 和保存结果。
  - 失败仍写入 semantic_status=failed；成功空结果由读取层解释为虚拟 unknown。
- [src-tauri/src/models.rs](E:/Code/Codex/photo-organizer/src-tauri/src/models.rs)
  - SemanticLabelResult 同时包含 modelVersion、analysisVersion、taxonomyVersion、categoryGroup、isManual、isPrimary。
  - Imaging、Color、Tone 的独立版本仍待后续 D 阶段完善。
- [src-tauri/src/db.rs](E:/Code/Codex/photo-organizer/src-tauri/src/db.rs)
  - semantic_labels 保存 label、threshold、source fingerprint、category group 和 taxonomy version。
  - semantic_labels_for_asset 通过当前模型、分析版本、taxonomy 和 fingerprint 读取结果。
  - list_semantic_groups 同时返回 scene、subject、context 的有效标签计数，并包含虚拟 unknown。
- [src-tauri/src/imaging.rs](E:/Code/Codex/photo-organizer/src-tauri/src/imaging.rs)
  - ANALYSIS_VERSION 当前同时承担基础 imaging 版本。
  - analyze_rgba 计算 brightness、saturation、chroma、neutral ratio、dominant color 和 coverage。
  - 当前颜色候选来自量化 RGB bin 的权重排序，没有显式 saliency map、感知空间聚类或空间连续性约束；虽然保存 top colors，但业务结果仍以单一 dominant color category 为主。
  - tone_label 当前返回 low_key、mid_tone、high_key。
- [src-tauri/src/bin/semantic-evaluate.rs](E:/Code/Codex/photo-organizer/src-tauri/src/bin/semantic-evaluate.rs)
  - 当前提供 Semantic evaluation 工具。
- [src-tauri/src/bin/semantic-benchmark.rs](E:/Code/Codex/photo-organizer/src-tauri/src/bin/semantic-benchmark.rs)
  - 当前提供 inference benchmark。
- [docs/model-evaluation.md](E:/Code/Codex/photo-organizer/docs/model-evaluation.md)
  - 记录当前模型评估背景。

### 当前分类注册表（D1-D3 基线已落地）

- scene：人像、多人、风景、建筑、产品、静物、食品、动物、截图、文档、抽象、其他。
- subject：车辆、花卉、山、水体、森林。
- context：室内、街道、夜景、日落。
- 未知不出现在 catalog；前端用虚拟 descriptor 提供筛选和手动恢复入口，但不显示模型分数。
- 这不代表 D4-D10 已完成，完整阈值校准、evaluation dataset、Color pipeline 和人工视觉复核仍按本 Checkpoint 的剩余任务执行。

## 6. Target State

### Domain Model

    SemanticAutoResult {
        status
        semanticModelVersion
        taxonomyVersion
        primaryCategory
        auxiliaryTags
        confidence
        margin
        sourceFingerprint
    }

    ImagingAutoResult {
        status
        imagingAnalysisVersion
        colorAlgorithmVersion
        toneAlgorithmVersion
        tone
        coveragePalette
        prominentPalette
        dominantColorCategories
        saturationLevel
        numericFeatures
        sourceFingerprint
    }

    ColorCandidate {
        rank
        rgb
        category
        areaCoverage
        saliencyCoverage
        localContrast
        chroma
        spatialCoherence
    }

### DB Model

- Auto semantic rows带 semanticModelVersion、taxonomyVersion、sourceFingerprint。
- Auto imaging rows带 imagingAnalysisVersion、colorAlgorithmVersion、toneAlgorithmVersion、sourceFingerprint。
- Auto imaging rows保存 `coveragePalette` 和 `prominentPalette`；旧的单一 `dominantColorCategory` 只作为兼容读取字段，不得覆盖新的多值结果。
- 失败运行有明确 status 和 error，但不生成当前 Auto 分类值。
- 历史结果可以保留，但必须按版本隔离。

### React State

- Semantic progress 显示 pipeline-specific status。
- DetailPanel 区分 COMPLETED/UNKNOWN、FAILED 和 no result。
- Color/Tone stale 状态可独立显示。
- 主色编辑继续使用色块多选；算法候选的面积/显著性/连续性等信息以只读明细展示，不改变人工修正的操作契约。

### IPC

目标 API 返回：

- Semantic runtime metadata。
- Semantic taxonomy metadata。
- Imaging/color/tone algorithm versions。
- 每个 pipeline 的 status、error、stale 信息。

### Rust Data Flow

    source fingerprint
      ↓
    selected pipeline version
      ↓
    inference / imaging analysis
      ↓
    versioned Auto result
      ↓
    Effective Resolver

### UI Behavior

- UNKNOWN 显示为“成功但无法可靠判断”。
- FAILED 显示为“分析失败”。
- OTHER 显示为明确的其他类别。
- UI 不把 FAILED 渲染为 UNKNOWN。

## 7. Detailed Implementation Steps

### D1 — Stable Taxonomy IDs

Goal：建立不依赖 UI 文案的稳定 Semantic taxonomy。

- Files to change：[src-tauri/src/semantic.rs](E:/Code/Codex/photo-organizer/src-tauri/src/semantic.rs)、[src-tauri/src/models.rs](E:/Code/Codex/photo-organizer/src-tauri/src/models.rs)、[src/types.ts](E:/Code/Codex/photo-organizer/src/types.ts)。
- DB/schema impact：未来独立 migration（A 已使用 0007、0008，B 使用 0009）或等价 schema 增加 taxonomyVersion 和稳定 label IDs；旧 displayName 只作为展示。
- API impact：catalog 返回 id、displayName、category role、taxonomyVersion。
- React state impact：筛选和人工修正使用 stable ID，不使用中文文案。
- Rust/domain impact：迁移当前 LABELS id，明确 Primary/ Auxiliary、OTHER、UNKNOWN。
- Tests to add/update：ID stability、display label change 不改变 query、taxonomy version mismatch。
- Completion condition：同一 taxonomy ID 在模型、数据库、前端和 Export context 一致。
- Dependency：B 完成。

### D2 — OTHER / UNKNOWN / FAILED Separation

Goal：严格实现 Semantic success、low confidence 和 failure 的状态矩阵。

- Files to change：[src-tauri/src/semantic.rs](E:/Code/Codex/photo-organizer/src-tauri/src/semantic.rs)、[src-tauri/src/semantic_tasks.rs](E:/Code/Codex/photo-organizer/src-tauri/src/semantic_tasks.rs)、[src-tauri/src/db.rs](E:/Code/Codex/photo-organizer/src-tauri/src/db.rs)、[src/types.ts](E:/Code/Codex/photo-organizer/src/types.ts)。
- DB/schema impact：semantic status 和 current auto result 分离；FAILED 不写 UNKNOWN。
- API impact：返回 semanticAnalysisStatus、primaryCategory、error 的明确组合。
- React state impact：FAILED、UNKNOWN、OTHER 有不同 label、filter 和 empty state。
- Rust/domain impact：select_predictions 只在成功且 confidence/margin 不足时返回 UNKNOWN；inference error 保存 FAILED 且 current auto result 为空。
- Tests to add/update：successful low confidence、explicit OTHER、model failure、missing result、manual override on failed asset。
- Completion condition：不存在通过 primaryCategory=UNKNOWN 替代 FAILED 的路径。
- Dependency：D1 完成。

### D3 — Primary / Auxiliary Separation

Goal：确保一个 Semantic 结果中 Primary Category 和 Auxiliary Tags 的职责明确。

- Files to change：[src-tauri/src/semantic.rs](E:/Code/Codex/photo-organizer/src-tauri/src/semantic.rs)、[src-tauri/src/semantic_tasks.rs](E:/Code/Codex/photo-organizer/src-tauri/src/semantic_tasks.rs)、[src-tauri/src/db.rs](E:/Code/Codex/photo-organizer/src-tauri/src/db.rs)。
- DB/schema impact：is_primary 只表示自动结果中的 primary role；Manual tags 继续由 B 的 override 表维护。
- API impact：Asset Auto DTO 明确 primaryCategory 和 auxiliaryTags。
- React state impact：Detail、Card、Filter 不将所有 label 当作同一种分类。
- Rust/domain impact：保证 primary 选择规则稳定；OTHER/UNKNOWN 只能出现在规定的 primary 语义中。
- Tests to add/update：multiple auxiliary labels、single primary、tag filtering、manual ADD/REMOVE。
- Completion condition：Primary 和 Auxiliary 在 Auto、Manual、Effective、Filter 四层一致。
- Dependency：D2 完成。

### D4 — Semantic Evaluation Dataset

Goal：建立可重复的 Semantic evaluation fixture 和标注格式。

- Files to change：[src-tauri/src/bin/semantic-evaluate.rs](E:/Code/Codex/photo-organizer/src-tauri/src/bin/semantic-evaluate.rs)、[src-tauri/src/bin/semantic-benchmark.rs](E:/Code/Codex/photo-organizer/src-tauri/src/bin/semantic-benchmark.rs)、[docs/model-evaluation.md](E:/Code/Codex/photo-organizer/docs/model-evaluation.md)、test-data evaluation fixtures。
- DB/schema impact：无业务 schema；评估结果写入 docs/benchmarks 或 test output，不写 SourceRoot。
- API impact：评估工具记录 taxonomy、model、threshold 和 status。
- React state impact：无产品 UI 变化。
- Rust/domain impact：数据集必须能区分 concrete、OTHER、UNKNOWN、FAILED。
- Tests to add/update：dataset load、missing sample、deterministic output、failure accounting。
- Completion condition：同一模型和版本可以重复生成可比较报告。
- Dependency：D2、D3 完成。

### D5 — TinyCLIP Prompt Ensemble / Thresholds

Goal：提升 Semantic 预测的稳定性，并记录 confidence/margin。

- Files to change：[src-tauri/src/semantic.rs](E:/Code/Codex/photo-organizer/src-tauri/src/semantic.rs)、[src-tauri/src/semantic_tasks.rs](E:/Code/Codex/photo-organizer/src-tauri/src/semantic_tasks.rs)、模型 metadata 相关代码。
- DB/schema impact：Auto rows 保存 semanticModelVersion、taxonomyVersion、confidence、margin、sourceFingerprint。
- API impact：Semantic progress 和 result 暴露实际运行版本与失败信息。
- React state impact：不改变 Manual override；显示版本和低置信度提示。
- Rust/domain impact：实现 prompt ensemble 或分阶段 prompt；明确 threshold、top score margin 和 UNKNOWN 条件。
- Tests to add/update：threshold boundary、margin boundary、ensemble deterministic、UNKNOWN generation、FAILED propagation。
- Completion condition：UNKNOWN 只由成功分析的置信度规则产生。
- Dependency：D4 完成；不得跳过 evaluation baseline。

### D6 — Semantic Before/After Evaluation

Goal：量化模型或 prompt 变更，不以主观 UI 观感作为完成依据。

- Files to change：[src-tauri/src/bin/semantic-evaluate.rs](E:/Code/Codex/photo-organizer/src-tauri/src/bin/semantic-evaluate.rs)、[docs/model-evaluation.md](E:/Code/Codex/photo-organizer/docs/model-evaluation.md)、evaluation output。
- DB/schema impact：无业务 schema。
- API impact：报告包含 old/new model、taxonomy、confidence、margin、OTHER、UNKNOWN、FAILED 计数。
- React state impact：无。
- Rust/domain impact：保持同一 fixture、同一指标和同一 failure accounting。
- Tests to add/update：before/after report schema、regression threshold、manual review sample selection。
- Completion condition：报告已保存，回归项已审阅，未接受的变化已记录。
- Dependency：D5 完成。

### D7 — Imaging Analysis Versioning

Goal：拆分 Imaging、Color 和 Tone pipeline version。

- Files to change：[src-tauri/src/imaging.rs](E:/Code/Codex/photo-organizer/src-tauri/src/imaging.rs)、[src-tauri/src/models.rs](E:/Code/Codex/photo-organizer/src-tauri/src/models.rs)、[src-tauri/src/db.rs](E:/Code/Codex/photo-organizer/src-tauri/src/db.rs)。
- DB/schema impact：未来 schema 增加 imagingAnalysisVersion、colorAlgorithmVersion、toneAlgorithmVersion；旧 algorithm_version 兼容读取。
- API impact：Asset Detail 和 stale status 分 pipeline 返回。
- React state impact：Color/Tone stale 不自动伪装为 Semantic stale。
- Rust/domain impact：基础 features、dominant color 和 tone 使用各自版本；source fingerprint 变化使相关 pipeline stale。
- Tests to add/update：version mismatch、independent stale、source fingerprint change、reanalysis isolation。
- Completion condition：Semantic reanalysis 与 Color upgrade 可以分别判断是否过期。
- Dependency：D6 完成。

### D8 — Visual Dominant Color Algorithm

Goal：建立有版本、可解释、可评估的多颜色视觉主色结果。

- Files to change：[src-tauri/src/imaging.rs](E:/Code/Codex/photo-organizer/src-tauri/src/imaging.rs)、[src-tauri/src/models.rs](E:/Code/Codex/photo-organizer/src-tauri/src/models.rs)。
- DB/schema impact：colorAlgorithmVersion 和 dominant color category 记录独立保存。
- API impact：返回 `coveragePalette`、`prominentPalette`、Effective category list、neutral ratio、candidate metrics 和 color algorithm version。
- React state impact：展示多个颜色 swatch 及其角色/占比；分类筛选使用 Effective category list，不把 raw RGB 当作单一分类。
- Rust/domain impact：
  - 复用现有 640px 分析缓存，先生成低分辨率显著性权重；默认采用轻量、可解释的显著性方法，不引入语义分割模型。
  - 在 Lab/OKLab 等感知颜色空间聚类并合并感知相近颜色。
  - 为每个候选计算 area coverage、saliency coverage、local contrast、chroma 和 spatial coherence。
  - 输出 3–5 个 `coveragePalette` 颜色和 1–3 个 `prominentPalette` 颜色；中性颜色单独参与 neutral 结果，不得吞掉有意义的有彩色候选。
  - 对孤立噪点设置最小面积/连续性约束，并用颜色距离阈值避免同一色相被重复计入。
- Tests to add/update：
  - neutral image、single hue、multi-color、low coverage、dark/highlight、Unicode source path。
  - 大面积背景 + 小面积高对比色、主体/背景反差、多色主体、孤立噪点和空间分散色。
  - 验证多色排序、面积与显著性占比、感知颜色合并、空间连续性和确定性。
- Completion condition：颜色类别可重复、版本可追踪；`coveragePalette` 与 `prominentPalette` 均稳定；neutral、chromatic、多个视觉主色不混淆。
- Dependency：D7 完成。

### D9 — Color Evaluation Dataset

Goal：建立固定 Dominant Color、Prominent Color 和 Saturation Level evaluation dataset。

- Files to change：[src-tauri/src/imaging.rs](E:/Code/Codex/photo-organizer/src-tauri/src/imaging.rs)、新增 evaluation harness、[docs/photo-evaluation.md](E:/Code/Codex/photo-organizer/docs/photo-evaluation.md)。
- DB/schema impact：无业务 schema；评估输出不写 SourceRoot。
- API impact：报告包含 colorAlgorithmVersion、toneAlgorithmVersion 和 source fingerprint。
- React state impact：无。
- Rust/domain impact：数据集覆盖 neutral、单色、多色、低饱和和高饱和图片，并标记 coverage palette 与 prominent palette 的人工参考结果。
- Tests to add/update：deterministic feature output、top-k palette agreement、category expected range、algorithm version recording、saliency/coverage ranking regression。
- Completion condition：Color/Tone 结果可重复比较，且多色结果可以通过固定人工参考集进行 before/after 评估。
- Dependency：D8 完成。

### D10 — Before/After + Manual Visual Review

Goal：完成 Semantic、Color、Tone 的综合回归和人工视觉复核。

- Files to change：[docs/model-evaluation.md](E:/Code/Codex/photo-organizer/docs/model-evaluation.md)、[docs/photo-evaluation.md](E:/Code/Codex/photo-organizer/docs/photo-evaluation.md)、evaluation outputs、相关 Rust tests。
- DB/schema impact：无新增业务 schema；确认版本化 Auto rows。
- API impact：确认 Detail、Filter 和 Export context 获得正确 versioned Effective inputs。
- React state impact：确认 UNKNOWN/FAILED、color/tone stale 和 manual override 显示正确。
- Rust/domain impact：确认 D 的 Auto updates 不删除 B 的 Manual rows。
- Tests to add/update：综合 before/after、manual review checklist、source integrity。
- Completion condition：所有未解决回归都有记录和审核结论。
- Dependency：D1-D9 全部完成。

## 8. Migration Strategy

本阶段规划版本字段和 taxonomy 迁移，但本次不创建 migration 文件。

### Planned taxonomy/version migration

- 迁移前备份 SQLite。
- 为 semantic auto result 增加 semanticModelVersion、taxonomyVersion。
- 为 imaging/color/tone result 增加各自 pipeline version。
- 用现有 MODEL_NAME、MODEL_VERSION、SEMANTIC_ANALYSIS_VERSION 和 imaging ANALYSIS_VERSION backfill。
- 历史 result 保留但标记旧版本；不能把旧版本伪装为当前版本。
- failed run 不生成 current auto classification。
- 旧 semantic_labels.is_manual/is_excluded 继续由 B 的 override migration 处理。

### Failure behavior

- 迁移失败回滚事务。
- 保留迁移前 DB backup。
- 不删除历史 Auto 或 Manual 数据。
- 不修改 SourceRoot。
- schema 采用 forward-only，回退使用数据库备份。

## 9. Automated Tests

### Rust unit

- stable taxonomy ID。
- primary/auxiliary split。
- OTHER/UNKNOWN/FAILED matrix。
- threshold and margin。
- pipeline-specific stale。
- color neutral/chromatic boundary。

### Rust integration

- semantic success low confidence。
- semantic inference failure。
- independent semantic/color reanalysis。
- source fingerprint invalidation。
- manual override preservation。

### Frontend

- status labels and filters。
- unknown/failed rendering。
- version and stale display。
- Effective classification remains correct。

### DB migration

- version backfill。
- historical result compatibility。
- no deletion of manual data。
- failed result has no current auto classification。

### Source integrity

- semantic and imaging analysis only read SourceRoot。
- evaluation reports are written outside SourceRoot。

### Evaluation

- fixed semantic dataset。
- fixed color/tone dataset。
- before/after report。
- manual visual review checklist。

## 10. Manual Verification

1. 对一个高置信度图片运行 Semantic。
   - 预期：status 为 COMPLETED，Primary 为具体类别。
2. 对一个低置信度图片运行 Semantic。
   - 预期：status 为 COMPLETED，Primary 为 UNKNOWN。
3. 模拟或使用一个推理失败样本。
   - 预期：status 为 FAILED，Primary 没有 Auto Result，不显示 UNKNOWN。
4. 检查明确的 OTHER 样本。
   - 预期：status 为 COMPLETED，Primary 为 OTHER。
5. 检查 Primary 和 Auxiliary Tags。
   - 预期：两者在 Detail 和 Filter 中分开显示。
6. 修改已有人工分类后重新运行 Semantic。
   - 预期：Auto 更新，Manual 和 Effective override 保留。
7. 升级 Color algorithm version。
   - 预期：Color stale 独立出现，Semantic 不被无条件标记 stale。
8. 运行 Dominant Color fixture。
   - 预期：neutral、单色、多色、低 coverage 和小面积高对比色结果稳定；同时可以分别查看 coveragePalette 与 prominentPalette。
9. 查看 evaluation report。
   - 预期：包含 model、taxonomy、pipeline version、confidence、margin 和 failure counts。
10. 检查 SourceRoot。
    - 预期：源文件内容和 hash 不变。

## 11. Exit Criteria

- [ ] taxonomy IDs 稳定且不依赖 UI 文案。
- [ ] Primary Category 和 Auxiliary Tags 分离。
- [ ] OTHER、UNKNOWN、FAILED 完全分离。
- [ ] Semantic version 和 taxonomy version 独立记录。
- [ ] Imaging、Color、Tone version 独立记录。
- [ ] Semantic 和 Color 可分别判断 stale。
- [ ] UNKNOWN 只由成功但低置信度分析产生。
- [ ] FAILED 没有 current Auto classification。
- [ ] TinyCLIP threshold/margin 可追踪。
- [ ] Dominant Color 算法有版本和 evaluation。
- [ ] Dominant Color 支持多个有序候选，并分别输出 coveragePalette 与 prominentPalette。
- [ ] Prominent Color 使用显著性、环境对比度、色度、面积和空间连续性综合排序。
- [ ] 感知相近颜色被合并，孤立噪点不会成为主色。
- [ ] before/after 报告和人工视觉复核完成。
- [ ] Manual Override 未被任何 Auto reanalysis 删除。
- [ ] Rust、frontend、evaluation 和 source integrity 测试通过。
- [ ] Manual Verification 全部通过。
- [ ] 创建独立 Checkpoint D commit。

## 12. Expected Files To Change

- [src/types.ts](E:/Code/Codex/photo-organizer/src/types.ts)
- [src-tauri/src/models.rs](E:/Code/Codex/photo-organizer/src-tauri/src/models.rs)
- [src-tauri/src/db.rs](E:/Code/Codex/photo-organizer/src-tauri/src/db.rs)
- [src-tauri/src/semantic.rs](E:/Code/Codex/photo-organizer/src-tauri/src/semantic.rs)
- [src-tauri/src/semantic_tasks.rs](E:/Code/Codex/photo-organizer/src-tauri/src/semantic_tasks.rs)
- [src-tauri/src/imaging.rs](E:/Code/Codex/photo-organizer/src-tauri/src/imaging.rs)
- [src-tauri/src/bin/semantic-evaluate.rs](E:/Code/Codex/photo-organizer/src-tauri/src/bin/semantic-evaluate.rs)
- [src-tauri/src/bin/semantic-benchmark.rs](E:/Code/Codex/photo-organizer/src-tauri/src/bin/semantic-benchmark.rs)
- [docs/model-evaluation.md](E:/Code/Codex/photo-organizer/docs/model-evaluation.md)
- [docs/photo-evaluation.md](E:/Code/Codex/photo-organizer/docs/photo-evaluation.md)
- evaluation fixture 和报告文件。
- 未来新增 taxonomy/version migration；本次不创建。

## 13. Risks

- Prompt ensemble 可能增加 CPU 时间和模型内存。
- taxonomy ID 变更可能影响历史 semantic rows 和人工 override 映射。
- UNKNOWN 阈值调整会改变 Filter 和 Export 结果。
- Semantic、Color、Tone 版本拆分可能暴露旧数据无法精确 backfill。
- Dominant Color 对中性、暗部和多色图片可能存在主观边界。
- analysis failure 如果与旧成功结果混存，必须避免旧结果被误当作当前结果。

## 14. Stop Condition

完成 D1-D10 后：

1. 运行全部 Rust、frontend、evaluation、DB 和 source integrity 测试。
2. 完成 before/after 和人工视觉复核。
3. Review diff，确认没有实现 E-F。
4. 更新 IMPLEMENTATION_STATUS.md。
5. 创建 Checkpoint D commit。
6. 停止，等待审核。
