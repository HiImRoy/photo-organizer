# MVP：模型收敛与百张以上导入吞吐

## 目标

收敛 MVP 的本地模型资源，并修复导入数量接近一页时出现的明显变慢问题。

## 当前问题

1. 题材识别同时保留 TinyCLIP、SigLIP 2 和 MobileCLIP 三套候选模型，但产品只需要一套稳定的默认题材模型；多余模型增加包体、启动检查和界面选择复杂度。
2. 扫描器在每个文件上重复查询图库归属、已有资产和缓存状态，并为每个结果单独建立 SQLite 事务。
3. 扫描进行时前端周期性重新读取整页资产；资产列表还会逐张读取语义标签和人工覆盖，随着图片数量增长会与缩略图请求叠加。

## 方案

- 保留 Places365-ResNet18（环境证据）、SigLIP 2 Base（摄影题材候选）、PicoDet-S + YuNet（主体标签）。移除 TinyCLIP 与 MobileCLIP 的随包资源、选择入口和运行时装载入口。
- 扫描时缓存本次任务的图库归属根，避免每个文件重复访问 SQLite；将已存在资产的判断和已缓存资产的更新时间合并为批量查询/事务。
- 资产列表使用批量语义标签、分类覆盖和 revision 查询，避免每张图片的 N+1 查询。
- 扫描进行中只更新进度状态，不重复拉取整页资产；扫描完成、取消或失败时再做一次完整刷新。缩略图请求继续使用全局缓存和浏览器 lazy loading。

## 验收

- 顶部题材模型入口只显示 SigLIP 2 Base；启动和装载模型不再检查 TinyCLIP/MobileCLIP 资源。
- `import-benchmark` 使用 100 张以上 fixture 完成冷导入、缓存复用和热扫描；归属解析不再按每张图片打开连接，资产列表的派生字段也不再逐张准备；相关测试覆盖列表结果与扫描结果。
- 导入期间不会每 750ms 重复拉取整页图片；结束后图库数量、缩略图和分类状态正确刷新。
- `cargo test --manifest-path src-tauri/Cargo.toml --no-default-features`、`npm run build` 和相关前端测试通过。

## 本轮验证记录

- 100 张、640×480 fixture：冷导入约 2.53 s，缓存复用约 2.30 s，热扫描约 0.78 s；基准只写入 `benchmark-output/`，未访问个人图片目录。
- 扫描性能中 100 张的归属解析累计约 1 ms；列表端语义标签、人工覆盖和 revision 已改为批量查询。
- 前端 Vitest 43/43、Rust tests 79/79、Clippy `-D warnings`、Rustfmt、TypeScript 和 production build 均通过。

## 非目标

- 不在本任务中改变分类 taxonomy、模型阈值或原图读写策略。
- 不触碰用户的个人照片目录；基准只使用 `benchmark-output/` 下的 fixture。
