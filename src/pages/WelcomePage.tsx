import { useEffect, useRef } from "react";
import type { EnvCheckResult, Language } from "../lib/types";
import { t } from "../lib/i18n";

interface WelcomePageProps {
  lang: Language;
  checking: boolean;
  env: EnvCheckResult | null;
  error: string | null;
  onCheck: () => void;
  onNext: () => void;
}

export function WelcomePage({ lang, checking, env, error, onCheck, onNext }: WelcomePageProps) {
  const hasAutoCheckedRef = useRef(false);

  useEffect(() => {
    // Auto-run pre-flight on first mount so users land on a meaningful state.
    if (!hasAutoCheckedRef.current && !env && !checking && !error) {
      hasAutoCheckedRef.current = true;
      onCheck();
    }
  }, [env, checking, error, onCheck]);

  const allDepsOk = env ? env.dependencies.every((dep) => dep.found) : false;
  const portOk = env ? !env.port_status.in_use : false;
  const ready = !!env && env.network_ok && allDepsOk;

  return (
    <div className="page">
      <h2>{t(lang, "welcomeTitle")}</h2>
      <p className="lead">{t(lang, "welcomeDesc")}</p>

      <div className="action-row">
        <button type="button" onClick={onCheck} disabled={checking} className={env ? "secondary" : ""}>
          {checking ? `${t(lang, "checkNow")}...` : env ? t(lang, "retry") : t(lang, "checkNow")}
        </button>
        <button type="button" onClick={onNext} disabled={!env}>
          {t(lang, "next")}
        </button>
        {!env && !checking && !error && (
          <span className="muted-inline">{t(lang, "welcomePromptCheck")}</span>
        )}
        {checking && <span className="muted-inline">{t(lang, "welcomeChecking")}</span>}
      </div>

      {error && <div className="alert error">{error}</div>}

      {env && !error && (
        <div className={ready ? "alert" : "alert warn-box"}>
          {ready ? t(lang, "envOk") : t(lang, "envBlocked")}
        </div>
      )}

      {env && (
        <div className="card-grid">
          <div className="card">
            <h3>{t(lang, "checkSummary")}</h3>
            <p>{env.os}</p>
            <p className={env.is_admin ? "ok" : "warn"}>
              {env.is_admin ? t(lang, "adminYes") : t(lang, "adminNo")}
            </p>
            <p className={env.network_ok ? "ok" : "error-text"}>
              {env.network_ok ? t(lang, "networkYes") : t(lang, "networkNo")}
            </p>
            <p className="muted-inline">{env.network_detail}</p>
          </div>
          <div className="card">
            <h3>{t(lang, "dependencies")}</h3>
            <ul className="list">
              {env.dependencies.map((dep) => (
                <li key={dep.name}>
                  <span>{dep.name}</span>
                  <span className={dep.found ? "ok" : "warn"}>{dep.found ? "OK" : "Missing"}</span>
                </li>
              ))}
            </ul>
          </div>
          <div className="card">
            <h3>{t(lang, "portStatus")}</h3>
            <p>
              <strong>{env.port_status.port}</strong>
            </p>
            <p className={portOk ? "ok" : "warn"}>
              {portOk
                ? t(lang, "portAvailable")
                : `${t(lang, "portInUseBy")} ${env.port_status.process_name ?? "unknown"} (${t(lang, "pid")} ${env.port_status.pid ?? "?"})`}
            </p>
          </div>
        </div>
      )}
    </div>
  );
}
