# Places365 ResNet-18 模型资源

- 模型：Places365 ResNet-18，365 个场景叶子类别
- 参考项目：[CSAILVision/places365](https://github.com/CSAILVision/places365)
- ONNX 发布资源：[ailia-models Places365](https://github.com/axinc-ai/ailia-models/tree/master/landmark_classification/places365)
- 权重下载地址：<https://storage.googleapis.com/ailia-models/places365/resnet18_places365.onnx>
- ONNX opset：11
- 许可证：`PLACES365-LICENSE.txt` 中的 MIT License
- 输入：`NCHW [batch, 3, 224, 224]`，ImageNet 均值/标准差
- 输出：每张图片 365 个场景 logits
- 类别清单的规范来源：[`categories_places365.txt`](https://github.com/CSAILVision/places365/blob/8a953ed56438726dc98bdef3796d042e7f1f171e/categories_places365.txt)，固定到 commit `8a953ed56438726dc98bdef3796d042e7f1f171e`；校验按 Git blob 原始 LF 字节执行。

## 文件完整性

| 文件                       | SHA-256                                                            |
| -------------------------- | ------------------------------------------------------------------ |
| `resnet18_places365.onnx`  | `3c3cd0d42693e2957fcaa0bc365ce78e169a2e1162356742adfbd11077e8f7bf` |
| `categories_places365.txt` | `2affba635eb657e7ca95f4e6cc69bd9fac29ef4c32aeb83cafdfcd06ec6a1ea6` |
| `IO_places365.txt`         | `0a65ceb7264413a15bb069a87be7ecf9ebb572a6803d48de2827a1d25fe287ba` |
| `PLACES365-LICENSE.txt`    | `0443593167099f156685221339c6e876cccd02ae5f2bec3e588c5231a14c1062` |

英文叶子 ID 只用于模型对齐和内部证据，不直接在图库界面展示；界面使用 `src-tauri/src/places365.rs` 中的中文场景簇名称。

类别清单的规范 LF 文件为 6,833 字节、364 行 LF、无 CRLF、无 BOM。旧 SHA-256 `6cc3f1f8…` 对应相同文本转换为 CRLF 后的字节，不是当前资源内容的规范哈希；当前 Git blob 和工作树文件均使用上述 LF 哈希。

`IO_places365.txt` 是保留的项目派生文件，不等同于上游原文。它以固定上游 commit `8a953ed56438726dc98bdef3796d042e7f1f171e` 的 [IO_places365.txt](https://github.com/CSAILVision/places365/blob/8a953ed56438726dc98bdef3796d042e7f1f171e/IO_places365.txt) 为基础；唯一已知内容差异是项目文件写作 `/b/bus_station/indoor 2`，上游写作 `/b/bus_station/indoor 1`。修改缘由未记录，因此这里不据此判断模型准确性。项目文件为 6,214 字节、365 行 LF、无 CRLF、无 BOM，当前 Git blob 为 `56047f7252b3ed9bf4582435d923ad4d6bacf1e9`，其规范原字节 SHA-256 为表中 `0a65ceb7…`；上游原文 SHA-256 为 `49f8c7fbeeb70deb055c040a1807a95b234bbac92902e1b6edfffbdd8411e1f1`。
