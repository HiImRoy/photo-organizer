# 图片跨图库拖动实施计划

## 目标

允许用户把图片从当前图库拖到任意其他 Library（包括父图库、子图库和无磁盘父子关系的其他图库），只改变 PhotoOrganizer 的虚拟归属，不修改源文件。

## 已确认的语义

- `assets.library_id` 继续表示图片实际文件的 Most Specific Imported Library owner。
- 新增单值手动归属映射；一张图片最多有一个手动目标 Library。
- 图片查询、图库数量和语义分组使用有效归属：手动归属优先，否则使用真实 owner；父图库继续按现有递归 Library scope 查询。
- 扫描、重新扫描、fingerprint、relativePath、thumbnail、EXIF 和原图路径不因拖动改变；重扫保留手动归属。
- 将图片拖回其真实 owner Library 会清除手动映射，恢复自动归属。
- 移除作为手动目标的 Library 时，映射随目标 Library 删除，图片回到真实 owner；移除真实 owner 时仍走现有 ownership reconciliation。
- 不移动、复制、重命名或修改任何磁盘文件。

## 实施范围

1. SQLite migration、Repository assignment API 和 Tauri command。
2. 所有 Library browse/count/semantic/organization scope 查询统一使用有效归属。
3. AssetCard 使用应用内 pointer drag，Sidebar Library 行作为目标并高亮；当拖动已选图片时，以当前选中集合整体移动。
4. 选择规则对齐资源管理器：普通选择框点击切换单张，Ctrl/Command 切换单张，Shift 从选择锚点直接选择连续范围；选择框和图片卡片都支持 Shift 范围选择。
5. 添加 Rust 数据库测试、前端拖动测试、多选拖动测试、Shift 范围选择测试和源文件不变回归测试。

## 非目标

- 不把图片拖动解释为文件系统移动。
- 不改变 Library hierarchy 或 Parent Library 的递归范围产品规则。
- 不复制图片到目标 SourceRoot。
