use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::Duration;

use anyhow::{anyhow, Result};

use crate::models::{HealthResult, InstallerStatus, OpenClawFileConfig, ProcessControlResult};

use super::{config, health, logger, paths, shell, state_store};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

const DETACHED_PROCESS: u32 = 0x00000008;
const CREATE_NO_WINDOW: u32 = 0x08000000;
// Break away from parent job to survive dev-runner/job kill-on-close on Windows.
const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x01000000;

static LAST_AUTOSTART_ATTEMPT_MS: OnceLock<Mutex<u128>> = OnceLock::new();

fn should_attempt_autostart(now_ms: u128, min_interval_ms: u128) -> bool {
    let lock = LAST_AUTOSTART_ATTEMPT_MS.get_or_init(|| Mutex::new(0u128));
    let mut last = lock.lock().unwrap_or_else(|e| e.into_inner());
    if now_ms.saturating_sub(*last) < min_interval_ms {
        return false;
    }
    *last = now_ms;
    true
}

pub fn start() -> Result<ProcessControlResult> {
    paths::ensure_dirs()?;
    // Idempotent start: if PID is alive, do not spawn a duplicate process.
    if let Some(pid) = running_pid() {
        return Ok(ProcessControlResult {
            running: true,
            pid: Some(pid),
            message: format!("OpenClaw already running (PID {pid})"),
        });
    }

    let install = state_store::load_install_state()?
        .ok_or_else(|| anyhow!("Install state not found. Run install_openclaw first."))?;
    let cfg = config::read_current_config()?;
    let args = build_gateway_args(&cfg);
    let runtime_command = resolve_runtime_command(&install.command_path)?;

    let spawn_with_flags = |creation_flags: u32| -> Result<std::process::Child> {
        let stdout_log = paths::logs_dir().join("openclaw-stdout.log");
        let stderr_log = paths::logs_dir().join("openclaw-stderr.log");
        let stdout = OpenOptions::new()
            .create(true)
            .append(true)
            .open(stdout_log)?;
        let stderr = OpenOptions::new()
            .create(true)
            .append(true)
            .open(stderr_log)?;

        let mut cmd = build_process_command(&runtime_command, &args)?;
        cmd.stdout(Stdio::from(stdout));
        cmd.stderr(Stdio::from(stderr));
        cmd.current_dir(&install.install_dir);
        for (k, v) in runtime_env(&cfg) {
            cmd.env(k, v);
        }
        #[cfg(windows)]
        {
            cmd.creation_flags(creation_flags);
        }
        Ok(cmd.spawn()?)
    };

    // Some job configurations disallow breakaway. Retry without breakaway if needed.
    let child = spawn_with_flags(DETACHED_PROCESS | CREATE_NO_WINDOW | CREATE_BREAKAWAY_FROM_JOB)
        .or_else(|err| {
        logger::warn(&format!(
            "OpenClaw spawn with breakaway failed, retrying without breakaway: {err}"
        ));
        spawn_with_flags(DETACHED_PROCESS | CREATE_NO_WINDOW)
    })?;
    let pid = child.id();
    write_pid(pid)?;
    // User intention: once started, keep it running unless explicitly ended via Maintenance.
    let _ = state_store::set_keep_running(true);
    logger::info(&format!(
        "OpenClaw process started at PID {pid} (command: {}).",
        runtime_command
    ));

    thread::sleep(Duration::from_millis(650));
    Ok(ProcessControlResult {
        running: true,
        pid: Some(pid),
        message: "OpenClaw process started.".to_string(),
    })
}

pub fn stop() -> Result<ProcessControlResult> {
    if let Some(pid) = read_pid() {
        let pid_text = pid.to_string();
        // /T ensures child processes are also terminated.
        let out = shell::run_command("taskkill", &["/PID", &pid_text, "/T", "/F"], None, &[])?;
        if out.code == 0 {
            remove_pid();
            logger::info(&format!("OpenClaw process stopped, PID {pid}."));
            return Ok(ProcessControlResult {
                running: false,
                pid: Some(pid),
                message: "Process stopped.".to_string(),
            });
        }
        return Err(anyhow!(
            "Failed to stop process PID {pid}: {}",
            if out.stderr.is_empty() {
                out.stdout
            } else {
                out.stderr
            }
        ));
    }
    Ok(ProcessControlResult {
        running: false,
        pid: None,
        message: "Process is not running.".to_string(),
    })
}

