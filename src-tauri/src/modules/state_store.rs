use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::models::{InstallState, OpenClawConfigInput};

use super::paths;

fn install_state_path() -> PathBuf {
    paths::state_dir().join("install_state.json")
}

fn config_state_path() -> PathBuf {
    paths::state_dir().join("last_config.json")
}

fn run_prefs_path() -> PathBuf {
    paths::state_dir().join("run_prefs.json")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RunPrefs {
    /// When true, the installer will try to keep OpenClaw gateway running in the background.
    /// "End OpenClaw" sets this to false so it stays off until user explicitly starts again.
    pub keep_running: bool,
}

impl Default for RunPrefs {
    fn default() -> Self {
        Self { keep_running: true }
    }
}

pub fn save_install_state(state: &InstallState) -> Result<()> {
    paths::ensure_dirs()?;
    let data = serde_json::to_string_pretty(state)?;
    fs::write(install_state_path(), data)?;
    Ok(())
}

pub fn load_install_state() -> Result<Option<InstallState>> {
    let path = install_state_path();
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(path)?;
    let value = serde_json::from_str::<InstallState>(&raw)?;
    Ok(Some(value))
}

pub fn save_last_config(payload: &OpenClawConfigInput) -> Result<()> {
    paths::ensure_dirs()?;
    let data = serde_json::to_string_pretty(payload)?;
    fs::write(config_state_path(), data)?;
    Ok(())
}

pub fn load_last_config() -> Result<Option<OpenClawConfigInput>> {
    let path = config_state_path();
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(path)?;
    let value = serde_json::from_str::<OpenClawConfigInput>(&raw)?;
    Ok(Some(value))
}

pub fn clear_install_state() -> Result<()> {
    let path = install_state_path();
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

pub fn clear_last_config() -> Result<()> {
    let path = config_state_path();
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

pub fn load_run_prefs() -> Result<RunPrefs> {
    let path = run_prefs_path();
    if !path.exists() {
        return Ok(RunPrefs::default());
    }
    let raw = fs::read_to_string(path)?;
    let value = serde_json::from_str::<RunPrefs>(&raw)?;
    Ok(value)
}

pub fn save_run_prefs(prefs: &RunPrefs) -> Result<()> {
    paths::ensure_dirs()?;
    let data = serde_json::to_string_pretty(prefs)?;
    fs::write(run_prefs_path(), data)?;
    Ok(())
}

pub fn set_keep_running(value: bool) -> Result<()> {
    let mut prefs = load_run_prefs()?;
    prefs.keep_running = value;
    save_run_prefs(&prefs)?;
    Ok(())
}

pub fn clear_run_prefs() -> Result<()> {
    let path = run_prefs_path();
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}
