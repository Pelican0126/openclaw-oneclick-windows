use anyhow::{anyhow, Result};
use serde::Deserialize;
use serde_json::{Deserializer, Value};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use crate::models::SkillCatalogItem;

use super::{logger, shell};

const SKILL_CATALOG_CLI_TIMEOUT: Duration = Duration::from_millis(1_600);

#[derive(Debug, Deserialize)]
struct SkillsListPayload {
    #[serde(default)]
    skills: Vec<SkillEntry>,
}

#[derive(Debug, Deserialize)]
struct SkillEntry {
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    eligible: bool,
    #[serde(default)]
    bundled: bool,
    #[serde(default)]
    source: String,
}

pub fn list_skill_catalog() -> Result<Vec<SkillCatalogItem>> {
    if let Ok(items) = list_from_openclaw_cli_with_timeout(SKILL_CATALOG_CLI_TIMEOUT) {
        if !items.is_empty() {
            return Ok(items);
        }
    }

    logger::warn(
        "Falling back to static skill catalog because OpenClaw CLI skill list is unavailable.",
    );
    Ok(fallback_catalog())
}

fn list_from_openclaw_cli_with_timeout(timeout: Duration) -> Result<Vec<SkillCatalogItem>> {
    let (tx, rx) = mpsc::channel::<Result<Vec<SkillCatalogItem>>>();
    thread::spawn(move || {
        let _ = tx.send(list_from_openclaw_cli());
    });
    match rx.recv_timeout(timeout) {
        Ok(result) => result,
        Err(mpsc::RecvTimeoutError::Timeout) => {
            logger::warn(&format!(
                "Skills catalog CLI timed out after {} ms; fallback catalog will be used.",
                timeout.as_millis()
            ));
            Ok(vec![])
        }
        Err(err) => Err(anyhow!("Skills catalog worker channel failed: {err}")),
    }
}

fn list_from_openclaw_cli() -> Result<Vec<SkillCatalogItem>> {
    let output = if let Some(openclaw) = shell::command_exists("openclaw") {
        shell::run_command(openclaw.as_str(), &["skills", "list", "--json"], None, &[])?
    } else if let Some(npx) = shell::command_exists("npx") {
        shell::run_command(
            npx.as_str(),
            &["--yes", "openclaw", "skills", "list", "--json"],
            None,
            &[],
        )?
    } else {
        return Ok(vec![]);
    };

    if output.code != 0 {
        logger::warn(&format!(
            "openclaw skills list failed: {}",
            if output.stderr.is_empty() {
                output.stdout
            } else {
                output.stderr
            }
        ));
        return Ok(vec![]);
    }

    let parsed: SkillsListPayload = parse_skills_payload(&output.stdout)?;
    let mut out = parsed
        .skills
        .into_iter()
        .map(|item| SkillCatalogItem {
            name: item.name,
            description: item.description,
            eligible: item.eligible,
            bundled: item.bundled,
            source: item.source,
        })
        .collect::<Vec<_>>();

    out.sort_by(|a, b| {
        b.eligible
            .cmp(&a.eligible)
            .then_with(|| a.name.cmp(&b.name))
    });
    Ok(out)
}

fn parse_skills_payload(raw: &str) -> Result<SkillsListPayload> {
    if let Ok(parsed) = serde_json::from_str::<SkillsListPayload>(raw) {
        return Ok(parsed);
    }

    let trimmed = raw.trim_start_matches('\u{feff}');
    if let Ok(parsed) = serde_json::from_str::<SkillsListPayload>(trimmed) {
        return Ok(parsed);
    }

    let mut search_start = 0usize;
    while let Some(offset) = trimmed[search_start..].find('{') {
        let start = search_start + offset;
        let candidate = &trimmed[start..];
        let mut stream = Deserializer::from_str(candidate).into_iter::<Value>();
        if let Some(value) = stream.next() {
            if let Ok(json) = value {
                if let Ok(parsed) = serde_json::from_value::<SkillsListPayload>(json) {
                    return Ok(parsed);
                }
            }
        }
        search_start = start + 1;
    }

    Err(anyhow!(
        "openclaw skills list did not return a valid JSON payload"
    ))
}

fn fallback_catalog() -> Vec<SkillCatalogItem> {
    vec![
        SkillCatalogItem {
            name: "healthcheck".to_string(),
            description: "Host security hardening and periodic security checks.".to_string(),
            eligible: true,
            bundled: true,
            source: "openclaw-bundled".to_string(),
        },
        SkillCatalogItem {
            name: "skill-creator".to_string(),
            description: "Create or update AgentSkills.".to_string(),
            eligible: true,
            bundled: true,
            source: "openclaw-bundled".to_string(),
        },
        SkillCatalogItem {
            name: "github".to_string(),
            description: "GitHub CLI integration for issues/PRs/actions.".to_string(),
            eligible: false,
            bundled: true,
            source: "openclaw-bundled".to_string(),
        },
        SkillCatalogItem {
            name: "weather".to_string(),
            description: "Weather queries and forecast helper.".to_string(),
            eligible: false,
            bundled: true,
            source: "openclaw-bundled".to_string(),
        },
        SkillCatalogItem {
            name: "clawhub".to_string(),
            description: "Manage extra skills from clawhub.com.".to_string(),
            eligible: false,
            bundled: true,
            source: "openclaw-bundled".to_string(),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::parse_skills_payload;

    #[test]
    fn parse_skills_payload_works_for_pure_json() {
        let raw = r#"{"skills":[{"name":"healthcheck","description":"ok","eligible":true,"bundled":true,"source":"openclaw-bundled"}]}"#;
        let parsed = parse_skills_payload(raw).expect("payload should parse");
        assert_eq!(parsed.skills.len(), 1);
        assert_eq!(parsed.skills[0].name, "healthcheck");
    }

    #[test]
    fn parse_skills_payload_works_for_log_prefixed_output() {
        let raw = r#"[plugins] feishu_doc: Registered
[plugins] feishu_wiki: Registered
{"skills":[{"name":"feishu-doc","description":"doc","eligible":true,"bundled":false,"source":"openclaw-extra"}]}"#;
        let parsed = parse_skills_payload(raw).expect("payload with logs should parse");
        assert_eq!(parsed.skills.len(), 1);
        assert_eq!(parsed.skills[0].name, "feishu-doc");
    }
}