pub fn end_openclaw() -> Result<ProcessControlResult> {
    // Stop should be idempotent; always record user intent first.
    let _ = state_store::set_keep_running(false);
    let result = stop()?;
    Ok(ProcessControlResult {
        running: false,
        pid: result.pid,
        message: "OpenClaw ended by user. It will stay stopped until you click Start again."
            .to_string(),
    })
}

pub fn restart() -> Result<ProcessControlResult> {
    let _ = stop();
    start()
}

pub async fn status() -> Result<InstallerStatus> {
    // Best-effort: keep OpenClaw running unless user explicitly ended it.
    // This is throttled to avoid repeated spawn storms on misconfiguration.
    let prefs = state_store::load_run_prefs().unwrap_or_default();

    let cfg = config::read_current_config().unwrap_or_else(|_| OpenClawFileConfig {
        provider: "unknown".to_string(),
        model_chain: crate::models::ModelChain {
            primary: "unknown".to_string(),
            fallbacks: vec![],
        },
        api_key: String::new(),
        base_url: None,
        proxy: None,
        bind_address: "127.0.0.1".to_string(),
        port: 28789,
        install_dir: String::new(),
        launch_args: "gateway".to_string(),
        updated_at: String::new(),
    });
    let install = state_store::load_install_state()?.unwrap_or(crate::models::InstallState {
        method: crate::models::SourceMethod::Npm,
        install_dir: String::new(),
        source_url: None,
        command_path: String::new(),
        version: "unknown".to_string(),
        launch_args: "gateway".to_string(),
    });
    let pid = running_pid();
    let health_result = health::health_check(&cfg.bind_address, cfg.port)
        .await
        .unwrap_or_else(|_| HealthResult::default());
    let running = pid.is_some() || health_result.ok;

    if !running && prefs.keep_running {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0u128);
        if should_attempt_autostart(now_ms, 20_000) {
            if let Ok(Some(_)) = state_store::load_install_state() {
                if paths::config_path().exists() {
                    if let Err(err) = start() {
                        logger::warn(&format!("Auto-start OpenClaw failed: {err}"));
                    }
                }
            }
        }
    }

    let version = if install.version.trim().is_empty() || install.version == "unknown" {
        detect_global_version().unwrap_or_else(|| "unknown".to_string())
    } else {
        install.version
    };
    let pid = running_pid();
    let health_result = health::health_check(&cfg.bind_address, cfg.port)
        .await
        .unwrap_or_else(|_| HealthResult::default());
    let running = pid.is_some() || health_result.ok;
    Ok(InstallerStatus {
        running,
        pid,
        version,
        provider: cfg.provider,
        current_model: cfg.model_chain.primary,
        port: cfg.port,
        health: health_result,
    })
}

pub fn clear_cache() -> Result<String> {
    let cache = paths::openclaw_home().join("cache");
    if cache.exists() {
        fs::remove_dir_all(&cache)?;
    }
    fs::create_dir_all(&cache)?;
    logger::info("Cache directory reset.");
    Ok(cache.to_string_lossy().to_string())
}

pub fn clear_sessions() -> Result<String> {
    let sessions = paths::openclaw_home().join("sessions");
    if sessions.exists() {
        fs::remove_dir_all(&sessions)?;
    }
    fs::create_dir_all(&sessions)?;
    let memory = paths::openclaw_home().join("memory");
    if memory.exists() {
        fs::remove_dir_all(&memory)?;
    }
    fs::create_dir_all(&memory)?;
    logger::info("Session and memory directories reset.");
    Ok("sessions,memory".to_string())
}

