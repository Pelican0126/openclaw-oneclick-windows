# 使用说明 / Usage

Releases（安装包下载）/ Download: `https://github.com/Pelican0126/openclaw-oneclick-windows/releases`

## 下载 / Download

- Releases（安装包下载）：`https://github.com/Pelican0126/openclaw-oneclick-windows/releases`
- 建议优先下载：
  - `OpenClaw Installer_*_x64-setup.exe`（NSIS 安装包，最通用）
  - 或 `OpenClaw Installer_*_x64_zh-CN.msi`（MSI 安装包）

## 安装与首次运行（Windows）/ Install & First Run (Windows)

### 1) 安装本程序 / Install this app

1. 双击运行安装包（`.exe` 或 `.msi`）
2. 安装完成后，从桌面快捷方式或开始菜单启动 `OpenClaw Installer`

说明：
- 本程序关闭窗口默认不会退出，而是隐藏到 Windows 右下角系统托盘（Tray）
- 需要彻底退出：在托盘菜单点“退出”，或在维护中心显式停止/退出

### 2) 一次安装向导 / One-shot wizard

向导是“勾选 + 下一步”的分步流程。建议按顺序完成：

1. 基础设置：安装目录、绑定地址、端口、管理地址
2. 模型与认证：选择 Provider -> 选择主模型 -> 填写该 Provider 的 API Key（可选）
   - Kimi k2.5：如使用 moonshot 的 Kimi k2.5，可选择区域（国内/海外）自动设置 baseUrl
   - 回退模型：首次安装默认只配置主模型；回退模型请到“维护中心”里再配置（可跨 Provider、可独立 API Key）
3. 功能勾选：按需勾选 skills / session memory / workspace memory 等
4. 高级安装：代理、镜像源、onboard 策略等（不确定就保持默认）
5. 确认提交：确认后进入“执行安装”

执行安装页会显示：
- 每个步骤的状态（可重试）
- 实时日志（可复制）
- 失败原因提示与建议

### 3) 安装完成 / After install

安装完成后你可以：
- 一键打开管理网页 URL（可在向导里勾选“安装完成后自动打开”）
- 进入“维护中心”继续做升级、备份/回滚、换模型等操作

## 维护中心 / Maintenance Center

维护中心提供常用操作入口：

- 启动 / 停止 / 重启 OpenClaw（OpenClaw 在后台常驻，除非你手动停止）
- 一键备份 / 一键回滚（回滚前会强制自动备份，防误操作）
- 一键升级 OpenClaw（失败会自动回滚）
- 切换模型链（可在这里配置回退模型；回退模型可选择不同 Provider 并配置不同 API Key）
- 日志查看与导出（并提供“打开日志目录”链接）
- 一键安全检查（权限/明文 Key 风险提示）
- 删除 OpenClaw（卸载 OpenClaw + 清理本地数据）

## 日志 / Logs

- Installer 日志目录：`%APPDATA%\\OpenClawInstaller\\logs\\`
- 备份目录：`%APPDATA%\\OpenClawInstaller\\backups\\`

## 卸载 / Uninstall

两种方式：

1. 在维护中心点击“删除 OpenClaw”（会停止 OpenClaw 并删除安装目录、`%USERPROFILE%\\.openclaw` 以及 `%APPDATA%\\OpenClawInstaller`）
2. 在 Windows “应用和功能”中卸载 `OpenClaw Installer`

## 常见问题 / FAQ

更多排障请看：`docs/TROUBLESHOOTING.md`

安全说明请看：`docs/SECURITY.md`
