use std::collections::{BTreeMap, HashSet};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use once_cell::sync::Lazy;
use serde::Deserialize;
use serde_json::{Deserializer, Value};
use std::sync::{mpsc, Mutex};
use std::thread;

use crate::models::ModelCatalogItem;

use super::{logger, paths, shell, state_store};

#[derive(Debug, Deserialize)]
struct ModelsListPayload {
    #[serde(default)]
    models: Vec<ModelsListEntry>,
}

#[derive(Debug, Deserialize)]
struct ModelsListEntry {
    key: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    available: Option<bool>,
    #[serde(default)]
    missing: bool,
}

const MODEL_CATALOG_CACHE_TTL: Duration = Duration::from_secs(45);
const MODEL_CATALOG_CLI_TIMEOUT: Duration = Duration::from_millis(2_200);

#[derive(Clone)]
struct ModelCatalogCache {
    loaded_at: Instant,
    items: Vec<ModelCatalogItem>,
}

static MODEL_CATALOG_CACHE: Lazy<Mutex<Option<ModelCatalogCache>>> = Lazy::new(|| Mutex::new(None));

pub fn list_model_catalog() -> Result<Vec<ModelCatalogItem>> {
    if let Some(items) = load_cached_catalog() {
        return Ok(items);
    }

    let cli_items = match list_from_openclaw_cli_with_timeout(MODEL_CATALOG_CLI_TIMEOUT) {
        Ok(items) => items,
        Err(err) => {
            logger::warn(&format!("Model catalog CLI query failed: {err}"));
            vec![]
        }
    };
    if cli_items.is_empty() {
        logger::warn("Model catalog CLI result is empty. Merging config and built-in catalog.");
    }

    let merged = merge_catalog_sources(&[cli_items, list_from_config_json(), fallback_catalog()]);
    save_cached_catalog(merged.clone());
    Ok(merged)
}

fn list_from_openclaw_cli_with_timeout(timeout: Duration) -> Result<Vec<ModelCatalogItem>> {
    let (tx, rx) = mpsc::channel::<Result<Vec<ModelCatalogItem>>>();
    thread::spawn(move || {
        let _ = tx.send(list_from_openclaw_cli());
    });

    match rx.recv_timeout(timeout) {
        Ok(result) => result,
        Err(mpsc::RecvTimeoutError::Timeout) => {
            logger::warn(&format!(
                "Model catalog CLI timed out after {} ms; using fallback catalog.",
                timeout.as_millis()
            ));
            Ok(vec![])
        }
        Err(err) => Err(anyhow!("Model catalog worker channel failed: {err}")),
    }
}

fn load_cached_catalog() -> Option<Vec<ModelCatalogItem>> {
    let guard = MODEL_CATALOG_CACHE.lock().ok()?;
    let cached = guard.as_ref()?;
    if cached.loaded_at.elapsed() <= MODEL_CATALOG_CACHE_TTL {
        return Some(cached.items.clone());
    }
    None
}

fn save_cached_catalog(items: Vec<ModelCatalogItem>) {
    if let Ok(mut guard) = MODEL_CATALOG_CACHE.lock() {
        *guard = Some(ModelCatalogCache {
            loaded_at: Instant::now(),
            items,
        });
    }
}

fn merge_catalog_sources(sources: &[Vec<ModelCatalogItem>]) -> Vec<ModelCatalogItem> {
    let mut map = BTreeMap::<String, ModelCatalogItem>::new();
    for source in sources {
        for item in source {
            // Keep first occurrence so priority is: CLI > config > fallback.
            map.entry(item.key.clone()).or_insert_with(|| item.clone());
        }
    }
    map.into_values().collect()
}

fn list_from_openclaw_cli() -> Result<Vec<ModelCatalogItem>> {
    let envs = vec![
        (
            "OPENCLAW_CONFIG_PATH".to_string(),
            paths::config_path().to_string_lossy().to_string(),
        ),
        (
            "OPENCLAW_STATE_DIR".to_string(),
            paths::openclaw_home().to_string_lossy().to_string(),
        ),
    ];

    let commands = resolve_openclaw_commands();
    for command in commands {
        let json_items = run_models_list_json(command.as_str(), &envs)?;
        if !json_items.is_empty() {
            return Ok(json_items);
        }

        let plain_items = run_models_list_plain(command.as_str(), &envs)?;
        if !plain_items.is_empty() {
            return Ok(plain_items);
        }
    }

    Ok(vec![])
}

