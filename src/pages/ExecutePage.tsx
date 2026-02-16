import { useEffect, useMemo, useRef, useState } from "react";
import {
  checkEnv,
  configure,
  getInstallLockInfo,
  getStatus,
  healthCheck,
  installEnv,
  installOpenClaw,
  listLogs,
  logsDirPath,
  openManagementUrl,
  openPath,
  readLog,
  releasePort,
  stopProcess,
  startProcess
} from "../lib/api";
import { LinearProgress } from "../components/LinearProgress";
import { t } from "../lib/i18n";
import type { InstallerStatus, Language, OpenClawConfigInput } from "../lib/types";

interface ExecutePageProps {
  lang: Language;
  payload: OpenClawConfigInput;
  onBack: () => void;
  onSuccess: (status: InstallerStatus) => void;
}

type StepState = "pending" | "running" | "done" | "failed";

interface StepItem {
  key: string;
  state: StepState;
  message: string;
}

const stepKeys = ["stepCheck", "stepInstallDeps", "stepInstallOpenClaw", "stepConfigure", "stepStart", "stepHealth"];

export function ExecutePage({ lang, payload, onBack, onSuccess }: ExecutePageProps) {
  const [steps, setSteps] = useState<StepItem[]>(stepKeys.map((k) => ({ key: k, state: "pending", message: "" })));
  const [uiLogs, setUiLogs] = useState<string[]>([]);
  const [backendLog, setBackendLog] = useState("");
  const [backendLogName, setBackendLogName] = useState("");
  const [runtimePayload, setRuntimePayload] = useState<OpenClawConfigInput>(payload);
  const [running, setRunning] = useState(false);
  const [started, setStarted] = useState(false);
  const [waitingNext, setWaitingNext] = useState(false);
  const [currentStep, setCurrentStep] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [alreadyInstalled, setAlreadyInstalled] = useState(false);
  const [logsDir, setLogsDir] = useState("");
  const cancelledRef = useRef(false);
  const payloadRef = useRef<OpenClawConfigInput>(payload);
  const lastBackendLogRef = useRef("");
  const canStart = !started && !alreadyInstalled;
  const canRetry = started && !!error;
  const canNext = started && waitingNext && !error;
  const totalSteps = steps.length || 1;
  const doneCount = steps.filter((item) => item.state === "done").length;
  const runningIndex = steps.findIndex((item) => item.state === "running");
  const progressRatio = !started
    ? 0
    : runningIndex >= 0
      ? Math.min(1, (doneCount + 0.45) / totalSteps)
      : doneCount / totalSteps;
  const progressPercent = Math.max(0, Math.min(100, Math.round(progressRatio * 100)));
  const activeStepLabel = !started
    ? t(lang, "stepCheck")
    : runningIndex >= 0
      ? t(lang, steps[runningIndex].key)
      : doneCount >= totalSteps
        ? t(lang, "done")
        : t(lang, steps[Math.min(currentStep, totalSteps - 1)].key);

  const logText = useMemo(() => {
    const chunks: string[] = [];
    if (uiLogs.length > 0) {
      chunks.push(`[UI]\n${uiLogs.join("\n")}`);
    }
    if (backendLog.trim()) {
      chunks.push(`[Backend ${backendLogName || "log"}]\n${backendLog.trim()}`);
    }
    return chunks.join("\n\n");
  }, [uiLogs, backendLog, backendLogName]);

  const appendUiLog = (line: string) => {
    setUiLogs((prev) => {
      const next = [...prev, `${new Date().toLocaleTimeString()} ${line}`];
      // Keep UI memory bounded to avoid render jank on long-running installs.
      return next.slice(-240);
    });
  };

  const setStep = (index: number, patch: Partial<StepItem>) => {
    setSteps((prev) => {
      const next = [...prev];
      next[index] = { ...next[index], ...patch };
      return next;
    });
  };

  const findAvailablePort = async (startPort: number, maxScan: number): Promise<number | null> => {
    for (let nextPort = startPort; nextPort < startPort + maxScan; nextPort += 1) {
      try {
        const env = await checkEnv(nextPort);
        if (!env.port_status.in_use) {
          return nextPort;
        }
      } catch {
        // Skip failed probes and continue scanning.
      }
    }
    return null;
  };

  const runStep = async (index: number) => {
    const currentPayload = payloadRef.current;
    setRunning(true);
    setWaitingNext(false);
    setError(null);
    setStep(index, { state: "running", message: "" });

    try {
      if (index === 0) {
        const env = await checkEnv(currentPayload.port);
        appendUiLog(`check_env: admin=${env.is_admin}, network=${env.network_ok}`);
        if (env.port_status.in_use) {
          appendUiLog(
            `port_conflict: port=${currentPayload.port}, pid=${env.port_status.pid ?? "unknown"}, process=${env.port_status.process_name ?? "unknown"}`
          );
          // If this is a re-install case and OpenClaw is already running, stop it automatically.
          let handled = false;
          try {
            const status = await getStatus();
            if (status.running && status.port === currentPayload.port) {
              const stopped = await stopProcess();
              appendUiLog(`auto_stop_running_openclaw: ${stopped.message}`);
              const recheck = await checkEnv(currentPayload.port);
              handled = !recheck.port_status.in_use;
              if (!handled) {
                appendUiLog(
                  `port_conflict_after_stop: port=${currentPayload.port}, pid=${recheck.port_status.pid ?? "unknown"}, process=${recheck.port_status.process_name ?? "unknown"}`
                );
              }
            }
          } catch (stopErr) {
            appendUiLog(
              `auto_stop_running_openclaw warning: ${stopErr instanceof Error ? stopErr.message : String(stopErr)}`
            );
          }
          if (!handled) {
            const processName = (env.port_status.process_name ?? "").toLowerCase();
            const canAutoRelease = processName.includes("node") || processName.includes("openclaw");
            if (canAutoRelease) {
              try {
                const released = await releasePort(currentPayload.port);
                appendUiLog(`release_port: ${released}`);
                const recheck = await checkEnv(currentPayload.port);
                handled = !recheck.port_status.in_use;
              } catch (releaseErr) {
                appendUiLog(
                  `release_port warning: ${releaseErr instanceof Error ? releaseErr.message : String(releaseErr)}`
                );
              }
            }
          }

          if (!handled) {
            // Last-resort auto-fix: use the next available local port.
            const nextPort = await findAvailablePort(currentPayload.port + 1, 20);
            if (nextPort != null) {
              const nextPayload = { ...currentPayload, port: nextPort };
              payloadRef.current = nextPayload;
              setRuntimePayload(nextPayload);
              appendUiLog(`auto_port_switch: ${currentPayload.port} -> ${nextPort}`);
              handled = true;
            }
          }

          if (!handled) {
            throw new Error(
              `Port ${currentPayload.port} is occupied by ${env.port_status.process_name ?? "unknown"} (pid=${env.port_status.pid ?? "unknown"}).`
            );
          }
        }
        setStep(index, { state: "done", message: t(lang, "done") });
      }

      if (index === 1) {
        const deps = await installEnv(payloadRef.current.port);
        appendUiLog(`install_env: installed=${deps.installed.join(",") || "none"}, warnings=${deps.warnings.join(" | ") || "none"}`);
        setStep(index, { state: "done", message: t(lang, "done") });
      }

      if (index === 2) {
        const install = await installOpenClaw(payloadRef.current);
        appendUiLog(`install_openclaw: version=${install.version}, command=${install.command_path}`);
        setStep(index, { state: "done", message: install.version });
      }

      if (index === 3) {
        const conf = await configure(payloadRef.current);
        appendUiLog(`configure: ${conf.config_path}`);
        if (conf.warnings.length > 0) {
          appendUiLog(`configure warnings: ${conf.warnings.join(" | ")}`);
        }
        setStep(index, { state: "done", message: t(lang, "done") });
      }

      if (index === 4) {
        const startedResult = await startProcess();
        appendUiLog(`start: ${startedResult.message}`);
        setStep(index, { state: "done", message: startedResult.pid ? `PID ${startedResult.pid}` : t(lang, "done") });
      }

      if (index === 5) {
        const effectivePayload = payloadRef.current;
        const health = await healthCheck(effectivePayload.bind_address, effectivePayload.port);
        appendUiLog(`health: ${health.ok} ${health.status} ${health.url}`);
        if (!health.ok) {
          throw new Error(`Health probe failed: ${health.status} ${health.body}`);
        }
        setStep(index, { state: "done", message: `${health.status}` });
        const host = effectivePayload.bind_address.trim() === "0.0.0.0" || effectivePayload.bind_address.trim() === "::"
          ? "127.0.0.1"
          : effectivePayload.bind_address.trim() || "127.0.0.1";
        const managementUrl = `http://${host}:${effectivePayload.port}/`;
        if (effectivePayload.auto_open_dashboard) {
          try {
            const opened = await openManagementUrl(managementUrl);
            appendUiLog(`open_management_url: ${opened}`);
          } catch (openErr) {
            appendUiLog(`open_management_url warning: ${openErr instanceof Error ? openErr.message : String(openErr)}`);
          }
        }
        setAlreadyInstalled(true);

        const status = await getStatus();
        if (!cancelledRef.current) {
          onSuccess(status);
        }
        setRunning(false);
        return;
      }

      const nextStep = index + 1;
      setCurrentStep(nextStep);
      setWaitingNext(true);
      setRunning(false);
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      appendUiLog(`ERROR: ${message}`);
      setError(message);
      setSteps((prev) => prev.map((item, i) => (i === index || item.state === "running" ? { ...item, state: "failed", message } : item)));
      setRunning(false);
      setWaitingNext(false);
    }
  };

  const startInstall = () => {
    if (alreadyInstalled) {
      appendUiLog("install blocked: OpenClaw is already installed. Uninstall first.");
      return;
    }
    payloadRef.current = payload;
    setRuntimePayload(payload);
    setStarted(true);
    setCurrentStep(0);
    setWaitingNext(false);
    setError(null);
    setUiLogs([]);
    setBackendLog("");
    setBackendLogName("");
    setSteps(stepKeys.map((k) => ({ key: k, state: "pending", message: "" })));
    appendUiLog(
      `features: skills_scan=${payload.enable_skills_scan}, selected_skills=${(payload.selected_skills ?? []).join(",") || "none"}, session_memory_hook=${payload.enable_session_memory_hook}, workspace_memory=${payload.enable_workspace_memory}, feishu=${payload.enable_feishu_channel}, kimi_region=${payload.kimi_region}`
    );
    runStep(0);
  };

  const continueNext = () => {
    if (running || !started) return;
    runStep(currentStep);
  };

  const retryCurrent = () => {
    if (!started || running) return;
    runStep(currentStep);
  };

  useEffect(() => {
    payloadRef.current = payload;
    setRuntimePayload(payload);
  }, [payload]);

  useEffect(() => {
    cancelledRef.current = false;
    getInstallLockInfo()
      .then((lockInfo) => {
        if (cancelledRef.current) return;
        if (lockInfo.installed) {
          setAlreadyInstalled(true);
          appendUiLog(
            `installed_lock: version=${lockInfo.version || "unknown"}, dir=${lockInfo.install_dir || "-"}`
          );
        }
      })
      .catch(() => undefined);
    logsDirPath()
      .then((dir) => {
        if (!cancelledRef.current) {
          setLogsDir(dir);
        }
      })
      .catch(() => undefined);
    return () => {
      cancelledRef.current = true;
    };
  }, []);

  useEffect(() => {
    if (!started) {
      return;
    }

    let active = true;
    const pullLogs = async () => {
      if (document.hidden) {
        return;
      }
      try {
        let name = backendLogName;
        if (!name) {
          const files = await listLogs();
          if (files.length > 0) {
            name = files[0].name;
            if (active) {
              setBackendLogName(name);
            }
          }
        }
        if (!name) {
          return;
        }
        const content = await readLog(name, 240);
        if (active && content !== lastBackendLogRef.current) {
          lastBackendLogRef.current = content;
          setBackendLog(content);
        }
      } catch {
        // Ignore polling failures to avoid interrupting the install flow.
      }
    };

    pullLogs();
    const pollMs = running ? 1300 : 2600;
    const timer = window.setInterval(pullLogs, pollMs);
    return () => {
      active = false;
      window.clearInterval(timer);
    };
  }, [started, backendLogName, running]);

  return (
    <div className="page">
      <h2>{t(lang, "executeTitle")}</h2>
      <p className="lead">{t(lang, "executeDesc")}</p>
      {!started && alreadyInstalled && (
        <div className="alert">
          <strong>{t(lang, "installAlreadyDone")}</strong>
          <div>{t(lang, "installAlreadyDoneDesc")}</div>
        </div>
      )}
      {started && runtimePayload.port !== payload.port && (
        <div className="alert warn-box">
          {t(lang, "autoPortSwitched")}: <strong>{payload.port}</strong> -&gt; <strong>{runtimePayload.port}</strong>
        </div>
      )}
      <LinearProgress
        label={t(lang, "installProgress")}
        value={progressPercent}
        active={runningIndex >= 0}
        hint={`${t(lang, "currentStep")}: ${activeStepLabel}`}
      />

      <div className="card-grid">
        <div className="card">
          <h3>Steps</h3>
          <ul className="list">
            {steps.map((step) => (
              <li key={step.key}>
                <span>{t(lang, step.key)}</span>
                <span className={step.state === "done" ? "ok" : step.state === "failed" ? "error-text" : "warn"}>
                  {step.state}
                </span>
              </li>
            ))}
          </ul>
          {error && <div className="alert error">{error}</div>}
          <div className="action-row">
            <button type="button" className="secondary" onClick={onBack} disabled={running}>
              {t(lang, "back")}
            </button>
            {canStart && (
              <button type="button" onClick={startInstall} disabled={running}>
                {t(lang, "startInstallNow")}
              </button>
            )}
            {canNext && (
              <button type="button" onClick={continueNext} disabled={running}>
                {t(lang, "nextStep")}
              </button>
            )}
            {canRetry && (
              <button type="button" onClick={retryCurrent} disabled={running}>
                {t(lang, "retry")}
              </button>
            )}
          </div>
        </div>
        <div className="card log-card">
          <h3>Logs</h3>
          <textarea value={logText} readOnly rows={20} />
          <div className="action-row">
            <button
              type="button"
              className="secondary"
              onClick={() => {
                navigator.clipboard.writeText(logText).catch(() => undefined);
              }}
            >
              Copy
            </button>
            {logsDir && (
              <button
                type="button"
                className="secondary"
                onClick={() => {
                  openPath(logsDir).catch(() => undefined);
                }}
              >
                {t(lang, "openLogsDir")}
              </button>
            )}
          </div>
          {logsDir && <div className="muted-inline">{t(lang, "logsPath")}: {logsDir}</div>}
        </div>
      </div>
    </div>
  );
}
