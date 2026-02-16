export type Language = "zh" | "en";

export type AppPage = "welcome" | "wizard" | "execute" | "success" | "maintenance";

export type SourceMethod = "npm" | "bun" | "git" | "binary";

export interface ModelChain {
  primary: string;
  fallbacks: string[];
}

export interface OpenClawConfigInput {
  install_dir: string;
  provider: string;
  model_chain: ModelChain;
  api_key: string;
  provider_api_keys: Record<string, string>;
  selected_skills: string[];
  base_url?: string;
  proxy?: string;
  port: number;
  bind_address: string;
  source_method: SourceMethod;
  source_url?: string;
  launch_args: string;
  onboarding_mode: "local" | "remote";
  onboarding_flow: "quickstart" | "advanced" | "manual";
  install_daemon: boolean;
  node_manager: "npm" | "pnpm" | "bun";
  skip_channels: boolean;
  skip_skills: boolean;
  skip_health: boolean;
  remote_url?: string;
  remote_token?: string;
  enable_skills_scan: boolean;
  enable_session_memory_hook: boolean;
  enable_workspace_memory: boolean;
  kimi_region: "cn" | "global";
  enable_feishu_channel: boolean;
  feishu_app_id: string;
  feishu_app_secret: string;
  enable_telegram_channel: boolean;
  telegram_bot_token: string;
  telegram_pair_code: string;
  auto_open_dashboard: boolean;
}

export interface DependencyStatus {
  name: string;
  found: boolean;
  path?: string;
}

export interface PortStatus {
  port: number;
  in_use: boolean;
  pid?: number;
  process_name?: string;
}

export interface EnvCheckResult {
  os: string;
  is_windows: boolean;
  is_admin: boolean;
  network_ok: boolean;
  network_detail: string;
  dependencies: DependencyStatus[];
  port_status: PortStatus;
}

export interface InstallEnvResult {
  installed: string[];
  skipped: string[];
  warnings: string[];
}

export interface InstallResult {
  method: string;
  install_dir: string;
  version: string;
  command_path: string;
}

export interface InstallLockInfo {
  installed: boolean;
  install_dir?: string;
  version?: string;
  command_path?: string;
}

export interface ConfigureResult {
  config_path: string;
  warnings: string[];
}

export interface ProcessControlResult {
  running: boolean;
  pid?: number;
  message: string;
}

export interface HealthResult {
  ok: boolean;
  status: number;
  url: string;
  body: string;
}

export interface BackupInfo {
  id: string;
  path: string;
  created_at: string;
  size: number;
}

export interface BackupResult {
  backup: BackupInfo;
}

export interface RollbackResult {
  from_backup: string;
  auto_backup: BackupInfo;
}

export interface UpgradeResult {
  old_version: string;
  new_version: string;
  rolled_back: boolean;
  backup_id: string;
  message: string;
}

export interface UninstallResult {
  stopped_process: boolean;
  removed_paths: string[];
  warnings: string[];
}

export interface SecurityIssue {
  severity: "low" | "medium" | "high";
  message: string;
  path?: string;
  suggestion?: string;
}

export interface SecurityResult {
  score: number;
  issues: SecurityIssue[];
}

export interface InstallerStatus {
  running: boolean;
  pid?: number;
  version: string;
  provider: string;
  current_model: string;
  port: number;
  health: HealthResult;
}

export interface LogSummary {
  name: string;
  path: string;
  size: number;
  modified_at: string;
}

export interface SkillCatalogItem {
  name: string;
  description: string;
  eligible: boolean;
  bundled: boolean;
  source: string;
}

export interface ModelCatalogItem {
  key: string;
  provider: string;
  name: string;
  available?: boolean | null;
  missing: boolean;
}

export interface OpenClawFileConfig {
  provider: string;
  model_chain: ModelChain;
  api_key: string;
  base_url?: string;
  proxy?: string;
  bind_address: string;
  port: number;
  install_dir: string;
  launch_args: string;
  updated_at: string;
}
