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
  setupTelegramPair,
  startProcess,
  endOpenClaw,
  switchModel,
  uninstallOpenClaw,
  updateProviderApiKey,
  upgrade
} from "../lib/api";
import { LinearProgress } from "../components/LinearProgress";
import { t } from "../lib/i18n";
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
const MODEL_RENDER_BATCH = 120;
const DONATE_USDT_ERC20_ADDRESS = "0x8dfe618c6278bbFc1518F27cc56EF569E59963c7";
const MODEL_LIST_CLI_COMMAND = "npx --yes openclaw --no-color models list --all --plain";
const MODEL_EXAMPLE_KEYS = "openai/gpt-4.1, moonshot/kimi-k2.5, openrouter/moonshotai/kimi-k2.5";

function normalizeProviderId(provider: string): string {
  const value = provider.trim().toLowerCase();
  if (value === "openai-codex") return "openai";
  if (value === "kimi-code") return "kimi-coding";
  return value;
}

function normalizeKnownModelKey(value: string): string {
  const trimmed = value.trim();
  if (!trimmed) return "";
  const lowered = trimmed.toLowerCase();
  if (lowered === "moonshot/kimi-2.5" || lowered === "moonshot/kimi2.5") {
    return "moonshot/kimi-k2.5";
  }
  return trimmed;
}

function parseModelKey(value: string): { provider: string; model: string } | null {
  const [provider, ...rest] = value.split("/");
  const model = rest.join("/").trim();
  if (!provider || !model) return null;
  return { provider: provider.trim(), model };
}

function composeModelKey(provider: string, modelInput: string): string {
  const normalizedProvider = normalizeProviderId(provider);
  const model = modelInput.trim();
  if (!model) return "";
  if (model.includes("/")) {
    return normalizeKnownModelKey(model);
  }
  return normalizeKnownModelKey(`${normalizedProvider || "openai"}/${model}`);
}

function modelNameFromKey(modelKey: string): string {
  const parsed = parseModelKey(modelKey);
  return parsed?.model ?? modelKey.trim();
}

function providerFromModel(modelKey: string): string {
  const idx = modelKey.indexOf("/");
  if (idx <= 0) return "unknown";
  return normalizeProviderId(modelKey.slice(0, idx));
}

function catalogFromConfig(cfg: OpenClawFileConfig | null): ModelCatalogItem[] {
  if (!cfg) return [];
  const keys = [cfg.model_chain.primary, ...(cfg.model_chain.fallbacks ?? [])]
    .map((item) => item.trim())
    .filter((item) => item.length > 0);
  const dedup = Array.from(new Set(keys));
  return dedup
    .map((key) => ({
      key,
      provider: providerFromModel(key),
      name: key,
      available: null,
      missing: false
    }))
    .sort((a, b) => a.key.localeCompare(b.key));
}

