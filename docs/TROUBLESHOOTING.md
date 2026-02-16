# Troubleshooting

## 1. `cargo not found` when launching
Cause:
- You are in dev-mode path (`tauri:dev`) without Rust.

Fix:
```powershell
winget install --id Rustlang.Rustup -e
```

Or build once and run Release shortcut:
```powershell
npm run tauri:build
```

## 2. `program not found` during install
Cause:
- Windows `.cmd` tools (`npm.cmd`, `openclaw.cmd`) were not launched correctly in old flow.

Current status:
- Fixed in execution layer: `.cmd/.bat` commands are wrapped through `cmd /C`.
- Installer logs command stdout/stderr snapshots for easier diagnosis.

If still failing:
- verify dependencies:
```powershell
where npm
where node
```

## 3. `openclaw onboard failed ... gateway closed (1006)`
Meaning:
- OpenClaw gateway probe closed abnormally during local onboarding on Windows.
- In most cases this is runtime startup/probe instability, not invalid wizard input.

Current status:
- Installer auto-retries onboarding with safer flags:
  - `--flow manual`
  - `--no-install-daemon`
  - `--skip-health`
  - `--skip-channels`
  - `--skip-skills`

If it still fails:
1. Ensure no stale port conflict on configured gateway port.
2. Re-run from Execute page and inspect `%APPDATA%\\OpenClawInstaller\\logs\\*.log`.

## 3.1 `npm install openclaw@latest failed (code=128)` with `ls-remote ... libsignal-node`
Meaning:
- OpenClaw dependency resolution reached a Git source (`whiskeysockets/libsignal-node`) and GitHub access failed.
- Typical causes: blocked `github.com:443`, corporate proxy/firewall, or SSH auth mismatch.

Current status:
- Installer now auto-retries npm install with three routes:
  - direct GitHub rewrite
  - mirror rewrite to `https://gitclone.com/github.com/`
  - mirror rewrite to `https://gh.llkk.cc/https://github.com/`

If it still fails:
1. Fill `HTTP(S) Proxy` in Wizard -> Advanced.
2. Allow outbound 443 to `github.com`, `gitclone.com`, `gh.llkk.cc`.
3. Retry from Execute page.

## 4. Installer opens too many terminal windows
Cause:
- GUI process spawning CLI children with visible consoles.

Current status:
- Fixed: command runner uses `CREATE_NO_WINDOW`.

## 5. OpenClaw start fails after install
Checklist:
1. Ensure config exists:
   - `%USERPROFILE%\.openclaw\openclaw.json`
2. Check logs:
   - `%APPDATA%\OpenClawInstaller\logs\openclaw-stdout.log`
   - `%APPDATA%\OpenClawInstaller\logs\openclaw-stderr.log`
3. Re-run wizard execute pipeline (idempotent).

## 5.1 `Cannot find module ... openclaw.mjs`
Meaning:
- A stale/broken local `openclaw.cmd` wrapper was resolved to an incomplete local package path.

Current status:
- Runtime now validates configured command path.
- It falls back to a usable global `openclaw` command, then `npx openclaw` if needed.

## 6. Health check failed
Notes:
- Gateway health is WebSocket-first; HTTP endpoints may not always respond.
- Installer now uses TCP probe with retries.

If still failing:
1. Confirm port listener:
```powershell
Get-NetTCPConnection -LocalPort <port>
```
2. Confirm no port conflict on Welcome page.

## 7. Upgrade failed
Behavior:
- Auto rollback to pre-upgrade backup is built-in.

Actions:
1. Check maintenance logs.
2. Re-run environment install.
3. Retry upgrade.

## 8. Security warnings
If ACL warning persists:
```powershell
icacls "$env:USERPROFILE\.openclaw\openclaw.json" /inheritance:r
icacls "$env:USERPROFILE\.openclaw\openclaw.json" /grant:r "$env:USERNAME:(R,W)"
icacls "$env:USERPROFILE\.openclaw\.env" /inheritance:r
icacls "$env:USERPROFILE\.openclaw\.env" /grant:r "$env:USERNAME:(R,W)"
```
