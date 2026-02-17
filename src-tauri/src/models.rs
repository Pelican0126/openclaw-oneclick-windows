use serde::{Deserialize, Serialize};
use std::collections::HashMap;

fn default_kimi_region() -> String {
    "cn".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyStatus {
    pub name: String,
    pub found: bool,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortStatus {
    pub port: u16,
    pub in_use: bool,
    pub pid: Option<u32>,
    pub process_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvCheckResult {
    pub os: String,
    pub is_windows: bool,
    pub is_admin: bool,
    pub network_ok: bool,
    pub network_detail: String,
    pub dependencies: Vec<DependencyStatus>,
    pub port_status: PortStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallEnvResult {
    pub installed: Vec<String>,
    pub skipped: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceMethod {
    Npm,
    Bun,
    Git,
    Binary,
}

impl Default for SourceMethod {
    fn default() -> Self {
        Self::Npm
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelChain {
    pub primary: String,
    pub fallbacks: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct OpenClawConfigInput {
    pub install_dir: String,
    pub provider: String,
    pub model_chain: ModelChain,
    pub api_key: String,
    pub provider_api_keys: HashMap<String, String>,
    pub selected_skills: Vec<String>,
    pub base_url: Option<String>,
    pub proxy: Option<String>,
    pub port: u16,
    pub bind_address: String,
    pub source_method: SourceMethod,
    pub source_url: Option<String>,
    pub launch_args: String,
    pub onboarding_mode: String,
    pub onboarding_flow: String,
    pub install_daemon: bool,
    pub node_manager: String,
    pub skip_channels: bool,
    pub skip_skills: bool,
    pub skip_health: bool,
    pub remote_url: Option<String>,
    pub remote_token: Option<String>,
    pub enable_skills_scan: bool,
    pub enable_session_memory_hook: bool,
    pub enable_workspace_memory: bool,
    #[serde(default = "default_kimi_region")]
    pub kimi_region: String,
    pub enable_feishu_channel: bool,
    pub feishu_app_id: String,
    pub feishu_app_secret: String,
    pub enable_telegram_channel: bool,
    pub telegram_bot_token: String,
    pub telegram_pair_code: String,
    pub auto_open_dashboard: bool,
}

impl Default for OpenClawConfigInput {
    fn default() -> Self {
        Self {
            // Isolated by default: do not touch an existing `%USERPROFILE%\\.openclaw`.
            install_dir: "%LOCALAPPDATA%\\OpenClawInstaller\\openclaw".to_string(),
            provider: "openai".to_string(),
            model_chain: ModelChain {
                primary: "openai/gpt-5.2".to_string(),
                fallbacks: vec![],
            },
            api_key: String::new(),
            provider_api_keys: HashMap::new(),
            selected_skills: vec!["healthcheck".to_string(), "skill-creator".to_string()],
            base_url: None,
            proxy: None,
            // Use a non-default port so we don't collide with an existing OpenClaw gateway.
            port: 28789,
            bind_address: "127.0.0.1".to_string(),
            source_method: SourceMethod::Npm,
            source_url: None,
            launch_args: "gateway".to_string(),
            onboarding_mode: "local".to_string(),
            onboarding_flow: "quickstart".to_string(),
            install_daemon: true,
            node_manager: "npm".to_string(),
            skip_channels: false,
            skip_skills: false,
            skip_health: true,
            remote_url: None,
            remote_token: None,
            enable_skills_scan: true,
            enable_session_memory_hook: true,
            enable_workspace_memory: true,
            kimi_region: default_kimi_region(),
            enable_feishu_channel: false,
            feishu_app_id: String::new(),
            feishu_app_secret: String::new(),
            enable_telegram_channel: false,
            telegram_bot_token: String::new(),
            telegram_pair_code: String::new(),
            auto_open_dashboard: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallResult {
    pub method: String,
    pub install_dir: String,
    pub version: String,
    pub command_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigureResult {
    pub config_path: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessControlResult {
    pub running: bool,
    pub pid: Option<u32>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HealthResult {
    pub ok: bool,
    pub status: u16,
    pub url: String,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupInfo {
    pub id: String,
    pub path: String,
    pub created_at: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupResult {
    pub backup: BackupInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackResult {
    pub from_backup: String,
    pub auto_backup: BackupInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpgradeResult {
    pub old_version: String,
    pub new_version: String,
    pub rolled_back: bool,
    pub backup_id: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UninstallResult {
    pub stopped_process: bool,
    pub removed_paths: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SecuritySeverity {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityIssue {
    pub severity: SecuritySeverity,
    pub message: String,
    pub path: Option<String>,
    pub suggestion: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityResult {
    pub score: u8,
    pub issues: Vec<SecurityIssue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallerStatus {
    pub running: bool,
    pub pid: Option<u32>,
    pub version: String,
    pub provider: String,
    pub current_model: String,
    pub port: u16,
    pub health: HealthResult,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogSummary {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub modified_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillCatalogItem {
    pub name: String,
    pub description: String,
    pub eligible: bool,
    pub bundled: bool,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCatalogItem {
    pub key: String,
    pub provider: String,
    pub name: String,
    pub available: Option<bool>,
    pub missing: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallState {
    pub method: SourceMethod,
    pub install_dir: String,
    pub source_url: Option<String>,
    pub command_path: String,
    pub version: String,
    pub launch_args: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallLockInfo {
    pub installed: bool,
    pub install_dir: Option<String>,
    pub version: Option<String>,
    pub command_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenClawFileConfig {
    pub provider: String,
    pub model_chain: ModelChain,
    pub api_key: String,
    pub base_url: Option<String>,
    pub proxy: Option<String>,
    pub bind_address: String,
    pub port: u16,
    pub install_dir: String,
    pub launch_args: String,
    pub updated_at: String,
}
