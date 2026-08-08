# 语义模型评估

## 当前选择

默认模型是 `TinyCLIP-ViT-8M-16-Text-3M-YFCC15M` 完整 INT8 ONNX conversion，许可证 MIT，随应用分发。推理使用 ONNX Runtime 1.24.1 CPU provider；模型和 runtime 都在本地加载，图片、embedding、标签和诊断不会上传。

| 候选                              | 代码/权重许可        | 体积与能力                                | Windows 分发判断                          |
| --------------------------------- | -------------------- | ----------------------------------------- | ----------------------------------------- |
| TinyCLIP ViT-8M/16 + Text-3M INT8 | MIT / MIT            | 24.3 MB；零样本图文匹配、512 维 embedding | 已选择；CPU 实测可用，模型与 runtime 随包 |
| SqueezeNet ONNX                   | Apache-2.0           | 约 5 MB；ImageNet 单标签                  | 类别覆盖不足                              |
| OpenAI CLIP ViT-B/32              | MIT / MIT            | 超过 300 MB；零样本能力成熟               | 包体、内存和 CPU 成本过高                 |
| MobileCLIP                        | Apple 仓库与模型许可 | 小型图文模型                              | 需额外导出、许可归档和 Windows 验证       |

## 分类协议

内部定义仍保留 21 个稳定 ID，以便历史记录和评估数据可读：`portrait`、`group`、`landscape`、`architecture`、`indoor`、`street`、`vehicle`、`product`、`still_life`、`food`、`animal`、`screenshot`、`document`、`night`、`flower`、`mountain`、`water`、`forest`、`sunset`、`abstract`、`unknown`。当前对用户开放的 catalog 收敛为 15 个 ID：主类别为 `portrait`、`landscape`、`architecture`、`product`、`animal`、`document`、`unknown`；辅助标签为 `group`、`indoor`、`street`、`vehicle`、`food`、`night`、`flower`、`abstract`。

每类使用版本化英文提示词和阈值（第一版通常为 0.16，文档为 0.17）。达到阈值并处于最高分 0.055 窗口内的活动标签按相似度排序，最多保留四个；只要有非未知标签就排除 `unknown`。当前 `photo-organizer-semantic-v2` 将主类别候选中的最高相似度类别写入 `is_primary=1`。`still_life`、`screenshot`、`mountain`、`water`、`forest`、`sunset` 仅作为历史 ID 保留，不再出现在当前 catalog、自动预测或相似度榜单中；已有记录仍通过 ID 映射显示中文名。相似度是 embedding 的余弦相似度，不是概率或准确率。

## 资源完整性

- `model-int8.onnx`：24,281,512 bytes；SHA-256 `10921310ddef06557ec1598d1260470a0a4db53f70ffe0deb60b946dcad6d27a`
- `tokenizer.json`：SHA-256 `6d9109cc838977f3ca94a379eec36aecc7c807e1785cd729660ca2fc0171fb35`
- `onnxruntime.dll`：14,131,232 bytes；SHA-256 `8a1aad8d59d02a5337d4e3f5bbd1158c3f7bf84fe3b3f0052f957dd3e75a91cb`

运行时在加载前验证模型、tokenizer 与 ONNX Runtime DLL；失败时返回 `model_unavailable`，不生成标签。来源和许可证归档在 `src-tauri/resources/models/` 与 `src-tauri/resources/runtime/`。

## 2026-08-07 CPU 基准

受测输入为仓库 `src-tauri/icons/` 中 48 个 PNG 测试资源，不包含个人图片；release 优化、batch size 1，实际后端为 CPU。最终实测：失败 0，平均单图 23.83 ms，P50 23.11 ms，P95 30.70 ms，吞吐 41.97 张/秒。报告中的真实样例包括 `128x128.png` → 产品 0.346 / 抽象 0.344 / 截图 0.335 / 文档 0.295，以及 `32x32.png` → 抽象 0.369 / 产品 0.350 / 截图 0.350 / 动物 0.315。

完整 JSON：`docs/benchmarks/tinyclip-cpu-2026-08-07.json`。

该数据只描述本机单次工程基准，不代表其他设备，也不构成分类质量或准确率声明。峰值内存和授权真实摄影评测集上的逐类 precision/recall 仍未完成。

## 基准入口

```powershell
cargo run --manifest-path src-tauri/Cargo.toml --no-default-features --bin semantic-benchmark -- --images src-tauri/icons --model tinyclip --backend cpu --batch-size 1
```

报告记录模型名称/版本/哈希、实际 backend、样本/失败数、平均/P50/P95、吞吐和最多八个样例预测。模型不可用时明确返回不可用，不伪造性能。

## 后续质量门槛

- 采用有授权且不含个人图库的目标领域夹具，逐类报告 precision、recall、coverage 和混淆。
- 校准截图/文档、多人/人像、产品/静物等易混类别的提示词与阈值。
- 增加峰值内存、冷启动、多轮稳态和扫描并发下的 CPU 占用。
- DirectML/GPU/NPU 只有在实际 provider 初始化、正确性和安装包 smoke 全部通过后才可显示。

## 授权摄影集评估工具

`semantic-evaluate` 使用被 Git 排除的 `evaluation-data/` 目录读取按稳定标签 ID 组织的摄影图片，输出逐类数量、Top-1/Top-3、多标签 micro/macro precision/recall、unknown 比例、混淆、每张图片的完整原始相似度、加载/推理/端到端耗时，以及 Windows 可取得的峰值工作集和 CPU 统计。详细目录约定、命令和指标定义见 `docs/photo-evaluation.md`。

仓库当前没有取得授权的真实摄影评测集，因此只交付评估工具和测试，不生成或声称任何摄影分类准确率。
