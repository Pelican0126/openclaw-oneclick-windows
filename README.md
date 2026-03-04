# OpenClaw Installer (Windows)

One-click GUI installer and maintenance tool for OpenClaw.

## Download

- GitHub Releases: `https://github.com/Pelican0126/openclaw-oneclick-windows/releases`
- Mainland-friendly distribution: share `OpenClawInstaller-v1.0.0-windows.zip` from the `release/` folder (for example via domestic drive or object storage).

## 中文（给小白）

### 三步安装（推荐）

1. 下载 `OpenClawInstaller-v1.0.0-windows.zip`
2. 解压后双击 `INSTALL_NOW.cmd`
3. 打开 `OpenClaw Installer`，按向导一直下一步

说明：
- 也可以直接双击 `OpenClawInstaller-v1.0.0-setup.exe` 或 `OpenClawInstaller-v1.0.0.msi`。
- 关闭窗口默认是最小化到系统托盘，不是退出。
- 要完全退出，请在托盘菜单点“退出”。

### 首次安装建议

1. 模型优先从预置选项里选。
2. 没有的模型手写 `provider/model`。
3. 安装完成后到维护中心再做升级、备份、回退模型。

### 常用路径

- 日志：`%APPDATA%\OpenClawInstaller\logs\*.log`
- 备份：`%APPDATA%\OpenClawInstaller\backups`
- 默认安装目录：`%LOCALAPPDATA%\OpenClawInstaller\openclaw`

### 文档

- 使用说明：`docs/USAGE.md`
- 故障排查：`docs/TROUBLESHOOTING.md`
- 安全说明：`docs/SECURITY.md`

---

## English

### 3-step install (recommended)

1. Download `OpenClawInstaller-v1.0.0-windows.zip`
2. Extract it and double-click `INSTALL_NOW.cmd`
3. Open `OpenClaw Installer` and complete the wizard

Notes:
- You can also run `OpenClawInstaller-v1.0.0-setup.exe` or `OpenClawInstaller-v1.0.0.msi` directly.
- Closing the window minimizes to tray (it does not fully exit).
- To fully exit, use tray menu "Exit".

### First-time recommendation

1. Pick model from preset options first.
2. If missing, type `provider/model` manually.
3. Use Maintenance Center for upgrade/backup/fallback models.

### Common paths

- Logs: `%APPDATA%\OpenClawInstaller\logs\*.log`
- Backups: `%APPDATA%\OpenClawInstaller\backups`
- Default install dir: `%LOCALAPPDATA%\OpenClawInstaller\openclaw`

### Docs

- Usage: `docs/USAGE.md`
- Troubleshooting: `docs/TROUBLESHOOTING.md`
- Security: `docs/SECURITY.md`
