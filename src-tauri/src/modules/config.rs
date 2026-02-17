use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::Path;

use anyhow::{anyhow, Result};
use chrono::Local;
use serde_json::{json, Deserializer, Value};
use url::Url;
use uuid::Uuid;

use crate::models::{ConfigureResult, ModelChain, OpenClawConfigInput, OpenClawFileConfig};

use super::{logger, paths, shell, state_store};

const AUTH_MAPPED_PROVIDERS: &[&str] = &[
    "openai",
    "google",
    "moonshot",
    "kimi-coding",
    "xai",
    "anthropic",
    "openrouter",
    "zai",
    "xiaomi",
    "minimax",
];
const KIMI_REGION_CN: &str = "cn";
const KIMI_REGION_GLOBAL: &str = "global";
const KIMI_BASE_URL_CN: &str = "https://api.moonshot.cn/v1";
const KIMI_BASE_URL_GLOBAL: &str = "https://api.moonshot.ai/v1";

pub fn configure(payload: &OpenClawConfigInput) -> Result<ConfigureResult> {
    validate_payload(payload)?;
    // Normalize known legacy model ids so old configs don't keep breaking new installs.
    // (Example: "moonshot/kimi-2.5" -> "moonshot/kimi-k2.5")
    let mut payload = payload.clone();
    payload.model_chain.primary = normalize_known_model_key(payload.model_chain.primary.as_str());
    payload.model_chain.fallbacks = payload
        .model_chain
        .fallbacks
        .iter()
        .map(|item| normalize_known_model_key(item))
        .filter(|item| !item.trim().is_empty())
        .collect();
    // Bind all OpenClaw state/config writes to the chosen install directory so we never
    // mix with an existing `%USERPROFILE%\\.openclaw` installation.
    let install_dir = paths::normalize_path(&payload.install_dir)?;
    std::env::set_var(
        "OPENCLAW_INSTALLER_OPENCLAW_HOME",
        install_dir.to_string_lossy().to_string(),
    );
    paths::ensure_dirs()?;
    std::fs::create_dir_all(paths::openclaw_home())?;
    std::fs::create_dir_all(&install_dir)?;

    let mut warnings = Vec::<String>::new();

    run_onboard(&payload, &mut warnings)?;
    apply_provider_keys(&payload, &mut warnings)?;
    apply_model_chain(&payload.model_chain, &mut warnings)?;
    apply_kimi_region_base_url(&payload, &mut warnings)?;
    apply_feature_toggles(&payload, &mut warnings)?;
    apply_selected_skills(&payload, &mut warnings)?;
    apply_channel_integrations(&payload, &mut warnings)?;

    let config_path = paths::config_path();
    warnings.extend(set_windows_acl(&config_path));
    let env_path = paths::openclaw_home().join(".env");
    if env_path.exists() {
        warnings.extend(set_windows_acl(&env_path));
    }

    state_store::save_last_config(&payload)?;

    logger::info(&format!(
        "Configuration updated via OpenClaw CLI: {}",
        config_path.to_string_lossy()
    ));

    if warnings.is_empty() {
        warnings.push("No warnings".to_string());
    }

    Ok(ConfigureResult {
        config_path: config_path.to_string_lossy().to_string(),
        warnings,
    })
}

pub fn switch_model(primary: &str, fallbacks: &[String]) -> Result<ConfigureResult> {
    if primary.trim().is_empty() {
        return Err(anyhow!("Primary model cannot be empty"));
    }
    let primary = normalize_known_model_key(primary);
    let mut warnings = Vec::<String>::new();
    apply_model_chain(
        &ModelChain {
            primary: primary.clone(),
            fallbacks: normalize_fallbacks(fallbacks)
                .into_iter()
                .map(|item| normalize_known_model_key(item.as_str()))
                .filter(|item| !item.trim().is_empty())
                .collect(),
        },
        &mut warnings,
    )?;
    if let Ok(Some(mut last)) = state_store::load_last_config() {
        last.model_chain.primary = primary.clone();
        last.model_chain.fallbacks = normalize_fallbacks(fallbacks)
            .into_iter()
            .map(|item| normalize_known_model_key(item.as_str()))
            .filter(|item| !item.trim().is_empty())
            .collect();
        if let Some(provider) = provider_from_model_key(primary.as_str()) {
            last.provider = provider.to_string();
        }
        state_store::save_last_config(&last)?;
    }
    logger::info("Model chain switched from maintenance page.");
    Ok(ConfigureResult {
        config_path: paths::config_path().to_string_lossy().to_string(),
        warnings,
    })
}

pub fn update_provider_api_key(provider: &str, api_key: &str) -> Result<String> {
    let provider_id = normalize_auth_provider(provider);
    let Some(env_name) = provider_env_name(provider_id.as_str()) else {
        return Err(anyhow!(
            "Provider '{}' cannot be converted to a valid API key environment variable.",
            provider
        ));
    };
    let env_path = paths::openclaw_home().join(".env");
    if let Some(value) = optional_non_empty(Some(api_key.to_string())) {
        let mut updates = BTreeMap::<String, String>::new();
        updates.insert(env_name.clone(), sanitize_env_value(&value));
        upsert_env_file(&env_path, &updates)?;
    } else {
        remove_env_keys(&env_path, &[env_name])?;
    }

    if let Ok(Some(mut last)) = state_store::load_last_config() {
        if let Some(value) = optional_non_empty(Some(api_key.to_string())) {
            last.provider_api_keys
                .insert(provider_id.clone(), value.clone());
            if normalize_auth_provider(last.provider.as_str()) == provider_id {
                last.api_key = value;
            }
        } else {
            last.provider_api_keys.remove(provider_id.as_str());
            if normalize_auth_provider(last.provider.as_str()) == provider_id {
                last.api_key.clear();
            }
        }
        state_store::save_last_config(&last)?;
    }

    logger::info(&format!(
        "Provider API key updated for provider '{}' via maintenance.",
        provider_id
    ));
    Ok(format!("Updated key for provider '{provider_id}'"))
}

