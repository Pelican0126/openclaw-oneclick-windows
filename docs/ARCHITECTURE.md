# Architecture

## 1. Stack
- GUI: `React + TypeScript + Vite`
- Native shell/runtime: `Tauri`
- Execution layer: `Rust` modules under `src-tauri/src/modules`

## 2. Separation of concerns
- UI (`src/`)
  - collects installer parameters
  - displays step progress and logs
  - triggers backend commands via Tauri invoke API
- Execution layer (`src-tauri/src/modules`)
  - performs all filesystem/network/process operations
  - owns idempotent install/maintenance logic
  - persists logs, state, backups

## 3. Command map
- `check_env`, `install_env` -> `modules/env.rs`
- `get_install_lock_info` -> `modules/state_store.rs`
- `install_openclaw` -> `modules/installer.rs`
- `configure`, `switch_model`, `reload_config` -> `modules/config.rs`
- `start`, `stop`, `restart`, `get_status`, clear actions -> `modules/process.rs`
- `health_check` -> `modules/health.rs`
- `backup`, `rollback`, `list_backups` -> `modules/backup.rs`
- `upgrade` -> `modules/upgrade.rs`
- `security_check` -> `modules/security.rs`
- log APIs -> `modules/logger.rs`

## 4. Runtime/data paths
- installer root: `%APPDATA%\OpenClawInstaller`
- installer logs: `%APPDATA%\OpenClawInstaller\logs`
- installer backups: `%APPDATA%\OpenClawInstaller\backups`
- installer state: `%APPDATA%\OpenClawInstaller\state`
- PID file: `%APPDATA%\OpenClawInstaller\run\openclaw.pid`
- OpenClaw state/config: Wizard `install_dir` (default `%LOCALAPPDATA%\OpenClawInstaller\openclaw`)

## 5. Install/config strategy
1. Install OpenClaw locally (isolated, no global install/uninstall):
   - `npm --prefix <install_dir> install openclaw@latest`
   - fallback when local install is unavailable: use existing `openclaw` from PATH or `npx openclaw ...`
2. Configure through official CLI onboarding:
   - `openclaw onboard --non-interactive ...`
3. Apply model chain:
   - `openclaw models set ...`
   - `openclaw models fallbacks clear/add ...`
4. Apply optional toggles:
   - `openclaw hooks enable/disable session-memory`
   - optional `openclaw skills check`
   - optional workspace memory bootstrap

This avoids hand-writing incompatible config schema.

## 6. Process model
- MVP mode uses background process + PID file.
- Start command launches OpenClaw gateway:
  - `openclaw gateway --port <port> --bind <mode> --allow-unconfigured`
- Runtime env includes:
  - `OPENCLAW_CONFIG_PATH`
  - `OPENCLAW_STATE_DIR`
- Windows process flags:
  - detached + no-window (reduces terminal popup noise).

## 6.1 Reinstall lock
- Installer is write-once by default:
  - if install state exists, `install_openclaw` returns an error and blocks reinstall
  - user must run uninstall in Maintenance first
- `upgrade` bypasses this lock internally and is still allowed

## 7. Health strategy
- primary probe: TCP connect to configured host/port with retry loop
- fallback probe: HTTP endpoints (`/health`, `/status`, etc.)
- result is exposed to Execute page and Maintenance page.

## 8. Upgrade and rollback
- upgrade flow:
  - create `pre-upgrade` backup
  - reinstall latest OpenClaw
  - auto-restore on failure
- rollback flow:
  - create `pre-rollback` snapshot
  - restore selected backup archive
