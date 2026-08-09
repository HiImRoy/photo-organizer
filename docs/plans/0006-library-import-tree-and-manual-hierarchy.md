# Library import tree and manual hierarchy

状态：已实现，待人工验证（I1-I4 完成；I5 自动验证完成）

## 目标

扩展图库导入与 Library Tree，使用户可以在导入时选择是否把源目录中的子文件夹导入为独立 Library，并可以在 PhotoOrganizer 内拖动 Library 调整从属关系。

所有层级操作只修改 PhotoOrganizer 数据库中的 `parentLibraryId`，不得移动、重命名或修改磁盘文件夹。

## 拟定产品规则

- 被选中的根目录始终建立一个独立 Library。
- 导入对话框增加“导入子文件夹结构”选项，默认关闭，避免现有导入行为突然创建大量节点。
- 开启后：包含受支持图片的子文件夹各自建立 Library；空文件夹不建立节点；磁盘嵌套关系只用于首次导入时建立初始层级。
- 关闭后：只建立根 Library；子文件夹中的图片仍可由根 Library 递归扫描，但不暴露为 Sidebar 节点。
- 导入后可以拖动 Library 到另一个 Library，或拖到根图库区域成为根节点。
- 拖动只更新 `parentLibraryId`，禁止移动源目录、重命名源目录或修改任何 Asset 的真实路径。
- 数据库/domain 层拒绝自父、自身和任意后代形成的循环关系。
- 手动调整后的层级在重新扫描、重启和再次导入其他图库时保持，不再被 SourcePath 自动重算覆盖。
- 首次导入时仍可根据 SourcePath 为新节点建立初始父级；这只是默认值，之后的人工调整优先。

## 实施阶段

### I1 — Import options and source discovery

- 在导入流程中增加选项状态和确认对话框。
- IPC 接收 `includeSubfolders`。
- 对开启结构导入的根目录发现含受支持图片的目录集合；不把空目录暴露为 Library。
- 保持现有递归扫描、缓存、fingerprint、missing detection 和源文件只读约束。

### I2 — Library hierarchy persistence

- 为 Library parent relation 增加“source-derived / manual”来源标记，或等价的持久化语义。
- 增加设置 parent 的 domain/IPC API。
- 事务内检查 self-parent、descendant-parent 和不存在的 parent。
- 更新删除 Library 后的子节点 reparent 规则及测试。

### I3 — Scan orchestration

- 结构导入时按 Library SourceRoot 的 ownership scope 扫描，不重复处理父图库和子图库的文件。
- 对外仍表现为一个可取消的导入任务，进度、失败和完成状态保持一致。
- 不允许父图库扫描隐式触发用户未选择的其他 Library 重扫。

### I4 — Sidebar drag and context actions

- Library 节点支持拖动、拖放目标高亮和根级放置区域。
- 右键菜单增加“移出当前父图库”；手动调整失败时保持原层级。
- 菜单外点击关闭，成功扫描提示自动收起，失败/缺失扫描仍保留提示。

### I5 — Verification

- 覆盖关闭/开启子文件夹结构导入、Parent/Child/Grandchild、手动拖动、根级放置、循环拒绝、重启保持、重扫保持和删除后 reparent。
- 验证源目录哈希、文件内容、Asset identity、fingerprint 和 thumbnail 未被层级操作改变。
- 运行前端格式/Lint/类型/测试、Rustfmt/Rust tests/Clippy 和 Release build。

## 已确认并实施的边界

“导入子文件夹结构”按“每个直接包含受支持图片的子文件夹成为独立
Library”实现；关闭时仍递归扫描，但不创建子图库节点。层级拖放只更新
`parentLibraryId`，并由数据库事务拒绝循环关系。父扫描按已导入 SourceRoot
的物理嵌套范围优先剪枝，子图库不会被父图库隐式重扫。
