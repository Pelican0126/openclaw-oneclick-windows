import { invoke } from "@tauri-apps/api/core";
import type {
  BackupInfo,
  BackupResult,
  ConfigureResult,
  EnvCheckResult,
  HealthResult,
  InstallEnvResult,
  InstallLockInfo,
  InstallerStatus,
  InstallResult,
  LogSummary,
  ModelCatalogItem,
  OpenClawConfigInput,
  OpenClawFileConfig,
  ProcessControlResult,
  RollbackResult,
  SecurityResult,
  SkillCatalogItem,
  UninstallResult,
  UpgradeResult
} from "./types";

function withTimeout<T>(promise: Promise<T>, timeoutMs: number, timeoutMessage: string): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    const timer = window.setTimeout(() => {
      reject(new Error(timeoutMessage));
    }, timeoutMs);

    promise
      .then((value) => resolve(value))
      .catch((error: unknown) => reject(error))
      .finally(() => {
        window.clearTimeout(timer);
      });
  });
}

export const checkEnv = (port: number) => invoke<EnvCheckResult>("check_env", { port });
export const installEnv = (port: number) => invoke<InstallEnvResult>("install_env", { port });
export const releasePort = (port: number) => invoke<string>("release_port", { port });
export const getInstallLockInfo = () => invoke<InstallLockInfo>("get_install_lock_info");
export const installOpenClaw = (payload: OpenClawConfigInput) => invoke<InstallResult>("install_openclaw", { payload });
export const uninstallOpenClaw = () => invoke<UninstallResult>("uninstall_openclaw");
export const configure = (payload: OpenClawConfigInput) => invoke<ConfigureResult>("configure", { payload });
export const getCurrentConfig = () => invoke<OpenClawFileConfig>("get_current_config");
export const updateProviderApiKey = (provider: string, apiKey: string) =>
  invoke<string>("update_provider_api_key", { provider, apiKey });
export const startProcess = () => invoke<ProcessControlResult>("start");
export const stopProcess = () => invoke<ProcessControlResult>("stop");
export const endOpenClaw = () => invoke<ProcessControlResult>("end_openclaw");
export const restartProcess = () => invoke<ProcessControlResult>("restart");
export const healthCheck = (host: string, port: number) => invoke<HealthResult>("health_check", { host, port });
export const getStatus = () => invoke<InstallerStatus>("get_status");
export const backupNow = () => invoke<BackupResult>("backup");
export const listBackups = () => invoke<BackupInfo[]>("list_backups");
export const rollback = (backupId: string) => invoke<RollbackResult>("rollback", { backupId });
export const upgrade = () => invoke<UpgradeResult>("upgrade");
export const switchModel = (primary: string, fallbacks: string[]) => invoke<ConfigureResult>("switch_model", { primary, fallbacks });
export const securityCheck = () => invoke<SecurityResult>("security_check");
export const listLogs = () => invoke<LogSummary[]>("list_logs");
export const readLog = (name: string, maxLines = 400) => invoke<string>("read_log", { name, maxLines });
export const exportLog = (name: string, outputPath: string) => invoke<string>("export_log", { name, outputPath });
export const clearCache = () => invoke<string>("clear_cache");
export const clearSessions = () => invoke<string>("clear_sessions");
export const reloadConfig = () => invoke<string>("reload_config");
export const openManagementUrl = (url: string) => invoke<string>("open_management_url", { url });
export const openPath = (path: string) => invoke<string>("open_path", { path });
export const logsDirPath = () => invoke<string>("logs_dir_path");
export const donateWechatQr = () => invoke<string>("donate_wechat_qr");
export const listSkillCatalog = () =>
  withTimeout(
    invoke<SkillCatalogItem[]>("list_skill_catalog"),
    4_000,
    "list_skill_catalog timed out"
  );
export const listModelCatalog = () =>
  withTimeout(
    invoke<ModelCatalogItem[]>("list_model_catalog"),
    // Model catalog may need to spawn OpenClaw CLI / npx on first run, which can take time on Windows.
    // Keep the UI responsive with a loader, but don't time out too aggressively.
    15_000,
    "list_model_catalog timed out"
  );
export const setupTelegramPair = (pairCode: string) => invoke<string>("setup_telegram_pair", { pairCode });