pub fn read_current_config() -> Result<OpenClawFileConfig> {
    let path = paths::config_path();
    if !path.exists() {
        return Err(anyhow!("Config file not found: {}", path.to_string_lossy()));
    }
    let raw = fs::read_to_string(&path)?;

    // Backward compatible: support legacy installer-written schema.
    if let Ok(cfg) = serde_json::from_str::<OpenClawFileConfig>(&raw) {
        return Ok(cfg);
    }

    let json: Value = serde_json::from_str(&raw)?;
    let last = state_store::load_last_config()?.unwrap_or_default();
    let install = state_store::load_install_state()?;

    let primary = json
        .pointer("/agents/defaults/model/primary")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| {
            json.pointer("/agents/defaults/model")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| last.model_chain.primary.clone());

    let fallbacks = json
        .pointer("/agents/defaults/model/fallbacks")
        .and_then(|v| v.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| last.model_chain.fallbacks.clone());

    let primary = normalize_known_model_key(primary.as_str());
    let fallbacks = fallbacks
        .into_iter()
        .map(|item| normalize_known_model_key(item.as_str()))
        .filter(|item| !item.trim().is_empty())
        .collect::<Vec<_>>();

    let provider = provider_from_model_key(&primary)
        .map(|s| s.to_string())
        .unwrap_or_else(|| last.provider.clone());

    let port = json
        .pointer("/gateway/port")
        .and_then(|v| v.as_u64())
        .map(|v| v as u16)
        .unwrap_or(last.port as u16);

    let bind_mode = json
        .pointer("/gateway/bind")
        .and_then(|v| v.as_str())
        .unwrap_or("loopback");
    let bind_address = match bind_mode {
        "lan" => "0.0.0.0".to_string(),
        "loopback" => "127.0.0.1".to_string(),
        _ => {
            if last.bind_address.trim().is_empty() {
                "127.0.0.1".to_string()
            } else {
                last.bind_address.clone()
            }
        }
    };
    let final_provider = if provider.trim().is_empty() {
        "unknown".to_string()
    } else {
        provider.clone()
    };
    let primary_api_key = provider_key_for_id(&last, normalize_auth_provider(&provider).as_str())
        .or_else(|| optional_non_empty(Some(last.api_key.clone())))
        .unwrap_or_default();

    let updated_at = json
        .pointer("/meta/lastTouchedAt")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| Local::now().to_rfc3339());

    Ok(OpenClawFileConfig {
        provider: final_provider,
        model_chain: ModelChain { primary, fallbacks },
        api_key: primary_api_key,
        base_url: optional_non_empty(last.base_url),
        proxy: optional_non_empty(last.proxy),
        bind_address,
        port: if port == 0 { 28789 } else { port },
        install_dir: install
            .map(|v| v.install_dir)
            .unwrap_or_else(|| paths::openclaw_home().to_string_lossy().to_string()),
        launch_args: if last.launch_args.trim().is_empty() {
            "gateway".to_string()
        } else {
            last.launch_args
        },
        updated_at,
    })
}

pub fn reload_config() -> Result<String> {
    let path = paths::config_path();
    if !path.exists() {
        return Err(anyhow!("Config file not found."));
    }
    logger::info("Reload config requested.");
    Ok("Configuration reloaded. If process is running, restart for full effect.".to_string())
}

fn run_onboard(payload: &OpenClawConfigInput, warnings: &mut Vec<String>) -> Result<()> {
    let flow = normalize_onboard_flow(&payload.onboarding_flow);
    let mode = normalize_onboard_mode(&payload.onboarding_mode);
    let node_manager = normalize_node_manager(&payload.node_manager);
    let gateway_token = existing_gateway_token().unwrap_or_else(|| generate_gateway_token(40));

    let mut args = vec![
        "onboard".to_string(),
        "--non-interactive".to_string(),
        "--accept-risk".to_string(),
        "--flow".to_string(),
        flow.to_string(),
        "--mode".to_string(),
        mode.to_string(),
        "--skip-ui".to_string(),
        "--gateway-port".to_string(),
        payload.port.to_string(),
        "--gateway-bind".to_string(),
        bind_address_to_mode(&payload.bind_address).to_string(),
        "--gateway-auth".to_string(),
        "token".to_string(),
        "--gateway-token".to_string(),
        gateway_token,
        "--workspace".to_string(),
        paths::openclaw_home()
            .join("workspace")
            .to_string_lossy()
            .to_string(),
        "--node-manager".to_string(),
        node_manager.to_string(),
    ];
    if payload.skip_channels {
        args.push("--skip-channels".to_string());
    }
    if payload.skip_skills {
        args.push("--skip-skills".to_string());
    }
    let mut effective_skip_health = payload.skip_health;
    if payload.install_daemon {
        if shell::is_admin() {
            args.push("--install-daemon".to_string());
        } else {
            warnings.push(
                "Install daemon requested but current process is not elevated; using --no-install-daemon."
                    .to_string(),
            );
            args.push("--no-install-daemon".to_string());
            if !effective_skip_health {
                warnings.push(
                    "Onboard health probe is skipped because daemon install was not allowed on this Windows session."
                        .to_string(),
                );
                effective_skip_health = true;
            }
        }
    } else {
        args.push("--no-install-daemon".to_string());
        if !effective_skip_health {
            warnings.push(
                "Onboard health probe is skipped because daemon install is disabled. A separate health_check step still runs in installer."
                    .to_string(),
            );
            effective_skip_health = true;
        }
    }
    if effective_skip_health {
        args.push("--skip-health".to_string());
    }
    if mode == "remote" {
        if let Some(url) = optional_non_empty(payload.remote_url.clone()) {
            args.push("--remote-url".to_string());
            args.push(url);
        }
        if let Some(token) = optional_non_empty(payload.remote_token.clone()) {
            args.push("--remote-token".to_string());
            args.push(token);
        }
    }

    let provider = resolve_provider(payload)?;
    let auth_provider = normalize_auth_provider(&provider);
    let primary_key = provider_key_for_payload(payload, auth_provider.as_str())
        .or_else(|| optional_non_empty(Some(payload.api_key.clone())));
    if primary_key.is_none() {
        args.push("--auth-choice".to_string());
        args.push("skip".to_string());
    } else if !AUTH_MAPPED_PROVIDERS.contains(&auth_provider.as_str()) {
        warnings.push(format!(
            "Provider '{}' is not mapped for non-interactive auth; skipped API key import.",
            provider
        ));
        args.push("--auth-choice".to_string());
        args.push("skip".to_string());
    } else {
        let primary_key = primary_key.unwrap_or_default();
        // Use provider-specific auth flags to let OpenClaw generate a valid config + env layout.
        match auth_provider.as_str() {
            "openai" => {
                args.extend([
                    "--auth-choice".to_string(),
                    "openai-api-key".to_string(),
                    "--openai-api-key".to_string(),
                    primary_key.clone(),
                ]);
            }
            "google" => {
                args.extend([
                    "--auth-choice".to_string(),
                    "gemini-api-key".to_string(),
                    "--gemini-api-key".to_string(),
                    primary_key.clone(),
                ]);
            }
            "moonshot" => {
                args.extend([
                    "--auth-choice".to_string(),
                    "moonshot-api-key".to_string(),
                    "--moonshot-api-key".to_string(),
                    primary_key.clone(),
                ]);
            }
            "kimi-coding" => {
                args.extend([
                    "--auth-choice".to_string(),
                    "kimi-code-api-key".to_string(),
                    "--kimi-code-api-key".to_string(),
                    primary_key.clone(),
                ]);
            }
            "xai" => {
                args.extend([
                    "--auth-choice".to_string(),
                    "xai-api-key".to_string(),
                    "--xai-api-key".to_string(),
                    primary_key.clone(),
                ]);
            }
            "anthropic" => {
                args.extend([
                    "--auth-choice".to_string(),
                    "anthropic-api-key".to_string(),
                    "--anthropic-api-key".to_string(),
                    primary_key.clone(),
                ]);
            }
            "openrouter" => {
                args.extend([
                    "--auth-choice".to_string(),
                    "openrouter-api-key".to_string(),
                    "--openrouter-api-key".to_string(),
                    primary_key.clone(),
                ]);
            }
            "zai" => {
                args.extend([
                    "--auth-choice".to_string(),
                    "zai-api-key".to_string(),
                    "--zai-api-key".to_string(),
                    primary_key.clone(),
                ]);
            }
            "xiaomi" => {
                args.extend([
                    "--auth-choice".to_string(),
                    "xiaomi-api-key".to_string(),
                    "--xiaomi-api-key".to_string(),
                    primary_key.clone(),
                ]);
            }
            "minimax" => {
                args.extend([
                    "--auth-choice".to_string(),
                    "minimax-api".to_string(),
                    "--minimax-api-key".to_string(),
                    primary_key.clone(),
                ]);
            }
            _ => {}
        }
    }

    let out = run_openclaw_cli(&args, payload.proxy.clone())?;
    if out.code == 0 {
        return Ok(());
    }

    let err_text = if out.stderr.is_empty() {
        out.stdout.clone()
    } else {
        out.stderr.clone()
    };
    if is_gateway_1006_error(&err_text) {
        warnings.push(
            "Onboard gateway probe failed (1006). Retrying with safer Windows flags.".to_string(),
        );
        logger::warn("Onboard failed with 1006, retrying with safe fallback flags.");
        let retry_args = force_safe_onboard_retry_args(&args);
        let retry = run_openclaw_cli(&retry_args, payload.proxy.clone())?;
        if retry.code == 0 {
            warnings.push(
                "Onboard recovered via fallback: --no-install-daemon --skip-health --skip-channels --skip-skills --flow manual".to_string(),
            );
            return Ok(());
        }

        // Keep first failure context and include retry failure details for troubleshooting.
        let retry_text = if retry.stderr.is_empty() {
            retry.stdout
        } else {
            retry.stderr
        };
        return Err(anyhow!(
            "openclaw onboard failed (first): {}; fallback retry failed: {}",
            err_text,
            retry_text
        ));
    }

    shell::ensure_success("openclaw onboard", &out)?;
    Ok(())
}

