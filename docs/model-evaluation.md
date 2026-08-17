# 语义模型评估

## 当前选择

当前 MVP 语义运行时由随包的 `Places365-ResNet18` 和 `SigLIP2-Base-Patch16-224` 组成：Places365 提供环境与可观察场景证据，SigLIP 2 提供摄影题材候选证据、文本编码和 768 维检索向量。所有模型都使用 ONNX Runtime CPU provider；模型、图片、embedding、标签和诊断均在本地处理，不上传。

| 候选                           | 代码/权重许可        | 体积与能力                                    | Windows 分发判断                          |
| ------------------------------ | -------------------- | --------------------------------------------- | ----------------------------------------- |
| Places365 ResNet-18            | MIT                  | 45.6 MB；365 类场景叶子证据                   | 已选择；聚合为环境证据                    |
| SigLIP 2 Base Patch16/224 INT8 | Apache-2.0           | 约 360.5 MB；零样本图文匹配、768 维 embedding | 已选择为默认题材模型；模型与 runtime 随包 |
| SqueezeNet ONNX                | Apache-2.0           | 约 5 MB；ImageNet 单标签                      | 类别覆盖不足                              |
| OpenAI CLIP ViT-B/32           | MIT / MIT            | 超过 300 MB；零样本能力成熟                   | 包体、内存和 CPU 成本过高                 |
| MobileCLIP                     | Apple 仓库与模型许可 | 小型图文模型                                  | 需额外导出、许可归档和 Windows 验证       |

## 分类协议

当前摄影题材 taxonomy 为 `photo-organizer-photography-topics-v3`：`photo_portrait`、`photo_landscape`、`photo_street`、`photo_architecture`、`photo_still_life`、`photo_food`、`photo_wildlife`、`photo_macro`、`photo_activity`、`photo_vehicle`、`photo_document`、`photo_abstract`。`photo_documentary` 已从摄影师筛选 taxonomy 移除；没有通过当前题材阈值的结果统一归入 `photo_abstract`（界面显示“抽象艺术”），不再展示“未知”。`indoor`/`outdoor` 是独立环境证据；主体标签收敛为 `single_person`、`multiple_people`、`animal`、`vehicle`、`food`、`plant`。旧的 `person`、`group`、`portrait`、`pet` 和 `unknown` 读取时会归并到当前标签，不再作为新自动标签。

SigLIP 2 使用每类多条版本化英文提示词的匹配 logits，经 sigmoid 后用独立阈值与候选间隔做拒识；Places365 365 类叶子概率按映射聚合，使用最低分数与类间隔门槛；主体模型在任务层只通过明确映射补充人像、动物、车辆、食品和植物题材。模型输出分数仅用于当前模型内排序和阈值评测，不能未经标注集校准直接解释为准确率。

## 资源完整性

默认 SigLIP 2 资源：`model_int8.onnx` 约 360.5 MB，SHA-256
`bfe28fe2ccdb685874586648035ea349593e487ce33bd0939b28813681a8f167`；
`tokenizer.json` SHA-256
`cb9140fae3ac5122c972d37adf83e1248471a38147ad76f8215c8872c6fd8322`。
完整来源、许可证和 tokenizer 资源见
`src-tauri/resources/models/siglip2-base-patch16-224/MODEL-SOURCE.md`。

- `model-int8.onnx`：24,281,512 bytes；SHA-256 `10921310ddef06557ec1598d1260470a0a4db53f70ffe0deb60b946dcad6d27a`
- `tokenizer.json`：SHA-256 `6d9109cc838977f3ca94a379eec36aecc7c807e1785cd729660ca2fc0171fb35`
- `onnxruntime.dll`：14,131,232 bytes；SHA-256 `8a1aad8d59d02a5337d4e3f5bbd1158c3f7bf84fe3b3f0052f957dd3e75a91cb`

运行时在加载前验证模型、tokenizer 与 ONNX Runtime DLL；失败时返回 `model_unavailable`，不生成标签。来源和许可证归档在 `src-tauri/resources/models/` 与 `src-tauri/resources/runtime/`。

## 2026-08-07 CPU 基准

受测输入为仓库 `src-tauri/icons/` 中 48 个 PNG 测试资源，不包含个人图片；release 优化、batch size 1，实际后端为 CPU。最终实测：失败 0，平均单图 23.83 ms，P50 23.11 ms，P95 30.70 ms，吞吐 41.97 张/秒。报告中的真实样例包括 `128x128.png` → 产品 0.346 / 抽象 0.344 / 截图 0.335 / 文档 0.295，以及 `32x32.png` → 抽象 0.369 / 产品 0.350 / 截图 0.350 / 动物 0.315。

完整 JSON：`docs/benchmarks/tinyclip-cpu-2026-08-07.json`（历史 TinyCLIP 基准，不代表当前默认模型）。

该数据只描述本机单次工程基准，不代表其他设备，也不构成分类质量或准确率声明。峰值内存和授权真实摄影评测集上的逐类 precision/recall 仍未完成。

## 基准入口

```powershell
cargo run --manifest-path src-tauri/Cargo.toml --no-default-features --bin semantic-benchmark -- --images src-tauri/icons --model places365 --backend cpu --batch-size 1
```

报告记录模型名称/版本/哈希、实际 backend、样本/失败数、平均/P50/P95、吞吐和最多八个样例预测及原始候选证据。模型不可用时明确返回不可用，不伪造性能。

## 当前质量评测实现

`semantic-evaluate` 现在默认加载 SigLIP 2，按 `--batch-size` 批量读取评测目录，记录每张图片的完整题材候选分数，并可用 `--calibrate` 运行逐类阈值候选和“标签阈值 + 拒识 margin”联合扫描。校准目标默认 precision 0.85、每类正负样本各至少 8 张；样本不足只生成未完成状态，不会修改 `src-tauri/src/topics.rs`。报告 schema 为 2，并记录当前模型对应的分数语义。

当前仓库没有授权真实摄影评测集，因此尚未把任何候选阈值写回运行时。需要将获得许可的摄影样本按 `docs/photo-evaluation.md` 的标签目录放入本地 `evaluation-data/`，再运行：

```powershell
npm.cmd run evaluate:photos
```

## 后续质量门槛

- 采用有授权且不含个人图库的目标领域夹具，逐类报告 precision、recall、coverage 和混淆，并由人工复核阈值候选。
- 校准截图/文档、多人/人像、产品/静物、街拍/建筑等易混类别的提示词与阈值。
- 增加峰值内存、冷启动、多轮稳态和扫描并发下的 CPU 占用。
- DirectML/GPU/NPU 只有在实际 provider 初始化、正确性和安装包 smoke 全部通过后才可显示。

## 授权摄影集评估工具

`semantic-evaluate` 使用被 Git 排除的 `evaluation-data/` 目录读取按稳定标签 ID 组织的摄影图片，输出逐类数量、Top-1/Top-3、多标签 micro/macro precision/recall、unknown 比例、混淆、每张图片的完整原始相似度、加载/推理/端到端耗时，以及 Windows 可取得的峰值工作集和 CPU 统计。详细目录约定、命令和指标定义见 `docs/photo-evaluation.md`。

仓库当前没有取得授权的真实摄影评测集，因此只交付评估工具和测试，不生成或声称任何摄影分类准确率。
