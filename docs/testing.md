# 测试策略

## 安全规则

所有自动化文件系统测试仅可使用仓库 `test-data/` 的只读输入或由测试框架创建并在测试结束回收的临时目录。测试不得枚举用户主目录、图片库或任意系统图库。源夹具在测试前后计算哈希，任何变化都失败。

## 分层

- Rust 单元测试：格式识别、路径规范化、特征计算、迁移、repository、组合 SQL 筛选、缓存键、语义 catalog/主标签、任务控制与 unavailable fallback。
- Rust 集成测试：复制夹具到临时源目录，验证扫描/增量/缺失/损坏/Unicode/缓存失效和源哈希不变。
- React 测试：Vitest + Testing Library，mock Tauri client，覆盖空状态、导入、紧凑任务入口的点击展开、进度、取消、网格、错误、排序和三栏详情。
- UI 可访问性回归：覆盖图片卡片 `aria-pressed` 选中态、进度条数值、错误提示和可见键盘焦点。
- 完整构建验证：由 CI、发布流程或高风险/跨模块变更执行，包括 TypeScript、ESLint、Prettier、Rustfmt、Clippy、Vite production build、Cargo test 和所需 Tauri bundle。
- 手工 smoke：只选择 `test-data/manual-library/`，验证系统对话框、增量展示、重启恢复和安装包启动。

## 本地验证矩阵与升级条件

日常开发按改动范围选择最小且充分的验证。文档或注释-only 改动只需对变更文件做格式检查并运行 `git diff --check`；无需新增测试，也无需运行代码测试、lint、类型检查或构建。若文档改动同时改变了可执行配置、生成流程或用户可见行为，再按相应代码层级验证。

| 改动范围                      | 本地日常验证                                                                                                                           | 何时扩大验证                                                                                                                                                      |
| ----------------------------- | -------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 文档/注释-only                | 变更文档的 Prettier 检查；`git diff --check`                                                                                           | 改动同时影响可执行配置、生成流程或产品行为时，按对应层级升级                                                                                                      |
| 纯 CSS 或小型局部组件         | 纯 CSS 只需检查变更文件格式，并在外观改变时检查受影响视图；TSX 行为或 DOM 契约变化时运行相关 Vitest。CSS-only 不要求 TS lint/typecheck | 涉及共享控件、全局样式、断点、导航/主布局，或无法确定影响范围时，扩大到相关页面、主题、窗口尺寸和测试套件；只有改动 TS/TSX 或影响类型契约时才加 TS lint/typecheck |
| 共享 TypeScript 逻辑          | 变更文件格式检查、相关 lint、typecheck、受影响 Vitest；共享行为需覆盖已知调用方                                                        | 被多个功能区使用、改变公共状态/数据契约或影响跨区工作流时，运行所有受影响套件；高风险时升级到全量前端验证                                                         |
| Rust、IPC、数据库、导入或分析 | 相关 Rust 测试、Rustfmt/Clippy 等受影响静态检查；IPC 或用户可见契约变更时运行对应前端测试/typecheck                                    | 迁移、文件安全边界、缩略图-only 处理边界、模型输入、跨前后端流程或多模块行为变化时，扩大到全量相关 Rust/前端测试和静态检查                                        |
| 启动或打包                    | 保持手动启动合同；运行 `scripts/manual-build-start.ps1 -CheckOnly` 和前端 build                                                        | 按启动/发布流程完成其余必要检查；发布包仍需对应的打包及验收流程                                                                                                   |
| 高风险、跨模块或影响无法隔离  | 完整前端测试、Rust 全目标测试、静态检查和 build                                                                                        | 由 CI/发布门禁确认全量结果；不得以局部通过替代现有 CI 或发布要求                                                                                                  |

升级到更大验证集的明确条件包括：共享控件/全局 CSS/响应式断点变化、IPC 或持久化数据结构变化、文件操作安全行为、导入/缩略图/模型分析边界变化、多个模块共同修改、依赖或构建配置变化，以及无法可靠圈定受影响调用方的情况。CI 的验证强度和发布验收要求保持不变；完整 `npm test`、Rust 全量测试、全量静态检查和构建可由 CI、发布或上述高风险改动承担，无需每个局部修复都在本地重复运行。

局部前端命令示例（把示例路径换成本次实际改动文件）：

