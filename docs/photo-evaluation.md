# 真实摄影语义评估

## 安全与数据许可

摄影质量评估只读取操作者明确放入仓库根目录 `evaluation-data/` 的图片。该目录被 `.gitignore` 排除，不得提交、打包或上传。只可使用已取得评估授权的素材；仓库不附带、也不会自动下载任何摄影数据集。

评估报告默认写入同样被忽略的 `benchmark-output/photo-evaluation.json`，不会写入图片目录，并以 `create_new` 模式拒绝覆盖已有报告。工具不会修改 EXIF/XMP、移动、重命名或删除评估图片。

## 目录约定

每个顶层目录名使用稳定英文标签 ID。多标签样本用 `+` 连接标签，并只放一份图片：

```text
evaluation-data/
├─ portrait/
│  └─ portrait-001.jpg
├─ group/
│  └─ group-001.webp
├─ portrait+night/
│  └─ portrait-night-001.jpg
├─ product+still_life/
│  └─ product-001.png
└─ landscape+mountain+forest/
   └─ mountain-forest-001.jpg
```

支持的扩展名为 JPEG、PNG 和 WebP；可在标签目录内继续建立来源或场景子目录。顶层标签必须来自当前 semantic catalog，未知目录名会让评估失败，而不是静默跳过。根目录中的散落文件不会参与评估。

## 运行

正式质量评估使用 release 优化和真实 TinyCLIP CPU 后端：

```powershell
cargo run --release --manifest-path src-tauri/Cargo.toml --no-default-features --bin semantic-evaluate -- --data evaluation-data --output benchmark-output/photo-evaluation.json --backend cpu
```

也可运行 `npm.cmd run evaluate:photos` 使用同一默认路径。若需要保留多轮报告，应为每轮选择新的输出文件名；工具不会覆盖旧报告。

## 输出定义

JSON 报告包含：

- 每类标注图片数、预测数、true positive 数、precision 和 recall；
- Top-1 与 Top-3：按 21 类原始余弦相似度排序，任一真实标签命中即计为成功；
- 多标签 micro/macro precision 和 recall：使用达到当前阈值的标签集合，`unknown` 不参与多标签分母；
- `unknown` 比例：最终阈值结果回退为 unknown 的图片比例；
- 类别混淆：每个真实标签到原始相似度 Top-1 的计数；
- 每张图片的真实标签、阈值后标签、完整原始相似度、延迟和错误；
- 模型加载时间、推理总耗时、端到端耗时；
- Windows 上通过当前进程统计取得的峰值工作集、CPU 秒和平均单核占用百分比；不可取得时相应字段为 `null`。

所有比率使用 `0..1`，相似度是余弦相似度，不是概率或准确率置信度。推理失败计入样本总数，并作为 Top-1/Top-3 未命中；不会从分母中隐藏。

## 首轮评估重点

评测集应有意覆盖下列易混淆边界，并同时包含困难负样本：

- 单人肖像、合影和背景人物：`portrait` / `group`；
- 商业产品照、生活静物和摆拍：`product` / `still_life`；
- 广角风景、山地和林地：`landscape` / `mountain` / `forest`；
- 街头场景与建筑主体：`street` / `architecture`；
- 真正夜景、室内暗光和普通低亮度日景：`night` 与低亮度非夜景。

当前仓库没有授权摄影评测集，因此没有质量数字。已有 PNG 图标 benchmark 仍只用于证明模型加载、推理链路和工程速度。
