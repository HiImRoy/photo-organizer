# 测试策略

## 安全规则

所有自动化文件系统测试仅可使用仓库 `test-data/` 的只读输入或由测试框架创建并在测试结束回收的临时目录。测试不得枚举用户主目录、图片库或任意系统图库。源夹具在测试前后计算哈希，任何变化都失败。

## 分层

- Rust 单元测试：格式识别、路径规范化、特征计算、迁移、repository、组合 SQL 筛选、缓存键、语义 catalog/主标签、任务控制与 unavailable fallback。
- Rust 集成测试：复制夹具到临时源目录，验证扫描/增量/缺失/损坏/Unicode/缓存失效和源哈希不变。
- React 测试：Vitest + Testing Library，mock Tauri client，覆盖空状态、导入、进度、取消、网格、错误、排序和三栏详情。
- UI 可访问性回归：覆盖图片卡片 `aria-pressed` 选中态、进度条数值、错误提示和可见键盘焦点。
- 构建验证：TypeScript、ESLint、Prettier、Rustfmt、Clippy、Vite production build、Cargo test、Tauri bundle。
- 手工 smoke：只选择 `test-data/manual-library/`，验证系统对话框、增量展示、重启恢复和安装包启动。

## 手动启动开发窗口

在仓库根目录双击 `启动 PhotoOrganizer.cmd`，或执行 `npm.cmd start`，保持该终端窗口开启即可手动测试桌面应用。启动入口会先构建前端，再通过 Tauri 静态资源协议启动，不依赖 Vite 本地端口；手动验收窗口使用独立的 `PhotoOrganizer Manual` 标题，避免与已安装旧版本混淆；默认将开发数据保存到 `%TEMP%\PhotoOrganizer-dev-data`，并为每次会话生成独立的临时 WebView2 profile，因此不会占用正式应用的数据库和 WebView2 缓存。手动入口默认启用 WebView2 软件/进程内 GPU 与开发沙箱兜底，避免部分 Windows 开发环境的 GPU/沙箱子进程导致黑屏；这些参数只作用于手动开发入口，不代表正式发布安全配置；如需测试默认 WebView2 路径，可在启动前设置 `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS`。开发数据库目录会跨次启动保留。只有在需要检查已有应用数据时，才通过 `PHOTO_ORGANIZER_DATA_DIR` 显式指定目录。

## 用例矩阵

扫描覆盖空/单层/嵌套目录、支持/不支持和大小写扩展名、重复/新增/修改/缺失、损坏/不可读文件，以及中文、俄文、空格、Emoji 和其他 Unicode。不可读场景在 Windows ACL 无法稳定构造时用 reader seam 注入错误；父图库重新扫描时覆盖已登记子图库的回归也由 Rust 测试验证。

数据库覆盖首次/重复初始化、迁移顺序、唯一约束、upsert 和重启恢复；组合筛选还必须验证色相范围的严格程度会按 8%～75% 单调提高最低命中色相占比，并保持色相环跨红色边界的行为。缩略图覆盖正常、损坏、方向、命中、失效、Unicode 路径和不写源目录；高分辨率 fixture 必须证明导入提取后的像素尺寸受限且 `source_decode_us=0`。分析覆盖黑、白、灰、高低饱和、透明、超小图和所有输出范围。

## 命令与通过标准

```powershell
npm.cmd run format:check
npm.cmd run lint
npm.cmd run typecheck
npm.cmd test -- --run
npm.cmd run test:rust
npm.cmd run clippy
npm.cmd run build
```

任何失败必须修复或在执行计划与最终报告中记录真实原因。未运行的检查不能描述为通过。

## 浏览器视觉回归

视觉验收使用 Vite 开发环境中隔离的确定性 fixture，不连接 SQLite、不调用目录选择器、不扫描个人图库：

```text
http://localhost:1420/?visual-fixture=library
http://localhost:1420/?visual-fixture=scanning
http://localhost:1420/?visual-fixture=error
```

每次结构或样式调整至少检查 1920×1080、1440×900、1366×768 和 960×720，并检查可调宽度侧栏、组合筛选、分组、单图/胶片栏、扫描/语义任务、错误、模型不可用、图片选中、详情与焦点。正式基准图保存到 `docs/screenshots/semantic-*.png`。production build 后确认确定性夹具模块只存在于开发动态分支。

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
- Places365、SigLIP 2、主体模型和缓存重算现在共用有界的分析缩略图解码器；超过 `640×640` 的缓存条目会被拒绝，不会被模型链路当作普通图片解码。
- 语义任务 worker 还会校验缓存路径位于应用私有 thumbnail 根目录且文件名匹配 `grid-640-v1`；恢复任务、批处理和失败重试均不能把 `absolute_path` 当作模型输入。
- 新导入图像处理的发现阶段只收集路径并先确定完整发现数量，处理阶段使用最多 2 个 worker 按 16 张批次连续处理；目录发现传输仍使用容量为 24 的滑动窗口，基础特征复用 OKLab 采样网格，处理结果按 16 张一次事务写入，并对单图像素解码设置 256MB/16,384px 安全限制；语义模型推理批次上限为 4；数据库写入、归属解析和进度持久化保持串行。
- 旧隔离 release 基准（2 张 4000×3000 JPEG）中的“源 decode 223 ms”仅是问题定位时的历史数据，不是当前验收标准；当前以 `docs/plans/0036-thumbnail-only-decode.md` 为准：bounded WIC 冷导入的 `sourceDecodeUs` 必须为 0，分析只能 decode 已缓存缩略图。
- 历史 release TinyCLIP 48 张仓库 PNG：批次 8 为 76.8 张/秒，批次 32 为 87.7 张/秒；TinyCLIP 已移除，这是历史模型吞吐 smoke，不代表当前 SigLIP 2 的性能或真实摄影分类质量。
- Rust 已覆盖嵌入 JPEG 预览、缓存重算、冷/暖扫描和语义缓存路径；真实图库仍需在用户环境中复测。
