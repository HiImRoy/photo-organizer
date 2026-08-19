# 贡献指南

感谢参与 PhotoOrganizer。项目仍处于 MVP 阶段，优先接受能改善本地照片安全性、缩略图性能、可验证性和摄影工作流的改动。

## 开始之前

- 使用 Windows 10/11、Node.js 22.13+ 和 Rust MSVC toolchain。
- 先阅读 [`AGENTS.md`](AGENTS.md)、[`README.md`](README.md) 和相关 `docs/plans/`。
- 复杂功能先添加或更新执行计划和决策记录。

## 必须遵守的边界

- 不要使用个人照片、真实图库路径或私有数据库作为测试数据。
- 导入、decode、特征提取和模型推理只能使用应用拥有的缩略图或有界缩略图。
- 正常浏览和分析不得修改原始文件。
- 文件操作必须先生成预览，复制优先，不得静默覆盖或删除。
- 不要提交模型下载缓存、应用数据库、日志、缩略图、密钥或 `.env` 文件。
- 前端相邻按钮必须使用统一的控件几何规范，避免引入方形按钮和不一致的高度。

## 提交前检查

```powershell
npm.cmd run format:check
npm.cmd run lint
npm.cmd run typecheck
npm.cmd test
npm.cmd run test:rust
npm.cmd run clippy
npm.cmd run build
```

Pull Request 请说明：改动目的、影响范围、测试命令、是否涉及数据库迁移，以及是否改变了原图安全边界。
