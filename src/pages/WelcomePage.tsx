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
  return (
    <div className="page">
      <h2>{t(lang, "welcomeTitle")}</h2>
      <p className="lead">{t(lang, "welcomeDesc")}</p>
      <div className="action-row">
        <button type="button" onClick={onCheck} disabled={checking}>
          {checking ? "..." : t(lang, "checkNow")}
        </button>
        <button type="button" onClick={onNext} disabled={!env} className="secondary">
          {t(lang, "next")}
        </button>
      </div>

      {error && <div className="alert error">{error}</div>}

      {env && (
        <div className="card-grid">
          <div className="card">
            <h3>{t(lang, "checkSummary")}</h3>
            <p>{env.os}</p>
            <p>{env.is_admin ? t(lang, "adminYes") : t(lang, "adminNo")}</p>
            <p>{env.network_ok ? t(lang, "networkYes") : t(lang, "networkNo")}</p>
            <p>{env.network_detail}</p>
          </div>
          <div className="card">
            <h3>Dependencies</h3>
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
            <h3>Port</h3>
            <p>{env.port_status.port}</p>
            <p>
              {env.port_status.in_use
                ? `In use by ${env.port_status.process_name ?? "unknown"} (PID ${env.port_status.pid ?? "?"})`
                : "Available"}
            </p>
          </div>
        </div>
      )}
    </div>
  );
}