fn apply_model_chain(model_chain: &ModelChain, warnings: &mut Vec<String>) -> Result<()> {
    let primary = normalize_known_model_key(model_chain.primary.as_str());
    if primary.trim().is_empty() {
        return Err(anyhow!("Primary model is required."));
    }
    let set_out = run_openclaw_cli(
        &[
            "models".to_string(),
            "set".to_string(),
            primary.clone(),
        ],
        None,
    )?;
    shell::ensure_success("openclaw models set", &set_out)?;

    let clear_out = run_openclaw_cli(
        &[
            "models".to_string(),
            "fallbacks".to_string(),
            "clear".to_string(),
        ],
        None,
    )?;
    shell::ensure_success("openclaw models fallbacks clear", &clear_out)?;

    for fallback in normalize_fallbacks(&model_chain.fallbacks) {
        let fallback = normalize_known_model_key(fallback.as_str());
        if fallback == primary {
            continue;
        }
        let out = run_openclaw_cli(
            &[
                "models".to_string(),
                "fallbacks".to_string(),
                "add".to_string(),
                fallback.clone(),
            ],
            None,
        )?;
        if out.code != 0 {
            warnings.push(format!(
                "Failed to add fallback model '{}': {}",
                fallback, out.stderr
            ));
        }
    }
    Ok(())
}

fn apply_kimi_region_base_url(
    payload: &OpenClawConfigInput,
    warnings: &mut Vec<String>,
) -> Result<()> {
    let kimi_providers = providers_from_model_chain(&payload.model_chain)
        .into_iter()
        .filter(|provider| provider == "moonshot" || provider == "kimi-coding")
        .collect::<Vec<_>>();
    if kimi_providers.is_empty() {
        return Ok(());
    }

    let region = normalize_kimi_region(payload.kimi_region.trim()).unwrap_or_else(|| {
        warnings.push(format!(
            "Invalid kimi_region='{}'; fallback to '{}' (mainland).",
            payload.kimi_region, KIMI_REGION_CN
        ));
        KIMI_REGION_CN.to_string()
    });
    let base_url = match region.as_str() {
        KIMI_REGION_GLOBAL => KIMI_BASE_URL_GLOBAL,
        _ => KIMI_BASE_URL_CN,
    };

    let mut target_providers = HashSet::<String>::new();
    for provider in kimi_providers {
        target_providers.insert(provider.clone());
        // Compatibility: older Kimi model definitions still live under moonshot provider namespace.
        if provider == "kimi-coding" {
            target_providers.insert("moonshot".to_string());
        }
    }

    for provider in target_providers {
        let path = format!("models.providers.{provider}.baseUrl");
        let out = run_openclaw_cli(
            &[
                "config".to_string(),
                "set".to_string(),
                path.clone(),
                base_url.to_string(),
            ],
            payload.proxy.clone(),
        )?;
        if out.code != 0 {
            warnings.push(format!(
                "Kimi baseUrl write failed ({path}): {}",
                cli_output_text(&out)
            ));
        } else {
            logger::info(&format!(
                "Kimi region baseUrl applied: provider={provider}, region={region}, baseUrl={base_url}"
            ));
        }
    }

    Ok(())
}

fn apply_provider_keys(payload: &OpenClawConfigInput, warnings: &mut Vec<String>) -> Result<()> {
    let mut env_values = BTreeMap::<String, String>::new();
    let mut unmapped = HashSet::<String>::new();

    for (provider, value) in &payload.provider_api_keys {
        let Some(key_value) = optional_non_empty(Some(value.clone())) else {
            continue;
        };
        let normalized = normalize_auth_provider(provider);
        if let Some(env_name) = provider_env_name(normalized.as_str()) {
            env_values.insert(env_name, sanitize_env_value(&key_value));
        } else {
            unmapped.insert(provider.to_string());
        }
    }

    // Backward compatibility: if legacy single API key is set, keep binding it to primary provider.
    if let Some(key_value) = optional_non_empty(Some(payload.api_key.clone())) {
        if let Ok(primary_provider) = resolve_provider(payload) {
            let normalized = normalize_auth_provider(primary_provider.as_str());
            if let Some(env_name) = provider_env_name(normalized.as_str()) {
                env_values
                    .entry(env_name)
                    .or_insert_with(|| sanitize_env_value(&key_value));
            } else {
                unmapped.insert(primary_provider);
            }
        }
    }

    // Surface missing key hints for fallback providers so users can fix quickly.
    for provider in providers_from_model_chain(&payload.model_chain) {
        if provider_key_for_payload(payload, provider.as_str()).is_some() {
            continue;
        }
        if provider_env_name(provider.as_str()).is_some() {
            warnings.push(format!(
                "No API key configured for provider '{}' in model chain; fallback calls to this provider may fail.",
                provider
            ));
        }
    }

    for provider in unmapped {
        warnings.push(format!(
            "Provider '{}' has no known environment variable mapping; key was not written to .env.",
            provider
        ));
    }

    if env_values.is_empty() {
        return Ok(());
    }

    let env_path = paths::openclaw_home().join(".env");
    upsert_env_file(&env_path, &env_values)?;
    logger::info(&format!(
        "Updated provider API keys in {} ({} entries).",
        env_path.to_string_lossy(),
        env_values.len()
    ));
    Ok(())
}

