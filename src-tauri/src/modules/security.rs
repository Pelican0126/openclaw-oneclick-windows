use std::fs;
use std::path::Path;

use anyhow::Result;
use regex::Regex;
use walkdir::WalkDir;

use crate::models::{SecurityIssue, SecurityResult, SecuritySeverity};

use super::{logger, paths, shell, state_store};

pub fn run_security_check() -> Result<SecurityResult> {
    let mut issues = Vec::<SecurityIssue>::new();
    let mut score: i32 = 100;

    let config_path = paths::config_path();
    if config_path.exists() {
        let content = fs::read_to_string(&config_path)?;
        if contains_plaintext_key(&content) {
            issues.push(SecurityIssue {
                severity: SecuritySeverity::Medium,
                message: "API key is stored in plaintext in openclaw.json.".to_string(),
                path: Some(config_path.to_string_lossy().to_string()),
                suggestion: Some(
                    "Restrict ACL to current user and rotate key regularly.".to_string(),
                ),
            });
            score -= 15;
        }
        if acl_is_wide_open(&config_path)? {
            issues.push(SecurityIssue {
                severity: SecuritySeverity::High,
                message: "Config ACL appears to allow broad read access.".to_string(),
                path: Some(config_path.to_string_lossy().to_string()),
                suggestion: Some("Run configure again or tighten ACL with icacls.".to_string()),
            });
            score -= 35;
        }
    } else {
        issues.push(SecurityIssue {
            severity: SecuritySeverity::Low,
            message: "Config file does not exist yet.".to_string(),
            path: Some(config_path.to_string_lossy().to_string()),
            suggestion: Some("Run setup wizard to generate config.".to_string()),
        });
        score -= 5;
    }

    let env_path = paths::openclaw_home().join(".env");
    if env_path.exists() {
        let content = fs::read_to_string(&env_path).unwrap_or_default();
        if contains_plaintext_env_key(&content) {
            issues.push(SecurityIssue {
                severity: SecuritySeverity::Medium,
                message: "API key/token appears in plaintext in .env.".to_string(),
                path: Some(env_path.to_string_lossy().to_string()),
                suggestion: Some(
                    "Restrict .env ACL to current user and rotate leaked keys.".to_string(),
                ),
            });
            score -= 15;
        }
        if acl_is_wide_open(&env_path)? {
            issues.push(SecurityIssue {
                severity: SecuritySeverity::High,
                message: ".env ACL appears to allow broad read access.".to_string(),
                path: Some(env_path.to_string_lossy().to_string()),
                suggestion: Some("Re-run setup or tighten ACL with icacls.".to_string()),
            });
            score -= 35;
        }
    }

    for finding in suspicious_scripts() {
        score -= 20;
        issues.push(finding);
    }

    score = score.clamp(0, 100);
    logger::info(&format!(
        "Security check completed. score={score}, issues={}",
        issues.len()
    ));
    Ok(SecurityResult {
        score: score as u8,
        issues,
    })
}

fn contains_plaintext_key(content: &str) -> bool {
    let re =
        Regex::new(r#"(?i)"api_key"\s*:\s*"[^"]+""#).unwrap_or_else(|_| Regex::new("$^").unwrap());
    re.is_match(content)
}

fn contains_plaintext_env_key(content: &str) -> bool {
    let re = Regex::new(r"(?im)^(?:[A-Z0-9_]*(?:API_KEY|TOKEN)[A-Z0-9_]*)\s*=\s*.+$")
        .unwrap_or_else(|_| Regex::new("$^").unwrap());
    re.is_match(content)
}

fn acl_is_wide_open(path: &Path) -> Result<bool> {
    let p = path.to_string_lossy().to_string();
    let out = shell::run_command("icacls", &[&p], None, &[])?;
    let lower = format!(
        "{}\n{}",
        out.stdout.to_lowercase(),
        out.stderr.to_lowercase()
    );
    Ok(lower.contains("everyone:(r)") || lower.contains("builtin\\users:(r)"))
}

fn suspicious_scripts() -> Vec<SecurityIssue> {
    let mut out = Vec::new();
    let mut roots = vec![paths::openclaw_home()];
    if let Ok(Some(state)) = state_store::load_install_state() {
        roots.push(Path::new(&state.install_dir).to_path_buf());
    }
    let pattern =
        Regex::new(r"(?i)(invoke-expression|downloadstring|frombase64string|powershell\s+-enc)")
            .unwrap_or_else(|_| Regex::new("$^").unwrap());
    for root in roots {
        if !root.exists() {
            continue;
        }
        for entry in WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let ext = path
                .extension()
                .map(|v| v.to_string_lossy().to_ascii_lowercase())
                .unwrap_or_default();
            if !["ps1", "bat", "cmd", "vbs", "js"].contains(&ext.as_str()) {
                continue;
            }
            let text = fs::read_to_string(path).unwrap_or_default();
            if pattern.is_match(&text) {
                out.push(SecurityIssue {
                    severity: SecuritySeverity::High,
                    message: "Suspicious script pattern detected.".to_string(),
                    path: Some(path.to_string_lossy().to_string()),
                    suggestion: Some("Review this script before execution.".to_string()),
                });
            }
        }
    }
    out
}