fn run_models_list_json(command: &str, envs: &[(String, String)]) -> Result<Vec<ModelCatalogItem>> {
    let output = if is_npx_command(command) {
        shell::run_command(
            command,
            &[
                "--yes",
                "openclaw",
                "--no-color",
                "models",
                "list",
                "--all",
                "--json",
            ],
            None,
            envs,
        )?
    } else {
        shell::run_command(
            command,
            &["--no-color", "models", "list", "--all", "--json"],
            None,
            envs,
        )?
    };

    if output.code != 0 {
        logger::warn(&format!(
            "openclaw models list --json failed via {}: {}",
            command,
            if output.stderr.is_empty() {
                output.stdout
            } else {
                output.stderr
            }
        ));
        return Ok(vec![]);
    }

    let parsed = match parse_models_payload(&output.stdout) {
        Ok(v) => v,
        Err(err) => {
            logger::warn(&format!(
                "openclaw models list --json parse failed via {}: {err}",
                command
            ));
            return Ok(vec![]);
        }
    };

    let mut items = parsed
        .models
        .into_iter()
        .filter(|entry| !entry.key.trim().is_empty())
        .map(|entry| ModelCatalogItem {
            provider: provider_from_key(entry.key.as_str()),
            key: entry.key.clone(),
            name: if entry.name.trim().is_empty() {
                entry.key
            } else {
                entry.name
            },
            available: entry.available,
            missing: entry.missing,
        })
        .collect::<Vec<_>>();

    items.sort_by(|a, b| a.key.cmp(&b.key));
    items.dedup_by(|a, b| a.key == b.key);
    Ok(items)
}

fn run_models_list_plain(
    command: &str,
    envs: &[(String, String)],
) -> Result<Vec<ModelCatalogItem>> {
    let output = if is_npx_command(command) {
        shell::run_command(
            command,
            &[
                "--yes",
                "openclaw",
                "--no-color",
                "models",
                "list",
                "--all",
                "--plain",
            ],
            None,
            envs,
        )?
    } else {
        shell::run_command(
            command,
            &["--no-color", "models", "list", "--all", "--plain"],
            None,
            envs,
        )?
    };

    if output.code != 0 {
        logger::warn(&format!(
            "openclaw models list --plain failed via {}: {}",
            command,
            if output.stderr.is_empty() {
                output.stdout
            } else {
                output.stderr
            }
        ));
        return Ok(vec![]);
    }

    let mut items = parse_models_plain(&output.stdout);
    items.sort_by(|a, b| a.key.cmp(&b.key));
    items.dedup_by(|a, b| a.key == b.key);
    Ok(items)
}

fn parse_models_plain(raw: &str) -> Vec<ModelCatalogItem> {
    raw.lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('[') || trimmed.starts_with('{') {
                return None;
            }
            let token = trimmed.split_whitespace().next().unwrap_or("");
            let key = token.trim_end_matches(',').trim_end_matches(':').trim();
            if !looks_like_model_key(key) {
                return None;
            }
            Some(ModelCatalogItem {
                key: key.to_string(),
                provider: provider_from_key(key),
                name: key.to_string(),
                available: None,
                missing: false,
            })
        })
        .collect()
}

fn looks_like_model_key(value: &str) -> bool {
    let Some((provider, model)) = value.split_once('/') else {
        return false;
    };
    !provider.trim().is_empty() && !model.trim().is_empty()
}

fn is_npx_command(command: &str) -> bool {
    let trimmed = command.trim().trim_matches('"');
    let file = std::path::Path::new(trimmed)
        .file_name()
        .and_then(|v| v.to_str())
        .unwrap_or(trimmed);
    let lower = file.to_ascii_lowercase();
    lower == "npx" || lower == "npx.cmd" || lower == "npx.exe"
}

fn parse_models_payload(raw: &str) -> Result<ModelsListPayload> {
    if let Ok(parsed) = serde_json::from_str::<ModelsListPayload>(raw) {
        return Ok(parsed);
    }

    let trimmed = raw.trim_start_matches('\u{feff}');
    if let Ok(parsed) = serde_json::from_str::<ModelsListPayload>(trimmed) {
        return Ok(parsed);
    }

    let mut search_start = 0usize;
    while let Some(offset) = trimmed[search_start..].find('{') {
        let start = search_start + offset;
        let candidate = &trimmed[start..];
        let mut stream = Deserializer::from_str(candidate).into_iter::<Value>();
        if let Some(value) = stream.next() {
            if let Ok(json) = value {
                if let Ok(parsed) = serde_json::from_value::<ModelsListPayload>(json) {
                    return Ok(parsed);
                }
            }
        }
        search_start = start + 1;
    }

    Err(anyhow!(
        "openclaw models list did not return a valid JSON payload"
    ))
}