fn apply_feature_toggles(payload: &OpenClawConfigInput, warnings: &mut Vec<String>) -> Result<()> {
    let hook_command = if payload.enable_session_memory_hook {
        vec![
            "hooks".to_string(),
            "enable".to_string(),
            "session-memory".to_string(),
        ]
    } else {
        vec![
            "hooks".to_string(),
            "disable".to_string(),
            "session-memory".to_string(),
        ]
    };
    let hook_out = run_openclaw_cli(&hook_command, payload.proxy.clone())?;
    if hook_out.code != 0 {
        warnings.push(format!(
            "Hook update failed (session-memory): {}",
            if hook_out.stderr.is_empty() {
                hook_out.stdout
            } else {
                hook_out.stderr
            }
        ));
    }

    if payload.enable_workspace_memory {
        let workspace = paths::openclaw_home().join("workspace");
        let memory_dir = workspace.join("memory");
        fs::create_dir_all(&memory_dir)?;
        let memory_md = workspace.join("MEMORY.md");
        if !memory_md.exists() {
            fs::write(
                &memory_md,
                "# MEMORY\n\n- Notes persisted by OpenClaw Installer.\n",
            )?;
        }
    }

    if payload.enable_skills_scan {
        let skills_out = run_openclaw_cli(
            &["skills".to_string(), "check".to_string()],
            payload.proxy.clone(),
        )?;
        if skills_out.code == 0 {
            logger::info("Skills check completed.");
        } else {
            warnings.push(format!(
                "Skills check failed: {}",
                if skills_out.stderr.is_empty() {
                    skills_out.stdout
                } else {
                    skills_out.stderr
                }
            ));
        }
    }

    Ok(())
}

fn apply_selected_skills(payload: &OpenClawConfigInput, warnings: &mut Vec<String>) -> Result<()> {
    let selected = normalize_selected_skills(&payload.selected_skills);
    if selected.is_empty() {
        return Ok(());
    }

    let config_path = paths::config_path();
    if !config_path.exists() {
        warnings.push(
            "Cannot apply selected skills because openclaw.json does not exist yet.".to_string(),
        );
        return Ok(());
    }

    let raw = fs::read_to_string(&config_path)?;
    let mut root: Value = serde_json::from_str(&raw)?;
    if !root.is_object() {
        warnings.push(
            "openclaw.json has unexpected schema; skipped selected skills apply.".to_string(),
        );
        return Ok(());
    }

    // Keep bundled skills explicit so only user-selected skills are enabled by default.
    root["skills"]["allowBundled"] = Value::Array(
        selected
            .iter()
            .map(|name| Value::String(name.clone()))
            .collect::<Vec<_>>(),
    );

    for name in &selected {
        root["skills"]["entries"][name]["enabled"] = Value::Bool(true);
    }

    fs::write(&config_path, serde_json::to_string_pretty(&root)?)?;
    logger::info(&format!(
        "Applied selected bundled skills to config: {}",
        selected.join(", ")
    ));

    let list_out = run_openclaw_cli(
        &[
            "skills".to_string(),
            "list".to_string(),
            "--json".to_string(),
        ],
        payload.proxy.clone(),
    )?;
    if list_out.code != 0 {
        warnings.push("Failed to verify selected skills (skills list command failed).".to_string());
        return Ok(());
    }
    let parsed: Value =
        parse_json_value_from_cli_output(&list_out.stdout).unwrap_or_else(|| json!({}));
    let Some(skills) = parsed.get("skills").and_then(|v| v.as_array()) else {
        return Ok(());
    };
    for selected_name in &selected {
        let item = skills.iter().find(|entry| {
            entry.get("name").and_then(|v| v.as_str()) == Some(selected_name.as_str())
        });
        let Some(item) = item else {
            warnings.push(format!(
                "Selected skill '{}' was not found in current OpenClaw skill catalog.",
                selected_name
            ));
            continue;
        };
        let eligible = item
            .get("eligible")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if eligible {
            continue;
        }
        let missing = item.get("missing").cloned().unwrap_or_else(|| json!({}));
        warnings.push(format!(
            "Skill '{}' is selected but not ready. Missing requirements: {}",
            selected_name, missing
        ));
    }

    Ok(())
}

fn parse_json_value_from_cli_output(raw: &str) -> Option<Value> {
    if raw.trim().is_empty() {
        return None;
    }
    if let Ok(v) = serde_json::from_str::<Value>(raw) {
        return Some(v);
    }

    let trimmed = raw.trim_start_matches('\u{feff}');
    if let Ok(v) = serde_json::from_str::<Value>(trimmed) {
        return Some(v);
    }

    // OpenClaw CLI often prints plugin registration logs before the JSON payload. Scan for a JSON value.
    let mut search_start = 0usize;
    while let Some(offset) = trimmed[search_start..].find('{') {
        let start = search_start + offset;
        let candidate = &trimmed[start..];
        let mut stream = Deserializer::from_str(candidate).into_iter::<Value>();
        if let Some(value) = stream.next() {
            if let Ok(json) = value {
                return Some(json);
            }
        }
        search_start = start + 1;
    }

    None
}

fn upsert_env_file(path: &Path, entries: &BTreeMap<String, String>) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let existing = if path.exists() {
        fs::read_to_string(path)?
    } else {
        String::new()
    };

    let mut out_lines = Vec::<String>::new();
    let mut touched = HashSet::<String>::new();

    for line in existing.lines() {
        let Some((key_raw, _)) = line.split_once('=') else {
            out_lines.push(line.to_string());
            continue;
        };
        let key = key_raw.trim();
        if let Some(next_value) = entries.get(key) {
            out_lines.push(format!("{key}={next_value}"));
            touched.insert(key.to_string());
        } else {
            out_lines.push(line.to_string());
        }
    }

    for (key, value) in entries {
        if touched.contains(key) {
            continue;
        }
        out_lines.push(format!("{key}={value}"));
    }

    let mut content = out_lines.join("\n");
    if !content.ends_with('\n') {
        content.push('\n');
    }
    fs::write(path, content)?;
    Ok(())
}

