# 开发交接（2026-09-05）

## 接续入口

仓库：https://github.com/HiImRoy/photo-organizer

当前开发分支：`codex/ui-redesign-baseline`。本次是工作进度备份，不表示所有功能已完成人工验收。请在此分支继续，不要误从 main 或 master 开始，也不要覆盖现有变更。

先阅读 `AGENTS.md`、本文件、`docs/ui-guidelines.md`，再按任务阅读对应计划。代码开发和琐碎执行任务使用 `gpt-5.6-luna` + `max` 子 agent；主 agent 负责规划、审查、整合和验收。

## 新 Windows 电脑启动

安装 Git（含 Git LFS）、Node.js 22.13+、Rust stable MSVC、Visual Studio C++ Build Tools（含 Windows SDK）、WebView2。

```powershell
git lfs install
git clone --branch codex/ui-redesign-baseline https://github.com/HiImRoy/photo-organizer.git
cd photo-organizer
git lfs pull
npm.cmd ci
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/verify-release-resources.ps1
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/manual-build-start.ps1 -CheckOnly
```

然后双击根目录 `启动 PhotoOrganizer.cmd`。首次 Rust 编译需要时间和网络，构建缓存会重新生成。SigLIP 2 权重由 Git LFS 管理，仅克隆代码但未下载 LFS 文件会导致模型校验失败。`-CheckOnly` 会检查并可能同步依赖，但不能替代完整桌面启动和 GPU 验收。

默认开发数据在 `%TEMP%\PhotoOrganizer-dev-data`，也可通过 `PHOTO_ORGANIZER_DATA_DIR` 指定。数据库、照片、缩略图和本地设置不随 Git 推送。建议新电脑用隔离测试库；如需迁移已有数据，先关闭旧应用，单独备份完整数据目录，注意照片绝对路径可能不同。不要将私人数据提交到仓库。

启动脚本默认包含 WebView2 GPU 兼容参数；这是界面渲染路径，与 DirectML 分析开关独立。`-UseDefaultWebView` 可用于对照测试，不代表已完成所有设备上的预览加速。

## 已实现的工作快照

- 0053：统一 Source / Collection 查询和导航，Phase 4 第一轮收藏夹管理已实现，包括层级管理、多目标加入、移出和虚拟移动；默认收藏为系统叶节点。2026-08-27 修复来源与收藏夹同时高亮、顶部名称和数量错配，清除普通筛选保留当前浏览根。
- 0056–0058：自适应网格预览质量、设置分栏和 UI/操作优化；包含预览切换、后台任务紧凑入口、标记及筛选布局的多轮修复。需在新机器复核桌面效果。
- 0059–0061：GPU 检测、设置、DirectML 会话及 CPU 回退、实际后端展示已实现。
- 0062：S2.1/S2.2 已实现，按专用显存分级初始化 batch（GPU 最高 32、CPU 最高 8），失败同后端递归降批。旧记录的 3 张缩略图 CPU/GPU 小样本测试不能代表长图库性能。
- 0063：九个题材（人像、风光、街拍、建筑、静物特写、动物、植物、交通工具、抽象艺术），六个主体（单人、多人、动物、植物、食物、风景）；带旧标签读取兼容，待人工验收准确性。

详细进度以 `docs/plans/0053-implementation-roadmap.md`、`0061-gpu-performance-s1-directml.md`、`0062-gpu-performance-s2-calibration.md`、`0063-photography-label-taxonomy-v4.md` 为依据。旧计划中的初始背景和未勾选检查项不能自动视为最新状态；也不能把“代码完成”当作人工验收通过。

## 下一步顺序

1. 新电脑通过资源校验、前端检查、Rust 测试与桌面启动，再执行下方人工清单。
2. 完成 0053 剩余：后端按完整 AssetQuery 批量操作（覆盖未加载结果）、收藏夹拖放/排序、兼容路径清理和迁移回归。不得重写 `assets.library_id` 来实现收藏。
3. 继续 S2.3：长图库稳定性、真实显存预算/峰值、模型级吞吐及回退诊断。保持缩略图输入，使用隔离测试数据。
4. 0054 整理/导出单独推进；不要顺带改写单来源 OrganizationPlan。跨来源收藏整理边界需单独设计和测试。

## 待人工验收

- 本地来源、默认收藏、普通收藏切换只能高亮一个；标题、计数和结果一致。
- 爱心与默认收藏数量即时同步，重启保留；普通收藏加入/移动仅改变虚拟关系。
- 大网格清晰度、快速切图时信息和右侧缩略图一致、Ctrl 选择和标记行为。
- 浅色/深色、窄窗口下后台任务进度填充、弹层和工具栏不遮挡。
- 九题材/六主体显示和旧结果兼容；分类准确性需人工审片。
- 独显启用、无独显置灰、初始化失败回退、实际后端显示及长批降级；分别验证 CPU 和 DirectML。

## 清理与验证边界

2026-08-27 已清理主项目和 lap 的 Rust target、lap 前端依赖、旧发布/评测输出及 Git 明确报告的临时垃圾对象。源码、模型资源和 Git 历史保留。按最后目录表加总约 1.53 GiB（此前口头报告 2.2GB 是未更新的中间值）；之后构建会增长。`lap/` 是被忽略的参考克隆，不是运行依赖，不随仓库交接。

旧记录报告过前端 90 项测试通过；这不是本次交接的重测结论。本次校验结果见下面记录。完整 Rust 重编译、长图库/GPU 实测和新电脑人工验收仍需执行。

### 本次交接校验

- 2026-09-05：前端 build（含 TypeScript）、lint、13 个文件/90 项测试全部通过。
- 手动启动脚本 `-CheckOnly` 和模型/运行库哈希校验通过；当前 Node 22.12.0 低于项目建议 22.13.0，新电脑按建议安装。
- 未重跑完整 Rust 构建、clippy 或 GPU 长图库测试；未进行本次桌面人工验收。此前清理的 Rust 缓存需在新机器重建。
- `git diff --check` 通过。此提交用于接续开发，不应视为发布验收。
