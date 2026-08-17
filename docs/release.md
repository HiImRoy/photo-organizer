# Windows 发布

## 目标产物

Tauri 配置生成 NSIS `.exe` 与 WiX `.msi`（环境允许时）。安装包包含 Web 前端、Rust core、SQLite、Places365、SigLIP 2 Base、PicoDet/YuNet 模型、tokenizer、许可证以及 ONNX Runtime 1.24.1 CPU DLL；不包含 Python、ExifTool、CUDA 或云端服务。

## 构建

```powershell
npm.cmd ci
npm.cmd run validate
npm.cmd run tauri build -- --target x86_64-pc-windows-msvc
```

CI 在 Windows runner 执行相同检查并上传 bundle artifact。版本由 `package.json`、`src-tauri/Cargo.toml` 和 `tauri.conf.json` 同步维护。

## 安装即用清单

- 不要求 Node、npm、Rust、Python、pip、Git、CMake、OpenCV、ONNX、ExifTool、SQLite、CUDA、环境变量、端口或 API Key。
- 数据库和缓存首次启动自动创建。
- 语义资源随包安装并在启动时做 SHA-256 校验；资源缺失或损坏不影响扫描、缩略图和基础分析。
- 应用不写入所选源图库。

## 签名

起步构建可不签名，但不能作为正式公开发布。正式发布需要受信任的 Windows 代码签名证书（优先硬件/云 HSM 保护的 EV 或适合分发渠道的证书），在受控 CI 中签署应用可执行文件和安装包、使用 RFC 3161 时间戳并验证签名。密钥不得进入仓库或普通 CI 日志。

## 许可与供应链

发布前生成依赖清单/SBOM，核对 `THIRD_PARTY_NOTICES.md` 与 lockfile。Places365、SigLIP 2、PicoDet/YuNet 权重分别归档来源和许可；ONNX Runtime DLL 记录来源、版本、哈希、许可和更新流程。历史 TinyCLIP/MobileCLIP 不属于当前安装包。模型权重不能只继承应用仓库许可结论。

## 验证矩阵

至少覆盖当前受支持 Windows 10/11 x64：无开发工具的干净 VM、离线启动、Unicode 路径、重复扫描、升级保留数据库、卸载不删除用户图库、Defender/SmartScreen 结果和安装包哈希。若未实际执行某项，发布报告必须明确标注。

## CI 与失败处理

`.github/workflows/windows-build.yml` 执行验证与 bundle。安装配置使用 Tauri 的 `offlineInstaller` 模式随安装器携带 WebView2 离线安装程序，以包体增大换取无网络安装能力。若本机缺少 MSVC、WebView2、WiX/NSIS 下载能力或签名证书，只能报告配置完成和具体错误，不得声称安装包通过。

普通 push 和 pull request 只运行 `windows-latest` MSVC 验证：资源校验、npm ci、Prettier、ESLint、TypeScript、Vitest、Rustfmt、Rust tests、Clippy 和前端 production build。只有手动 `workflow_dispatch` 或 `v*` tag 在验证成功后进入 bundle job：NSIS 是必需产物，MSI 是允许失败但保留日志的可选产物；成功安装包复制到独立 artifact，并同时上传 `SHA256SUMS.txt`。每条命令通过 `scripts/invoke-ci-command.ps1` 写入日志，验证和打包 job 都在 `always()` 步骤上传日志。

`scripts/verify-release-resources.ps1` 在本地和 CI 中验证 Places365、SigLIP 2、PicoDet/YuNet 以及 ONNX Runtime DLL 的固定 SHA-256，并要求模型配置、许可、第三方声明和来源文件存在。正式模型与 runtime 是离线功能所需资源，应提交；历史 TinyCLIP/MobileCLIP 不应重新加入资源目录；临时模型下载、缓存和 `.part`/`.download` 文件必须被忽略。

## 2026-08-06 本机验收记录

- 官方 MSVC/NSIS 命令成功下载 Rust 依赖并完成前端构建，但在 Rust 链接阶段因本机没有 `link.exe` 退出；C++ Build Tools 安装尝试此前以 1602 退出。
- 临时 GNU/LLVM 回退环境成功生成 NSIS：`src-tauri/target/x86_64-pc-windows-gnu/release/bundle/nsis/PhotoOrganizer_0.1.0_x64-setup.exe`。
- 产物大小 214,539,667 bytes（204.60 MiB），SHA-256 为 `8EC45E5C07EDCEF9B8C1A2E75B0E4B7E05BCDD3D6D58A5307D8AB0D10EA48D86`，签名状态为 `NotSigned`。
- 当前用户静默安装退出码 0；安装后主程序持续运行且 `Responding=True`；自带卸载器退出码 0，并移除安装目录。
- 该回退产物只证明应用/NSIS/离线 WebView2 打包链路可运行，不是正式发布候选。尚未验证 MSI、签名、干净 VM、升级、杀软/SmartScreen；正式产物必须由 MSVC CI 重新生成并复测。

## 2026-08-07 语义工作区里程碑构建记录

- 实际执行 `npm.cmd run tauri build`；TypeScript 与 Vite production build 成功，Rust MSVC target 随后因 `linker link.exe not found` 失败。
- 本机未安装 Visual Studio/Build Tools 的“使用 C++ 的桌面开发”组件，故本次未生成新的 NSIS/MSI，也无法执行当时包含 TinyCLIP 模型、ONNX Runtime DLL 与新工作区的打包 WebView2 smoke；该记录对应历史模型包。
- `src-tauri/target/x86_64-pc-windows-gnu/release/bundle/nsis/PhotoOrganizer_0.1.0_x64-setup.exe` 的时间戳为 2026-08-06，是 M0 的旧 GNU/LLVM 产物，不包含本次变更；发布闭环审计已将其删除。
- 解除阻塞后需在 MSVC 环境重新运行完整 validate 和 Tauri build，再核对 resource 打包、离线首次启动、真实 CPU 分类、源图哈希、卸载、签名与安装包 SHA-256。