fn remove_env_keys(path: &Path, keys: &[String]) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let existing = fs::read_to_string(path)?;
    let keyset = keys
        .iter()
        .map(|k| k.trim().to_string())
        .filter(|k| !k.is_empty())
        .collect::<HashSet<_>>();
    if keyset.is_empty() {
        return Ok(());
    }

    let mut lines = Vec::<String>::new();
    for line in existing.lines() {
        let Some((key_raw, _)) = line.split_once('=') else {
            lines.push(line.to_string());
            continue;
        };
        let key = key_raw.trim();
        if keyset.contains(key) {
            continue;
        }
        lines.push(line.to_string());
    }
    let mut content = lines.join("\n");
    if !content.is_empty() && !content.ends_with('\n') {
        content.push('\n');
    }
    fs::write(path, content)?;
    Ok(())
}

fn normalize_selected_skills(skills: &[String]) -> Vec<String> {
    let mut uniq = Vec::<String>::new();
    for skill in skills {
        let item = skill.trim();
        if item.is_empty() {
            continue;
        }
        if !uniq.iter().any(|x| x == item) {
            uniq.push(item.to_string());
        }
    }
    uniq
}

fn apply_channel_integrations(
    payload: &OpenClawConfigInput,
    warnings: &mut Vec<String>,
) -> Result<()> {
    apply_feishu_integration(payload, warnings)?;

    if !payload.enable_telegram_channel {
        return Ok(());
    }
    if payload.telegram_bot_token.trim().is_empty() {
        warnings
            .push("Telegram enabled but bot token is empty; skipped Telegram setup.".to_string());
        return Ok(());
    }

    let args = vec![
        "channels".to_string(),
        "add".to_string(),
        "--channel".to_string(),
        "telegram".to_string(),
        "--token".to_string(),
        payload.telegram_bot_token.trim().to_string(),
    ];
    if !payload.telegram_pair_code.trim().is_empty() {
        // Telegram pair code is intentionally postponed to post-install maintenance flow.
        warnings.push(
            "Telegram pair code is deferred. Finish install first, then apply pair code from Maintenance."
                .to_string(),
        );
    }

    let mut out = run_openclaw_cli(&args, payload.proxy.clone())?;
    if out.code != 0 && is_unknown_channel_error(&out, "telegram") {
        // Telegram plugin can be disabled by default.
        // Enable and retry once to avoid false "token invalid" reports.
        let _ = run_openclaw_cli(
            &[
                "plugins".to_string(),
                "enable".to_string(),
                "telegram".to_string(),
            ],
            payload.proxy.clone(),
        );
        let _ = run_openclaw_cli(
            &["gateway".to_string(), "restart".to_string()],
            payload.proxy.clone(),
        );
        out = run_openclaw_cli(&args, payload.proxy.clone())?;
    }

    if out.code == 0 {
        logger::info("Telegram channel configured successfully.");
    } else {
        warnings.push(format!(
            "Telegram setup failed: {}",
            if out.stderr.is_empty() {
                out.stdout
            } else {
                out.stderr
            }
        ));
    }
    Ok(())
}

fn apply_feishu_integration(
    payload: &OpenClawConfigInput,
    warnings: &mut Vec<String>,
) -> Result<()> {
    if !payload.enable_feishu_channel {
        return Ok(());
    }

    let app_id = payload.feishu_app_id.trim();
    let app_secret = payload.feishu_app_secret.trim();
    if app_id.is_empty() || app_secret.is_empty() {
        warnings.push(
            "Feishu enabled but app_id/app_secret is empty; skipped Feishu setup.".to_string(),
        );
        return Ok(());
    }

    let plugin_enable_args = vec![
        "plugins".to_string(),
        "enable".to_string(),
        "feishu".to_string(),
    ];
    let plugin_enable_out = run_openclaw_cli(&plugin_enable_args, payload.proxy.clone())?;
    if plugin_enable_out.code != 0 {
        warnings.push(format!(
            "Feishu plugin enable failed: {}",
            redact_known_values(cli_output_text(&plugin_enable_out), &[app_secret])
        ));
    }

    let add_channel_args = vec![
        "channels".to_string(),
        "add".to_string(),
        "--channel".to_string(),
        "feishu".to_string(),
    ];
    let mut add_channel_out = run_openclaw_cli(&add_channel_args, payload.proxy.clone())?;
    if add_channel_out.code != 0 && is_unknown_channel_error(&add_channel_out, "feishu") {
        let _ = run_openclaw_cli(&plugin_enable_args, payload.proxy.clone());
        let _ = run_openclaw_cli(
            &["gateway".to_string(), "restart".to_string()],
            payload.proxy.clone(),
        );
        add_channel_out = run_openclaw_cli(&add_channel_args, payload.proxy.clone())?;
    }
    if add_channel_out.code != 0 {
        warnings.push(format!(
            "Feishu setup failed (channels add): {}",
            redact_known_values(cli_output_text(&add_channel_out), &[app_secret])
        ));
        return Ok(());
    }

    let writes = vec![
        ("channels.feishu.enabled", "true".to_string()),
        ("channels.feishu.appId", app_id.to_string()),
        ("channels.feishu.appSecret", app_secret.to_string()),
        ("channels.feishu.domain", "feishu".to_string()),
        ("channels.feishu.connectionMode", "websocket".to_string()),
    ];
    for (path, value) in writes {
        let out = run_openclaw_cli(
            &[
                "config".to_string(),
                "set".to_string(),
                path.to_string(),
                value,
            ],
            payload.proxy.clone(),
        )?;
        if out.code != 0 {
            warnings.push(format!(
                "Feishu config write failed ({path}): {}",
                redact_known_values(cli_output_text(&out), &[app_secret])
            ));
        }
    }

    let restart_out = run_openclaw_cli(
        &["gateway".to_string(), "restart".to_string()],
        payload.proxy.clone(),
    )?;
    if restart_out.code != 0 {
        warnings.push(format!(
            "Feishu gateway restart failed: {}",
            redact_known_values(cli_output_text(&restart_out), &[app_secret])
        ));
    } else {
        logger::info("Feishu channel configured successfully (china direct websocket).");
    }

    Ok(())
}

