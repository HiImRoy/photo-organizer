# PhotoOrganizer

PhotoOrganizer 是一款 Windows 优先、本地优先的图片分类与整理桌面应用。它可以从系统目录选择器导入图库，递归索引 JPEG、PNG 和 WebP，在应用私有目录生成缩略图，计算亮度、对比度、饱和度和主色，并用随包分发的 TinyCLIP INT8 模型在本机完成多标签语义分类。深色三栏工作区支持网格、单图胶片栏、完整检查器和 SQLite 层组合筛选。

> 当前状态：语义分类与专业工作区里程碑已实现。语义结果来自真实本地 ONNX 推理；模型缺失或校验失败时会明确禁用语义功能，不生成伪造标签。复制整理、dry-run 与回滚不在本里程碑范围。

## 安全边界

- 扫描、元数据读取、缩略图和分析只读源图片。
- SQLite、日志和缩略图保存在应用数据目录，不写入图库。
- 模型随应用分发；图片、embedding 和标签不会上传。
- 当前版本不删除、移动、重命名、覆盖或写回原图元数据。
- 自动化文件系统测试只使用 `test-data/` 或测试临时目录。

## 开发环境

- Windows 10/11
- Node.js 22.13+
- Rust stable（MSVC toolchain）
- Microsoft C++ Build Tools 与 WebView2（Tauri 开发/打包需要）

这些仅是开发和构建要求；发布安装包的最终用户不需要安装 Node、Rust、Python、SQLite 或命令行工具。

## 开发命令

```powershell
npm.cmd install
npm.cmd run tauri dev
```

正式 Windows 验证和打包使用下列脚本；它会先检查 MSVC、Windows SDK、`link.exe` 与 `cl.exe`：

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/build-windows.ps1
```

常用验证：

```powershell
npm.cmd run format:check
npm.cmd run lint
npm.cmd run typecheck
npm.cmd test
npm.cmd run test:rust
npm.cmd run build
npm.cmd run tauri build -- --target x86_64-pc-windows-msvc
```

正式 Windows 构建使用 Rust MSVC target 和 Microsoft C++ Build Tools。仓库 CI 也以该受支持路径生成安装产物；本机若缺少 `link.exe`，请先补齐 C++ Build Tools，不要把 GNU 回退产物当作正式发布版本。

真实语义 CPU 基准入口：

```powershell
cargo run --manifest-path src-tauri/Cargo.toml --no-default-features --bin semantic-benchmark -- --images src-tauri/icons --model tinyclip --backend cpu --batch-size 1
```

授权真实摄影集的质量评估入口（`evaluation-data/` 与输出均不会进入 Git）：

```powershell
npm.cmd run evaluate:photos
```

## 数据位置

生产运行时通过 Tauri 的应用数据目录保存 `photo-organizer.sqlite3`、`thumbnails/` 和日志。开发/测试不会扫描任何预设的个人目录，只有用户在系统对话框中明确选择后才开始扫描。

## 文档

- [需求](docs/requirements.md)
- [架构](docs/architecture.md)
- [数据模型](docs/data-model.md)
- [路线图](docs/roadmap.md)
- [测试策略](docs/testing.md)
- [界面设计规范](docs/ui-guidelines.md)
- [发布与签名](docs/release.md)
- [模型评估](docs/model-evaluation.md)
- [真实摄影评估说明](docs/photo-evaluation.md)
- [起步执行计划](docs/plans/0001-bootstrap.md)
- [专业化界面重构计划](docs/plans/0002-professional-ui.md)
- [语义分类与专业工作区计划](docs/plans/0003-semantic-classification-and-workspace.md)
- [整理预览 Dry-run 计划](docs/plans/0004-organization-dry-run.md)

项目决策记录在 `docs/decisions/`。当前版本与限制以执行计划和发布说明为准。
