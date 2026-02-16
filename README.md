# OpenClaw Installer (Windows, Tauri)

Windows GUI installer/maintainer for OpenClaw.

- Stack: `Tauri + Rust + React + TypeScript`
- Default language: Chinese (switchable to English)
- UI only collects/displays; all install/maintenance logic is in Rust execution modules.

## 中文

### 你现在能做什么
- 一次安装向导：环境检查 -> 依赖安装 -> OpenClaw 安装 -> 配置 -> 启动 -> 健康检查
- 向导是“勾选 + 下一步”流程，支持功能勾选（skills/session memory/workspace memory/Feishu 大陆直连）和 Kimi 2.5 国内/海外区分
- 安装完成可自动打开管理页 URL
- 安装完成后默认禁止重复安装（需先在维护中心“删除 OpenClaw”后才能重新安装）
- 维护中心支持：启动/停止/重启、备份/回滚、升级、换模型链、日志导出、安全检查
- 关闭窗口不会退出：会隐藏到 Windows 系统托盘（右下角），从托盘菜单或维护中心显式停止/退出
- 提供桌面快捷方式（可一键启动）

### 关键路径
- Installer 日志：`%APPDATA%\OpenClawInstaller\logs\*.log`
- OpenClaw 默认目录：`%USERPROFILE%\.openclaw`
- 备份目录：`%APPDATA%\OpenClawInstaller\backups`
- 微信捐赠收款码（编译进程序）：`src-tauri\assets\donate-wechat.jpg`
  - 说明：该文件会通过 Rust `include_bytes!` 嵌入到安装器二进制中，发布后用户无法通过替换前端静态资源来更改
  - 更新方式：替换该 JPG 后重新执行 `npm run tauri:build`

### WSL2 隔离测试（不影响 Windows 现有 OpenClaw）
```powershell
# 只做隔离环境准备（不安装、不启动）
.\scripts\start-isolated-test.ps1 -PrepareOnly

# 进入隔离 shell（会自动设置 OPENCLAW_HOME / WORKSPACE / LOG_DIR）
.\scripts\enter-isolated-shell.ps1

# 在当前终端启动隔离网关（前台运行）
.\scripts\start-isolated-test.ps1
```

默认隔离参数：
- Distro: `Ubuntu`
- HOME: `$HOME/openclaw-isolated/home`
- Port: `28789`

可自定义示例：
```powershell
.\scripts\start-isolated-test.ps1 -Distro Ubuntu -Port 28789 -BaseDir '$HOME/openclaw-isolated'
```

### 一键启动（推荐）
1. 双击 `Create-Desktop-Shortcut.cmd`（只需一次）
2. 桌面会生成：
   - `OpenClaw Installer.lnk`
   - `OpenClaw Installer - Preflight.lnk`
3. 双击 `OpenClaw Installer.lnk` 运行 GUI
4. 启动脚本使用 `%~dp0` 相对路径解析，项目放在任意目录都可运行

### 开发运行
```powershell
npm install
npm run tauri:dev
```

### 上传 GitHub 前（强烈建议）
1. 运行敏感信息扫描（只输出文件名，不会把疑似密钥打印出来）：
```powershell
.\scripts\scan-secrets.ps1
```
2. 如果你打算“打包整个文件夹上传/发给别人”（而不是用 `git push`），先清理本地生成物：
   - 双击 `Cleanup-For-GitHub.cmd`
   - 该脚本会删除 `node_modules/`、`dist/`、`src-tauri/target*`、`.smoke*`、`%TEMP%/`、`.tmp-*.log` 等本地生成物
   - 不会触碰你 Windows 的 `%USERPROFILE%\.openclaw`

### 生产打包（EXE + MSI）
```powershell
npm run tauri:build
```

生成物（默认目录）：
- `src-tauri\target\release\openclaw-installer.exe`
- `src-tauri\target\release\bundle\nsis\OpenClaw Installer_0.1.0_x64-setup.exe`
- `src-tauri\target\release\bundle\msi\OpenClaw Installer_0.1.0_x64_zh-CN.msi`

### 设置 App 图标（Windows）
1. 把你想要的图标保存为项目根目录的 `app-icon.png`（建议 1024x1024、透明背景、四周留白）
2. 二选一执行：
   - 双击 `Update-App-Icon.cmd`
   - 或运行：`npm run icons`
3. 重新打包：`npm run tauri:build`