pub fn setup_telegram_pair(pair_code: &str) -> Result<String> {
    let code = pair_code.trim();
    if code.is_empty() {
        return Err(anyhow!("Telegram pair code cannot be empty."));
    }

    let Some(last) = state_store::load_last_config()? else {
        return Err(anyhow!(
            "No saved install config found. Complete installation first."
        ));
    };
    let args = vec![
        "pairing".to_string(),
        "approve".to_string(),
        "telegram".to_string(),
        code.to_string(),
    ];
    let mut out = run_openclaw_cli(&args, last.proxy.clone())?;
    if out.code != 0 && is_unknown_channel_error(&out, "telegram") {
        let _ = run_openclaw_cli(
            &[
                "plugins".to_string(),
                "enable".to_string(),
                "telegram".to_string(),
            ],
            last.proxy.clone(),
        );
        let _ = run_openclaw_cli(
            &["gateway".to_string(), "restart".to_string()],
            last.proxy.clone(),
        );
        out = run_openclaw_cli(&args, last.proxy.clone())?;
    }
    if out.code == 0 && is_pairing_output_error(&out) {
        out.code = 1;
    }

    if out.code == 0 {
        logger::info("Telegram pairing approved from maintenance.");
        return Ok(format!("Telegram pairing approved: {code}"));
    }

    if is_unknown_command_error(&out, "pairing") {
        logger::warn(
            "OpenClaw CLI does not support `pairing` command; fallback to legacy Telegram account pairing.",
        );
        return setup_telegram_pair_legacy(code, &last);
    }

    shell::ensure_success("openclaw pairing approve telegram", &out)?;
    Ok(format!("Telegram pairing approved: {code}"))
}

fn is_unknown_channel_error(out: &shell::CmdOutput, channel: &str) -> bool {
    let merged = format!(
        "{}\n{}",
        out.stdout.to_ascii_lowercase(),
        out.stderr.to_ascii_lowercase()
    );
    merged.contains("unknown channel") && merged.contains(&channel.to_ascii_lowercase())
}

fn is_unknown_command_error(out: &shell::CmdOutput, command: &str) -> bool {
    let merged = format!(
        "{}\n{}",
        out.stdout.to_ascii_lowercase(),
        out.stderr.to_ascii_lowercase()
    );
    merged.contains("unknown command") && merged.contains(&command.to_ascii_lowercase())
}

fn is_pairing_output_error(out: &shell::CmdOutput) -> bool {
    let merged = format!(
        "{}\n{}",
        out.stdout.to_ascii_lowercase(),
        out.stderr.to_ascii_lowercase()
    );
    merged.contains("failed to start cli:")
        || merged.contains("no pending pairing request found")
        || (merged.contains("pairing") && merged.contains("error"))
}

fn setup_telegram_pair_legacy(code: &str, payload: &OpenClawConfigInput) -> Result<String> {
    let token = payload.telegram_bot_token.trim();
    if token.is_empty() {
        return Err(anyhow!(
            "Telegram bot token is missing. Re-run install wizard and enable Telegram first."
        ));
    }

    let args = vec![
        "channels".to_string(),
        "add".to_string(),
        "--channel".to_string(),
        "telegram".to_string(),
        "--token".to_string(),
        token.to_string(),
        "--account".to_string(),
        code.to_string(),
    ];
    let mut out = run_openclaw_cli(&args, payload.proxy.clone())?;
    if out.code != 0 && is_unknown_channel_error(&out, "telegram") {
        let _ = run_openclaw_cli(
            &[
                "plugins".to_string(),
                "enable".to_string(),
                "telegram".to_string(),
            ],
            payload.proxy.clone(),
        );
        let _ = run_openclaw_cli(
            &["gateway".to_string(), "restart".to_string()],
            payload.proxy.clone(),
        );
        out = run_openclaw_cli(&args, payload.proxy.clone())?;
    }
    shell::ensure_success("openclaw channels add telegram account (legacy)", &out)?;
    logger::info("Telegram pairing applied via legacy account fallback.");
    Ok(format!(
        "Telegram pairing applied (legacy fallback): {code}"
    ))
}

fn run_openclaw_cli(args: &[String], proxy: Option<String>) -> Result<shell::CmdOutput> {
    let install = state_store::load_install_state()?
        .ok_or_else(|| anyhow!("Install state not found. Run install_openclaw first."))?;
    let command_path = resolve_working_cli_command(&install.command_path)?;

    let mut envs = vec![
        (
            "OPENCLAW_CONFIG_PATH".to_string(),
            paths::config_path().to_string_lossy().to_string(),
        ),
        (
            "OPENCLAW_STATE_DIR".to_string(),
            paths::openclaw_home().to_string_lossy().to_string(),
        ),
    ];
    if let Some(proxy) = optional_non_empty(proxy) {
        envs.push(("HTTP_PROXY".to_string(), proxy.clone()));
        envs.push(("HTTPS_PROXY".to_string(), proxy.clone()));
        envs.push(("ALL_PROXY".to_string(), proxy));
    }

    let masked = mask_sensitive_args(args);
    logger::info(&format!(
        "openclaw cli: {} {}",
        command_path,
        masked.join(" ")
    ));

    if command_path.eq_ignore_ascii_case("npx") {
        let Some(npx_exe) = shell::command_exists("npx") else {
            return Err(anyhow!("npx not found. Please install Node.js first."));
        };
        let mut full_args = vec!["--yes".to_string(), "openclaw".to_string()];
        full_args.extend_from_slice(args);
        let refs = full_args.iter().map(String::as_str).collect::<Vec<_>>();
        let out = shell::run_command(npx_exe.as_str(), &refs, None, &envs)?;
        log_cli_result(&out);
        return Ok(out);
    }

    let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
    let out = shell::run_command(command_path.as_str(), &refs, None, &envs)?;
    log_cli_result(&out);
    Ok(out)
}

fn bind_address_to_mode(bind: &str) -> &'static str {
    let trimmed = bind.trim();
    if trimmed == "0.0.0.0" {
        return "lan";
    }
    "loopback"
}

