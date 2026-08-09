# M5：核心图库交互、高清预览与图库管理

## 目标

在暂停 Organization Dry-run 的前提下，修复日常浏览中活动图片、批量选择、单图预览之间的状态耦合，建立独立的高清预览资源层，并提供安全的“从 PhotoOrganizer 移除图库”操作。本计划不实现任何真实文件复制、移动、重命名或删除。

## 实施范围

1. 明确 `currentLibraryId`、`activeAssetId`、`selectedAssetIds`、`viewMode`、`previewAssetId`、筛选/排序和预览变换状态；普通单击只更新活动图片，复选框/Ctrl/Shift 才更新批量集合。
2. 修复胶片栏与左右键导航，使其严格使用当前筛选和排序结果，切换时同步活动图片、高清预览和检查器，并保留批量选择。
3. 增加应用私有 screen preview/original 读取 IPC。网格/胶片栏继续使用缩略图，适应窗口使用约 2560px screen preview，100% 使用原始资源；旧请求通过 generation token 忽略，缓存不写入源目录。
4. 实现 Lightroom 风格 Navigator：右侧信息面板显示尽量放大的当前图片全图和视口框，主画布只显示图片；支持点击/拖动导航，Navigator 只显示当前倍率，缩放以主画布中心为基准，滚轮采用连续小步进，双击控制，支持 Fit、6.25%～1100%（11:1）、受限拖动和 Esc 返回网格。
5. 为图库提供重新扫描、在资源管理器中打开、图库信息和“从资料库移除”菜单；移除通过 `library_id` 事务清理数据库与应用缓存，不接触源目录。
6. 收敛顶部操作为常驻浏览动作和选择上下文动作；保留“整理预览”入口但不宣称已执行真实整理。

## 安全与性能规则

- 所有源文件只读；移除图库不调用源目录删除 API。
- screen preview 和原图读取仅通过受控的 asset id IPC，临时数据不长期存入 React 状态；screen 缓存位于应用私有目录。
- 切换图片只保留当前高清资源，胶片栏继续使用缩略图，避免批量解码原图。
- 缺失源文件和高清读取错误明确反馈，同时保留缩略图和重试入口。

## 验证

补充状态、胶片栏、预览资源、缩放/拖动、异步请求竞态、图库移除与原图哈希测试；执行 Prettier、ESLint、TypeScript、Vitest、Rustfmt、Rust tests、Clippy 和 production build。若修改资源协议或打包资源，则生成新的 Windows 测试安装包。

## 状态

已完成实现：独立活动/批量/预览状态、筛选排序范围内的胶片栏导航、screen/original 预览 IPC 与应用私有缓存、位于右侧信息面板的 Lightroom 风格 Navigator、Fit/6.25%～1100% 多档缩放/指针中心滚轮/拖动约束、图库菜单和只清理索引与应用缓存的移除事务。一级语义分类、辅助标签、主色分析和相对路径目录树沿用 M4 的稳定实现，没有恢复旧的二十多个一级标签。

后续修正：单图预览不再沿用网格分页；当前筛选结果会在单图模式一次加载到胶片栏，隐藏图库“上一页/下一页”控件。单图工作区锁定纵向溢出，图片画布上的滚轮只执行以主画布中心为基准的连续小步进缩放。

验证记录：前端 Prettier、ESLint、TypeScript、Vitest（9 项）和 production build 通过；Rustfmt、Rust tests（26+3 项）和 Clippy `-D warnings` 通过。MSVC release 可执行文件已生成，并输出了当前版本的 portable x64 测试压缩包；NSIS/MSI 封装在受限终端访问 Tauri 工具下载地址时失败（Windows socket `os error 10013`），没有把旧安装包当作本轮产物。桌面人工 smoke 受当前无独立测试图库/自动化桌面会话限制，需用 portable 包或在有网络权限的构建机补充点击验收。
