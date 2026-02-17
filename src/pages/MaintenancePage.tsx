import { useEffect, useMemo, useRef, useState } from "react";
import {
  backupNow,
  clearCache,
  clearSessions,
  exportLog,
  getCurrentConfig,
  getStatus,
  listBackups,
  listLogs,
  listModelCatalog,
  logsDirPath,
  donateWechatQr,
  openPath,
  openManagementUrl,
  readLog,
  reloadConfig,
  restartProcess,
  rollback,
  securityCheck,
  startProcess,
  endOpenClaw,
  switchModel,
  uninstallOpenClaw,
  updateProviderApiKey,
  upgrade
} from "../lib/api";
import { LinearProgress } from "../components/LinearProgress";
import { t } from "../lib/i18n";
import { FALLBACK_MODEL_CATALOG, mergeModelCatalog } from "../lib/modelCatalog";
import type {
  BackupInfo,
  InstallerStatus,
  Language,
  LogSummary,
  ModelCatalogItem,
  OpenClawFileConfig,
  SecurityResult
} from "../lib/types";

interface MaintenancePageProps {
  lang: Language;
  onStatusUpdate: (status: InstallerStatus) => void;
}

const KNOWN_PROVIDER_IDS = [
  "openai",
  "anthropic",
  "google",
  "moonshot",
  "kimi-coding",
  "xai",
  "openrouter",
  "azure",
  "zai",
  "xiaomi",
  "minimax"
];

function providerFromModel(modelKey: string): string {
  const idx = modelKey.indexOf("/");
  if (idx <= 0) return "unknown";
  const provider = modelKey.slice(0, idx).trim().toLowerCase();
  if (provider === "openai-codex") return "openai";
  if (provider === "kimi-code") return "kimi-coding";
  return provider;
}

