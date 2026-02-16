use std::fs;

use anyhow::{anyhow, Result};
use serde_json::Value;
use url::Url;

use super::{logger, paths, shell, state_store};

pub fn open_management_url(url: &str) -> Result<String> {
    let parsed = Url::parse(url).map_err(|err| anyhow!("Invalid URL '{url}': {err}"))?;
    let scheme = parsed.scheme().to_ascii_lowercase();
    if scheme != "http" && scheme != "https" {
        return Err(anyhow!("Only http/https URLs are allowed."));
    }

    // Prefer OpenClaw official dashboard URL output. Fallback to local tokenized URL assembly.
    let with_auth = resolve_management_url(parsed)?;
    webbrowser::open(with_auth.as_str())
        .map_err(|err| anyhow!("Failed to open browser for '{}': {err}", with_auth.as_str()))?;

    let masked = mask_management_url(with_auth.as_str());
    logger::info(&format!("Opened management URL: {}", masked));
    Ok(masked)
}

pub fn open_path(path: &str) -> Result<String> {
    let normalized = paths::normalize_path(path)?;
    if !normalized.exists() {
        return Err(anyhow!(
            "Path does not exist: {}",
            normalized.to_string_lossy()
        ));
    }

    let normalized_text = normalized.to_string_lossy().to_string();
    let out = if normalized.is_file() {
        let arg = format!("/select,{}", normalized_text);
        shell::run_command("explorer", &[arg.as_str()], None, &[])?
    } else {
        shell::run_command("explorer", &[normalized_text.as_str()], None, &[])?
    };
    if out.code != 0 {
        return Err(anyhow!(
            "Failed to open path '{}': {}",
            normalized.to_string_lossy(),
            if out.stderr.is_empty() {
                out.stdout
            } else {
                out.stderr
            }
        ));
    }

    logger::info(&format!(
        "Opened path in explorer: {}",
        normalized.to_string_lossy()
    ));
    Ok(normalized.to_string_lossy().to_string())
}

fn resolve_management_url(url: Url) -> Result<Url> {
    if has_auth_fragment(url.fragment()) {
        return Ok(url);
    }
    // Prefer local config token assembly to avoid invoking the CLI (fewer side effects).
    if let Some(token) = read_gateway_token_from_config()? {
        return Ok(with_gateway_token_fragment(url, Some(token.as_str())));
    }

    // Fallback: ask the CLI for the canonical dashboard URL (may include a token fragment).
    if let Some(cli_url) = dashboard_url_from_cli()? {
        let parsed = Url::parse(&cli_url)
            .map_err(|err| anyhow!("Invalid dashboard URL from CLI '{}': {err}", cli_url))?;
        let scheme = parsed.scheme().to_ascii_lowercase();
        if scheme == "http" || scheme == "https" {
            return Ok(parsed);
        }
    }

    Ok(url)
}

fn dashboard_url_from_cli() -> Result<Option<String>> {
    let Some(command) = resolve_dashboard_cli_command() else {
        return Ok(None);
    };

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

    let out = if is_npx_command(command.as_str()) {
        shell::run_command(
            command.as_str(),
            &["--yes", "openclaw", "dashboard", "--no-open"],
            None,
            &envs,
        )?
    } else {
        shell::run_command(command.as_str(), &["dashboard", "--no-open"], None, &envs)?
    };
    if out.code != 0 {
        return Ok(None);
    }
    let merged = format!("{}\n{}", out.stdout, out.stderr);
    Ok(parse_dashboard_url_from_output(&merged))
}

fn resolve_dashboard_cli_command() -> Option<String> {
    if let Ok(Some(install)) = state_store::load_install_state() {
        let command = install.command_path.trim().trim_matches('"').to_string();
        if !command.is_empty() {
            if command.eq_ignore_ascii_case("npx") {
                return shell::command_exists("npx");
            }
            return Some(command);
        }
    }
    if let Some(global) = shell::command_exists("openclaw") {
        return Some(global);
    }
    shell::command_exists("npx")
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

fn parse_dashboard_url_from_output(output: &str) -> Option<String> {
    for line in output.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("Dashboard URL:") {
            let url = rest.trim();
            if Url::parse(url).is_ok() {
                return Some(url.to_string());
            }
        }
        for token in line.split_whitespace() {
            if (token.starts_with("http://") || token.starts_with("https://"))
                && Url::parse(token).is_ok()
            {
                return Some(token.to_string());
            }
        }
    }
    None
}

