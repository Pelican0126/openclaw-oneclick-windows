pub fn normalize_known_model_key(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let lowered = trimmed.to_ascii_lowercase();
    if lowered == "moonshot/kimi-2.5" || lowered == "moonshot/kimi2.5" {
        return "moonshot/kimi-k2.5".to_string();
    }

    trimmed.to_string()
}

pub fn provider_from_model_key(model: &str) -> Option<&str> {
    let (provider, model_name) = model.split_once('/')?;
    if provider.trim().is_empty() || model_name.trim().is_empty() {
        return None;
    }
    Some(provider.trim())
}

pub fn provider_from_model_key_or_unknown(model: &str) -> String {
    provider_from_model_key(model)
        .map(|provider| provider.to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

pub fn normalize_auth_provider(provider: &str) -> String {
    match provider.trim().to_ascii_lowercase().as_str() {
        "openai-codex" => "openai".to_string(),
        "kimi-code" => "kimi-coding".to_string(),
        other => other.to_string(),
    }
}

pub fn provider_env_name(provider: &str) -> Option<String> {
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

#[cfg(test)]
mod tests {
    use super::{
        normalize_auth_provider, normalize_known_model_key, provider_env_name,
        provider_from_model_key, provider_from_model_key_or_unknown,
    };

    #[test]
    fn normalize_known_model_key_maps_legacy_kimi_alias() {
        assert_eq!(
            normalize_known_model_key(" moonshot/kimi-2.5 "),
            "moonshot/kimi-k2.5"
        );
        assert_eq!(
            normalize_known_model_key("moonshot/kimi2.5"),
            "moonshot/kimi-k2.5"
        );
    }

    #[test]
    fn provider_from_model_key_extracts_provider_prefix() {
        assert_eq!(provider_from_model_key("openai/gpt-5.2"), Some("openai"));
        assert_eq!(
            provider_from_model_key("vercel-ai-gateway/openai/gpt-5.2"),
            Some("vercel-ai-gateway")
        );
        assert_eq!(provider_from_model_key(""), None);
    }

    #[test]
    fn provider_from_model_key_or_unknown_handles_invalid_values() {
        assert_eq!(
            provider_from_model_key_or_unknown("openai/gpt-5.2"),
            "openai"
        );
        assert_eq!(provider_from_model_key_or_unknown("invalid"), "unknown");
    }

    #[test]
    fn normalize_auth_provider_maps_known_aliases() {
        assert_eq!(normalize_auth_provider("openai-codex"), "openai");
        assert_eq!(normalize_auth_provider("KIMI-CODE"), "kimi-coding");
        assert_eq!(normalize_auth_provider("xai"), "xai");
    }

    #[test]
    fn provider_env_name_resolves_known_and_generic_providers() {
        assert_eq!(
            provider_env_name("openai-codex"),
            Some("OPENAI_API_KEY".to_string())
        );
        assert_eq!(
            provider_env_name("kimi-code"),
            Some("KIMI_API_KEY".to_string())
        );
        assert_eq!(
            provider_env_name("custom-provider"),
            Some("CUSTOM_PROVIDER_API_KEY".to_string())
        );
    }
}