如果默认 release 可执行文件被占用（Windows 正在运行旧进程），可用备用目标目录构建：
```powershell
$env:CARGO_TARGET_DIR="$PWD\\src-tauri\\target-alt"
npm run tauri:build
```

### OpenClaw 安装策略（已对齐官方）
- 官方安装命令：`npm install -g openclaw@latest`
- 配置通过官方 CLI：`openclaw onboard --non-interactive ...`
- 当配置命令失败（例如 Windows 本地 `gateway closed (1006)`）时，会自动回退到更稳妥参数重试：
  - `--flow manual --no-install-daemon --skip-health --skip-channels --skip-skills`

### 常见问题
1. `cargo not found`
- 你在 dev 模式下运行了 Tauri，需要 Rust：
```powershell
winget install --id Rustlang.Rustup -e
```

2. `openclaw onboard failed ... gateway closed (1006)`
- 这是 Windows 本地网关探活/连接异常关闭，通常不是你配置填错。
- 安装器已内置自动回退重试逻辑（见上）。

3. `Cannot find module ... openclaw.mjs`
- 通常是命中了损坏的本地 `openclaw.cmd` 包装器。
- 安装器现在会优先选择可用的全局命令，失败时回退 `npx openclaw`。

4. 反复弹黑窗
- 执行层已使用 `CREATE_NO_WINDOW`，并尽量后台执行，减少弹窗。
6. 点右上角关闭后找不到窗口
- 本程序默认“关闭即隐藏到托盘”，不会退出。
- 去任务栏右下角托盘图标，左键可显示/隐藏；菜单中可退出。

5. `npm install openclaw@latest failed (code=128)`（`ls-remote ... libsignal-node`）
- 含义：安装依赖时访问 GitHub 失败（网络不可达或 SSH 认证失败）。
- 当前版本已内置自动重试链路：
  - `direct-github`
  - `mirror:https://gitclone.com/github.com/`
  - `mirror:https://gh.llkk.cc/https://github.com/`
- 若仍失败：在向导高级参数中填写 `HTTP(S) Proxy`，或放行上述域名的 443 出站。

---

## English

### What this app provides
- One-shot wizard: env check -> dependency install -> OpenClaw install -> configure -> start -> health
- Step-by-step flow with checkboxes and confirmations
- Auto-open management URL after successful install
- Maintenance center: start/stop/restart, backup/rollback, upgrade, model-chain switch, logs, security scan
- Desktop shortcuts for one-click launch

### Paths
- Installer logs: `%APPDATA%\OpenClawInstaller\logs\*.log`
- Default OpenClaw home: `%USERPROFILE%\.openclaw`
- Backups: `%APPDATA%\OpenClawInstaller\backups`
- WeChat donation QR (embedded in binary): `src-tauri\assets\donate-wechat.jpg`
  - Notes: embedded via Rust `include_bytes!`, not shipped as a swappable frontend asset
  - To update: replace the JPG and rebuild with `npm run tauri:build`

### WSL2 isolated testing (does not touch Windows OpenClaw)
```powershell
# Prepare only (no install, no gateway start)
.\scripts\start-isolated-test.ps1 -PrepareOnly

# Enter isolated shell with env vars preloaded
.\scripts\enter-isolated-shell.ps1

# Start isolated gateway in foreground
.\scripts\start-isolated-test.ps1
```

### One-click launch
1. Run `Create-Desktop-Shortcut.cmd` once
2. Use desktop shortcut `OpenClaw Installer.lnk`

### Dev
```powershell
npm install
npm run tauri:dev
```

### Build EXE + MSI
```powershell
npm run tauri:build
```

Artifacts (default target):
- `src-tauri\target\release\openclaw-installer.exe`
- `src-tauri\target\release\bundle\nsis\OpenClaw Installer_0.1.0_x64-setup.exe`
- `src-tauri\target\release\bundle\msi\OpenClaw Installer_0.1.0_x64_zh-CN.msi`

### OpenClaw install strategy
- Official install command: `npm install -g openclaw@latest`
- Configuration uses official CLI onboarding: `openclaw onboard --non-interactive ...`
- If onboard fails with Windows local `gateway closed (1006)`, installer auto-retries with safer flags.
- If npm install fails with GitHub `code=128` (`ls-remote ... libsignal-node`), installer auto-retries with mirror git rewrite routes (`gitclone.com`, `gh.llkk.cc`) before failing.
