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
- Node.js 22.13+
- Rust stable，MSVC toolchain
- Microsoft C++ Build Tools
- WebView2

最终用户使用打包安装程序时不需要安装 Node、Rust、Python、SQLite 或命令行工具。

## 快速启动

安装依赖：

```powershell
npm.cmd install
```

启动隔离的桌面开发环境：

```powershell
npm.cmd run start:desktop
```

也可以双击项目根目录的 `启动 PhotoOrganizer.cmd`。开发环境默认使用 `%TEMP%\PhotoOrganizer-dev-data` 保存测试数据库、缩略图和日志，不会自动扫描个人照片目录。需要测试已有应用数据时，再显式设置 `PHOTO_ORGANIZER_DATA_DIR`。

## 质量检查

提交前运行：

```powershell
npm.cmd run format:check
npm.cmd run lint
npm.cmd run typecheck
npm.cmd test
npm.cmd run test:rust
npm.cmd run clippy
npm.cmd run build
```

Windows 打包和环境检查：

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