pub fn running_pid() -> Option<u32> {
    let pid = read_pid()?;
    if shell::is_process_alive(pid) {
        Some(pid)
    } else {
        // Remove stale PID from crash/forced kill cases.
        remove_pid();
        None
    }
}

fn build_gateway_args(cfg: &OpenClawFileConfig) -> Vec<String> {
    // Keep user override capability, but enforce stable defaults for OpenClaw CLI.
    let mut args = parse_args(&cfg.launch_args);
    if args.is_empty() || args[0].eq_ignore_ascii_case("serve") {
        args = vec!["gateway".to_string()];
    } else if !args[0].eq_ignore_ascii_case("gateway") {
        args.insert(0, "gateway".to_string());
    }
    if !has_arg(&args, "--port") {
        args.push("--port".to_string());
        args.push(cfg.port.to_string());
    }
    if !has_arg(&args, "--bind") {
        args.push("--bind".to_string());
        args.push(bind_mode_from_address(&cfg.bind_address).to_string());
    }
    if !has_arg(&args, "--allow-unconfigured") {
        args.push("--allow-unconfigured".to_string());
    }
    args
}

fn build_process_command(command_path: &str, args: &[String]) -> Result<Command> {
    let (exe, argv) = if command_path.eq_ignore_ascii_case("npx") {
        let npx_exe = shell::command_exists("npx")
            .ok_or_else(|| anyhow!("npx not found. Please install Node.js first."))?;
        let mut out = vec!["--yes".to_string(), "openclaw".to_string()];
        out.extend_from_slice(args);
        (npx_exe, out)
    } else {
        (command_path.to_string(), args.to_vec())
    };

    let path = PathBuf::from(&exe);
    let ext = path
        .extension()
        .map(|v| v.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    if ext == "cmd" || ext == "bat" {
        let mut cmd = Command::new("cmd");
        cmd.arg("/D").arg("/C").arg(&exe);
        for arg in &argv {
            cmd.arg(arg);
        }
        return Ok(cmd);
    }
    if ext == "ps1" {
        let mut cmd = Command::new("powershell");
        cmd.arg("-NoProfile")
            .arg("-NonInteractive")
            .arg("-ExecutionPolicy")
            .arg("Bypass")
            .arg("-File")
            .arg(&exe);
        for arg in &argv {
            cmd.arg(arg);
        }
        return Ok(cmd);
    }

    let mut cmd = Command::new(&exe);
    for arg in &argv {
        cmd.arg(arg);
    }
    Ok(cmd)
}

fn runtime_env(cfg: &OpenClawFileConfig) -> Vec<(String, String)> {
    let mut envs = vec![
        (
            "OPENCLAW_CONFIG_PATH".to_string(),
            paths::config_path().to_string_lossy().to_string(),
        ),
        (
            "OPENCLAW_STATE_DIR".to_string(),
            paths::openclaw_home().to_string_lossy().to_string(),
        ),
    ];

    if let Some(proxy) = cfg.proxy.clone().filter(|s| !s.trim().is_empty()) {
        envs.push(("HTTP_PROXY".to_string(), proxy.clone()));
        envs.push(("HTTPS_PROXY".to_string(), proxy.clone()));
        envs.push(("ALL_PROXY".to_string(), proxy));
    }

    let mut provider_env = BTreeMap::<String, String>::new();
    if let Ok(Some(last)) = state_store::load_last_config() {
        for (provider, key) in last.provider_api_keys {
            let value = key.trim();
            if value.is_empty() {
                continue;
            }
            if let Some(name) = provider_env_name(provider.as_str()) {
                provider_env.insert(name, value.to_string());
            }
        }
    }
    // Backward compatibility for old single-key payloads.
    if !cfg.api_key.trim().is_empty() {
        if let Some(name) = provider_env_name(cfg.provider.as_str()) {
            provider_env
                .entry(name)
                .or_insert_with(|| cfg.api_key.clone());
        }
    }
    for (k, v) in provider_env {
        envs.push((k, v));
    }

    envs
}

fn bind_mode_from_address(address: &str) -> &'static str {
    match address.trim() {
        "0.0.0.0" => "lan",
        _ => "loopback",
    }
}

