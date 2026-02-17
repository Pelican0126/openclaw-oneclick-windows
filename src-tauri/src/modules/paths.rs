use std::env;
use std::path::PathBuf;

use anyhow::{anyhow, Result};
use regex::Regex;

pub fn appdata_root() -> PathBuf {
    if let Ok(value) = env::var("OPENCLAW_INSTALLER_DATA_DIR") {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    dirs::data_dir()
        .unwrap_or_else(env::temp_dir)
        .join("OpenClawInstaller")
}

pub fn logs_dir() -> PathBuf {
    appdata_root().join("logs")
}

pub fn backups_dir() -> PathBuf {
    appdata_root().join("backups")
}

pub fn state_dir() -> PathBuf {
    appdata_root().join("state")
}

pub fn run_dir() -> PathBuf {
    appdata_root().join("run")
}

pub fn openclaw_home() -> PathBuf {
    if let Ok(value) = env::var("OPENCLAW_INSTALLER_OPENCLAW_HOME") {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    default_isolated_openclaw_home()
}

pub fn config_path() -> PathBuf {
    openclaw_home().join("openclaw.json")
}

pub fn default_isolated_openclaw_home() -> PathBuf {
    // Default to an isolated per-user directory so the installer never
    // touches an existing `%USERPROFILE%\\.openclaw` installation unless
    // the user explicitly points to it via Wizard -> Install directory.
    dirs::data_local_dir()
        .or_else(dirs::data_dir)
        .unwrap_or_else(env::temp_dir)
        .join("OpenClawInstaller")
        .join("openclaw")
}

pub fn is_user_profile_default_openclaw_dir(path: &std::path::Path) -> bool {
    // This is the classic OpenClaw state directory on Windows. We must never
    // use it for the installer-managed instance because it can overwrite a
    // user's existing setup.
    let Some(home) = dirs::home_dir() else {
        return false;
    };

    let candidates = [
        home.join(".openclaw"),
        home.join(".clawdbot"),
        home.join(".moldbot"),
        home.join(".moltbot"),
    ];

    let normalize = |p: &std::path::Path| {
        p.to_string_lossy()
            .replace('/', "\\")
            .trim_end_matches('\\')
            .to_ascii_lowercase()
    };

    let needle = normalize(path);
    candidates.iter().any(|candidate| needle == normalize(candidate))
}

pub fn ensure_dirs() -> Result<()> {
    for dir in [
        appdata_root(),
        logs_dir(),
        backups_dir(),
        state_dir(),
        run_dir(),
        openclaw_home(),
    ] {
        std::fs::create_dir_all(&dir)?;
    }
    Ok(())
}

pub fn normalize_path(raw: &str) -> Result<PathBuf> {
    let expanded = expand_env_vars(raw)?;
    let expanded = expanded.replace('/', "\\");
    let with_home = if expanded.starts_with("~\\") || expanded == "~" {
        let home = dirs::home_dir().ok_or_else(|| anyhow!("Cannot resolve home directory"))?;
        let tail = expanded.trim_start_matches('~').trim_start_matches('\\');
        if tail.is_empty() {
            home
        } else {
            home.join(tail)
        }
    } else {
        PathBuf::from(expanded)
    };
    Ok(with_home)
}

pub fn expand_env_vars(raw: &str) -> Result<String> {
    let re = Regex::new(r"%([A-Za-z0-9_]+)%")?;
    let mut output = raw.to_string();
    for caps in re.captures_iter(raw) {
        if let Some(var) = caps.get(1) {
            let key = var.as_str();
            let value = env::var(key).unwrap_or_default();
            output = output.replace(&format!("%{key}%"), &value);
        }
    }
    Ok(output)
}
