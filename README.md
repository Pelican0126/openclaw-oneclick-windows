# OpenClaw Installer (Windows)

Windows GUI installer/maintainer for OpenClaw (Tauri + Rust + React).

## Releases / 下载

- Releases（安装包下载）：`https://github.com/Pelican0126/openclaw-oneclick-windows/releases`
- 推荐优先下载：
  - `OpenClaw Installer_*_x64-setup.exe`（NSIS 安装包，最通用）
  - 或 `OpenClaw Installer_*_x64_zh-CN.msi`（MSI 安装包）

## 中文

### 快速开始

1. 从 Releases 下载并安装（`.exe` 或 `.msi`）
2. 启动 `OpenClaw Installer`
3. 按向导完成一次安装：环境检查 -> 安装依赖 -> 安装 OpenClaw -> 写配置 -> 启动 -> 健康检查
4. 安装完成可自动打开管理页 URL

说明：
- 关闭窗口不会退出：会隐藏到 Windows 系统托盘（右下角）
- 需要彻底退出：在托盘菜单点“退出”，或在维护中心显式停止/退出
- 安装完成后默认禁止重复安装：需先在维护中心“删除 OpenClaw”后才能重新安装

### 关键路径

- Installer 日志：`%APPDATA%\\OpenClawInstaller\\logs\\*.log`
- 备份目录：`%APPDATA%\\OpenClawInstaller\\backups`
- OpenClaw 安装目录（隔离，默认）：`%LOCALAPPDATA%\\OpenClawInstaller\\openclaw`（可在向导修改；默认不会触碰你的 `%USERPROFILE%\\.openclaw`）

### 文档

- 使用说明：`docs/USAGE.md`
- 故障排查：`docs/TROUBLESHOOTING.md`
- 安全说明：`docs/SECURITY.md`
- 架构说明：`docs/ARCHITECTURE.md`
- 开发者文档（打包/脚本/隔离自测等）：`docs/DEV.md`

---

## English

### Quick start

1. Download and install from Releases (`.exe` or `.msi`)
2. Launch `OpenClaw Installer`
3. Finish the wizard: env check -> deps -> install OpenClaw -> configure -> start -> health probe
4. The installer can auto-open the management URL when done

Notes:
- Closing the window hides the app into the Windows tray (it does not exit)
- To fully exit: use tray menu "Exit" or stop/exit from Maintenance
- Reinstall is disabled by default after installation; uninstall first in Maintenance if needed

### Paths

- Installer logs: `%APPDATA%\\OpenClawInstaller\\logs\\*.log`
- Backups: `%APPDATA%\\OpenClawInstaller\\backups`
- OpenClaw install dir (isolated by default): `%LOCALAPPDATA%\\OpenClawInstaller\\openclaw` (change it in Wizard -> Install directory; by default it will not touch `%USERPROFILE%\\.openclaw`)

### Docs

- Usage: `docs/USAGE.md`
- Troubleshooting: `docs/TROUBLESHOOTING.md`
- Security: `docs/SECURITY.md`
- Architecture: `docs/ARCHITECTURE.md`
- Developer notes: `docs/DEV.md`