export function MaintenancePage({ lang, onStatusUpdate }: MaintenancePageProps) {
  const [status, setStatus] = useState<InstallerStatus | null>(null);
  const [currentConfig, setCurrentConfig] = useState<OpenClawFileConfig | null>(null);
  const [modelCatalog, setModelCatalog] = useState<ModelCatalogItem[]>([]);
  const [catalogLoading, setCatalogLoading] = useState(false);
  const [modelFilter, setModelFilter] = useState("");
  const [modelRenderCount, setModelRenderCount] = useState(MODEL_RENDER_BATCH);
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
  const [fallbackDraft, setFallbackDraft] = useState("");
  const [providerDrafts, setProviderDrafts] = useState<Record<string, string>>({});
  const [customProvider, setCustomProvider] = useState("");
  const [customProviderKey, setCustomProviderKey] = useState("");
  const [telegramPairCode, setTelegramPairCode] = useState("");
  const [donateCopied, setDonateCopied] = useState(false);
  const [modelCliCopied, setModelCliCopied] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [uninstallProgress, setUninstallProgress] = useState(0);
  const [uninstallStage, setUninstallStage] = useState("");
  const [uninstallRunning, setUninstallRunning] = useState(false);
  const uninstallTimerRef = useRef<number | null>(null);

  const modelProviderOptions = useMemo(() => {
    const set = new Set<string>();
    modelCatalog.forEach((item) => {
      const normalized = normalizeProviderId(item.provider);
      if (normalized) {
        set.add(normalized);
      }
    });
    const current = normalizeProviderId(parseModelKey(modelPrimary)?.provider ?? "");
    if (current) {
      set.add(current);
    }
    if (currentConfig?.model_chain.primary) {
      set.add(providerFromModel(currentConfig.model_chain.primary));
    }
    return Array.from(set).filter((v) => v && v !== "unknown").sort();
  }, [currentConfig?.model_chain.primary, modelCatalog, modelPrimary]);

  const selectedModelProvider = useMemo(() => {
    const current = normalizeProviderId(parseModelKey(modelPrimary)?.provider ?? "");
    if (current) {
      return current;
    }
    return modelProviderOptions[0] ?? "openai";
  }, [modelPrimary, modelProviderOptions]);

  const providerScopedModels = useMemo(
    () => modelCatalog.filter((item) => normalizeProviderId(item.provider) === selectedModelProvider),
    [modelCatalog, selectedModelProvider]
  );

  const providerModelNames = useMemo(() => {
    const names = new Set<string>();
    providerScopedModels.forEach((item) => {
      const modelName = modelNameFromKey(item.key);
      if (modelName) {
        names.add(modelName);
      }
    });
    return Array.from(names).sort();
  }, [providerScopedModels]);

  const primaryModelInput = useMemo(() => {
    const raw = modelPrimary.trim();
    if (!raw) {
      return "";
    }
    const parsed = parseModelKey(raw);
    if (!parsed) {
      return raw;
    }
    if (normalizeProviderId(parsed.provider) === selectedModelProvider) {
      return parsed.model;
    }
    return raw;
  }, [modelPrimary, selectedModelProvider]);

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

  useEffect(() => {
    setModelRenderCount(MODEL_RENDER_BATCH);
  }, [modelCatalog, modelFilter, modelPrimary]);

  const visibleModels = useMemo(
    () => filteredModels.slice(0, modelRenderCount),
    [filteredModels, modelRenderCount]
  );
  const hasMoreModels = filteredModels.length > visibleModels.length;

  const providerList = useMemo(() => {
    const set = new Set<string>();
    modelCatalog.forEach((m) => set.add(normalizeProviderId(m.provider)));
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
      setModelCatalog(models);
    } catch (e) {
      setModelCatalog((prev) => {
        const fromConfig = catalogFromConfig(currentConfig);
        return fromConfig.length > 0 ? fromConfig : prev;
      });
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

  const onPrimaryProviderChange = (rawProvider: string) => {
    const normalized = normalizeProviderId(rawProvider);
    if (!normalized) {
      return;
    }
    const scoped = modelCatalog.filter((item) => normalizeProviderId(item.provider) === normalized);
    const keepCurrent = scoped.some((item) => item.key === modelPrimary);
    const fallbackModelName = parseModelKey(modelPrimary)?.model ?? "gpt-5.2";
    const nextPrimary = keepCurrent
      ? modelPrimary
      : (scoped[0]?.key ?? composeModelKey(normalized, fallbackModelName));
    setModelPrimary(nextPrimary);
    setFallbacks((prev) => prev.filter((item) => item !== nextPrimary));
  };

  const onPrimaryModelInput = (rawModel: string) => {
    const next = composeModelKey(selectedModelProvider, rawModel);
    setModelPrimary(next);
    setFallbacks((prev) => prev.filter((item) => item !== next));
  };

  const addFallbackFromDraft = () => {
    const next = composeModelKey(selectedModelProvider, fallbackDraft);
    if (!next || next === modelPrimary) {
      setFallbackDraft("");
      return;
    }
    setFallbacks((prev) => Array.from(new Set([...prev, next])).filter((item) => item !== modelPrimary));
    setFallbackDraft("");
  };

  const copyUsdtAddress = async () => {
    try {
      await navigator.clipboard.writeText(DONATE_USDT_ERC20_ADDRESS);
      setDonateCopied(true);
      window.setTimeout(() => setDonateCopied(false), 1400);
    } catch {
      setDonateCopied(false);
    }
  };

  const copyModelListCommand = async () => {
    try {
      await navigator.clipboard.writeText(MODEL_LIST_CLI_COMMAND);
      setModelCliCopied(true);
      window.setTimeout(() => setModelCliCopied(false), 1400);
    } catch {
      setModelCliCopied(false);
    }
  };

  const applyTelegramPairCode = async () => {
    const code = telegramPairCode.trim();
    if (!code) return;
    await runAction(
      "telegram-pair",
      async () => {
        await setupTelegramPair(code);
      },
      false
    );
  };

  const saveModelChain = async () => {
    await runAction(
      "switch-model",
      async () => {
        await switchModel(modelPrimary, fallbacks.filter((f) => f !== modelPrimary));
        await refreshAll();
      },
      false
    );
  };

  const upgradeAndRefresh = async () => {
    await runAction(
      "upgrade",
      async () => {
        await upgrade();
        await refreshAll();
      },
      false
    );
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
          <div className="donate-crypto-block">
            <strong>{t(lang, "donateUsdtTitle")}</strong>
            <p className="muted-inline">{t(lang, "donateUsdtHint")}</p>
            <div className="inline">
              <input value={DONATE_USDT_ERC20_ADDRESS} readOnly />
              <button type="button" className="secondary" onClick={copyUsdtAddress}>
                {donateCopied ? t(lang, "copied") : t(lang, "copyAddress")}
              </button>
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
            <button type="button" onClick={upgradeAndRefresh} disabled={loading}>
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
            <span>{t(lang, "provider")}</span>
            {modelCatalog.length > 0 ? (
              <select value={selectedModelProvider} onChange={(e) => onPrimaryProviderChange(e.target.value)}>
                {modelProviderOptions.length === 0 && (
                  <option value="">{t(lang, "noProviderModels")}</option>
                )}
                {modelProviderOptions.map((providerId) => (
                  <option key={`switch-provider-${providerId}`} value={providerId}>
                    {providerId}
                  </option>
                ))}
              </select>
            ) : (
              <input
                value={selectedModelProvider}
                onChange={(e) => onPrimaryProviderChange(e.target.value)}
                placeholder="openai / moonshot / xai"
              />
            )}
            <small>{modelCatalog.length > 0 ? t(lang, "providerSelectHint") : t(lang, "providerManualHint")}</small>
          </label>
          <label>
            <span>{t(lang, "primaryModel")}</span>
            <input
              value={primaryModelInput}
              onChange={(e) => onPrimaryModelInput(e.target.value)}
              placeholder={modelCatalog.length > 0 ? "gpt-5.2 / provider/model" : "provider/model"}
              list={modelCatalog.length > 0 && providerModelNames.length > 0 ? "maintenance-model-options" : undefined}
            />
            {modelCatalog.length > 0 && providerModelNames.length > 0 && (
              <datalist id="maintenance-model-options">
                {providerModelNames.map((modelName) => (
                  <option
                    key={`maintenance-model-${selectedModelProvider}-${modelName}`}
                    value={modelName}
                    label={`${selectedModelProvider}/${modelName}`}
                  />
                ))}
              </datalist>
            )}
            {modelCatalog.length > 0 && <small>{t(lang, "showingModels")}: {modelCatalog.length}</small>}
            <small>{t(lang, "modelInputHint")}</small>
            <small>
              {t(lang, "modelExamplesLabel")}: {MODEL_EXAMPLE_KEYS}
            </small>
            <div className="inline">
              <input value={MODEL_LIST_CLI_COMMAND} readOnly />
              <button type="button" className="secondary" onClick={copyModelListCommand}>
                {modelCliCopied ? t(lang, "copied") : t(lang, "copyCliCommand")}
              </button>
            </div>
            <small>{t(lang, "modelCliHint")}</small>
          </label>
          {modelCatalog.length > 0 && providerModelNames.length === 0 && (
            <div className="alert">
              {t(lang, "noProviderModels")} ({selectedModelProvider})
            </div>
          )}

          {modelCatalog.length > 0 ? (
            <>
              <label>
                <span>{t(lang, "fallbackModels")}</span>
                <input
                  value={modelFilter}
                  onChange={(e) => setModelFilter(e.target.value)}
                  placeholder={t(lang, "searchModel")}
                />
              </label>
              <div className="inline">
                <input
                  value={fallbackDraft}
                  onChange={(e) => setFallbackDraft(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") {
                      e.preventDefault();
                      addFallbackFromDraft();
                    }
                  }}
                  placeholder="model-id / provider/model"
                />
                <button
                  type="button"
                  className="secondary"
                  onClick={addFallbackFromDraft}
                  disabled={!fallbackDraft.trim()}
                >
                  {t(lang, "addModel")}
                </button>
              </div>
              <div className="model-select-list">
                {visibleModels.map((item) => (
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
              <div className="model-render-footer">
                <span className="muted-inline">
                  {t(lang, "showingModels")}: {visibleModels.length} / {filteredModels.length}
                </span>
                {hasMoreModels && (
                  <button
                    type="button"
                    className="secondary"
                    onClick={() => setModelRenderCount((prev) => prev + MODEL_RENDER_BATCH)}
                  >
                    {t(lang, "loadMoreModels")}
                  </button>
                )}
              </div>
            </>
          ) : (
            <label>
              <span>{t(lang, "fallbackModels")}</span>
              <input
                value={fallbacks.join(", ")}
                onChange={(e) => {
                  const next = Array.from(
                    new Set(
                      e.target.value
                        .split(",")
                        .map((item) => item.trim())
                        .filter((item) => item.length > 0 && item !== modelPrimary)
                    )
                  );
                  setFallbacks(next);
                }}
                placeholder="provider/model, provider/model"
              />
            </label>
          )}
          <button type="button" onClick={saveModelChain} disabled={loading || !modelPrimary}>
            {t(lang, "saveModel")}
          </button>
        </div>

        <div className="card">
          <h3>{t(lang, "telegramPairPostTitle")}</h3>
          <p className="muted-inline">{t(lang, "telegramPairPostHint")}</p>
          <label>
            <span>{t(lang, "telegramPairCode")}</span>
            <div className="inline">
              <input
                value={telegramPairCode}
                onChange={(e) => setTelegramPairCode(e.target.value)}
                placeholder="123456"
              />
              <button
                type="button"
                onClick={applyTelegramPairCode}
                disabled={loading || !telegramPairCode.trim()}
              >
                {t(lang, "applyTelegramPair")}
              </button>
            </div>
          </label>
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
