# 0060 分类修复与验证计划

## 目标

审查并修复当前未提交的分类改动，保持 SigLIP 2 Base 的 v3 题材结果、无可靠 primary 时不虚构 `photo_abstract`、以及主体 DirectML 一次性 CPU 回退行为。同时保证坏缩略图只影响对应图片，CPU 批次失败可以继续逐图恢复，并用测试证明旧 v2 结果会按新版本重新排队。

## 已确认的现状与假设

- 当前 `std::fs::metadata(&model_path)` 已经是借用；本任务不再把它当作 move 编译问题修改。
- 现有未提交改动还包含 SigLIP 2 v3 primary 选择、Places365 不再生成摄影题材 primary、以及数据库不再把“无 primary”虚拟成 `photo_abstract`；这些改动必须保留。
- `load_analysis_thumbnail` 是分类唯一允许的像素输入边界。其错误应作为图片/缩略图输入错误处理，而不是模型或 Provider 故障。
- 测试只使用仓库 `test-data/` 或测试创建的临时目录；不读取或修改用户照片、用户数据库和原图。

## 实施步骤

1. 在语义错误边界区分无效缩略图输入与 ONNX/Provider 推理错误；让 Places365、SigLIP 2 和主体分类保留缩略图专用输入约束及可诊断路径。
2. 串行化主体模型的推理/后端切换临界区，避免并发调用同时触发 DirectML→CPU 重建；CPU 批次推理失败时保持 CPU fallback 状态并把错误交给任务层递归拆批，不能把整个模型永久标记为 `Failed`。只有 CPU 会话重建或完整性校验失败才进入模型错误状态。
3. CPU 回退重建前重新校验主体模型（及已安装的 YuNet）SHA-256 和会话合同；不改变模型资源或运行时资源。
4. 补充 `semantic_tasks.rs`、`subject.rs` 和 `db.rs` 行为测试：坏缩略图不会触发后端切换/模型永久失败，CPU 批次失败后可逐图成功，调用始终使用应用缩略图，且已有 SigLIP 2 v2 结果在 v3 作业中被选入候选。
5. 同步 `docs/current-functionality.md` 和 `docs/photo-evaluation.md` 中关于拒识、primary、版本和错误分母的描述；不扩展本次里程碑。

## 验证

- `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check`
- `cargo check --manifest-path src-tauri/Cargo.toml --no-default-features`
- `cargo test --manifest-path src-tauri/Cargo.toml --no-default-features`
- 检查最终 diff 和工作树，确认只写入本计划限定的 Rust/文档文件，现有前端改动保持原样且没有测试夹具被修改。

## 已执行结果（2026-09-14）

- `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check`：退出码 0，无输出。
- `cargo check --manifest-path src-tauri/Cargo.toml --no-default-features`：退出码 0；`Compiling photo-organizer v0.1.0`，`Finished dev profile [unoptimized + debuginfo]`。
- `cargo test --manifest-path src-tauri/Cargo.toml --no-default-features`：退出码 0；库测试 `124 passed, 0 failed`，`import-benchmark` 为 `0 passed`，`semantic-benchmark` 为 `1 passed`，`semantic-evaluate` 为 `3 passed`，Doc-tests 为 `0 passed`。
- `git diff --check`：退出码 0，无输出。
- `git status --short -- test-data`：退出码 0，无输出，测试夹具没有改动。
- 额外尝试 `cargo check --manifest-path src-tauri/Cargo.toml`（默认桌面 feature）时，提权审批在进程创建前超时，工具返回 `CreateProcess ... Rejected("The automatic permission approval review did not finish before its deadline")`；这不是 toolchain/sysroot 失败，未修改全局工具链。
- 随后使用同一 stable MSVC cargo 路径单独重试 `cargo check --manifest-path src-tauri/Cargo.toml`：退出码 0；`Checking photo-organizer v0.1.0`，`Finished dev profile [unoptimized + debuginfo] target(s) in 3.59s`。

## 残余风险

- 本地没有授权摄影评测集；本任务验证版本筛选、状态机和缩略图边界，不声称提升真实摄影准确率。
- DirectML 设备实际行为依赖 Windows Provider 和硬件；测试使用确定性替身覆盖切换协议，真实 GPU 运行仍需桌面验收。