```powershell
npx.cmd prettier --check src/components/BackgroundTaskStatus.tsx src/components/backgroundTaskStatus.css
npx.cmd vitest run src/components/BackgroundTaskStatus.test.tsx
npx.cmd eslint --max-warnings 0 src/components/BackgroundTaskStatus.tsx
npm.cmd run typecheck
```

只运行与改动相关的命令：CSS-only 执行格式和必要视觉检查即可，不运行上述 TS lint/typecheck；组件测试只在交互、渲染或 DOM 契约受到影响时运行。全局 CSS/共享控件按升级条件扩展视觉检查；TS lint/typecheck 仍以是否改动 TS/TSX 或其类型契约为准。

失败时先隔离并重跑失败的单项，以判断是可复现问题还是偶发/环境问题。不得通过关闭或弱化测试、隐藏输出、反复重跑直到变绿来掩盖失败；可复现失败应修复，无法修复时要记录证据和原因。最终报告必须逐项说明未运行的检查。

## 手动启动开发窗口

在仓库根目录双击 `启动 PhotoOrganizer.cmd`，或执行 `npm.cmd start`，保持该终端窗口开启即可手动测试桌面应用。启动入口会先构建前端，再通过 Tauri 静态资源协议启动，不依赖 Vite 本地端口；手动验收窗口使用独立的 `PhotoOrganizer Manual` 标题，避免与已安装旧版本混淆；默认将开发数据保存到 `%TEMP%\PhotoOrganizer-dev-data`，并为每次会话生成独立的临时 WebView2 profile，因此不会占用正式应用的数据库和 WebView2 缓存。手动入口默认启用 WebView2 软件/进程内 GPU 与开发沙箱兜底，避免部分 Windows 开发环境的 GPU/沙箱子进程导致黑屏；这些参数只作用于手动开发入口，不代表正式发布安全配置；如需测试默认 WebView2 路径，可在启动前设置 `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS`。开发数据库目录会跨次启动保留。只有在需要检查已有应用数据时，才通过 `PHOTO_ORGANIZER_DATA_DIR` 显式指定目录。

## 用例矩阵

扫描覆盖空/单层/嵌套目录、支持/不支持和大小写扩展名、重复/新增/修改/缺失、损坏/不可读文件，以及中文、俄文、空格、Emoji 和其他 Unicode。不可读场景在 Windows ACL 无法稳定构造时用 reader seam 注入错误；父图库重新扫描时覆盖已登记子图库的回归也由 Rust 测试验证。

数据库覆盖首次/重复初始化、迁移顺序、唯一约束、upsert 和重启恢复；组合筛选还必须验证色相范围严格度按 4%～32% 单调提高目标色的全图面积要求，高严格度要求面积主色匹配，并覆盖中性/低色度排除、多个面积色/强调色、显著小色块、跨红色边界、旧调色板与无效数据。缩略图覆盖正常、损坏、方向、命中、失效、Unicode 路径和不写源目录；高分辨率 fixture 必须证明导入提取后的像素尺寸受限且 `source_decode_us=0`。分析覆盖黑、白、灰、肤色、低彩度冷暖偏色、高低饱和、透明、超小图和所有输出范围。

## CI/发布全量验证命令示例与通过标准

以下是全量验证示例，不是每次小改动的本地必跑清单。CI 当前执行完整前端格式/lint/typecheck/Vitest、Rustfmt、Rust all-targets/all-features tests、Clippy 和前端 production build；发布流程还构建 Windows 安装包。不得通过本地分层策略降低这些 CI/发布门槛。

Windows workflow 在 Node 安装后直接运行 `pwsh -NoLogo -NoProfile -ExecutionPolicy Bypass -File ./scripts/test-invoke-ci-command.ps1`，独立验证 CI 命令包装器；测试只使用自有临时目录，并在 PowerShell 7 与可用的 Windows PowerShell 5.1 中检查原生命令参数、标准输出/错误和精确退出码。workflow 中每个包装器调用都显式传入 `-Command @(...)` 字符串数组，并在该调用后执行 `exit $LASTEXITCODE`，确保自定义 `pwsh -File` 步骤把原生失败状态交给 Actions。结构断言同时检查两个 checkout 的 `lfs: true` 及日志/安装包 artifact 保留策略。

```powershell
npm.cmd run format:check
npm.cmd run lint
npm.cmd run typecheck
npm.cmd test -- --run
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --manifest-path src-tauri/Cargo.toml --all-targets --all-features
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
npm.cmd run build
```

