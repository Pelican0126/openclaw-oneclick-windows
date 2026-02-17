import { useDeferredValue, useEffect, useMemo, useState, useTransition } from "react";
import { LinearProgress } from "../components/LinearProgress";
import type { Language, ModelCatalogItem, OpenClawConfigInput, SkillCatalogItem } from "../lib/types";
import { t } from "../lib/i18n";
import { listModelCatalog, listSkillCatalog } from "../lib/api";
import { FALLBACK_MODEL_CATALOG, mergeModelCatalog } from "../lib/modelCatalog";

interface WizardPageProps {
  lang: Language;
  initial: OpenClawConfigInput;
  onBack: () => void;
  onSubmit: (payload: OpenClawConfigInput) => void;
}

const WIZARD_STEPS = [
  "wizardStepBasic",
  "wizardStepModel",
  "wizardStepFeatures",
  "wizardStepAdvanced",
  "wizardStepConfirm"
] as const;
const MODEL_RENDER_BATCH = 40;

const KIMI_BASE_URL_BY_REGION: Record<OpenClawConfigInput["kimi_region"], string> = {
  cn: "https://api.moonshot.cn/v1",
  global: "https://api.moonshot.ai/v1"
};

function normalizeProviderId(provider: string): string {
  const value = provider.trim().toLowerCase();
  if (value === "openai-codex") return "openai";
  if (value === "kimi-code") return "kimi-coding";
  return value;
}

function isKimiProvider(provider: string): boolean {
  const normalized = normalizeProviderId(provider);
  return normalized === "moonshot" || normalized === "kimi-coding";
}

function normalizeKimiRegion(
  rawRegion: string | undefined,
  baseUrl: string | undefined
): OpenClawConfigInput["kimi_region"] {
  const value = (rawRegion ?? "").trim().toLowerCase();
  if (value === "cn" || value === "global") {
    return value;
  }
  const url = (baseUrl ?? "").trim().toLowerCase();
  if (url.includes("moonshot.ai")) return "global";
  if (url.includes("moonshot.cn")) return "cn";
  return "cn";
}

function kimiBaseUrlForRegion(region: OpenClawConfigInput["kimi_region"]): string {
  return KIMI_BASE_URL_BY_REGION[region] ?? KIMI_BASE_URL_BY_REGION.cn;
}

function parseModelKey(value: string): { provider: string; model: string } | null {
  const [provider, ...rest] = value.split("/");
  const model = rest.join("/").trim();
  if (!provider || !model) return null;
  return { provider: provider.trim(), model };
}

function normalizeKnownModelKey(value: string): string {
  const trimmed = value.trim();
  if (!trimmed) return "";
  const lowered = trimmed.toLowerCase();
  if (lowered === "moonshot/kimi-2.5" || lowered === "moonshot/kimi2.5") return "moonshot/kimi-k2.5";
  return trimmed;
}

function normalizeModelKey(raw: string, fallbackProvider: string): string {
  const value = raw.trim();
  if (!value) return `${fallbackProvider || "openai"}/gpt-5.2`;
  if (value.includes("/")) return normalizeKnownModelKey(value);
  return normalizeKnownModelKey(`${fallbackProvider || "openai"}/${value}`);
}

function migrateLegacyInput(input: OpenClawConfigInput): OpenClawConfigInput {
  const primary = normalizeModelKey(input.model_chain.primary, input.provider || "openai");
  const parsedPrimary = parseModelKey(primary);
  const provider = parsedPrimary?.provider ?? input.provider;
  const normalizedPrimary = normalizeProviderId(provider);
  const kimiRegion = normalizeKimiRegion(
    (input as Partial<OpenClawConfigInput>).kimi_region,
    input.base_url
  );
  const providerApiKeys = Object.entries(input.provider_api_keys ?? {}).reduce<Record<string, string>>(
    (acc, [providerKey, keyValue]) => {
      const normalized = normalizeProviderId(providerKey);
      if (keyValue?.trim()) {
        acc[normalized] = keyValue;
      }
      return acc;
    },
    {}
  );
  if (!providerApiKeys[normalizedPrimary] && input.api_key?.trim()) {
    providerApiKeys[normalizedPrimary] = input.api_key.trim();
  }
  return {
    ...input,
    provider,
    kimi_region: kimiRegion,
    base_url: isKimiProvider(normalizedPrimary) ? kimiBaseUrlForRegion(kimiRegion) : input.base_url,
    provider_api_keys: providerApiKeys,
    selected_skills: input.selected_skills?.length ? input.selected_skills : ["healthcheck", "skill-creator"],
    model_chain: {
      primary,
      // First install does not configure fallbacks; move this to maintenance page.
      fallbacks: []
    }
  };
}

function endpointUrl(bindAddress: string, port: number): string {
  const host = bindAddress.trim() === "0.0.0.0" || bindAddress.trim() === "::" ? "127.0.0.1" : bindAddress.trim();
  return `http://${host || "127.0.0.1"}:${port}/`;
}

