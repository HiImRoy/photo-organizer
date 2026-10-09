# PhotoOrganizer

PhotoOrganizer 是一个 Windows 优先、local-first 的桌面照片管理工具。它帮助个人摄影师从本地文件夹导入照片、使用缩略图完成分析、通过筛选和收藏夹整理照片，并在真正执行文件操作前生成可检查的整理预览。

> English summary: PhotoOrganizer is a local-first Tauri desktop photo organizer for photographers. It indexes user-selected folders, performs thumbnail-only analysis, provides virtual collections and filters, and creates safe dry-run organization plans without silently changing source files.

## 当前状态

项目处于 MVP / early-access 阶段，适合本地测试和代码审查，不建议直接用于唯一的生产照片归档。

目前已实现：

- 递归扫描 JPEG、PNG 和 WebP，并在应用数据目录缓存缩略图。
- 基于 SQLite 保存本地图库索引、分析结果和用户标记。
- 亮度、对比度、饱和度、影调、主色和强调色提取。
- SigLIP 2 Base INT8 本地语义分析，以及摄影题材、主体标签和环境属性筛选。
- Windows 上可选 DirectML 推理；Provider 自检或模型会话失败时自动回退 CPU，GPU 选项不会把“检测到显卡”误报成可用。DirectML 的有效批大小按专用显存分级（最高 32）；GPU 推理执行失败会一次性回退 CPU，只有 CPU 批次失败才递归降批。
- 分析任务详情会显示实际后端：\`DirectML GPU\` 表示分类器会话已绑定 DirectML，\`CPU\` 表示当前任务走 CPU。任务前后显存没有明显变化不能单独证明未使用 GPU；DirectML/ONNX Runtime 可能复用已分配的模型显存，验收应同时看任务后端、GPU 计算占用和失败回退提示。
- 物理本地来源与虚拟收藏夹分离；一张图片可以加入多个收藏夹。
- 网格、单图预览、信息检查器、直方图、分组、星级和颜色标记。
- 本地 AI 搜索、相似图片/重复审阅和基础整理预览。
- 整理操作默认复制，并在执行前检查目标路径、命名冲突和安全边界。

仍在规划或受模型/许可限制的能力：

- 人脸身份识别与身份聚类。
- 完整 RAW 专业冲印、视频管理、云同步和账号系统。
- 永久删除、覆盖原图和向原图写回元数据。

## 设计边界

PhotoOrganizer 的核心模型有两层：

1. **本地来源（Source）**：绑定真实磁盘目录，只负责索引原始文件和读取必要的文件/元数据。
2. **收藏夹（Collection）**：应用内的虚拟关系，不改变源目录结构；收藏夹支持父子层级，图片可以同时属于多个收藏夹。

导入、缩略图生成、基础特征提取、语义分析和模型推理都必须使用应用生成的缩略图或有界缩略图衍生物。完整原图像素不会进入这些处理链路；只有用户主动查看原图时，查看器才允许走独立的原图预览路径。

正常浏览和分析不会移动、重命名、删除或覆盖原始照片。整理功能先生成 dry-run 预览，复制是默认动作，真正执行需要用户明确确认。

## 隐私

- 图片、缩略图、embedding、标签和数据库默认只保存在本机。
- 项目没有云端分析服务，也不要求账号。
- 应用不会把照片上传到 PhotoOrganizer 服务。
- 不要把个人照片、真实本地路径、应用数据库、模型密钥或日志直接提交到公开仓库。
- 公开仓库中的模型和第三方依赖仍受各自许可证约束，详见 [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md)。

## 开发环境

- Windows 10/11
- Node.js 22.12+（`package.json` 声明的版本；手动启动器会对较低版本提示警告并继续尝试）
- Rust stable，MSVC toolchain
- Microsoft C++ Build Tools
- WebView2

最终用户使用打包安装程序时不需要安装 Node、Rust、Python、SQLite 或命令行工具。

## 快速启动

安装依赖：

```powershell
npm.cmd install
```

手动构建并启动桌面应用（推荐）：

1. 双击项目根目录的 `启动 PhotoOrganizer.cmd`。
2. 脚本会检查 Node.js、npm、Rust MSVC toolchain、Microsoft C++ Build Tools、WebView2、配置文件和前端依赖状态。
3. `package-lock.json` 与依赖 marker 不一致、marker 缺失或关键 CLI 缺失时，正常启动会运行 `npm install` 并刷新 marker。
   Node.js 22.12 可满足项目的构建要求；依赖同步时，npm 仍可能为开发依赖链中的
   `eslint-visitor-keys@5.0.1` 显示非阻断的 `EBADENGINE` 警告（其声明的 Node 范围不包含 22.12）。
   这是依赖自身的 engine 警告，与启动器检查的项目最低版本不同。依赖已同步时，日常启动不会运行
   `npm install`，也就不会重复显示该警告。该警告不会被隐藏。
4. 前端构建通过后，脚本会使用手动 Tauri 配置启动桌面窗口。该配置将 `beforeDevCommand` 设为 `null`，并通过 `--no-dev-server` 禁止启动开发服务器；默认 WebView2 兼容参数不包含 `--no-sandbox`。
5. 构建或桌面启动失败时，脚本会返回对应命令的非零退出码。

也可以从终端运行：

```powershell
npm.cmd run start:desktop
```

只检查环境，不构建和启动：

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts\manual-build-start.ps1 -CheckOnly
```

