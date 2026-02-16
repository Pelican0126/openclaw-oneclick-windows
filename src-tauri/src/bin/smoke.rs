//! Smoke test helper for verifying installer process management on Windows.
//!
//! This binary is for developer verification; it is not used by the GUI.
//! It runs against an isolated root directory via environment overrides:
//! - `OPENCLAW_INSTALLER_DATA_DIR`
//! - `OPENCLAW_INSTALLER_OPENCLAW_HOME`
//!
//! Usage examples (PowerShell):
//! - `cargo run --bin smoke -- start C:\\Temp\\openclaw-smoke`
//! - `cargo run --bin smoke -- status C:\\Temp\\openclaw-smoke`
//! - `cargo run --bin smoke -- end C:\\Temp\\openclaw-smoke`
//! - `cargo run --bin smoke -- cleanup C:\\Temp\\openclaw-smoke`

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use uuid::Uuid;

#[path = "../models.rs"]
mod models;
#[path = "../modules/mod.rs"]
mod modules;

use models::{InstallState, SourceMethod};
use modules::{logger, paths, port, process, shell, state_store};

fn print_usage() {
    eprintln!(
        "Usage: smoke <start|status|end|cleanup> <root_dir>\n\
\n\
Env overrides are applied automatically under <root_dir>:\n\
  - appdata: <root_dir>\\appdata\n\
  - openclaw home: <root_dir>\\openclaw-home\n\
"
    );
}

fn set_isolated_roots(root: &Path) -> Result<(PathBuf, PathBuf)> {
    // Use absolute paths so child processes resolve OPENCLAW_* paths correctly even when cwd changes.
    let abs_root = if root.is_absolute() {
        root.to_path_buf()
    } else {
        std::env::current_dir()?.join(root)
    };
    fs::create_dir_all(&abs_root)?;

    let appdata = abs_root.join("appdata");
    let openclaw_home = abs_root.join("openclaw-home");
    fs::create_dir_all(&appdata)?;
    fs::create_dir_all(&openclaw_home)?;

    let appdata = appdata.canonicalize().unwrap_or(appdata);
    let openclaw_home = openclaw_home.canonicalize().unwrap_or(openclaw_home);
    std::env::set_var(
        "OPENCLAW_INSTALLER_DATA_DIR",
        appdata.to_string_lossy().to_string(),
    );
    std::env::set_var(
        "OPENCLAW_INSTALLER_OPENCLAW_HOME",
        openclaw_home.to_string_lossy().to_string(),
    );
    Ok((appdata, openclaw_home))
}

fn pick_free_port(start: u16, max_scan: u16) -> Result<u16> {
    for offset in 0..max_scan {
        let port_num = start + offset;
        if !port::check_port(port_num)?.in_use {
            return Ok(port_num);
        }
    }
    Err(anyhow!(
        "No free port found in range {start}..{}",
        start + max_scan
    ))
}

fn generate_gateway_token(len: usize) -> String {
    let mut out = String::new();
    while out.len() < len {
        out.push_str(&Uuid::new_v4().simple().to_string());
    }
    out.truncate(len);
    out
}

fn run_onboard(port_num: u16) -> Result<PathBuf> {
    paths::ensure_dirs()?;
    fs::create_dir_all(paths::openclaw_home())?;

    let cfg_path = paths::config_path();
    let workspace = paths::openclaw_home().join("workspace");
    fs::create_dir_all(&workspace)?;

    // This token is only for the local gateway auth (Control UI / HTTP API).
    // It is intentionally random per smoke run.
    let token = generate_gateway_token(40);

    let openclaw = shell::command_exists("openclaw").unwrap_or_else(|| "openclaw".to_string());
    let args = vec![
        "onboard".to_string(),
        "--non-interactive".to_string(),
        "--accept-risk".to_string(),
        "--flow".to_string(),
        "manual".to_string(),
        "--mode".to_string(),
        "local".to_string(),
        "--gateway-port".to_string(),
        port_num.to_string(),
        "--gateway-bind".to_string(),
        "loopback".to_string(),
        "--gateway-auth".to_string(),
        "token".to_string(),
        "--gateway-token".to_string(),
        token,
        "--workspace".to_string(),
        workspace.to_string_lossy().to_string(),
        "--node-manager".to_string(),
        "npm".to_string(),
        "--skip-ui".to_string(),
        "--skip-channels".to_string(),
        "--skip-skills".to_string(),
        "--skip-health".to_string(),
        "--no-install-daemon".to_string(),
        "--auth-choice".to_string(),
        "skip".to_string(),
    ];

    // Ensure OpenClaw reads/writes to our isolated root (set by set_isolated_roots()).
    let envs = vec![
        (
            "OPENCLAW_CONFIG_PATH".to_string(),
            cfg_path.to_string_lossy().to_string(),
        ),
        (
            "OPENCLAW_STATE_DIR".to_string(),
            paths::openclaw_home().to_string_lossy().to_string(),
        ),
    ];

    let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
    let out = shell::run_command(openclaw.as_str(), &refs, None, &envs)?;
    if out.code != 0 {
        return Err(anyhow!(
            "openclaw onboard failed (code={}): {}",
            out.code,
            if out.stderr.is_empty() {
                out.stdout
            } else {
                out.stderr
            }
        ));
    }

    if !cfg_path.exists() {
        return Err(anyhow!(
            "openclaw onboard completed but config file is missing: {}",
            cfg_path.to_string_lossy()
        ));
    }

    // Stamp meta so it's obvious this is a test profile if opened manually.
    let mut json: serde_json::Value = serde_json::from_str(&fs::read_to_string(&cfg_path)?)?;
    json["meta"]["lastTouchedAt"] = serde_json::Value::String(Utc::now().to_rfc3339());
    json["meta"]["lastTouchedVersion"] = serde_json::Value::String("smoke".to_string());
    fs::write(&cfg_path, serde_json::to_string_pretty(&json)?)?;

    Ok(cfg_path)
}