fn resolve_openclaw_commands() -> Vec<String> {
    let mut out = Vec::<String>::new();

    if let Ok(Some(state)) = state_store::load_install_state() {
        let cmd = state.command_path.trim().trim_matches('"').to_string();
        if !cmd.is_empty() {
            if cmd.eq_ignore_ascii_case("npx") {
                if let Some(npx) = shell::command_exists("npx") {
                    out.push(npx);
                }
            } else {
                out.push(cmd);
            }
        }
    }

    if let Some(openclaw) = shell::command_exists("openclaw") {
        out.push(openclaw);
    }
    if let Some(npx) = shell::command_exists("npx") {
        out.push(npx);
    }

    let mut dedup = HashSet::<String>::new();
    out.retain(|command| {
        let key = command.to_ascii_lowercase();
        if dedup.contains(&key) {
            return false;
        }
        dedup.insert(key);
        true
    });

    out.into_iter()
        .filter(|command| is_model_list_command_usable(command))
        .collect()
}

fn is_model_list_command_usable(command: &str) -> bool {
    if is_npx_command(command) {
        let Ok(out) = shell::run_command(command, &["--yes", "openclaw", "--version"], None, &[])
        else {
            return false;
        };
        return out.code == 0;
    }

    let Ok(out) = shell::run_command(command, &["--version"], None, &[]) else {
        return false;
    };
    out.code == 0
}

fn list_from_config_json() -> Vec<ModelCatalogItem> {
    let path = paths::config_path();
    let raw = match std::fs::read_to_string(path) {
        Ok(v) => v,
        Err(_) => return vec![],
    };
    let json: Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(_) => return vec![],
    };

    let mut out = Vec::<ModelCatalogItem>::new();
    if let Some(entries) = json
        .pointer("/agents/defaults/models")
        .and_then(|v| v.as_object())
    {
        for (key, item) in entries {
            let name = item
                .get("alias")
                .and_then(|v| v.as_str())
                .map(|v| v.to_string())
                .unwrap_or_else(|| key.to_string());
            out.push(ModelCatalogItem {
                key: key.to_string(),
                provider: provider_from_key(key),
                name,
                available: None,
                missing: false,
            });
        }
    }

    if let Some(primary) = json
        .pointer("/agents/defaults/model/primary")
        .and_then(|v| v.as_str())
        .filter(|v| !v.trim().is_empty())
    {
        out.push(ModelCatalogItem {
            key: primary.to_string(),
            provider: provider_from_key(primary),
            name: primary.to_string(),
            available: None,
            missing: false,
        });
    }
    if let Some(fallbacks) = json
        .pointer("/agents/defaults/model/fallbacks")
        .and_then(|v| v.as_array())
    {
        for item in fallbacks {
            if let Some(model_key) = item.as_str() {
                out.push(ModelCatalogItem {
                    key: model_key.to_string(),
                    provider: provider_from_key(model_key),
                    name: model_key.to_string(),
                    available: None,
                    missing: false,
                });
            }
        }
    }

    out.sort_by(|a, b| a.key.cmp(&b.key));
    out.dedup_by(|a, b| a.key == b.key);
    out
}

