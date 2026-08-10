# PicoDet 主体检测模型

- 模型：PicoDet-S 320，COCO 80 类，LCNet 主干，已包含后处理的 ONNX 导出
- 来源：[PaddleDetection](https://github.com/PaddlePaddle/PaddleDetection)
- 下载地址：https://paddledet.bj.bcebos.com/deploy/third_engine/picodet_s_320_lcnet_postprocessed.onnx
- 许可证：Apache-2.0（PaddleDetection 项目）
- SHA-256：`09fc88131be8ad224f13739a5cf8fc838600d76a77539af7f0400fa90506c5f3`
- 输入约定：缩放到 320×320 的 RGB 缩略图，并额外传入 `[1, 1]` 的 `scale_factor`
- 输出约定：后处理检测行 `[class_id, score, x1, y1, x2, y2]`

PhotoOrganizer 仅将 COCO 检测结果聚合为中文主体标签，不保存检测框、原图副本或身份特征。