完整验证中的失败必须修复，或如实记录可复现原因及后续处置。任何未运行的检查都不得描述为通过；对局部日常改动，按上方矩阵报告实际选择和未运行项目即可。

## 浏览器视觉回归

视觉验收使用 Vite 开发环境中隔离的确定性 fixture，不连接 SQLite、不调用目录选择器、不扫描个人图库：

```text
http://localhost:1420/?visual-fixture=library
http://localhost:1420/?visual-fixture=scanning
http://localhost:1420/?visual-fixture=error
```

视觉检查按受影响范围和响应式风险选择。局部样式修复检查对应视图，以及相关的常规尺寸和边界/窄窗口；不要求每次都覆盖全部四个尺寸。全局结构、断点、侧栏、网格/胶片布局或多页面共享样式变更，应覆盖 1920×1080、1440×900、1366×768、960×720 中受影响的尺寸，并检查相关状态。涉及主题变量时检查受影响主题。

相邻主/次按钮操作组的几何契约始终有效：凡修改该组或其共享样式，都要核对高度、圆角、边框、基线和间距；至少检查深色、白天主题及一个窄窗口，并覆盖混合主/次按钮。其他局部样式修改只检查实际受影响的控件与状态。截图作为当次验收产物按需生成，不将失效截图长期留在仓库。仅在改动视觉 fixture 或相关构建分支时，额外确认确定性 fixture 模块只存在于开发动态分支。

## 发布前附加验证

安装产物需在干净 Windows VM 检查安装、首次启动、离线运行、卸载、不要求开发工具，并验证源图库哈希不变。起步环境若无法进行 VM 安装测试，必须明确标为“已构建、未安装验证”。

## M0 实际结果（2026-08-06）

- 前端：Vitest 6/6；Prettier、ESLint、TypeScript 与 Vite production build 通过。
- Rust：core/benchmark 16/16；`rustfmt` 与 Clippy（all targets/features，warnings denied）通过。
- 桌面：实际可执行程序启动成功；浏览器像素级 smoke 覆盖空状态和无 Tauri Web fallback，DOM 无错误提示。
- 安装：GNU/LLVM 本机验证 NSIS 完成安装、启动、响应检查和卸载；正式 MSVC/MSI 与干净 VM 仍未验证。
- 安全：扫描测试只在临时夹具目录运行，源图片前后字节/哈希不变；没有对个人图片目录执行测试。

## 语义工作区里程碑实际结果（2026-08-07）

- 前端：Prettier、ESLint、TypeScript、Vitest 6/6 和 Vite production build 通过；开发视觉 fixture 未进入 production bundle。
- Rust core：`rustfmt --check` 通过；gnullvm `cargo test --no-default-features --all-targets` 18/18；对应 Clippy `-D warnings` 通过。
- 历史真实模型基线：release CPU 路径曾对 48 张仓库 PNG 完成 TinyCLIP 推理，失败 0，平均 23.8269 ms，P50 23.1080 ms，P95 30.6964 ms，吞吐 41.9687 张/秒；TinyCLIP 已从当前 MVP 移除，该数字不能代表 SigLIP 2。
- 浏览器：验证网格/单图/胶片栏、双侧宽度调整、语义 AND 筛选与分组；1920×1080、1440×900、1366×768、960×720 无页面溢出；最终新页面控制台无 error/warn。
- 大图库浏览：用 3000 张以上的元数据夹具确认首次只请求一个 120 张批次；滚动接近结果底部后再请求下一批，且没有上一页/下一页控件。缩略图请求必须只在视口附近进入队列，并且前端并发不超过 6。
- 导入范围：使用包含根目录图片、一级子目录图片和更深层图片的临时夹具，分别验证“只导入根目录”“递归导入但不建子图库”“递归导入并按子文件夹建图库”三种组合；重新扫描后仍保持首次选择，关闭子文件夹图片时不得创建或处理子目录图片。
- 桌面打包：`npm.cmd run tauri build` 的前端阶段通过，Rust MSVC 编译因本机没有 `link.exe` 失败；没有生成包含本里程碑变更的安装包，因此打包 WebView2 smoke 未执行。
- 安全：Rust 文件系统测试只使用临时夹具，语义基准只使用仓库图标；未读取或修改个人图库。

## 发布闭环与摄影评估工具结果（2026-08-07）

