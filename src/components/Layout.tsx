import type { Language } from "../lib/types";
import { t } from "../lib/i18n";

interface LayoutProps {
  lang: Language;
  setLang: (lang: Language) => void;
  currentPage: string;
  onNavigate: (page: string) => void;
  statusText: string;
  version: string;
  model: string;
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
  version,
  model,
  children
}: LayoutProps) {
  return (
    <div className="shell">
      <aside className="sidebar glass">
        <div className="brand">
          <h1>{t(lang, "appTitle")}</h1>
          <p>{t(lang, "subtitle")}</p>
        </div>
        <nav className="nav">
          {pages.map((item) => (
            <button
              type="button"
              key={item.key}
              className={currentPage === item.key ? "nav-item active" : "nav-item"}
              onClick={() => onNavigate(item.key)}
            >
              {t(lang, item.label)}
            </button>
          ))}
        </nav>
      </aside>
      <main className="content-wrap">
        <header className="topbar glass">
          <div className="status-pill">{statusText}</div>
          <div className="meta">
            <span>
              {t(lang, "version")}: {version || "-"}
            </span>
            <span>
              {t(lang, "currentModel")}: {model || "-"}
            </span>
          </div>
          <div className="lang-switch">
            <span>{t(lang, "language")}</span>
            <button type="button" onClick={() => setLang("zh")} className={lang === "zh" ? "active" : ""}>
              中文
            </button>
            <button type="button" onClick={() => setLang("en")} className={lang === "en" ? "active" : ""}>
              EN
            </button>
          </div>
        </header>
        <section className="content glass">{children}</section>
      </main>
    </div>
  );
}