export function MaintenancePage({ lang, onStatusUpdate }: MaintenancePageProps) {
  const [status, setStatus] = useState<InstallerStatus | null>(null);
  const [currentConfig, setCurrentConfig] = useState<OpenClawFileConfig | null>(null);
  const [modelCatalog, setModelCatalog] = useState<ModelCatalogItem[]>(() => FALLBACK_MODEL_CATALOG);
  const [catalogLoading, setCatalogLoading] = useState(false);
  const [modelFilter, setModelFilter] = useState("");
  const [donateQrSrc, setDonateQrSrc] = useState("");
  const [backups, setBackups] = useState<BackupInfo[]>([]);
  const [selectedBackup, setSelectedBackup] = useState("");
  const [logs, setLogs] = useState<LogSummary[]>([]);
  const [selectedLog, setSelectedLog] = useState("");
  const [logBody, setLogBody] = useState("");
  const [security, setSecurity] = useState<SecurityResult | null>(null);
  const [outputPath, setOutputPath] = useState("%USERPROFILE%\\Desktop\\openclaw-installer.log");
  const [logsDir, setLogsDir] = useState("");
  const [modelPrimary, setModelPrimary] = useState("");
  const [fallbacks, setFallbacks] = useState<string[]>([]);
  const [providerDrafts, setProviderDrafts] = useState<Record<string, string>>({});
  const [customProvider, setCustomProvider] = useState("");
  const [customProviderKey, setCustomProviderKey] = useState("");
  const [message, setMessage] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [uninstallProgress, setUninstallProgress] = useState(0);
  const [uninstallStage, setUninstallStage] = useState("");
  const [uninstallRunning, setUninstallRunning] = useState(false);
  const uninstallTimerRef = useRef<number | null>(null);

  const filteredModels = useMemo(() => {
    const keyword = modelFilter.trim().toLowerCase();
    const source = modelCatalog.filter((item) => item.key !== modelPrimary);
    if (!keyword) return source;
    return source.filter((item) =>
      item.key.toLowerCase().includes(keyword) ||
      item.name.toLowerCase().includes(keyword) ||
      item.provider.toLowerCase().includes(keyword)
    );
  }, [modelCatalog, modelFilter, modelPrimary]);

  const providerList = useMemo(() => {
    const set = new Set<string>();
    modelCatalog.forEach((m) => set.add(m.provider));
    KNOWN_PROVIDER_IDS.forEach((provider) => set.add(provider));
    if (currentConfig?.model_chain.primary) {
      set.add(providerFromModel(currentConfig.model_chain.primary));
    }
    currentConfig?.model_chain.fallbacks.forEach((model) => set.add(providerFromModel(model)));
    return Array.from(set).filter((v) => v && v !== "unknown").sort();
  }, [modelCatalog, currentConfig]);

  useEffect(() => {
    let alive = true;
    donateWechatQr()
      .then((dataUrl) => {
        if (alive) setDonateQrSrc(dataUrl);
      })
      .catch(() => {
        // Donation QR is optional; keep UI functional even if it fails.
        if (alive) setDonateQrSrc("");
      });
    return () => {
      alive = false;
    };
  }, []);

  const refresh = async () => {
    const [s, cfg, b, l] = await Promise.all([
      getStatus(),
      getCurrentConfig(),
      listBackups(),
      listLogs()
    ]);
    setStatus(s);
    setCurrentConfig(cfg);
    setBackups(b);
    setLogs(l);
    if (b.length > 0 && !selectedBackup) {
      setSelectedBackup(b[0].id);
    }
    if (l.length > 0 && !selectedLog) {
      setSelectedLog(l[0].name);
    }
    setModelPrimary(cfg.model_chain.primary || "");
    setFallbacks(cfg.model_chain.fallbacks || []);
    onStatusUpdate(s);
  };

  const loadModelCatalog = async () => {
    setCatalogLoading(true);
    try {
      const models = await listModelCatalog();
      setModelCatalog(mergeModelCatalog(models, FALLBACK_MODEL_CATALOG));
    } catch (e) {
      setModelCatalog(FALLBACK_MODEL_CATALOG);
      setMessage(`models failed: ${e instanceof Error ? e.message : String(e)}`);
    } finally {
      setCatalogLoading(false);
    }
  };

  const refreshAll = async () => {
    await Promise.all([refresh(), loadModelCatalog()]);
  };

  useEffect(() => {
    refreshAll().catch((e) => setMessage(String(e)));
    logsDirPath().then(setLogsDir).catch(() => undefined);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    return () => {
      if (uninstallTimerRef.current != null) {
        window.clearInterval(uninstallTimerRef.current);
      }
    };
  }, []);

  useEffect(() => {
    if (!selectedLog) return;
    readLog(selectedLog)
      .then((content) => setLogBody(content))
      .catch((e) => setMessage(String(e)));
  }, [selectedLog]);

  const runAction = async (
    label: string,
    action: () => Promise<unknown>,
    shouldRefresh = true
  ) => {
    try {
      setLoading(true);
      setMessage(`${label}...`);
      await action();
      if (shouldRefresh) {
        await refresh();
      }
      setMessage(`${label} OK`);
    } catch (e) {
      setMessage(`${label} failed: ${e instanceof Error ? e.message : String(e)}`);
    } finally {
      setLoading(false);
    }
  };

  const exportLogNow = async () => {
    if (!selectedLog) return;
    try {
      setLoading(true);
      setMessage("export-log...");
      const exported = await exportLog(selectedLog, outputPath);
      setOutputPath(exported);
      setMessage(`export-log OK: ${exported}`);
    } catch (e) {
      setMessage(`export-log failed: ${e instanceof Error ? e.message : String(e)}`);
    } finally {
      setLoading(false);
    }
  };

  const uninstallNow = async () => {
    if (!window.confirm(t(lang, "uninstallConfirm"))) {
      return;
    }
    if (uninstallTimerRef.current != null) {
      window.clearInterval(uninstallTimerRef.current);
    }
    try {
      setLoading(true);
      setUninstallRunning(true);
      setUninstallProgress(8);
      setUninstallStage(t(lang, "uninstallPreparing"));
      uninstallTimerRef.current = window.setInterval(() => {
        setUninstallProgress((prev) => Math.min(92, prev + 4));
      }, 260);
      setMessage("uninstall...");
      window.setTimeout(() => setUninstallStage(t(lang, "uninstallRunning")), 350);
      const result = await uninstallOpenClaw();
      setUninstallStage(t(lang, "uninstallFinishing"));
      await refreshAll();
      setUninstallProgress(100);
      setUninstallStage(t(lang, "uninstallCompleted"));
      const warningTail =
        result.warnings.length > 0 ? ` | warnings: ${result.warnings.join(" | ")}` : "";
      setMessage(`uninstall OK: removed=${result.removed_paths.length}${warningTail}`);
      window.setTimeout(() => {
        setUninstallRunning(false);
        setUninstallProgress(0);
        setUninstallStage("");
      }, 900);
    } catch (e) {
      setUninstallRunning(false);
      setUninstallProgress(0);
      setUninstallStage("");
      setMessage(`uninstall failed: ${e instanceof Error ? e.message : String(e)}`);
    } finally {
      if (uninstallTimerRef.current != null) {
        window.clearInterval(uninstallTimerRef.current);
        uninstallTimerRef.current = null;
      }
      setLoading(false);
    }
  };

  const toggleFallback = (key: string, checked: boolean) => {
    setFallbacks((prev) => {
      if (checked) {
        return Array.from(new Set([...prev, key])).filter((item) => item !== modelPrimary);
      }
      return prev.filter((item) => item !== key);
    });
  };

  const saveModelChain = async () => {
    await runAction("switch-model", () => switchModel(modelPrimary, fallbacks.filter((f) => f !== modelPrimary)));
  };

  const saveProviderKey = async (provider: string) => {
    const value = providerDrafts[provider] ?? "";
    await runAction(`save-key:${provider}`, () => updateProviderApiKey(provider, value));
    setProviderDrafts((prev) => ({ ...prev, [provider]: "" }));
  };

  const clearProviderKey = async (provider: string) => {
    await runAction(`clear-key:${provider}`, () => updateProviderApiKey(provider, ""));
    setProviderDrafts((prev) => ({ ...prev, [provider]: "" }));
  };

  const saveCustomProviderKey = async () => {
    const provider = customProvider.trim().toLowerCase();
    if (!provider) return;
    await runAction(`save-key:${provider}`, () => updateProviderApiKey(provider, customProviderKey));
    setCustomProviderKey("");
  };

  const clearCustomProviderKey = async () => {
    const provider = customProvider.trim().toLowerCase();
    if (!provider) return;
    await runAction(`clear-key:${provider}`, () => updateProviderApiKey(provider, ""));
    setCustomProviderKey("");
  };

  const managementUrl =
    status?.port != null ? `http://127.0.0.1:${status.port}/` : "http://127.0.0.1:28789/";

  const riskScore = useMemo(() => security?.score ?? 0, [security]);

  return (
    <div className="page">
      <h2>{t(lang, "maintenanceTitle")}</h2>
      <p className="lead">{t(lang, "maintenanceDesc")}</p>

      <div className="action-row">
        <button
          type="button"
          className="secondary"
          onClick={() => runAction("refresh", refreshAll, false)}
          disabled={loading}
        >
          {t(lang, "refreshStatus")}
        </button>
      </div>

      {message && <div className="alert">{message}</div>}

      <div className="card-grid">
        <div className="card">
          <h3>Status</h3>
          <p>PID: {status?.pid ?? "-"}</p>
          <p>
            {t(lang, "version")}: {status?.version || "-"}
          </p>
          <p>
            {t(lang, "currentModel")}: {status?.current_model || "-"}
          </p>
          <p>
            {t(lang, "health")}: {status?.health.ok ? "OK" : "FAIL"}
          </p>
          <button type="button" className="secondary" onClick={() => runAction("open-dashboard", () => openManagementUrl(managementUrl))}>
            {t(lang, "openDashboard")}
          </button>
        </div>

        <div className="card donate-card">
          <h3>{t(lang, "donateTitle")}</h3>
          <div className="donate-layout">
            {donateQrSrc ? (
              <img className="donate-qr" src={donateQrSrc} alt={t(lang, "donateQrAlt")} loading="lazy" />
            ) : (
              <div className="donate-qr donate-qr-placeholder" aria-hidden="true">
                QR
              </div>
            )}
            <div className="donate-copy">
              {t(lang, "donateText")}
            </div>
          </div>
        </div>

        <div className="card">
          <h3>{t(lang, "integrationLinks")}</h3>
          <ul className="link-list">
            <li>
              <a href="https://docs.openclaw.ai/channels/feishu" target="_blank" rel="noreferrer">
                {t(lang, "feishuDocs")}
              </a>
            </li>
            <li>
              <a href="https://docs.openclaw.ai/reference/channels" target="_blank" rel="noreferrer">
                {t(lang, "wecomDocs")}
              </a>
            </li>
          </ul>
        </div>

        <div className="card">
          <h3>{t(lang, "riskTips")}</h3>
          <p>{t(lang, "riskTipsText")}</p>
          <p>Security score: {riskScore}/100</p>
          <button
            type="button"
            onClick={() => runAction("security check", async () => setSecurity(await securityCheck()))}
            disabled={loading}
          >
            {t(lang, "securityCheck")}
          </button>
          {security?.issues.map((issue, index) => (
            <div key={`${issue.severity}-${index}`} className={`alert ${issue.severity === "high" ? "error" : "warn-box"}`}>
              <strong>{issue.severity.toUpperCase()}:</strong> {issue.message}
            </div>
          ))}
        </div>

        <div className="card">
          <h3>{t(lang, "commonFixes")}</h3>
          <div className="button-grid">
            <button type="button" onClick={() => runAction("start", startProcess)} disabled={loading}>
              {t(lang, "start")}
            </button>
            <button
              type="button"
              className="secondary"
              onClick={() => {
                if (!window.confirm(t(lang, "endOpenClawConfirm"))) return;
                runAction("end-openclaw", endOpenClaw);
              }}
              disabled={loading}
            >
              {t(lang, "endOpenClaw")}
            </button>
            <button type="button" onClick={() => runAction("restart", restartProcess)} disabled={loading}>
              {t(lang, "restart")}
            </button>
            <button type="button" onClick={() => runAction("reload", reloadConfig)} disabled={loading}>
              {t(lang, "reload")}
            </button>
            <button type="button" onClick={() => runAction("clear-cache", clearCache)} disabled={loading}>
              {t(lang, "clearCache")}
            </button>
            <button type="button" onClick={() => runAction("clear-session", clearSessions)} disabled={loading}>
              {t(lang, "clearSession")}
            </button>
          </div>
          <div className="alert warn-box">{t(lang, "uninstallHint")}</div>
          {uninstallRunning && (
            <LinearProgress
              label={t(lang, "uninstallProgress")}
              value={uninstallProgress}
              active
              hint={uninstallStage}
            />
          )}
          <button type="button" className="secondary" onClick={uninstallNow} disabled={loading}>
            {t(lang, "uninstallOpenClaw")}
          </button>
        </div>

        <div className="card">
          <h3>
            {t(lang, "backupNow")} / {t(lang, "rollback")}
          </h3>
          <div className="button-grid">
            <button type="button" onClick={() => runAction("backup", backupNow)} disabled={loading}>
              {t(lang, "backupNow")}
            </button>
            <button
              type="button"
              onClick={() => runAction("rollback", () => rollback(selectedBackup))}
              disabled={loading || !selectedBackup}
            >
              {t(lang, "rollback")}
            </button>
            <button type="button" onClick={() => runAction("upgrade", upgrade)} disabled={loading}>
              {t(lang, "upgrade")}
            </button>
          </div>
          <label>
            <span>{t(lang, "selectBackup")}</span>
            <select value={selectedBackup} onChange={(e) => setSelectedBackup(e.target.value)}>
              {backups.length === 0 && <option value="">{t(lang, "noBackups")}</option>}
              {backups.map((b) => (
                <option key={b.id} value={b.id}>
                  {b.id}
                </option>
              ))}
            </select>
          </label>
        </div>

        <div className="card wide-card">
          <h3>{t(lang, "switchModel")}</h3>
          {catalogLoading && (
            <LinearProgress
              label={t(lang, "loadingModels")}
              indeterminate
              hint={t(lang, "providerManualHint")}
            />
          )}
          <label>
            <span>{t(lang, "primaryModel")}</span>
            <select
              value={modelPrimary}
              onChange={(e) => {
                const next = e.target.value;
                setModelPrimary(next);
                setFallbacks((prev) => prev.filter((v) => v !== next));
              }}
            >
              {modelCatalog.map((item) => (
                <option key={`primary-${item.key}`} value={item.key}>
                  {item.key}
                </option>
              ))}
            </select>
          </label>

          <label>
            <span>{t(lang, "fallbackModels")}</span>
            <input
              value={modelFilter}
              onChange={(e) => setModelFilter(e.target.value)}
              placeholder={t(lang, "searchModel")}
            />
          </label>
          <div className="model-select-list">
            {filteredModels.map((item) => (
              <label className="check-item model-item" key={`fallback-${item.key}`}>
                <input
                  type="checkbox"
                  checked={fallbacks.includes(item.key)}
                  onChange={(e) => toggleFallback(item.key, e.target.checked)}
                />
                <div>
                  <strong>{item.key}</strong>
                  <div className="muted-inline">{item.name}</div>
                </div>
              </label>
            ))}
          </div>
          <button type="button" onClick={saveModelChain} disabled={loading || !modelPrimary}>
            {t(lang, "saveModel")}
          </button>
        </div>

        <div className="card wide-card">
          <h3>{t(lang, "providerKeysTitle")}</h3>
          <p className="muted-inline">{t(lang, "providerKeysHint")}</p>
          <div className="form-grid">
            {providerList.map((provider) => (
              <label key={`provider-row-${provider}`}>
                <span>{provider} {t(lang, "providerKeyFor")}</span>
                <div className="inline">
                  <input
                    type="password"
                    value={providerDrafts[provider] ?? ""}
                    onChange={(e) =>
                      setProviderDrafts((prev) => ({ ...prev, [provider]: e.target.value }))
                    }
                    placeholder="sk-..."
                  />
                  <button
                    type="button"
                    onClick={() => saveProviderKey(provider)}
                    disabled={loading || !(providerDrafts[provider] ?? "").trim()}
                  >
                    {t(lang, "saveKey")}
                  </button>
                  <button
                    type="button"
                    className="secondary"
                    onClick={() => clearProviderKey(provider)}
                    disabled={loading}
                  >
                    {t(lang, "clearKey")}
                  </button>
                </div>
              </label>
            ))}
            <label>
              <span>{t(lang, "customProvider")}</span>
              <div className="inline">
                <input
                  value={customProvider}
                  onChange={(e) => setCustomProvider(e.target.value)}
                  placeholder={t(lang, "customProviderPlaceholder")}
                />
                <input
                  type="password"
                  value={customProviderKey}
                  onChange={(e) => setCustomProviderKey(e.target.value)}
                  placeholder="sk-..."
                />
                <button
                  type="button"
                  onClick={saveCustomProviderKey}
                  disabled={loading || !customProvider.trim() || !customProviderKey.trim()}
                >
                  {t(lang, "saveKey")}
                </button>
                <button
                  type="button"
                  className="secondary"
                  onClick={clearCustomProviderKey}
                  disabled={loading || !customProvider.trim()}
                >
                  {t(lang, "clearKey")}
                </button>
              </div>
            </label>
          </div>
        </div>

        <div className="card log-card wide-card">
          <h3>{t(lang, "logs")}</h3>
          <label>
            <span>{t(lang, "logs")}</span>
            <select value={selectedLog} onChange={(e) => setSelectedLog(e.target.value)}>
              {logs.length === 0 && <option value="">{t(lang, "noLogs")}</option>}
              {logs.map((item) => (
                <option key={item.name} value={item.name}>
                  {item.name}
                </option>
              ))}
            </select>
          </label>
          <textarea value={logBody} readOnly rows={14} />
          <div className="inline">
            <input value={outputPath} onChange={(e) => setOutputPath(e.target.value)} />
            <button
              type="button"
              onClick={exportLogNow}
              disabled={loading || !selectedLog}
            >
              {t(lang, "exportLogs")}
            </button>
            {logsDir && (
              <button
                type="button"
                className="secondary"
                onClick={() => runAction("open-logs-dir", () => openPath(logsDir))}
                disabled={loading}
              >
                {t(lang, "openLogsDir")}
              </button>
            )}
            <button
              type="button"
              className="secondary"
              onClick={() => runAction("open-export-path", () => openPath(outputPath))}
              disabled={loading}
            >
              {t(lang, "openExportPath")}
            </button>
          </div>
          {logsDir && <div className="muted-inline">{t(lang, "logsPath")}: {logsDir}</div>}
        </div>
      </div>
    </div>
  );
}