- 前端与格式：Prettier、ESLint、TypeScript、Vitest 6/6、Rustfmt 和 Vite production build 通过。
- Rust core 回退验证：在本机可用的 gnullvm 环境运行 `--no-default-features --all-targets`，库测试 18/18、benchmark CLI 1/1、摄影评估 CLI 2/2；对应 Clippy `-D warnings` 通过。
- 历史摄影评估 smoke：仅在系统临时目录复制一张仓库应用图标，release CPU 真实加载 TinyCLIP；样本 1、失败 0、原始相似度 21 类、模型加载约 309.54 ms、端到端约 336.41 ms、峰值工作集 120,868,864 bytes。临时报告和夹具随后删除；TinyCLIP 已移除，这些数字不作为当前 SigLIP 2 的摄影质量结果。
- 正式 MSVC 命令：`npm.cmd run test:rust`、`npm.cmd run clippy` 和 `npm.cmd run tauri build` 均已运行，均因本机缺少 `link.exe` 失败。Tauri 调用中的前端 production build 通过。
- 安装 smoke：未生成当前安装包，故安装、启动、导入测试图库、真实分类、暂停/恢复、关闭重启续作、组合筛选和卸载均未运行，不能描述为通过。
- 发布资源：模型、tokenizer、runtime DLL 固定哈希复核通过；配置、许可、第三方声明和来源文件全部存在。
- Git 安全审计：未发现密钥或个人图片；个人绝对路径已改为环境变量写法；旧 benchmark 临时报告和过期 GNU 安装包已删除。

## Windows MSVC 正式打包与安装验收（2026-08-07 当前）

- MSVC 环境实际可用：Build Tools 17.14.37、MSVC 14.44.35207、Windows SDK 10.0.26100.0、`link.exe`/`cl.exe`/MSBuild；Rust `stable-x86_64-pc-windows-msvc`。通过 x64 `VsDevCmd.bat` 加载环境，并补入 `%USERPROFILE%\.cargo\bin`。
- 完整 validate 通过：Prettier、ESLint、TypeScript、Vitest 6/6、Rustfmt、MSVC Cargo tests 21 个、Clippy warnings denied、Vite production build。Node 22.12.0 低于 `package.json` 的 22.13.0 下限，已记录为环境风险。
- Tauri 显式 `--target x86_64-pc-windows-msvc` 后生成 NSIS 与中英文 MSI；三个产物均 `NotSigned`，哈希和路径记录在 `docs/release.md`。该历史资源目录包含 TinyCLIP；当前资源目录应以 Places365、SigLIP 2、PicoDet/YuNet 和 ONNX Runtime 为准。
- 历史 NSIS 安装验收退出码 0；安装后主进程响应，应用数据目录和 SQLite 可打开（本机已有旧测试数据库，未删除用户数据）。当时已安装的 TinyCLIP benchmark（3 张临时夹具）和评估 CLI（2 张 `unknown` 夹具）均真实 CPU 完成且失败 0；关闭/重启后 SQLite 保留；自带卸载器退出码 0 且安装目录移除。临时源夹具 SHA-256 未变化；该记录不代表当前 SigLIP 2 安装包。
- 打包 WebView UI 的导入、暂停/继续、组合筛选和重启续作点击流未能执行：桌面自动化 helper 返回 `EnumWindows failed: 0x80070003`，按规范重试和重置后仍失败。不得将这些 UI 步骤标记为通过；需人工或修复桌面自动化后补测。

## Lap-inspired 智能工作台 MVP（2026-08-09）

- Rust 新增覆盖：migration 11 重复初始化与重复资产/图库关系合并；共享缩略图只在无数据库引用时清理；收藏、重复查询覆盖子图库和虚拟归属；收藏与集合保持虚拟；完整 BLAKE3 重复分组；Unicode 编辑副本目标；计划后执行、拒绝覆盖；生成副本预览撤销；源夹具导出与回滚前后 BLAKE3 不变。
- React 新增覆盖：卡片收藏状态与星级更新相互独立；原有图库、选择、筛选、预览和侧栏交互回归保持通过。
- 手动入口 `scripts/start-desktop.cmd` 已改为 setup hook 创建窗口，并将每次会话的 WebView2 数据目录放在 `%TEMP%`；本机已验证前端构建、Rust desktop 编译和进程启动。当前桌面自动化会话无法枚举该终端创建的窗口，因此这只是启动 smoke，不等同于 G-UI 人工验收通过。
- 发布前必须补充桌面手工 smoke：收藏重启恢复、集合成员、SigLIP 2 文本/以图结果、重复审阅集合、2/4 图比较、编辑预览与另存确认、已有目标拒绝、人物 clear-all。
- 人脸检测/身份聚类不在当前自动化验收中，因为安装包没有经产品许可审核的模型；验收标准是明确 `model_unavailable`、默认关闭、无云端回退且 clear-all 可用。

