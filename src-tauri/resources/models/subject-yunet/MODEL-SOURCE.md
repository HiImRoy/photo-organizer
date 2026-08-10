# YuNet 人脸辅助模型

- 模型：YuNet `face_detection_yunet_2023mar.onnx`
- 来源：[OpenCV Zoo](https://github.com/opencv/opencv_zoo/tree/main/models/face_detection_yunet)
- 下载地址：https://github.com/opencv/opencv_zoo/raw/main/models/face_detection_yunet/face_detection_yunet_2023mar.onnx
- 许可证：MIT（OpenCV Zoo 模型目录）
- SHA-256：`8f2383e4dd3cfbb4553ea8718107fc0423210dc964f9f4280604804ed2552fa4`
- 输入约定：缩放到 640×640 的 BGR 缩略图
- 输出约定：12 个 `cls/obj/bbox/kps` 多尺度张量，由 PhotoOrganizer 按 YuNet/OpenCV 规则解码

PhotoOrganizer 只使用人脸数量和置信度辅助判断“人像”，不保存人脸框、脸部裁剪、向量或身份信息。
