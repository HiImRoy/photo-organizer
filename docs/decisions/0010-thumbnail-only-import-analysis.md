# ADR 0010：导入与分析只使用应用缩略图

## 状态

已接受（2026-08-17）

## 背景

导入大图库时，传统 `image` 解码器会先把 JPEG/PNG/WebP 的完整源图像素解码到内存，再缩放到 `640×640`。这会把本来只需要缩略图的操作变成高峰内存、CPU 和 I/O 操作，也是用户在数千张图片导入时从几十张开始卡顿甚至闪退的主要风险。

## 决策

- 导入、基础特征、摄影题材/环境/主体分析和模型推理只允许读取应用私有缩略图或由缩略图生成的固定尺寸输入。
- Windows 首次导入使用 WIC 的 bounded source transform/scaler，直接请求不超过 `640×640` 的 RGBA 输出；应用代码不接收完整源图像素缓冲区。
- JPEG EXIF 中的有效内嵌预览继续优先使用，因为它本身已经是缩略图。
- 原文件只允许用于文件元数据、BLAKE3 指纹、EXIF/内嵌预览元数据，以及受控的“从源提取目标尺寸缩略图”读取。受控读取不得把完整源图像素交给应用分析链路。
- 用户主动打开高质量预览仍是独立的查看行为；screen tier 必须走有界尺寸缓存。original tier 是显式查看器例外，不参与导入、分析或模型推理。
- 非 Windows 平台没有可用的 bounded decoder 时，不回退到完整源图 decode，而是报告缩略图提取不可用。

## 备选方案

1. 继续使用 `image` crate 完整解码再 resize：实现简单，但违反缩略图内存边界，在大图库和高分辨率照片上风险不可接受。
2. 只使用 EXIF 内嵌缩略图：最安全，但大量 PNG/WebP/JPEG 没有内嵌预览，会导致导入缺失。
3. 引入 Python/OpenCV sidecar：跨平台能力强，但增加进程、打包、启动和故障边界；当前 MVP 不引入。
4. Windows WIC bounded decode：复用系统编解码器，能够在目标尺寸输出，适合 Windows-first 产品；代价是增加 Windows native 依赖，并需要对 WIC 编解码器不支持的格式给出明确失败。

## 影响与迁移

- 增加 `windows` crate 的 Windows-only 依赖和 `wic_thumbnail` 模块。
- `ImageProcessingTimings.source_decode_us` 在新导入路径应为 0；WIC 提取时间记录在 `thumbnail_decode_us`。
- 旧版本已生成的 `grid-640-v1` 缩略图可继续复用；首次重建或缓存失效时按新路径生成。
- 不改变原图库、EXIF 或用户文件；失败图片保留可重试的导入错误。