### G-UI 桌面验收脚本（当前门槛）

以下脚本必须在桌面窗口中完成；每一步都要确认主界面的左侧来源、中心 Grid/单图预览、右侧信息栏仍保留，返回后查询、页码、排序、显式选择和当前焦点没有被意外重置：

1. 从图库 Grid 选中图片 → “找相似” → 打开比较 → 标记一张 → 返回图库；
2. 从顶部搜索或左侧集合进入结果 → “重复审阅”或“找相似” → 返回；
3. 保持当前查询并显式选择多张图片 → “整理预览” → 查看查询范围/显式选择范围 → 返回；
4. 在集合工具中切换“加入集合”的目标集合，确认主 Grid 的 query/page/sort 和显式选择不变化；
5. 在 Search/Similar/Duplicate/Collection 结果中点击图片，确认只改变右侧详情焦点，不覆盖当前显式选择。

G-UI 只有在以上脚本和当前窗口尺寸下的滚动/布局检查均通过后，才能从 `IMPLEMENTED_PENDING_MANUAL` 改为 `COMPLETED`，并恢复 N1。

## 缩略图优先导入与分析（2026-08-10）

- 首次导入不再把源图像素完整 decode 到应用内存；JPEG 有有效 EXIF 内嵌缩略图时优先使用它，否则 Windows 通过 WIC 直接生成 `grid-640-v1`，只返回不超过 `640×640` 的目标像素。完整 BLAKE3 指纹仍保留。WIC 不可用的 WebP 仅允许在源尺寸本身不超过目标尺寸时使用有界后端，大图明确失败。
- 已有当前缓存的基础特征重算只读取源 EXIF/尺寸并 decode 缩略图，`sourceDecodeUs=0`；基础特征和语义模型都不读取原图像素。
- 语义任务只调度当前、有效的 `grid-640-v1` 缓存路径；缺失缓存会在模型调用前失败，批处理失败后的单图重试也只使用该缓存，不回读原图。
- 多图网格少于 8 张/行时只对视口附近卡片请求应用私有 screen preview：2、4、6 张/行分别为 1280、960、768px，高清队列最多并发 2 个；8～12 张/行继续只使用 `grid-640-v1`。
- Places365、SigLIP 2、主体模型和缓存重算现在共用有界的分析缩略图解码器；超过 `640×640` 的缓存条目会被拒绝，不会被模型链路当作普通图片解码。
- 语义任务 worker 还会校验缓存路径位于应用私有 thumbnail 根目录且文件名匹配 `grid-640-v1`；恢复任务、批处理和失败重试均不能把 `absolute_path` 当作模型输入。
- 新导入图像处理的发现阶段只收集路径并先确定完整发现数量，处理阶段使用最多 2 个 worker 按 16 张批次连续处理；目录发现传输仍使用容量为 24 的滑动窗口，基础特征复用 OKLab 采样网格，处理结果按 16 张一次事务写入，并对单图像素解码设置 256MB/16,384px 安全限制；语义模型推理批次上限为 4；数据库写入、归属解析和进度持久化保持串行。
- 旧隔离 release 基准（2 张 4000×3000 JPEG）中的“源 decode 223 ms”仅是问题定位时的历史数据，不是当前验收标准；当前以 `docs/plans/0036-thumbnail-only-decode.md` 为准：bounded WIC 冷导入的 `sourceDecodeUs` 必须为 0，分析只能 decode 已缓存缩略图。
- 历史 release TinyCLIP 48 张仓库 PNG：批次 8 为 76.8 张/秒，批次 32 为 87.7 张/秒；TinyCLIP 已移除，这是历史模型吞吐 smoke，不代表当前 SigLIP 2 的性能或真实摄影分类质量。
- Rust 已覆盖嵌入 JPEG 预览、缓存重算、冷/暖扫描和语义缓存路径；真实图库仍需在用户环境中复测。