fn fallback_catalog() -> Vec<ModelCatalogItem> {
    let mut items = vec![
        catalog_item("openai/gpt-5.2", "GPT-5.2"),
        catalog_item("openai/gpt-4.1", "GPT-4.1"),
        catalog_item("openai/o3", "o3"),
        catalog_item("openai/o4-mini", "o4-mini"),
        catalog_item("anthropic/claude-opus-4-6", "Claude Opus 4.6"),
        catalog_item("anthropic/claude-sonnet-4-5", "Claude Sonnet 4.5"),
        catalog_item(
            "anthropic/claude-3-7-sonnet-latest",
            "Claude 3.7 Sonnet Latest",
        ),
        catalog_item("google/gemini-2.5-pro", "Gemini 2.5 Pro"),
        catalog_item("google/gemini-2.5-flash", "Gemini 2.5 Flash"),
        catalog_item("google/gemini-2.0-flash", "Gemini 2.0 Flash"),
        catalog_item("moonshot/kimi-k2-0905-preview", "Kimi K2 0905 Preview"),
        catalog_item("moonshot/kimi-k2-250711", "Kimi K2 250711"),
        catalog_item("moonshot/kimi-2.5", "Kimi 2.5"),
        catalog_item("xai/grok-4", "Grok 4"),
        catalog_item("xai/grok-3", "Grok 3"),
        catalog_item("openrouter/moonshotai/kimi-k2", "OpenRouter Kimi K2"),
        catalog_item(
            "openrouter/anthropic/claude-sonnet-4-5",
            "OpenRouter Claude Sonnet 4.5",
        ),
        catalog_item("zai/glm-4.5", "GLM 4.5"),
        catalog_item("zai/glm-4.5-air", "GLM 4.5 Air"),
        catalog_item("minimax/minimax-m1", "MiniMax M1"),
        catalog_item("minimax/minimax-01", "MiniMax 01"),
        catalog_item("qwen/qwen3-max", "Qwen 3 Max"),
        catalog_item("qwen/qwen3-coder-plus", "Qwen 3 Coder Plus"),
        catalog_item("xiaomi/miq-3", "MiQ 3"),
        catalog_item("venice/llama-3.3-70b", "Venice Llama 3.3 70B"),
        catalog_item("venice/claude-opus-45", "Venice Claude Opus 4.5"),
        catalog_item(
            "bedrock/anthropic.claude-sonnet-4-5",
            "Bedrock Claude Sonnet 4.5",
        ),
        catalog_item("qianfan/ernie-4.0-turbo", "ERNIE 4.0 Turbo"),
    ];
    items.sort_by(|a, b| a.key.cmp(&b.key));
    items.dedup_by(|a, b| a.key == b.key);
    items
}

fn catalog_item(key: &str, name: &str) -> ModelCatalogItem {
    ModelCatalogItem {
        key: key.to_string(),
        provider: provider_from_key(key),
        name: name.to_string(),
        available: None,
        missing: false,
    }
}

fn provider_from_key(model_key: &str) -> String {
    model_key
        .split_once('/')
        .map(|(provider, _)| provider.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

#[cfg(test)]
mod tests {
    use super::{fallback_catalog, parse_models_payload, parse_models_plain, provider_from_key};

    #[test]
    fn parse_models_payload_works_for_pure_json() {
        let raw = r#"{"count":1,"models":[{"key":"openai/gpt-5.2","name":"GPT 5.2","available":true,"missing":false}]}"#;
        let parsed = parse_models_payload(raw).expect("payload should parse");
        assert_eq!(parsed.models.len(), 1);
        assert_eq!(parsed.models[0].key, "openai/gpt-5.2");
    }

    #[test]
    fn parse_models_payload_works_for_log_prefixed_output() {
        let raw = r#"[plugins] feishu_doc: Registered
[plugins] feishu_wiki: Registered
{"count":1,"models":[{"key":"moonshot/kimi-k2-250711","name":"Kimi K2","available":false,"missing":false}]}"#;
        let parsed = parse_models_payload(raw).expect("payload with logs should parse");
        assert_eq!(parsed.models.len(), 1);
        assert_eq!(parsed.models[0].key, "moonshot/kimi-k2-250711");
    }

    #[test]
    fn parse_models_plain_extracts_model_keys() {
        let raw = r#"
openai/gpt-5.2
anthropic/claude-sonnet-4-5 available
[plugins] preface log
"#;
        let parsed = parse_models_plain(raw);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].key, "openai/gpt-5.2");
        assert_eq!(parsed[1].key, "anthropic/claude-sonnet-4-5");
    }

    #[test]
    fn provider_from_key_uses_first_segment() {
        assert_eq!(provider_from_key("openai/gpt-5.2"), "openai");
        assert_eq!(
            provider_from_key("vercel-ai-gateway/openai/gpt-5.2"),
            "vercel-ai-gateway"
        );
        assert_eq!(provider_from_key(""), "unknown");
    }

    #[test]
    fn fallback_catalog_includes_multiple_providers_and_kimi_25() {
        let items = fallback_catalog();
        assert!(items.iter().any(|item| item.key == "moonshot/kimi-2.5"));
        assert!(items.iter().any(|item| item.key == "openai/gpt-5.2"));
        assert!(items
            .iter()
            .any(|item| item.key == "anthropic/claude-sonnet-4-5"));
        assert!(items.iter().any(|item| item.key == "google/gemini-2.5-pro"));
    }
}