fn validate_payload(payload: &OpenClawConfigInput) -> Result<()> {
    if payload.install_dir.trim().is_empty() {
        return Err(anyhow!("Install directory is required."));
    }
    let install_dir = paths::normalize_path(&payload.install_dir)?;
    if paths::is_user_profile_default_openclaw_dir(&install_dir) {
        return Err(anyhow!(
            "Unsafe install directory detected: {}. For isolation, choose a different folder (recommended: %LOCALAPPDATA%\\\\OpenClawInstaller\\\\openclaw).",
            install_dir.to_string_lossy()
        ));
    }

    let provider = resolve_provider(payload)?;
    if provider.trim().is_empty() {
        return Err(anyhow!("Provider is required."));
    }
    if payload.model_chain.primary.trim().is_empty() {
        return Err(anyhow!("Primary model is required."));
    }
    if payload.port == 0 {
        return Err(anyhow!("Port must be within 1-65535"));
    }
    if payload.bind_address.trim().is_empty() {
        return Err(anyhow!("Bind address cannot be empty."));
    }
    if let Some(url) = optional_non_empty(payload.base_url.clone()) {
        let _ = Url::parse(&url).map_err(|_| anyhow!("base_url is not a valid URL"))?;
    }
    if let Some(proxy) = optional_non_empty(payload.proxy.clone()) {
        let _ = Url::parse(&proxy).map_err(|_| anyhow!("proxy is not a valid URL"))?;
    }
    if normalize_kimi_region(payload.kimi_region.trim()).is_none() {
        return Err(anyhow!("kimi_region must be cn|global"));
    }
    if payload.enable_telegram_channel && payload.telegram_bot_token.trim().is_empty() {
        return Err(anyhow!(
            "Telegram bot token is required when Telegram channel is enabled."
        ));
    }
    if !matches!(
        payload.onboarding_flow.trim(),
        "quickstart" | "advanced" | "manual"
    ) {
        return Err(anyhow!(
            "onboarding_flow must be quickstart|advanced|manual"
        ));
    }
    if !matches!(payload.onboarding_mode.trim(), "local" | "remote") {
        return Err(anyhow!("onboarding_mode must be local|remote"));
    }
    if !matches!(payload.node_manager.trim(), "npm" | "pnpm" | "bun") {
        return Err(anyhow!("node_manager must be npm|pnpm|bun"));
    }
    if payload.onboarding_mode.trim() == "remote" {
        let remote_url = optional_non_empty(payload.remote_url.clone())
            .ok_or_else(|| anyhow!("remote_url is required when onboarding_mode is remote"))?;
        let _ = Url::parse(&remote_url).map_err(|_| anyhow!("remote_url is not a valid URL"))?;
    }
    Ok(())
}

fn optional_non_empty(value: Option<String>) -> Option<String> {
    value.and_then(|v| {
        let s = v.trim().to_string();
        if s.is_empty() {
            None
        } else {
            Some(s)
        }
    })
}

fn existing_gateway_token() -> Option<String> {
    let path = paths::config_path();
    if !path.exists() {
        return None;
    }
    let raw = fs::read_to_string(path).ok()?;
    let json: Value = serde_json::from_str(&raw).ok()?;
    let mode = json
        .pointer("/gateway/auth/mode")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    if !mode.eq_ignore_ascii_case("token") {
        return None;
    }
    json.pointer("/gateway/auth/token")
        .and_then(|v| v.as_str())
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn generate_gateway_token(len: usize) -> String {
    let mut out = String::new();
    while out.len() < len {
        out.push_str(&Uuid::new_v4().simple().to_string());
    }
    out.truncate(len);
    out
}

fn normalize_fallbacks(fallbacks: &[String]) -> Vec<String> {
    let mut uniq = Vec::<String>::new();
    for item in fallbacks {
        let value = item.trim();
        if value.is_empty() {
            continue;
        }
        if !uniq.iter().any(|x| x == value) {
            uniq.push(value.to_string());
        }
    }
    uniq
}

fn set_windows_acl(path: &Path) -> Vec<String> {
    let mut warnings = Vec::new();
    let username = std::env::var("USERNAME").unwrap_or_else(|_| "CurrentUser".to_string());
    let path_text = path.to_string_lossy().to_string();

    // Remove inherited broad permissions first, then re-grant current user explicitly.
    match shell::run_command("icacls", &[&path_text, "/inheritance:r"], None, &[]) {
        Ok(out) if out.code == 0 => {}
        Ok(out) => warnings.push(format!("Failed to disable ACL inheritance: {}", out.stderr)),
        Err(err) => warnings.push(format!("ACL operation failed: {err}")),
    }
    let grant = format!("{username}:(R,W)");
    match shell::run_command("icacls", &[&path_text, "/grant:r", &grant], None, &[]) {
        Ok(out) if out.code == 0 => {}
        Ok(out) => warnings.push(format!(
            "Failed to grant ACL to current user: {}",
            out.stderr
        )),
        Err(err) => warnings.push(format!("ACL grant failed: {err}")),
    }
    warnings
}

fn resolve_provider(payload: &OpenClawConfigInput) -> Result<String> {
    if let Some(provider) = provider_from_model_key(&payload.model_chain.primary) {
        return Ok(provider.to_string());
    }
    let provider = payload.provider.trim();
    if provider.is_empty() {
        return Err(anyhow!("Provider is required."));
    }
    Ok(provider.to_string())
}

fn provider_from_model_key(model: &str) -> Option<&str> {
    let (provider, model_name) = model.split_once('/')?;
    if provider.trim().is_empty() || model_name.trim().is_empty() {
        return None;
    }
    Some(provider.trim())
}

fn normalize_known_model_key(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    // Backward compatibility: older UI/builds used the wrong Kimi 2.5 id.
    // OpenClaw uses `kimi-k2.5`.
    let lowered = trimmed.to_ascii_lowercase();
    if lowered == "moonshot/kimi-2.5" || lowered == "moonshot/kimi2.5" {
        return "moonshot/kimi-k2.5".to_string();
    }
    trimmed.to_string()
}

fn normalize_auth_provider(provider: &str) -> String {
    match provider.trim().to_ascii_lowercase().as_str() {
        // `openai-codex/*` models still authenticate with OpenAI API key.
        "openai-codex" => "openai".to_string(),
        // Keep kimi coding provider id stable across aliases.
        "kimi-code" => "kimi-coding".to_string(),
        other => other.to_string(),
    }
}

fn provider_key_for_payload(payload: &OpenClawConfigInput, provider: &str) -> Option<String> {
    let normalized = normalize_auth_provider(provider);
    let direct = payload
        .provider_api_keys
        .get(normalized.as_str())
        .cloned()
        .or_else(|| payload.provider_api_keys.get(provider).cloned())
        .and_then(|v| optional_non_empty(Some(v)));
    if direct.is_some() {
        return direct;
    }
    if normalized == "openai" {
        return payload
            .provider_api_keys
            .get("openai-codex")
            .cloned()
            .and_then(|v| optional_non_empty(Some(v)));
    }
    if normalized == "kimi-coding" {
        return payload
            .provider_api_keys
            .get("kimi-code")
            .cloned()
            .and_then(|v| optional_non_empty(Some(v)));
    }
    None
}

fn provider_key_for_id(payload: &OpenClawConfigInput, provider_id: &str) -> Option<String> {
    provider_key_for_payload(payload, provider_id)
}

fn provider_env_name(provider: &str) -> Option<String> {
    match normalize_auth_provider(provider).as_str() {
        "openai" => Some("OPENAI_API_KEY".to_string()),
        "google" => Some("GEMINI_API_KEY".to_string()),
        "moonshot" => Some("MOONSHOT_API_KEY".to_string()),
        "kimi-coding" => Some("KIMI_API_KEY".to_string()),
        "xai" => Some("XAI_API_KEY".to_string()),
        "anthropic" => Some("ANTHROPIC_API_KEY".to_string()),
        "openrouter" => Some("OPENROUTER_API_KEY".to_string()),
        "azure" => Some("AZURE_OPENAI_API_KEY".to_string()),
        "zai" => Some("ZAI_API_KEY".to_string()),
        "xiaomi" => Some("XIAOMI_API_KEY".to_string()),
        "minimax" => Some("MINIMAX_API_KEY".to_string()),
        other => generic_provider_env_name(other),
    }
}

fn generic_provider_env_name(provider: &str) -> Option<String> {
    let normalized = provider
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches('_')
        .to_string();
    if normalized.is_empty() {
        return None;
    }
    Some(format!("{normalized}_API_KEY"))
}

fn providers_from_model_chain(model_chain: &ModelChain) -> Vec<String> {
    let mut providers = HashSet::<String>::new();
    if let Some(provider) = provider_from_model_key(&model_chain.primary) {
        providers.insert(normalize_auth_provider(provider));
    }
    for fallback in &model_chain.fallbacks {
        if let Some(provider) = provider_from_model_key(fallback) {
            providers.insert(normalize_auth_provider(provider));
        }
    }
    let mut out = providers.into_iter().collect::<Vec<_>>();
    out.sort();
    out
}

fn sanitize_env_value(raw: &str) -> String {
    raw.replace('\r', "").replace('\n', "")
}

fn normalize_onboard_flow(raw: &str) -> &str {
    match raw.trim() {
        "quickstart" | "advanced" | "manual" => raw.trim(),
        _ => "quickstart",
    }
}

fn normalize_onboard_mode(raw: &str) -> &str {
    match raw.trim() {
        "local" | "remote" => raw.trim(),
        _ => "local",
    }
}

fn normalize_node_manager(raw: &str) -> &str {
    match raw.trim() {
        "npm" | "pnpm" | "bun" => raw.trim(),
        _ => "npm",
    }
}

fn normalize_kimi_region(raw: &str) -> Option<String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "" => Some(KIMI_REGION_CN.to_string()),
        KIMI_REGION_CN => Some(KIMI_REGION_CN.to_string()),
        KIMI_REGION_GLOBAL => Some(KIMI_REGION_GLOBAL.to_string()),
        _ => None,
    }
}

