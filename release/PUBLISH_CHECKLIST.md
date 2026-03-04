# Publish Checklist (v1.0.0)

## 1) Safety checks

Run secret scan before publish:

```powershell
.\scripts\scan-secrets.ps1
```

Optional cleanup before packaging:

```powershell
.\Cleanup-For-GitHub.cmd
```

## 2) Build packages

```powershell
npm install
npm run tauri:build
```

Expected output in `release/`:

- `OpenClawInstaller-v1.0.0-setup.exe`
- `OpenClawInstaller-v1.0.0.msi`
- `OpenClawInstaller-v1.0.0-windows.zip`
- `SHA256SUMS.txt`

## 3) Publish assets

GitHub release assets:

- ZIP (recommended)
- EXE
- MSI
- SHA256SUMS

Mainland channel:

- Upload ZIP to domestic drive/object storage
- Share direct download link