fn with_gateway_auth_fragment(url: Url) -> Result<Url> {
    if has_auth_fragment(url.fragment()) {
        return Ok(url);
    }

    let Some(token) = read_gateway_token_from_config()? else {
        return Ok(url);
    };
    Ok(with_gateway_token_fragment(url, Some(token.as_str())))
}

fn read_gateway_token_from_config() -> Result<Option<String>> {
    let cfg_path = paths::config_path();
    if !cfg_path.exists() {
        return Ok(None);
    }

    let raw = fs::read_to_string(cfg_path)?;
    let json: Value = serde_json::from_str(&raw)?;
    let mode = json
        .pointer("/gateway/auth/mode")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    if !mode.eq_ignore_ascii_case("token") {
        return Ok(None);
    }
    Ok(json
        .pointer("/gateway/auth/token")
        .and_then(|v| v.as_str())
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty()))
}

fn has_auth_fragment(fragment: Option<&str>) -> bool {
    let Some(fragment) = fragment else {
        return false;
    };
    fragment
        .split('&')
        .filter_map(|entry| entry.split('=').next())
        .any(|k| k.eq_ignore_ascii_case("token") || k.eq_ignore_ascii_case("password"))
}

fn mask_management_url(url: &str) -> String {
    let Ok(mut parsed) = Url::parse(url) else {
        return url.to_string();
    };
    let masked_fragment = parsed.fragment().map(mask_fragment);
    if let Some(fragment) = masked_fragment {
        parsed.set_fragment(Some(&fragment));
    }
    parsed.to_string()
}

fn mask_fragment(fragment: &str) -> String {
    fragment
        .split('&')
        .map(|entry| {
            let mut parts = entry.splitn(2, '=');
            let key = parts.next().unwrap_or_default();
            let value = parts.next();
            if key.eq_ignore_ascii_case("token") || key.eq_ignore_ascii_case("password") {
                format!("{key}=***")
            } else if let Some(v) = value {
                format!("{key}={v}")
            } else {
                key.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("&")
}

fn with_gateway_token_fragment(mut url: Url, token: Option<&str>) -> Url {
    if has_auth_fragment(url.fragment()) {
        return url;
    }
    let Some(token) = token else {
        return url;
    };
    if token.trim().is_empty() {
        return url;
    }
    let encoded: String = url::form_urlencoded::byte_serialize(token.as_bytes()).collect();
    url.set_fragment(Some(&format!("token={encoded}")));
    url
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_auth_fragment_keys() {
        assert!(has_auth_fragment(Some("token=abc")));
        assert!(has_auth_fragment(Some("foo=1&password=abc")));
        assert!(!has_auth_fragment(Some("foo=1&bar=2")));
        assert!(!has_auth_fragment(None));
    }

    #[test]
    fn masks_token_in_fragment() {
        let out = mask_management_url("http://127.0.0.1:18789/#token=abcdef&tab=chat");
        assert!(out.contains("#token=***&tab=chat"));
    }

    #[test]
    fn appends_token_fragment_when_missing() {
        let url = Url::parse("http://127.0.0.1:18789/").expect("url");
        let out = with_gateway_token_fragment(url, Some("abc123"));
        assert_eq!(out.as_str(), "http://127.0.0.1:18789/#token=abc123");
    }

    #[test]
    fn keeps_existing_token_fragment() {
        let url = Url::parse("http://127.0.0.1:18789/#token=existing").expect("url");
        let out = with_gateway_token_fragment(url, Some("newone"));
        assert_eq!(out.as_str(), "http://127.0.0.1:18789/#token=existing");
    }

    #[test]
    fn parses_dashboard_url_from_cli_output() {
        let raw = "Dashboard URL: http://127.0.0.1:18789/#token=abc123";
        let out = parse_dashboard_url_from_output(raw);
        assert_eq!(out.as_deref(), Some("http://127.0.0.1:18789/#token=abc123"));
    }
}
