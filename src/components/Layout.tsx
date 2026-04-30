import type { Language } from "../lib/types";
import { t } from "../lib/i18n";

interface LayoutProps {
  lang: Language;
  setLang: (lang: Language) => void;
  currentPage: string;
  onNavigate: (page: string) => void;
  statusText: string;
  statusKind: "running" | "stopped" | "unknown";
  version: string;
  model: string;
  navAvailability: Record<string, boolean>;
  children: React.ReactNode;
}

const pages = [
  { key: "welcome", label: "navWelcome" },
  { key: "wizard", label: "navWizard" },
  { key: "execute", label: "navExecute" },
  { key: "success", label: "navSuccess" },
  { key: "maintenance", label: "navMaintenance" }
] as const;

export function Layout({
  lang,
  setLang,
  currentPage,
  onNavigate,
  statusText,
  statusKind,
  version,
  model,
  navAvailability,
  children
}: LayoutProps) {
  return (
    <div className="shell">
      <aside className="sidebar glass">
        <div className="brand">
          <h1>{t(lang, "appTitle")}</h1>
          <p>{t(lang, "subtitle")}</p>
        </div>
        <nav className="nav" aria-label="Primary">
          {pages.map((item) => {
            const isActive = currentPage === item.key;
            const isAvailable = navAvailability[item.key] !== false;
            const className = `nav-item${isActive ? " active" : ""}${isAvailable ? "" : " disabled"}`;
            return (
              <button
                type="button"
                key={item.key}
                className={className}
                disabled={!isAvailable && !isActive}
                aria-current={isActive ? "page" : undefined}
                onClick={() => {
                  if (isAvailable || isActive) onNavigate(item.key);
                }}
              >
                {t(lang, item.label)}
              </button>
            );
          })}
        </nav>
      </aside>
      <main className="content-wrap">
        <header className="topbar glass">
          <div className={`status-pill status-${statusKind}`} role="status">
            <span className="status-dot" aria-hidden="true" />
            {statusText}
          </div>
          <div className="meta">
            <span>
              {t(lang, "version")}: <strong>{version || "-"}</strong>
            </span>
            <span>
              {t(lang, "currentModel")}: <strong>{model || "-"}</strong>
            </span>
          </div>
          <div className="lang-switch" role="group" aria-label={t(lang, "language")}>
            <span>{t(lang, "language")}</span>
            <button type="button" onClick={() => setLang("zh")} className={lang === "zh" ? "active" : ""}>
              中文
            </button>
            <button type="button" onClick={() => setLang("en")} className={lang === "en" ? "active" : ""}>
              EN
            </button>
          </div>
        </header>
        <section key={currentPage} className="content glass page-fade">
          {children}
        </section>
      </main>
    </div>
  );
}