function validateStep(stepIndex: number, form: OpenClawConfigInput, lang: Language): string {
  if (stepIndex === 0) {
    if (!form.install_dir.trim()) {
      return `${t(lang, "installDir")} is required.`;
    }
    // Hard safety guard: never allow the classic user profile state dir.
    const installDirLower = form.install_dir
      .trim()
      .toLowerCase()
      .replace(/\//g, "\\")
      .replace(/\\+$/, ""); // tolerate trailing slashes like "...\\.openclaw\\"
    const unsafeSuffixes = ["\\.openclaw", "\\.clawdbot", "\\.moldbot", "\\.moltbot"];
    const unsafeExact = unsafeSuffixes.map((suffix) => `%userprofile%${suffix}`);
    if (unsafeExact.includes(installDirLower) || unsafeSuffixes.some((suffix) => installDirLower.endsWith(suffix))) {
      return t(lang, "installDirUnsafe");
    }
    if (!form.bind_address.trim()) {
      return `${t(lang, "bindAddress")} is required.`;
    }
    if (!Number.isInteger(form.port) || form.port < 1 || form.port > 65535) {
      return `${t(lang, "port")} must be between 1 and 65535.`;
    }
  }
  if (stepIndex === 1) {
    const resolvedProvider = normalizeProviderId(
      parseModelKey(form.model_chain.primary)?.provider ?? form.provider
    );
    if (!resolvedProvider) {
      return `${t(lang, "provider")} is required.`;
    }
    if (!form.model_chain.primary.trim()) {
      return `${t(lang, "primaryModel")} is required.`;
    }
  }
  if (stepIndex === 2) {
    if (form.enable_feishu_channel) {
      if (!form.feishu_app_id.trim()) {
        return `${t(lang, "feishuAppId")} is required when Feishu is enabled.`;
      }
      if (!form.feishu_app_secret.trim()) {
        return `${t(lang, "feishuAppSecret")} is required when Feishu is enabled.`;
      }
    }
  }
  if (stepIndex === 3) {
    if (form.source_method === "binary" && !(form.source_url ?? "").trim()) {
      return `${t(lang, "sourceUrl")} is required when source is binary.`;
    }
    if (form.onboarding_mode === "remote" && !(form.remote_url ?? "").trim()) {
      return `${t(lang, "remoteUrl")} is required when mode is remote.`;
    }
  }
  return "";
}

export function WizardPage({ lang, initial, onBack, onSubmit }: WizardPageProps) {
  const [form, setForm] = useState<OpenClawConfigInput>(() => migrateLegacyInput(initial));
  const [showKey, setShowKey] = useState(false);
  const [error, setError] = useState("");
  const [stepIndex, setStepIndex] = useState(0);
  const [isSwitchingStep, startStepTransition] = useTransition();
  const [confirmChecked, setConfirmChecked] = useState(false);
  const [skillCatalog, setSkillCatalog] = useState<SkillCatalogItem[]>([]);
  const [skillsLoading, setSkillsLoading] = useState(false);
  const [skillsLoaded, setSkillsLoaded] = useState(false);
  const [skillsLoadError, setSkillsLoadError] = useState("");
  const [modelCatalog, setModelCatalog] = useState<ModelCatalogItem[]>(() => FALLBACK_MODEL_CATALOG);
  const [modelsLoading, setModelsLoading] = useState(false);
  const [modelsLoaded, setModelsLoaded] = useState(false);
  const [modelsLoadError, setModelsLoadError] = useState("");
  const [modelFilter, setModelFilter] = useState("");
  const [modelRenderCount, setModelRenderCount] = useState(MODEL_RENDER_BATCH);
  const deferredModelFilter = useDeferredValue(modelFilter);

  useEffect(() => {
    setConfirmChecked(false);
  }, [form]);

  useEffect(() => {
    // Load skills lazily only when entering the feature step to avoid first-render jank.
    if (stepIndex !== 2 || skillsLoaded || skillsLoading) {
      return;
    }

    let alive = true;
    setSkillsLoadError("");
    setSkillsLoading(true);
    listSkillCatalog()
      .then((items) => {
        if (!alive) return;
        setSkillCatalog(items);
        setForm((prev) => {
          const validNames = new Set(items.map((item) => item.name));
          const selected = (prev.selected_skills ?? []).filter((name) => validNames.has(name));
          return {
            ...prev,
            selected_skills: selected.length > 0 ? selected : prev.selected_skills ?? [],
          };
        });
      })
      .catch((loadError) => {
        if (!alive) return;
        const message = loadError instanceof Error ? loadError.message : String(loadError);
        setSkillsLoadError(message.trim());
        // Keep default selected skills when catalog cannot be loaded.
      })
      .finally(() => {
        if (!alive) return;
        setSkillsLoading(false);
        setSkillsLoaded(true);
      });

    return () => {
      alive = false;
    };
  }, [stepIndex, skillsLoaded, skillsLoading]);

  useEffect(() => {
    // Load models lazily only when entering the model step to keep step transitions responsive.
    if (stepIndex !== 1 || modelsLoaded || modelsLoading) {
      return;
    }

    let alive = true;
    setModelsLoadError("");
    setModelsLoading(true);
    listModelCatalog()
      .then((items) => {
        if (!alive) return;
        const sorted = mergeModelCatalog(items, FALLBACK_MODEL_CATALOG);
        setModelCatalog(sorted);
        if (sorted.length === 0) return;
        startStepTransition(() => {
          setForm((prev) => {
            const currentProvider = normalizeProviderId(
              parseModelKey(prev.model_chain.primary)?.provider ?? prev.provider
            );
            const providerOptions = Array.from(
              new Set(
                sorted
                  .map((item) => normalizeProviderId(item.provider))
                  .filter((provider) => provider.length > 0)
              )
            ).sort();
            const effectiveProvider = currentProvider || providerOptions[0] || "openai";
            const providerModels = sorted.filter(
              (item) => normalizeProviderId(item.provider) === effectiveProvider
            );
            const primaryExists = sorted.some((item) => item.key === prev.model_chain.primary);
            const nextPrimary = primaryExists
              ? prev.model_chain.primary
              : (providerModels[0]?.key ?? sorted[0]?.key ?? "");
            if (!nextPrimary) {
              return prev;
            }
            const parsedPrimary = parseModelKey(nextPrimary);
            const nextProvider = normalizeProviderId(parsedPrimary?.provider ?? effectiveProvider);
            const nextIsKimi = isKimiProvider(nextProvider);
            return {
              ...prev,
              provider: nextProvider || prev.provider,
              api_key: nextProvider
                ? (prev.provider_api_keys?.[nextProvider] ?? prev.api_key)
                : prev.api_key,
              base_url: nextIsKimi ? kimiBaseUrlForRegion(prev.kimi_region) : prev.base_url,
              model_chain: { ...prev.model_chain, primary: nextPrimary, fallbacks: [] }
            };
          });
        });
      })
      .catch((loadError) => {
        if (!alive) return;
        const message = loadError instanceof Error ? loadError.message : String(loadError);
        setModelsLoadError(message.trim());
        setModelCatalog((prev) => mergeModelCatalog(prev, FALLBACK_MODEL_CATALOG));
        // Keep manual input path when model catalog cannot be fetched.
      })
      .finally(() => {
        if (!alive) return;
        setModelsLoading(false);
        setModelsLoaded(true);
      });

    return () => {
      alive = false;
    };
  }, [stepIndex, modelsLoaded, modelsLoading, startStepTransition]);

  const isLastStep = stepIndex === WIZARD_STEPS.length - 1;

  const providerOptions = useMemo(() => {
    const set = new Set<string>();
    modelCatalog.forEach((item) => {
      const normalized = normalizeProviderId(item.provider);
      if (normalized) {
        set.add(normalized);
      }
    });
    const explicit = normalizeProviderId(form.provider);
    if (explicit) {
      set.add(explicit);
    }
    const fromPrimary = normalizeProviderId(parseModelKey(form.model_chain.primary)?.provider ?? "");
    if (fromPrimary) {
      set.add(fromPrimary);
    }
    return Array.from(set).sort();
  }, [modelCatalog, form.provider, form.model_chain.primary]);

  const selectedProvider = useMemo(() => {
    const explicit = normalizeProviderId(form.provider);
    const fromPrimary = normalizeProviderId(parseModelKey(form.model_chain.primary)?.provider ?? "");
    if (modelCatalog.length === 0) {
      return explicit || fromPrimary;
    }
    if (explicit && providerOptions.includes(explicit)) {
      return explicit;
    }
    if (fromPrimary && providerOptions.includes(fromPrimary)) {
      return fromPrimary;
    }
    return providerOptions[0] ?? "";
  }, [form.provider, form.model_chain.primary, modelCatalog.length, providerOptions]);

  const providerModels = useMemo(() => {
    if (!selectedProvider) return [] as ModelCatalogItem[];
    return modelCatalog.filter((item) => normalizeProviderId(item.provider) === selectedProvider);
  }, [modelCatalog, selectedProvider]);

  const primaryProvider = useMemo(
    () => normalizeProviderId(parseModelKey(form.model_chain.primary)?.provider ?? selectedProvider ?? form.provider),
    [form.model_chain.primary, form.provider, selectedProvider]
  );
  const isPrimaryKimi = useMemo(() => isKimiProvider(primaryProvider), [primaryProvider]);

  const filteredModels = useMemo(() => {
    const keyword = deferredModelFilter.trim().toLowerCase();
    if (!keyword) return providerModels;
    return providerModels.filter((item) =>
      item.key.toLowerCase().includes(keyword) ||
      item.name.toLowerCase().includes(keyword) ||
      item.provider.toLowerCase().includes(keyword)
    );
  }, [providerModels, deferredModelFilter]);

  useEffect(() => {
    // Reset render window when source changes to avoid mounting large DOM trees at once.
    setModelRenderCount(MODEL_RENDER_BATCH);
  }, [stepIndex, selectedProvider, deferredModelFilter]);

  const visibleModels = useMemo(() => {
    if (stepIndex !== 1) {
      return [] as ModelCatalogItem[];
    }
    const base = filteredModels.slice(0, modelRenderCount);
    const selected = form.model_chain.primary.trim();
    if (!selected || base.some((item) => item.key === selected)) {
      return base;
    }
    const selectedItem = filteredModels.find((item) => item.key === selected);
    return selectedItem ? [selectedItem, ...base] : base;
  }, [filteredModels, form.model_chain.primary, modelRenderCount, stepIndex]);
  const hasMoreModels = filteredModels.length > visibleModels.length;

  const toggleSkill = (skillName: string, checked: boolean) => {
    setForm((prev) => {
      const current = new Set(prev.selected_skills ?? []);
      if (checked) {
        current.add(skillName);
      } else {
        current.delete(skillName);
      }
      return {
        ...prev,
        selected_skills: Array.from(current).sort(),
      };
    });
  };

  const onPrimaryPick = (value: string) => {
    const parsed = parseModelKey(value);
    const providerFromCatalog = modelCatalog.find((item) => item.key === value)?.provider;
    startStepTransition(() => {
      setForm((prev) => {
        const nextProvider = normalizeProviderId(providerFromCatalog ?? parsed?.provider ?? prev.provider);
        const nextIsKimi = isKimiProvider(nextProvider);
        return {
          ...prev,
          provider: nextProvider || prev.provider,
          api_key: nextProvider
            ? (prev.provider_api_keys?.[nextProvider] ?? prev.api_key)
            : prev.api_key,
          base_url: nextIsKimi ? kimiBaseUrlForRegion(prev.kimi_region) : prev.base_url,
          model_chain: {
            primary: value,
            fallbacks: []
          }
        };
      });
    });
  };

  const onProviderPick = (rawProvider: string) => {
    const normalized = normalizeProviderId(rawProvider);
    setModelFilter("");
    startStepTransition(() => {
      setForm((prev) => {
        const providerScopedModels = modelCatalog.filter(
          (item) => normalizeProviderId(item.provider) === normalized
        );
        const keepCurrent = providerScopedModels.some((item) => item.key === prev.model_chain.primary);
        const nextPrimary = keepCurrent ? prev.model_chain.primary : (providerScopedModels[0]?.key ?? "");
        const resolvedProvider = normalizeProviderId(
          parseModelKey(nextPrimary)?.provider ?? normalized
        );
        const nextIsKimi = isKimiProvider(resolvedProvider);
        return {
          ...prev,
          provider: resolvedProvider || normalized || prev.provider,
          api_key: resolvedProvider
            ? (prev.provider_api_keys?.[resolvedProvider] ?? prev.api_key)
            : prev.api_key,
          base_url: nextIsKimi ? kimiBaseUrlForRegion(prev.kimi_region) : prev.base_url,
          model_chain: {
            primary: nextPrimary,
            fallbacks: []
          }
        };
      });
    });
  };

  const updatePrimaryProviderKey = (value: string) => {
    setForm((prev) => {
      const targetProvider = primaryProvider || normalizeProviderId(prev.provider);
      if (!targetProvider) {
        return {
          ...prev,
          api_key: value
        };
      }
      return {
        ...prev,
        api_key: value,
        provider_api_keys: {
          ...(prev.provider_api_keys ?? {}),
          [targetProvider]: value
        }
      };
    });
  };

  const onPrimaryProviderInput = (rawProvider: string) => {
    const normalized = normalizeProviderId(rawProvider);
    setForm((prev) => {
      // Keep current model name segment and only switch provider prefix.
      const parsed = parseModelKey(prev.model_chain.primary);
      const modelName = (parsed?.model ?? "gpt-5.2").trim() || "gpt-5.2";
      if (!normalized) {
        return {
          ...prev,
          provider: rawProvider
        };
      }
      const nextIsKimi = isKimiProvider(normalized);
      return {
        ...prev,
        provider: normalized,
        api_key: prev.provider_api_keys?.[normalized] ?? prev.api_key,
        base_url: nextIsKimi ? kimiBaseUrlForRegion(prev.kimi_region) : prev.base_url,
        model_chain: {
          primary: `${normalized}/${modelName}`,
          fallbacks: []
        }
      };
    });
  };

  const gotoNext = () => {
    const message = validateStep(stepIndex, form, lang);
    if (message) {
      setError(message);
      return;
    }
    setError("");
    startStepTransition(() => {
      setStepIndex((prev) => Math.min(prev + 1, WIZARD_STEPS.length - 1));
    });
  };

  const gotoBack = () => {
    if (stepIndex === 0) {
      onBack();
      return;
    }
    setError("");
    startStepTransition(() => {
      setStepIndex((prev) => Math.max(prev - 1, 0));
    });
  };

  const submit = () => {
    for (let i = 0; i < WIZARD_STEPS.length - 1; i += 1) {
      const message = validateStep(i, form, lang);
      if (message) {
        setStepIndex(i);
        setError(message);
        return;
      }
    }
    if (!confirmChecked) {
      setError(t(lang, "wizardConfirmCheck"));
      return;
    }
    setError("");
    onSubmit({
      ...form,
      base_url: isPrimaryKimi ? kimiBaseUrlForRegion(form.kimi_region) : form.base_url,
      model_chain: {
        primary: form.model_chain.primary,
        fallbacks: []
      }
    });
  };

  const busyMaskText = useMemo(() => t(lang, "switchingStep"), [lang]);
  // Only block input while switching steps; catalog loading remains interactive.
  const showBusyMask = isSwitchingStep;

  const summary = [
    { label: t(lang, "installDir"), value: form.install_dir },
    { label: t(lang, "wizardEndpoint"), value: endpointUrl(form.bind_address, form.port) },
    { label: t(lang, "sourceMethod"), value: form.source_method },
    { label: t(lang, "onboardingMode"), value: form.onboarding_mode },
    { label: t(lang, "onboardingFlow"), value: form.onboarding_flow },
    { label: t(lang, "nodeManager"), value: form.node_manager },
    { label: t(lang, "installDaemon"), value: form.install_daemon ? t(lang, "wizardSelected") : t(lang, "wizardNotSelected") },
    { label: t(lang, "skipChannels"), value: form.skip_channels ? t(lang, "wizardSelected") : t(lang, "wizardNotSelected") },
    { label: t(lang, "skipSkills"), value: form.skip_skills ? t(lang, "wizardSelected") : t(lang, "wizardNotSelected") },
    { label: t(lang, "skipHealth"), value: form.skip_health ? t(lang, "wizardSelected") : t(lang, "wizardNotSelected") },
    { label: t(lang, "remoteUrl"), value: form.onboarding_mode === "remote" ? form.remote_url || "-" : "-" },
    { label: t(lang, "launchArgs"), value: form.launch_args || "gateway" },
    { label: t(lang, "primaryModel"), value: form.model_chain.primary },
    { label: t(lang, "fallbackModels"), value: "-" },
    {
      label: t(lang, "kimiRegion"),
      value: isPrimaryKimi
        ? t(lang, form.kimi_region === "global" ? "kimiRegionGlobal" : "kimiRegionCn")
        : "-"
    },
    { label: t(lang, "configuredProviders"), value: primaryProvider || "-" },
    { label: t(lang, "selectableSkills"), value: (form.selected_skills ?? []).join(", ") || "-" },
    {
      label: t(lang, "skillsScan"),
      value: form.enable_skills_scan ? t(lang, "wizardSelected") : t(lang, "wizardNotSelected")
    },
    {
      label: t(lang, "sessionMemoryHook"),
      value: form.enable_session_memory_hook ? t(lang, "wizardSelected") : t(lang, "wizardNotSelected")
    },
    {
      label: t(lang, "workspaceMemory"),
      value: form.enable_workspace_memory ? t(lang, "wizardSelected") : t(lang, "wizardNotSelected")
    },
    {
      label: t(lang, "feishuEnable"),
      value: form.enable_feishu_channel
        ? t(lang, "wizardSelected")
        : t(lang, "wizardNotSelected")
    },
    {
      label: t(lang, "autoOpenDashboard"),
      value: form.auto_open_dashboard ? t(lang, "wizardSelected") : t(lang, "wizardNotSelected")
    }
  ];

  return (
    <div className="page page-busy-wrap">
      <h2>{t(lang, "wizardTitle")}</h2>
      <p className="lead">{t(lang, "wizardDesc")}</p>

      <div className="wizard-stepper">
        {WIZARD_STEPS.map((stepKey, index) => (
          <button
            key={stepKey}
            type="button"
            className={
              index === stepIndex
                ? "wizard-step-chip active"
                : index < stepIndex
                  ? "wizard-step-chip done"
                  : "wizard-step-chip"
            }
            onClick={() => {
              if (index <= stepIndex) {
                startStepTransition(() => {
                  setStepIndex(index);
                  setError("");
                });
              }
            }}
          >
            {index + 1}. {t(lang, stepKey)}
          </button>
        ))}
      </div>

      {stepIndex === 0 && (
        <div className="form-grid">
          <label>
            <span>{t(lang, "installDir")}</span>
            <input value={form.install_dir} onChange={(e) => setForm({ ...form, install_dir: e.target.value })} />
          </label>
          <label>
            <span>{t(lang, "bindAddress")}</span>
            <input value={form.bind_address} onChange={(e) => setForm({ ...form, bind_address: e.target.value })} />
          </label>
          <label>
            <span>{t(lang, "port")}</span>
            <input
              type="number"
              min={1}
              max={65535}
              value={form.port}
              onChange={(e) => setForm({ ...form, port: Number(e.target.value) || 28789 })}
            />
          </label>
          <label>
            <span>{t(lang, "wizardEndpoint")}</span>
            <input value={endpointUrl(form.bind_address, form.port)} readOnly />
          </label>
        </div>
      )}

      {stepIndex === 1 && (
        <div className="form-grid">
          <label>
            <span>{t(lang, "provider")}</span>
            {modelCatalog.length > 0 ? (
              <select value={selectedProvider} onChange={(e) => onProviderPick(e.target.value)}>
                {providerOptions.length === 0 && (
                  <option value="">{t(lang, "noProviderModels")}</option>
                )}
                {providerOptions.map((providerId) => (
                  <option key={`provider-${providerId}`} value={providerId}>
                    {providerId}
                  </option>
                ))}
              </select>
            ) : (
              <input
                value={form.provider}
                onChange={(e) => onPrimaryProviderInput(e.target.value)}
                placeholder="openai / moonshot / xai"
              />
            )}
            <small>{modelCatalog.length > 0 ? t(lang, "providerSelectHint") : t(lang, "providerManualHint")}</small>
          </label>

          <label className="wide">
            <span>{t(lang, "primaryModel")}</span>
            <input
              value={modelFilter}
              onChange={(e) => setModelFilter(e.target.value)}
              placeholder={t(lang, "searchModel")}
            />
          </label>

          {modelsLoading && (
            <LinearProgress
              className="wide"
              label={t(lang, "loadingModels")}
              indeterminate
              hint={t(lang, "providerManualHint")}
            />
          )}
          {modelsLoadError && (
            <div className="alert wide">
              {t(lang, "noModelCatalog")} ({modelsLoadError})
            </div>
          )}

          {modelCatalog.length > 0 && visibleModels.length > 0 && (
            <div className="model-select-list wide">
              {visibleModels.map((item) => (
                <label className="check-item model-item" key={`model-${item.key}`}>
                  <input
                    type="radio"
                    name="primary-model-radio"
                    checked={form.model_chain.primary === item.key}
                    onChange={() => onPrimaryPick(item.key)}
                  />
                  <div>
                    <strong>{item.key}</strong>
                    <div className="muted-inline">{item.name}</div>
                    <div className={item.available === false || item.missing ? "warn" : "ok"}>
                      {item.available === false || item.missing ? t(lang, "skillNotReady") : t(lang, "skillReady")}
                    </div>
                  </div>
                </label>
              ))}
            </div>
          )}

          {modelCatalog.length > 0 && (
            <div className="wide model-render-footer">
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
          )}

          {modelCatalog.length > 0 && filteredModels.length === 0 && (
            <div className="alert wide">
              {t(lang, "noProviderModels")} {selectedProvider ? `(${selectedProvider})` : ""}
            </div>
          )}

          {modelCatalog.length === 0 && (
            <label className="wide">
              <span>{t(lang, "primaryModel")}</span>
              <input
                value={form.model_chain.primary}
                onChange={(e) => onPrimaryPick(e.target.value)}
                placeholder="provider/model"
              />
              <small>{t(lang, "noModelCatalog")}</small>
            </label>
          )}

          <label className="wide">
            <span>{(primaryProvider || selectedProvider || t(lang, "provider"))} {t(lang, "providerKeyFor")}</span>
            <div className="inline">
              <input
                type={showKey ? "text" : "password"}
                value={
                  (primaryProvider || selectedProvider)
                    ? (form.provider_api_keys?.[primaryProvider || selectedProvider] ?? form.api_key)
                    : form.api_key
                }
                onChange={(e) => updatePrimaryProviderKey(e.target.value)}
              />
              <button type="button" className="secondary" onClick={() => setShowKey((v) => !v)}>
                {showKey ? t(lang, "hide") : t(lang, "show")}
              </button>
            </div>
            <small>{t(lang, "apiKeyOptional")}</small>
            <small>{t(lang, "maskWarning")}</small>
          </label>

          {isPrimaryKimi ? (
            <>
              <label>
                <span>{t(lang, "kimiRegion")}</span>
                <select
                  value={form.kimi_region}
                  onChange={(e) => {
                    const nextRegion = e.target.value as OpenClawConfigInput["kimi_region"];
                    setForm({
                      ...form,
                      kimi_region: nextRegion,
                      base_url: kimiBaseUrlForRegion(nextRegion)
                    });
                  }}
                >
                  <option value="cn">{t(lang, "kimiRegionCn")}</option>
                  <option value="global">{t(lang, "kimiRegionGlobal")}</option>
                </select>
                <small>{t(lang, "kimiRegionHint")}</small>
              </label>

              <label>
                <span>{t(lang, "baseUrl")}</span>
                <input value={kimiBaseUrlForRegion(form.kimi_region)} readOnly />
              </label>
            </>
          ) : (
            <label>
              <span>{t(lang, "baseUrl")}</span>
              <input value={form.base_url ?? ""} onChange={(e) => setForm({ ...form, base_url: e.target.value })} />
            </label>
          )}

          <label>
            <span>{t(lang, "proxy")}</span>
            <input value={form.proxy ?? ""} onChange={(e) => setForm({ ...form, proxy: e.target.value })} />
          </label>

          <div className="alert wide">{t(lang, "fallbackHint")}</div>
        </div>
      )}

      {stepIndex === 2 && (
        <div className="feature-box">
          <h3>{t(lang, "featureOptions")}</h3>
          <h3>{t(lang, "selectableSkills")}</h3>
          <small>{t(lang, "selectableSkillsHint")}</small>
          {skillsLoading && <p className="muted-inline">{t(lang, "loadingSkills")}</p>}
          {!skillsLoading && skillsLoadError && (
            <p className="warn">{t(lang, "noSkillCatalog")} ({skillsLoadError})</p>
          )}
          {!skillsLoading && skillCatalog.length === 0 && (
            <p className="muted-inline">{t(lang, "noSkillCatalog")}</p>
          )}
          {!skillsLoading && skillCatalog.length > 0 && (
            <div className="skill-select-list">
              {skillCatalog.map((skill) => (
                <label className="check-item skill-item" key={`skill-${skill.name}`}>
                  <input
                    type="checkbox"
                    checked={(form.selected_skills ?? []).includes(skill.name)}
                    onChange={(e) => toggleSkill(skill.name, e.target.checked)}
                  />
                  <div>
                    <strong>{skill.name}</strong>{" "}
                    <span className={skill.eligible ? "ok" : "warn"}>
                      {skill.eligible ? t(lang, "skillReady") : t(lang, "skillNotReady")}
                    </span>
                    {skill.description && <div className="muted-inline">{skill.description}</div>}
                  </div>
                </label>
              ))}
            </div>
          )}

          <label className="check-item">
            <input
              type="checkbox"
              checked={form.enable_skills_scan}
              onChange={(e) => setForm({ ...form, enable_skills_scan: e.target.checked })}
            />
            <span>{t(lang, "skillsScan")}</span>
          </label>
          <label className="check-item">
            <input
              type="checkbox"
              checked={form.enable_session_memory_hook}
              onChange={(e) => setForm({ ...form, enable_session_memory_hook: e.target.checked })}
            />
            <span>{t(lang, "sessionMemoryHook")}</span>
          </label>
          <label className="check-item">
            <input
              type="checkbox"
              checked={form.enable_workspace_memory}
              onChange={(e) => setForm({ ...form, enable_workspace_memory: e.target.checked })}
            />
            <span>{t(lang, "workspaceMemory")}</span>
          </label>

          <h3>{t(lang, "feishuIntegration")}</h3>
          <label className="check-item">
            <input
              type="checkbox"
              checked={form.enable_feishu_channel}
              onChange={(e) => setForm({ ...form, enable_feishu_channel: e.target.checked })}
            />
            <span>{t(lang, "feishuEnable")}</span>
          </label>
          {form.enable_feishu_channel && (
            <div className="form-grid advanced-grid">
              <label>
                <span>{t(lang, "feishuAppId")}</span>
                <input
                  type="text"
                  value={form.feishu_app_id}
                  onChange={(e) => setForm({ ...form, feishu_app_id: e.target.value })}
                  placeholder="cli_xxx"
                />
              </label>
              <label>
                <span>{t(lang, "feishuAppSecret")}</span>
                <input
                  type="password"
                  value={form.feishu_app_secret}
                  onChange={(e) => setForm({ ...form, feishu_app_secret: e.target.value })}
                  placeholder="xxx"
                />
              </label>
              <label className="wide">
                <small>{t(lang, "feishuHint")}</small>
              </label>
            </div>
          )}

          <label className="check-item">
            <input
              type="checkbox"
              checked={form.auto_open_dashboard}
              onChange={(e) => setForm({ ...form, auto_open_dashboard: e.target.checked })}
            />
            <span>{t(lang, "autoOpenDashboard")}</span>
          </label>
        </div>
      )}

      {stepIndex === 3 && (
        <div className="advanced wizard-advanced-block">
          <h3>{t(lang, "advanced")}</h3>
          <div className="form-grid advanced-grid">
            <label>
              <span>{t(lang, "onboardingMode")}</span>
              <select
                value={form.onboarding_mode}
                onChange={(e) =>
                  setForm({
                    ...form,
                    onboarding_mode: e.target.value as OpenClawConfigInput["onboarding_mode"]
                  })
                }
              >
                <option value="local">local</option>
                <option value="remote">remote</option>
              </select>
            </label>
            <label>
              <span>{t(lang, "onboardingFlow")}</span>
              <select
                value={form.onboarding_flow}
                onChange={(e) =>
                  setForm({
                    ...form,
                    onboarding_flow: e.target.value as OpenClawConfigInput["onboarding_flow"]
                  })
                }
              >
                <option value="quickstart">quickstart</option>
                <option value="advanced">advanced</option>
                <option value="manual">manual</option>
              </select>
            </label>
            <label>
              <span>{t(lang, "nodeManager")}</span>
              <select
                value={form.node_manager}
                onChange={(e) =>
                  setForm({
                    ...form,
                    node_manager: e.target.value as OpenClawConfigInput["node_manager"]
                  })
                }
              >
                <option value="npm">npm</option>
                <option value="pnpm">pnpm</option>
                <option value="bun">bun</option>
              </select>
            </label>
            <label>
              <span>{t(lang, "sourceMethod")}</span>
              <select
                value={form.source_method}
                onChange={(e) =>
                  setForm({
                    ...form,
                    source_method: e.target.value as OpenClawConfigInput["source_method"]
                  })
                }
              >
                <option value="npm">npm package</option>
                <option value="bun">bun package</option>
                <option value="git">git repo</option>
                <option value="binary">binary</option>
              </select>
            </label>
            <label>
              <span>{t(lang, "sourceUrl")}</span>
              <input value={form.source_url ?? ""} onChange={(e) => setForm({ ...form, source_url: e.target.value })} />
            </label>
            <label>
              <span>{t(lang, "launchArgs")}</span>
              <input value={form.launch_args} onChange={(e) => setForm({ ...form, launch_args: e.target.value })} />
            </label>
            {form.onboarding_mode === "remote" && (
              <>
                <label>
                  <span>{t(lang, "remoteUrl")}</span>
                  <input value={form.remote_url ?? ""} onChange={(e) => setForm({ ...form, remote_url: e.target.value })} />
                </label>
                <label>
                  <span>{t(lang, "remoteToken")}</span>
                  <input
                    type="password"
                    value={form.remote_token ?? ""}
                    onChange={(e) => setForm({ ...form, remote_token: e.target.value })}
                  />
                </label>
              </>
            )}
            <label className="check-item">
              <input
                type="checkbox"
                checked={form.install_daemon}
                onChange={(e) => setForm({ ...form, install_daemon: e.target.checked })}
              />
              <span>{t(lang, "installDaemon")}</span>
            </label>
            <label className="check-item">
              <input
                type="checkbox"
                checked={form.skip_channels}
                onChange={(e) => setForm({ ...form, skip_channels: e.target.checked })}
              />
              <span>{t(lang, "skipChannels")}</span>
            </label>
            <label className="check-item">
              <input
                type="checkbox"
                checked={form.skip_skills}
                onChange={(e) => setForm({ ...form, skip_skills: e.target.checked })}
              />
              <span>{t(lang, "skipSkills")}</span>
            </label>
            <label className="check-item">
              <input
                type="checkbox"
                checked={form.skip_health}
                onChange={(e) => setForm({ ...form, skip_health: e.target.checked })}
              />
              <span>{t(lang, "skipHealth")}</span>
            </label>
            <label className="wide">
              <small>{t(lang, "daemonHint")}</small>
            </label>
          </div>
        </div>
      )}

      {stepIndex === 4 && (
        <div className="card wizard-confirm-card">
          <h3>{t(lang, "wizardConfirmTitle")}</h3>
          <p className="lead">{t(lang, "wizardConfirmDesc")}</p>
          <ul className="summary-list">
            {summary.map((item) => (
              <li key={item.label}>
                <span className="summary-key">{item.label}</span>
                <span>{item.value}</span>
              </li>
            ))}
          </ul>
          <label className="check-item">
            <input
              type="checkbox"
              checked={confirmChecked}
              onChange={(e) => setConfirmChecked(e.target.checked)}
            />
            <span>{t(lang, "wizardConfirmCheck")}</span>
          </label>
        </div>
      )}

      {error && <div className="alert error">{error}</div>}
      {isSwitchingStep && <div className="alert">{t(lang, "switchingStep")}</div>}

      {showBusyMask && (
        <div className="busy-mask" aria-live="polite" aria-busy="true">
          <div className="busy-card">
            <span className="busy-spinner" />
            <span>{busyMaskText}</span>
          </div>
        </div>
      )}

      <div className="action-row">
        <button type="button" className="secondary" onClick={gotoBack}>
          {t(lang, "back")}
        </button>
        {!isLastStep && (
          <button type="button" onClick={gotoNext}>
            {t(lang, "next")}
          </button>
        )}
        {isLastStep && (
          <button type="button" onClick={submit} disabled={!confirmChecked}>
            {t(lang, "runInstall")}
          </button>
        )}
      </div>
    </div>
  );
}
