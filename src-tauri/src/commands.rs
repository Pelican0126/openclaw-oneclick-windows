use crate::models::{
    BackupInfo, BackupResult, ConfigureResult, EnvCheckResult, HealthResult, InstallEnvResult,
    InstallLockInfo, InstallResult, InstallerStatus, LogSummary, ModelCatalogItem,
    OpenClawConfigInput, OpenClawFileConfig, ProcessControlResult, RollbackResult, SecurityResult,
    SkillCatalogItem, UninstallResult, UpgradeResult,
};
use crate::modules::{
    backup, browser, config, donate, env, health, installer, logger, model_catalog, paths, port,
    process, security, skills, state_store, upgrade,
};

// Convert internal anyhow errors into UI-friendly strings while keeping a server-side log.
fn map_err<T>(result: anyhow::Result<T>) -> Result<T, String> {
    result.map_err(|err| {
        logger::error(&err.to_string());
        err.to_string()
    })
}

#[tauri::command]
pub async fn check_env(port: u16) -> Result<EnvCheckResult, String> {
    map_err(env::check_env(port).await)
}

#[tauri::command]
pub fn install_env(port: u16) -> Result<InstallEnvResult, String> {
    map_err(env::install_env(port))
}

#[tauri::command]
pub fn release_port(port: u16) -> Result<String, String> {
    map_err(port::release_port(port))
}

#[tauri::command]
pub fn get_install_lock_info() -> Result<InstallLockInfo, String> {
    map_err((|| {
        let lock = state_store::load_install_state()?;
        if let Some(state) = lock {
            Ok(InstallLockInfo {
                installed: true,
                install_dir: Some(state.install_dir),
                version: Some(state.version),
                command_path: Some(state.command_path),
            })
        } else {
            Ok(InstallLockInfo {
                installed: false,
                install_dir: None,
                version: None,
                command_path: None,
            })
        }
    })())
}

#[tauri::command]
pub async fn install_openclaw(payload: OpenClawConfigInput) -> Result<InstallResult, String> {
    map_err(installer::install_openclaw(&payload).await)
}

#[tauri::command]
pub fn uninstall_openclaw() -> Result<UninstallResult, String> {
    map_err(installer::uninstall_openclaw())
}

#[tauri::command]
pub fn configure(payload: OpenClawConfigInput) -> Result<ConfigureResult, String> {
    map_err(config::configure(&payload))
}

#[tauri::command]
pub fn get_current_config() -> Result<OpenClawFileConfig, String> {
    map_err(config::read_current_config())
}

#[tauri::command]
pub fn update_provider_api_key(provider: String, api_key: String) -> Result<String, String> {
    map_err(config::update_provider_api_key(&provider, &api_key))
}

#[tauri::command]
pub fn start() -> Result<ProcessControlResult, String> {
    map_err(process::start())
}

#[tauri::command]
pub fn stop() -> Result<ProcessControlResult, String> {
    map_err(process::stop())
}

#[tauri::command]
pub fn end_openclaw() -> Result<ProcessControlResult, String> {
    map_err(process::end_openclaw())
}

#[tauri::command]
pub fn restart() -> Result<ProcessControlResult, String> {
    map_err(process::restart())
}

#[tauri::command]
pub async fn health_check(host: String, port: u16) -> Result<HealthResult, String> {
    map_err(health::health_check(&host, port).await)
}

#[tauri::command]
pub async fn get_status() -> Result<InstallerStatus, String> {
    map_err(process::status().await)
}

#[tauri::command]
pub fn backup() -> Result<BackupResult, String> {
    map_err(backup::backup())
}

#[tauri::command]
pub fn list_backups() -> Result<Vec<BackupInfo>, String> {
    map_err(backup::list_backups())
}

#[tauri::command]
pub fn rollback(backup_id: String) -> Result<RollbackResult, String> {
    map_err(backup::rollback(&backup_id))
}

#[tauri::command]
pub async fn upgrade() -> Result<UpgradeResult, String> {
    map_err(upgrade::upgrade().await)
}

#[tauri::command]
pub fn switch_model(primary: String, fallbacks: Vec<String>) -> Result<ConfigureResult, String> {
    map_err(config::switch_model(&primary, &fallbacks))
}

#[tauri::command]
pub fn security_check() -> Result<SecurityResult, String> {
    map_err(security::run_security_check())
}

#[tauri::command]
pub fn list_logs() -> Result<Vec<LogSummary>, String> {
    map_err(logger::list_logs())
}

#[tauri::command]
pub fn read_log(name: String, max_lines: Option<usize>) -> Result<String, String> {
    map_err(logger::read_log(&name, max_lines.unwrap_or(400)))
}

#[tauri::command]
pub fn export_log(name: String, output_path: String) -> Result<String, String> {
    map_err((|| {
        // Accept environment variables like %USERPROFILE% in exported path.
        let out = paths::normalize_path(&output_path)?;
        logger::export_log(&name, &out)
    })())
}

#[tauri::command]
pub fn clear_cache() -> Result<String, String> {
    map_err(process::clear_cache())
}

#[tauri::command]
pub fn clear_sessions() -> Result<String, String> {
    map_err(process::clear_sessions())
}

#[tauri::command]
pub fn reload_config() -> Result<String, String> {
    map_err(config::reload_config())
}

#[tauri::command]
pub fn open_management_url(url: String) -> Result<String, String> {
    map_err(browser::open_management_url(&url))
}

#[tauri::command]
pub fn open_path(path: String) -> Result<String, String> {
    map_err(browser::open_path(&path))
}

#[tauri::command]
pub fn logs_dir_path() -> Result<String, String> {
    map_err(logger::logs_dir_path())
}

#[tauri::command]
pub fn donate_wechat_qr() -> Result<String, String> {
    map_err(donate::wechat_qr_data_url())
}

#[tauri::command]
pub fn list_skill_catalog() -> Result<Vec<SkillCatalogItem>, String> {
    map_err(skills::list_skill_catalog())
}

#[tauri::command]
pub fn list_model_catalog() -> Result<Vec<ModelCatalogItem>, String> {
    map_err(model_catalog::list_model_catalog())
}

#[tauri::command]
pub fn setup_telegram_pair(pair_code: String) -> Result<String, String> {
    map_err(config::setup_telegram_pair(&pair_code))
}
