# Places365 ResNet-18 模型资源

- 模型：Places365 ResNet-18，365 个场景叶子类别
- 参考项目：[CSAILVision/places365](https://github.com/CSAILVision/places365)
- ONNX 发布资源：[ailia-models Places365](https://github.com/axinc-ai/ailia-models/tree/master/landmark_classification/places365)
- 权重下载地址：<https://storage.googleapis.com/ailia-models/places365/resnet18_places365.onnx>
- ONNX opset：11
- 许可证：`PLACES365-LICENSE.txt` 中的 MIT License
- 输入：`NCHW [batch, 3, 224, 224]`，ImageNet 均值/标准差
- 输出：每张图片 365 个场景 logits

## 文件完整性

| 文件 | SHA-256 |
| --- | --- |
| `resnet18_places365.onnx` | `3c3cd0d42693e2957fcaa0bc365ce78e169a2e1162356742adfbd11077e8f7bf` |
| `categories_places365.txt` | `6cc3f1f8eae85b7016dc634e2d333cdcce5fd16cfada4afd87977fff5f8b12ba` |
| `IO_places365.txt` | `d7e6abfeb228d789720326e630bedd231a7eaedcae8fd13d6d9dcd8eca95f59e` |
| `PLACES365-LICENSE.txt` | `0443593167099f156685221339c6e876cccd02ae5f2bec3e588c5231a14c1062` |

英文叶子 ID 只用于模型对齐和内部证据，不直接在图库界面展示；界面使用 `src-tauri/src/places365.rs` 中的中文场景簇名称。