## 2026-08-07 发布闭环环境诊断

本机实际检测结果：

- Visual Studio Installer 与 `vswhere.exe`：未安装；
- Desktop development with C++ workload：未安装；
- `Microsoft.VisualStudio.Component.VC.Tools.x86.x64`：未安装；
- Windows 10/11 SDK 注册表和 Lib/Include：未发现；
- `link.exe`、`cl.exe`、`msbuild.exe`：PATH 和标准 Visual Studio 目录均未发现；
- Rust 默认 host：`x86_64-pc-windows-msvc`，Rust 1.97.1；已安装 MSVC 和 gnullvm targets，但 Rust target 本身不包含微软 linker/SDK。

因此不存在可通过 Developer PowerShell 或 `vcvars` 重新加载的已安装环境。`npm.cmd run test:rust`、`npm.cmd run clippy` 和 `npm.cmd run tauri build` 的正式 MSVC 路径均真实失败于 `linker link.exe not found`；Tauri 的前端 production 阶段通过，但安装包数量为 0，安装/启动/分类/重启续作/卸载 smoke 均未执行。

解除本机阻塞需要使用 Visual Studio Installer 安装 **Desktop development with C++** workload（`Microsoft.VisualStudio.Workload.VCTools`），至少包含 MSVC x64/x86 tools（`Microsoft.VisualStudio.Component.VC.Tools.x86.x64`）和当前受支持的 Windows 10/11 SDK。安装需要系统权限和交互式安装器，本任务未代替用户执行。完成后从 x64 Developer PowerShell 运行：

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/diagnose-msvc.ps1
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/build-windows.ps1
```

微软官方组件目录与命令行环境说明：<https://learn.microsoft.com/en-us/visualstudio/install/workload-component-id-vs-build-tools?view=vs-2022>、<https://learn.microsoft.com/en-us/cpp/build/building-on-the-command-line?view=msvc-170>。

## 2026-08-07 MSVC 正式打包与安装验收（当前）

- 环境已确认：Visual Studio Build Tools 17.14.37 位于 `D:\Develop\buildtools\product`，`Microsoft.VisualStudio.Workload.VCTools`、`Microsoft.VisualStudio.Component.VC.Tools.x86.x64`、Windows SDK `10.0.26100.0`、`link.exe`、`cl.exe` 和 MSBuild 均存在。普通终端最初未加载开发环境；本次通过 x64 `VsDevCmd.bat` 导入，并将 `%USERPROFILE%\.cargo\bin` 加入当前进程 PATH。Rust 为 `stable-x86_64-pc-windows-msvc`，rustc/cargo `1.97.1`。
- `npm.cmd ci`、Prettier、ESLint、TypeScript、Vitest 6/6、Rustfmt、MSVC Cargo tests（21 个）、Clippy `-D warnings` 和 Vite production build 均通过。npm 报告 Node `22.12.0` 低于项目声明的 `>=22.13.0`，但本次命令实际完成；发布 CI 仍固定使用 Node 22，建议升级本机 Node 后再做同版本复核。
- Tauri 构建使用显式 `--target x86_64-pc-windows-msvc`；此前仅依赖 `CARGO_BUILD_TARGET` 会导致 bundler 错找 `target\\release`，已同步修正 `scripts/build-windows.ps1` 与 CI。NSIS、WiX 下载在普通沙箱网络下返回 WinSock 10013，获准访问官方构建工具后完成。
- 当前 MSVC 产物（均未签名）：
  - NSIS：`src-tauri/target/x86_64-pc-windows-msvc/release/bundle/nsis/PhotoOrganizer_0.1.0_x64-setup.exe`，236,452,475 bytes，SHA-256 `728550F36A1A2CC5680F54A59231C1EE4C31239E884F116B4326DCCE6881194D`。
  - MSI 简体中文：`src-tauri/target/x86_64-pc-windows-msvc/release/bundle/msi/PhotoOrganizer_0.1.0_x64_zh-CN.msi`，238,862,336 bytes，SHA-256 `CF2EBC45300A6B59CEB79AFF8E6F402DF235E3CD10F045F1AB54B90D03092161`。
  - MSI 英文：`src-tauri/target/x86_64-pc-windows-msvc/release/bundle/msi/PhotoOrganizer_0.1.0_x64_en-US.msi`，238,862,336 bytes，SHA-256 `951CE5769C66865D93F1815677E6C5C78819C24A12C9CD03BD766DA8463D4B77`。
- 历史安装验收：NSIS 静默安装退出码 0，安装目录为 `%LOCALAPPDATA%\\PhotoOrganizer`；当时的 TinyCLIP ONNX、tokenizer、ONNX Runtime DLL、许可证和来源文件均存在。该记录只证明旧模型包的安装链路，不代表当前 SigLIP 2 包的验收结果。启动后应用数据目录和 SQLite 可打开，关闭并重启后主进程 `Responding=True` 且数据库保留（本机已有旧测试数据库，未删除用户数据）。
- 安装后的 WebView UI 点击验收未完成：桌面自动化 helper 在枚举窗口时两次返回 `EnumWindows failed: 0x80070003`，重置后仍失败；因此本次不能声称已在打包 WebView 中完成“导入、暂停/继续、组合筛选、关闭重启续作”的 UI 操作。相关 IPC/Rust 任务控制测试和 Vite 视觉 fixture 已通过，需在可用桌面自动化或人工桌面上补做一次。
