# 开发者文档 / Developer Notes

普通用户请直接看：`docs/USAGE.md`（从 Releases 安装即可）

## 本地开发运行 / Dev

```powershell
npm install
npm run tauri:dev
```

说明：
- 你可以右键 `Launch-OpenClawInstaller.cmd` 选择“以管理员身份运行”（管理员/普通权限都可用）
- 脚本使用 `%~dp0` 相对路径解析，项目放在任意目录都可运行

## 生产打包（EXE + MSI）/ Build (EXE + MSI)

```powershell
npm run tauri:build
```

## 设置 App 图标（Windows）/ App Icon (Windows)

1. 把图标保存为项目根目录 `app-icon.png`（建议 1024x1024、透明背景、四周留白）
2. 生成图标资源：

```powershell
npm run icons
```

3. 重新打包：

```powershell
npm run tauri:build
```

## 上传 GitHub 前（避免隐私泄露）/ Before pushing to GitHub

1. 运行敏感信息扫描（只输出文件名，不会打印疑似密钥内容）：

```powershell
.\scripts\scan-secrets.ps1
```

2. 如果你打算“打包整个文件夹上传/发给别人”（不是用 `git push`），建议先清理本地生成物：

- 双击 `Cleanup-For-GitHub.cmd`
- 会删除 `node_modules/`、`dist/`、`src-tauri/target*`、`.smoke*`、`%TEMP%/`、`.tmp-*.log` 等本地生成物
- 不会触碰你 Windows 的 `%USERPROFILE%\\.openclaw`

## WSL2 隔离自测（不影响 Windows 现有 OpenClaw）

目标：
- 文件隔离：测试版用独立 `OPENCLAW_HOME`
- 端口隔离：测试版 gateway 用非默认端口（例如 `28789`）
- 进程隔离：测试版只在 WSL2 里跑，不影响 Windows 正在用的 OpenClaw

```powershell
# 只做隔离环境准备（不安装、不启动）
.\scripts\start-isolated-test.ps1 -PrepareOnly

# 进入隔离 shell（会自动设置 OPENCLAW_HOME / WORKSPACE / LOG_DIR）
.\scripts\enter-isolated-shell.ps1

# 在当前终端启动隔离网关（前台运行）
.\scripts\start-isolated-test.ps1
```

