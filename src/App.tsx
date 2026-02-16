import { useEffect, useMemo, useState } from "react";
import { Layout } from "./components/Layout";
import { t } from "./lib/i18n";
import { checkEnv, getStatus, openManagementUrl as openManagementUrlApi } from "./lib/api";
import type { AppPage, EnvCheckResult, InstallerStatus, Language, OpenClawConfigInput } from "./lib/types";
import { WelcomePage } from "./pages/WelcomePage";
import { WizardPage } from "./pages/WizardPage";
import { ExecutePage } from "./pages/ExecutePage";
import { SuccessPage } from "./pages/SuccessPage";
import { MaintenancePage } from "./pages/MaintenancePage";

const defaultConfig: OpenClawConfigInput = {
  install_dir: "%USERPROFILE%\\.openclaw",
  provider: "openai",
  model_chain: {
    primary: "openai/gpt-5.2",
    fallbacks: []
  },
  api_key: "",
  provider_api_keys: {
    openai: "",
    anthropic: "",
    google: "",
    moonshot: "",
    "kimi-coding": "",
    xai: ""
  },
  selected_skills: ["healthcheck", "skill-creator"],
  base_url: "",
  proxy: "",
  port: 18789,
  bind_address: "127.0.0.1",
  source_method: "npm",
  source_url: "",
  launch_args: "gateway",
  onboarding_mode: "local",
  // `quickstart` often expects provider auth to be set up; `manual` is more resilient for first-run.
  onboarding_flow: "manual",
  // Keep first install stable on Windows; daemon can still be enabled in advanced options.
  install_daemon: false,
  node_manager: "npm",
  skip_channels: false,
  skip_skills: false,
  skip_health: true,
  remote_url: "",
  remote_token: "",
  enable_skills_scan: true,
  enable_session_memory_hook: true,
  enable_workspace_memory: true,
  kimi_region: "cn",
  enable_feishu_channel: false,
  feishu_app_id: "",
  feishu_app_secret: "",
  enable_telegram_channel: false,
  telegram_bot_token: "",
  telegram_pair_code: "",
  auto_open_dashboard: true
};

function App() {
  const [lang, setLang] = useState<Language>("zh");
  const [page, setPage] = useState<AppPage>("welcome");
  const [checking, setChecking] = useState(false);
  const [env, setEnv] = useState<EnvCheckResult | null>(null);
  const [envError, setEnvError] = useState<string | null>(null);
  const [payload, setPayload] = useState<OpenClawConfigInput>(defaultConfig);
  const [status, setStatus] = useState<InstallerStatus | null>(null);

  const managementUrl = useMemo(() => {
    const activePort = status?.port ?? payload.port;
    const host = payload.bind_address.trim() === "0.0.0.0" || payload.bind_address.trim() === "::"
      ? "127.0.0.1"
      : payload.bind_address.trim() || "127.0.0.1";
    return `http://${host}:${activePort}/`;
  }, [payload.bind_address, payload.port, status?.port]);

  const statusText = useMemo(() => {
    if (!status) return t(lang, "statusStopped");
    return status.running ? t(lang, "statusRunning") : t(lang, "statusStopped");
  }, [lang, status]);

  const runCheck = async () => {
    try {
      setChecking(true);
      setEnvError(null);
      const result = await checkEnv(payload.port);
      setEnv(result);
    } catch (e) {
      setEnvError(e instanceof Error ? e.message : String(e));
    } finally {
      setChecking(false);
    }
  };

  useEffect(() => {
    let active = true;
    const refresh = async () => {
      try {
        const next = await getStatus();
        if (active) {
          setStatus(next);
        }
      } catch {
        // Keep last status when backend is temporarily unavailable.
      }
    };
    refresh();
    const timer = window.setInterval(refresh, 3000);
    return () => {
      active = false;
      window.clearInterval(timer);
    };
  }, []);

  return (
    <Layout
      lang={lang}
      setLang={setLang}
      currentPage={page}
      onNavigate={(next) => setPage(next as AppPage)}
      statusText={statusText}
      version={status?.version || "-"}
      model={status?.current_model || "-"}
    >
      {page === "welcome" && (
        <WelcomePage
          lang={lang}
          checking={checking}
          env={env}
          error={envError}
          onCheck={runCheck}
          onNext={() => setPage("wizard")}
        />
      )}

      {page === "wizard" && (
        <WizardPage
          lang={lang}
          initial={payload}
          onBack={() => setPage("welcome")}
          onSubmit={(next) => {
            setPayload(next);
            setPage("execute");
          }}
        />
      )}

      {page === "execute" && (
        <ExecutePage
          lang={lang}
          payload={payload}
          onBack={() => setPage("wizard")}
          onSuccess={(nextStatus) => {
            setStatus(nextStatus);
            setPage("success");
          }}
        />
      )}

      {page === "success" && (
        <SuccessPage
          lang={lang}
          status={status}
          managementUrl={managementUrl}
          onOpenManagementUrl={() => {
            openManagementUrlApi(managementUrl).catch(() => undefined);
          }}
          onFinish={() => setPage("maintenance")}
        />
      )}

      {page === "maintenance" && (
        <MaintenancePage
          lang={lang}
          onStatusUpdate={(nextStatus) => setStatus(nextStatus)}
        />
      )}
    </Layout>
  );
}

export default App;