fn is_gateway_1006_error(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains("gateway closed (1006")
        || (lower.contains("gateway closed") && lower.contains("1006"))
}

fn force_safe_onboard_retry_args(args: &[String]) -> Vec<String> {
    let mut out = Vec::<String>::new();
    let mut i = 0usize;
    while i < args.len() {
        let cur = args[i].as_str();
        if matches!(
            cur,
            "--install-daemon"
                | "--no-install-daemon"
                | "--skip-health"
                | "--skip-channels"
                | "--skip-skills"
        ) {
            i += 1;
            continue;
        }
        if cur == "--flow" {
            i += 1;
            if i < args.len() {
                i += 1;
            }
            continue;
        }
        out.push(args[i].clone());
        i += 1;
    }

    out.push("--flow".to_string());
    out.push("manual".to_string());
    out.push("--no-install-daemon".to_string());
    out.push("--skip-health".to_string());
    out.push("--skip-channels".to_string());
    out.push("--skip-skills".to_string());
    out
}

fn resolve_working_cli_command(preferred: &str) -> Result<String> {
    let preferred = preferred.trim().trim_matches('"').to_string();
    if is_cli_command_usable(preferred.as_str()) {
        return Ok(preferred);
    }

    logger::warn(&format!(
        "Configured OpenClaw command is not usable: {}",
        preferred
    ));

    if let Some(global) = shell::command_exists("openclaw") {
        if !global.eq_ignore_ascii_case(preferred.as_str())
            && is_cli_command_usable(global.as_str())
        {
            logger::warn(&format!(
                "Falling back to global OpenClaw command from PATH: {}",
                global
            ));
            return Ok(global);
        }
    }

    if is_cli_command_usable("npx") {
        logger::warn("Falling back to npx openclaw.");
        return Ok("npx".to_string());
    }

    Err(anyhow!(
        "No usable OpenClaw command found. Tried configured command, PATH openclaw, and npx."
    ))
}

fn is_cli_command_usable(command: &str) -> bool {
    if command.eq_ignore_ascii_case("npx") {
        let Some(npx_exe) = shell::command_exists("npx") else {
            return false;
        };
        let Ok(out) = shell::run_command(
            npx_exe.as_str(),
            &["--yes", "openclaw", "--version"],
            None,
            &[],
        ) else {
            return false;
        };
        return out.code == 0;
    }

    let Ok(out) = shell::run_command(command, &["--version"], None, &[]) else {
        return false;
    };
    out.code == 0
}

fn log_cli_result(out: &shell::CmdOutput) {
    if out.code == 0 {
        return;
    }
    if !out.stderr.trim().is_empty() {
        logger::warn(&format!(
            "openclaw cli stderr: {}",
            compact_text(&out.stderr, 2000)
        ));
    } else if !out.stdout.trim().is_empty() {
        logger::warn(&format!(
            "openclaw cli stdout: {}",
            compact_text(&out.stdout, 2000)
        ));
    }
}

fn mask_sensitive_args(args: &[String]) -> Vec<String> {
    let mut out = Vec::with_capacity(args.len());
    let mut mask_next = false;
    for item in args {
        if mask_next {
            out.push("******".to_string());
            mask_next = false;
            continue;
        }
        let lower = item.to_ascii_lowercase();
        if is_secret_flag(lower.as_str()) || is_secret_config_path(lower.as_str()) {
            out.push(item.clone());
            mask_next = true;
            continue;
        }
        out.push(item.clone());
    }
    out
}

fn is_secret_flag(flag: &str) -> bool {
    matches!(
        flag,
        "--openai-api-key"
            | "--gemini-api-key"
            | "--moonshot-api-key"
            | "--xai-api-key"
            | "--anthropic-api-key"
            | "--token"
            | "--remote-token"
            | "--gateway-token"
            | "--gateway-password"
            | "--access-token"
            | "--app-token"
            | "--bot-token"
            | "--password"
    )
}

fn is_secret_config_path(path: &str) -> bool {
    matches!(path, "channels.feishu.appsecret")
}

fn compact_text(raw: &str, max_len: usize) -> String {
    let mut text = raw.replace('\r', "");
    if text.len() > max_len {
        text.truncate(max_len);
        text.push_str(" ...<truncated>");
    }
    text
}

fn cli_output_text(out: &shell::CmdOutput) -> String {
    if out.stderr.is_empty() {
        out.stdout.clone()
    } else {
        out.stderr.clone()
    }
}

fn redact_known_values(mut text: String, values: &[&str]) -> String {
    for value in values {
        let secret = value.trim();
        if !secret.is_empty() {
            text = text.replace(secret, "******");
        }
    }
    text
}