fn has_arg(args: &[String], name: &str) -> bool {
    args.iter().any(|item| item.eq_ignore_ascii_case(name))
}

fn detect_global_version() -> Option<String> {
    let cmd = shell::command_exists("openclaw")?;
    let out = shell::run_command(cmd.as_str(), &["--version"], None, &[]).ok()?;
    if out.code != 0 {
        return None;
    }
    out.stdout
        .lines()
        .next()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn parse_args(raw: &str) -> Vec<String> {
    raw.split_whitespace().map(|s| s.to_string()).collect()
}

fn provider_env_name(provider: &str) -> Option<String> {
    let normalized = provider.trim().to_ascii_lowercase();
    let id = if normalized == "openai-codex" {
        "openai"
    } else if normalized == "kimi-code" {
        "kimi-coding"
    } else {
        normalized.as_str()
    };
    match id {
        "openai" => Some("OPENAI_API_KEY".to_string()),
        "google" => Some("GEMINI_API_KEY".to_string()),
        "moonshot" => Some("MOONSHOT_API_KEY".to_string()),
        "kimi-coding" => Some("KIMI_API_KEY".to_string()),
        "xai" => Some("XAI_API_KEY".to_string()),
        "anthropic" => Some("ANTHROPIC_API_KEY".to_string()),
        "openrouter" => Some("OPENROUTER_API_KEY".to_string()),
        "azure" => Some("AZURE_OPENAI_API_KEY".to_string()),
        "zai" => Some("ZAI_API_KEY".to_string()),
        "xiaomi" => Some("XIAOMI_API_KEY".to_string()),
        "minimax" => Some("MINIMAX_API_KEY".to_string()),
        other => generic_provider_env_name(other),
    }
}

fn generic_provider_env_name(provider: &str) -> Option<String> {
    let normalized = provider
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches('_')
        .to_string();
    if normalized.is_empty() {
        return None;
    }
    Some(format!("{normalized}_API_KEY"))
}

fn resolve_runtime_command(preferred: &str) -> Result<String> {
    let preferred = preferred.trim().trim_matches('"').to_string();
    if is_runtime_command_usable(preferred.as_str()) {
        return Ok(preferred);
    }

    logger::warn(&format!(
        "Configured runtime command is not usable: {}",
        preferred
    ));

    if let Some(global) = shell::command_exists("openclaw") {
        if !global.eq_ignore_ascii_case(preferred.as_str())
            && is_runtime_command_usable(global.as_str())
        {
            logger::warn(&format!(
                "Runtime fallback to global OpenClaw command: {}",
                global
            ));
            return Ok(global);
        }
    }

    if is_runtime_command_usable("npx") {
        logger::warn("Runtime fallback to npx openclaw.");
        return Ok("npx".to_string());
    }

    Err(anyhow!(
        "No usable OpenClaw runtime command found. Tried configured command, PATH openclaw, and npx."
    ))
}

fn is_runtime_command_usable(command: &str) -> bool {
    if command.eq_ignore_ascii_case("npx") {
        let Some(npx_exe) = shell::command_exists("npx") else {
            return false;
        };
        let Ok(out) = shell::run_command(
            npx_exe.as_str(),
            &["--yes", "openclaw", "--version"],
            None,
            &[],
        ) else {
            return false;
        };
        return out.code == 0;
    }

    let Ok(out) = shell::run_command(command, &["--version"], None, &[]) else {
        return false;
    };
    out.code == 0
}

fn pid_file() -> PathBuf {
    paths::run_dir().join("openclaw.pid")
}

fn write_pid(pid: u32) -> Result<()> {
    let path = pid_file();
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)?;
    file.write_all(pid.to_string().as_bytes())?;
    Ok(())
}

fn read_pid() -> Option<u32> {
    let path = pid_file();
    let raw = fs::read_to_string(path).ok()?;
    raw.trim().parse::<u32>().ok()
}

fn remove_pid() {
    let _ = fs::remove_file(pid_file());
}