fn write_install_state(install_dir: &Path) -> Result<()> {
    fs::create_dir_all(install_dir)?;
    let command_path = shell::command_exists("openclaw").unwrap_or_else(|| "openclaw".to_string());
    let version = shell::run_command(command_path.as_str(), &["--version"], None, &[])
        .ok()
        .and_then(|out| out.stdout.lines().next().map(|s| s.trim().to_string()))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string());

    let state = InstallState {
        method: SourceMethod::Npm,
        install_dir: install_dir.to_string_lossy().to_string(),
        source_url: None,
        command_path,
        version,
        launch_args: "gateway".to_string(),
    };
    state_store::save_install_state(&state)?;
    Ok(())
}

fn print_status_json(status: &models::InstallerStatus, root: &Path) {
    let json = serde_json::json!({
        "root": root.to_string_lossy().to_string(),
        "running": status.running,
        "pid": status.pid,
        "port": status.port,
        "version": status.version,
        "model": status.current_model,
        "config_path": paths::config_path().to_string_lossy().to_string(),
        "data_dir": paths::appdata_root().to_string_lossy().to_string(),
        "openclaw_home": paths::openclaw_home().to_string_lossy().to_string()
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&json).unwrap_or_default()
    );
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 2 {
        print_usage();
        return Err(anyhow!("invalid arguments"));
    }

    let cmd = args.remove(0);
    let root = PathBuf::from(args.remove(0));
    set_isolated_roots(&root)?;
    let _ = paths::ensure_dirs();
    logger::info(&format!(
        "Smoke command: {cmd}, root={}",
        root.to_string_lossy()
    ));

    match cmd.as_str() {
        "start" => {
            let install_dir = root.join("install");
            write_install_state(&install_dir).context("write install state")?;
            // Use a non-default range to avoid interfering with a real user gateway.
            let port_num = pick_free_port(20010, 80)?;
            let cfg_path = run_onboard(port_num).context("openclaw onboard")?;
            logger::info(&format!(
                "OpenClaw config written: {}",
                cfg_path.to_string_lossy()
            ));

            let started = process::start().context("process::start")?;
            logger::info(&format!("Start result: {}", started.message));

            // Give the gateway a moment to bind the port before status probe.
            tokio::time::sleep(Duration::from_millis(700)).await;
            let status = process::status().await.context("process::status")?;
            if !status.running {
                return Err(anyhow!("status.running=false after start"));
            }
            print_status_json(&status, &root);
            Ok(())
        }
        "status" => {
            let status = process::status().await.context("process::status")?;
            print_status_json(&status, &root);
            Ok(())
        }
        "end" => {
            let ended = process::end_openclaw().context("process::end_openclaw")?;
            logger::info(&format!("End result: {}", ended.message));
            tokio::time::sleep(Duration::from_millis(400)).await;
            let status = process::status().await.context("process::status")?;
            if status.running {
                return Err(anyhow!("status.running=true after end"));
            }
            print_status_json(&status, &root);
            Ok(())
        }
        "cleanup" => {
            // Best effort: end process first so directories can be removed.
            let _ = process::end_openclaw();
            let _ = state_store::clear_install_state();
            let _ = state_store::clear_last_config();
            let _ = state_store::clear_run_prefs();
            if root.exists() {
                fs::remove_dir_all(&root).ok();
            }
            println!("OK");
            Ok(())
        }
        _ => {
            print_usage();
            Err(anyhow!("unknown command: {cmd}"))
        }
    }
}
