import { t } from "../lib/i18n";
import type { InstallerStatus, Language } from "../lib/types";

interface SuccessPageProps {
  lang: Language;
  status: InstallerStatus | null;
  managementUrl: string;
  onOpenManagementUrl: () => void;
  onFinish: () => void;
}

export function SuccessPage({ lang, status, managementUrl, onOpenManagementUrl, onFinish }: SuccessPageProps) {
  return (
    <div className="page">
      <h2>{t(lang, "successTitle")}</h2>
      <p className="lead">{t(lang, "successDesc")}</p>

      <div className="card-grid">
        <div className="card">
          <h3>{t(lang, "version")}</h3>
          <p>{status?.version ?? "-"}</p>
        </div>
        <div className="card">
          <h3>{t(lang, "currentModel")}</h3>
          <p>{status?.current_model ?? "-"}</p>
        </div>
        <div className="card">
          <h3>{t(lang, "health")}</h3>
          <p>{status?.health.ok ? `${status.health.status} OK` : `${status?.health.status ?? "-"} FAILED`}</p>
        </div>
      </div>

      <div className="action-row">
        <button type="button" className="secondary" onClick={onOpenManagementUrl}>
          {t(lang, "openDashboard")}
        </button>
        <span className="muted-inline">
          {t(lang, "managementUrl")}: {managementUrl}
        </span>
        <button type="button" onClick={onFinish}>
          {t(lang, "finish")}
        </button>
      </div>
    </div>
  );
}