`-CheckOnly` 只读取并报告环境与依赖状态，不安装依赖、不写入 marker、不构建也不启动应用。缺少必要工具或依赖未同步时会返回非零退出码。Node.js 低于 `package.json` 声明的 22.12 时会显示项目 engine 警告，但不会仅因版本号阻止手动启动；实际构建失败时会报告构建命令的退出码。

开发环境默认使用 `%TEMP%\PhotoOrganizer-dev-data` 保存测试数据库、缩略图和日志，不会自动扫描个人照片目录。需要测试已有应用数据时，再显式设置 `PHOTO_ORGANIZER_DATA_DIR`。

### 模型启动与失败恢复

`启动 PhotoOrganizer.cmd` 负责检查环境、构建并启动桌面窗口；窗口创建后，应用再于后台准备随包模型，因此缺失或损坏的模型资源不会阻止图库打开。语义题材模型确实 ready 后才会恢复数据库中排队的语义任务。模型准备失败时，任务保持 queued，等待后续成功启动恢复，不会被假报为完成。

当前包只包含 SigLIP 2 Base。直接请求仓库未提供资源的 SigLIP 2 SO400M/14-384 profile 会返回明确错误，不会拿 Base 目录尝试装载 SO400M 权重。

语义分类运行期间若 DirectML 推理失败并回退到 CPU，语义状态和 GPU Provider 状态会同步反映回退结果，活动语义模型的数据库 backend 也会更新为 CPU；数据库同步失败会显示在语义状态中。后续同一会话的准备请求和下一次启动会沿用 CPU，避免不断重试已失败的 DirectML 路径。主体模型的 DirectML 回退则显示在主体状态及 GPU Provider 状态中。

## 质量检查

日常改动按[测试策略](docs/testing.md)选择与改动范围相符的检查。文档/注释-only 改动只检查变更文档格式和 `git diff --check`，不需要新增或运行代码测试。CI 与发布保留完整验证门槛。

CI/发布全量验证命令示例（不是每个提交的本地必跑清单）：

```powershell
npm.cmd run format:check
npm.cmd run lint
npm.cmd run typecheck
npm.cmd test -- --run
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --manifest-path src-tauri/Cargo.toml --all-targets --all-features
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
npm.cmd run build
npm.cmd run tauri build -- --target x86_64-pc-windows-msvc --bundles nsis
```

Windows 打包和环境检查（按需或发布时）：

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/build-windows.ps1
```

测试只能使用 `test-data/` 或测试临时目录，不能使用个人照片目录。涉及文件整理的测试必须保持 dry-run，不得覆盖或删除已有文件。

## 项目结构

```text
src/                 React + TypeScript 前端
src-tauri/src/       Tauri Rust 后端、SQLite、扫描和分析任务
src-tauri/migrations/数据库迁移
src-tauri/resources/ 模型和运行时资源
docs/                架构、决策、测试和阶段计划
scripts/             开发、检查和 Windows 构建脚本
```

## 文档

- [当前换机交接（2026-10-10）](docs/handoff/2026-10-10.md)
- [换机开发交接与待办](docs/HANDOFF.md)
- [架构](docs/architecture.md)
- [数据模型](docs/data-model.md)
- [当前功能](docs/current-functionality.md)
- [测试策略](docs/testing.md)
- [界面设计规范](docs/ui-guidelines.md)
- [发布与签名](docs/release.md)
- [模型评估](docs/model-evaluation.md)
- [统一来源与收藏夹方案](docs/plans/0053-unified-library-and-favorite-folders.md)
- [阶段实施路线图](docs/plans/0053-implementation-roadmap.md)
- [第三方依赖与模型说明](THIRD_PARTY_NOTICES.md)

## 贡献和代码审查

欢迎通过 Issue 或 Pull Request 提交问题和改进建议。提交前请阅读 [`CONTRIBUTING.md`](CONTRIBUTING.md)。公开仓库默认只提供代码读取权限；不需要给代码审查工具或普通协作者授予写入权限。

## 许可证

PhotoOrganizer 自有代码采用 [MIT License](LICENSE)。第三方依赖、模型权重、ONNX Runtime、WebView2 和其他随包资源不自动继承本项目许可证，必须遵守各自的许可证和再分发条款。
