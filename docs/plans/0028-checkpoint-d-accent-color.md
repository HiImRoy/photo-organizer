# Checkpoint D8 — 多强调色提取

状态：IMPLEMENTED_PENDING_EVALUATION（面积主色/强调色职责修订 v3）

## 目标

在现有 `grid-640-v1` 缩略图上提取一组可解释、可重复的图片强调色。图片可以同时拥有多个强调色；“主色”旧字段继续作为兼容字段，但不再是唯一的颜色结果。

## 方案决策

1. 使用确定性的加权 K-means 颜色量化。K-means 是成熟的颜色量化方法；本项目不引入 OpenCV 或新的运行时依赖，避免增加桌面包体积和模型处理开销。
2. 聚类距离使用 OKLab。OKLab 的三维欧氏距离更接近感知颜色差异，适合合并视觉相近的 RGB 桶。
3. 每个缩略图像素的权重由面积、局部颜色对比、色度和轻量构图中心先验组成。局部对比优先于中心先验，中心先验只用于同面积候选的稳定排序，不代替真实面积。
4. 通过相邻采样点的一致性和空间分散度计算空间连续性；低面积、低连续性的孤立噪点不能单独成为强调色。
5. 输出两个调色板：
   - `coveragePalette`：按面积覆盖排序，最多 5 个候选；
   - `prominentPalette`：按显著性、面积、局部对比、色度和空间连续性综合排序，最多 3 个候选。
6. 颜色候选的 RGB、面积占比、显著性占比、局部对比、色度和空间连续性只属于 Imaging Auto detail。分类 Registry 只消费候选产生的稳定颜色类别列表，人工颜色覆盖逻辑不变。

## 兼容性与数据流

- 继续写入现有 `color_features.dominant_colors_json`，新值改为 `ColorPalette` 对象；旧数组格式不会导致读取失败，并由旧的标量主色字段继续提供兼容分类/色块结果；新调色板字段在重新分析后可用。
- `dominant_color_rgb` 和 `dominant_color_category` 取面积主色候选，供旧查询、排序和组织变量继续使用；视觉强调色不再污染兼容主色字段。
- Asset DTO 新增 `colorPalette`；分类自动值从 `coveragePalette` 中达到主色覆盖率阈值的类别列表生成，`prominentPalette` 仅用于强调色展示，手工覆盖仍由 Checkpoint B 的 Effective Resolver 决定。
- 将颜色算法版本写入调色板对象，并提升基础分析版本，使旧缓存/结果在重新扫描时失效重建。
- 所有像素计算只使用应用私有缩略图，不读取原图像素。
- `accent-oklab-v3` 延续低饱和色相保留规则，并将 `coveragePalette` 作为面积主色来源；纯灰仍归为 `neutral`，灰蓝、灰绿和低饱和暖色按色相保留为彩色主色。达到明显占比的彩色区域可以优先于暗色剪影，小面积高对比主体只进入 `prominentPalette`，并通过 `basic-color-v6` 触发旧分析结果重算。

## 验收用例

- 单色图：返回一个稳定候选。
- 红蓝等分图：返回两个不同类别，面积排序稳定。
- 大面积背景 + 小面积高对比主体：主体可以进入 `prominentPalette`，但孤立单点不进入。
- 大面积暗色剪影 + 达到明显占比的彩色区域：彩色区域作为面积主色；小于主色占比阈值的彩色主体不改变主色筛选。
- 近似颜色渐变：感知距离足够近时合并，不重复占用候选名额。
- 中性图：不把黑白灰吞掉有彩色结果；无有彩色时返回中性色候选。
- 同一缩略图重复分析结果完全一致。
- 中文、空格、Unicode 源路径的源文件内容不改变。

## 参考

- [W3C CSS Color 4 — OKLab](https://www.w3.org/TR/css-color-4/)
- [Color Thief](https://github.com/lokesh/color-thief)
- [OpenCV — K-Means color quantization](https://docs.opencv.org/3.0-last-rst/doc/py_tutorials/py_ml/py_kmeans/py_kmeans_opencv.html)
- [AndroidX Palette — prominent and target swatches](https://developer.android.com/reference/androidx/palette/graphics/Palette)
- [Vibrant — MMCQ quantization, filters and generators](https://github.com/akigami/vibrant)
- [CIE — CIEDE2000 perceptual colour difference](https://www.cie.co.at/publications/colorimetry-part-6-ciede2000-colour-difference-formula-1)
