# 开发者文档 / Developer Notes

普通用户请直接看：`docs/USAGE.md`（从 Releases 安装即可）

## 本地开发运行 / Dev

```powershell
npm install
npm run tauri:dev
```

## 生产打包（EXE + MSI）/ Build (EXE + MSI)

```powershell
npm run tauri:build
```

打包完成后，安装包会自动复制到项目根目录的 `release/`，方便直接上传到 GitHub Releases：

- `release/OpenClawInstaller-v{version}-setup.exe`
- `release/OpenClawInstaller-v{version}.msi`
- `release/OpenClawInstaller-v{version}-windows.zip`
- `release/SHA256SUMS.txt`

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
- 会删除 `node_modules/`、`dist/`、`src-tauri/target*`、`%TEMP%/`、`.tmp-*.log` 等本地生成物
- 不会触碰你 Windows 的 `%USERPROFILE%\\.openclaw`
