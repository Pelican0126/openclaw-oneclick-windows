use anyhow::Result;
use reqwest::Client;
use std::time::Duration;

use crate::models::{DependencyStatus, EnvCheckResult, InstallEnvResult};

use super::{logger, paths, port, shell};

pub async fn check_env(port_number: u16) -> Result<EnvCheckResult> {
    paths::ensure_dirs()?;
    let dependencies = dependency_status();
    let port_status = port::check_port(port_number)?;
    let os = shell::run_command("cmd", &["/C", "ver"], None, &[])
        .map(|o| o.stdout)
        .unwrap_or_else(|_| "Windows".to_string());

    let network = check_network().await;

    Ok(EnvCheckResult {
        os,
        is_windows: cfg!(windows),
        is_admin: shell::is_admin(),
        network_ok: network.0,
        network_detail: network.1,
        dependencies,
        port_status,
    })
}

pub fn install_env(_port_number: u16) -> Result<InstallEnvResult> {
    let mut installed = Vec::new();
    let mut skipped = Vec::new();
    let mut warnings = Vec::new();

    let deps = dependency_status();
    let has_git = deps.iter().any(|d| d.name == "git" && d.found);
    let has_node = deps.iter().any(|d| d.name == "node" && d.found);
    let has_npm = deps.iter().any(|d| d.name == "npm" && d.found);
    let has_bun = deps.iter().any(|d| d.name == "bun" && d.found);
    let has_winget = deps.iter().any(|d| d.name == "winget" && d.found);
    let has_choco = deps.iter().any(|d| d.name == "choco" && d.found);
    let has_vcredist = deps.iter().any(|d| d.name == "vcredist" && d.found);
    let node_major = node_major_version();
    let node_supported = node_major.map(|v| v >= 22).unwrap_or(false);

    if has_git {
        skipped.push("git".to_string());
    } else if has_winget {
        match shell::run_command(
            "winget",
            &[
                "install",
                "--id",
                "Git.Git",
                "-e",
                "--source",
                "winget",
                "--accept-package-agreements",
                "--accept-source-agreements",
            ],
            None,
            &[],
        ) {
            Ok(out) if out.code == 0 => installed.push("git".to_string()),
            Ok(out) => warnings.push(format!("git install failed: {}", out.stderr)),
            Err(err) => warnings.push(format!("git install failed: {err}")),
        }
    } else if has_choco {
        match shell::run_command("choco", &["install", "git", "-y"], None, &[]) {
            Ok(out) if out.code == 0 => installed.push("git".to_string()),
            Ok(out) => warnings.push(format!("git install failed: {}", out.stderr)),
            Err(err) => warnings.push(format!("git install failed: {err}")),
        }
    } else {
        warnings.push("Neither winget nor choco found. Install Git manually.".to_string());
    }

    if has_bun || (has_node && has_npm && node_supported) {
        skipped.push("node-or-bun".to_string());
    } else if has_node && has_npm && !node_supported {
        warnings.push(format!(
            "Node.js version {:?} detected, OpenClaw requires Node.js 22+; trying upgrade.",
            node_major
        ));
        if has_winget {
            match shell::run_command(
                "winget",
                &[
                    "install",
                    "--id",
                    "OpenJS.NodeJS.LTS",
                    "-e",
                    "--source",
                    "winget",
                    "--accept-package-agreements",
                    "--accept-source-agreements",
                ],
                None,
                &[],
            ) {
                Ok(out) if out.code == 0 => installed.push("nodejs-lts".to_string()),
                Ok(out) => warnings.push(format!("node upgrade failed: {}", out.stderr)),
                Err(err) => warnings.push(format!("node upgrade failed: {err}")),
            }
        } else if has_choco {
            match shell::run_command("choco", &["upgrade", "nodejs-lts", "-y"], None, &[]) {
                Ok(out) if out.code == 0 => installed.push("nodejs-lts".to_string()),
                Ok(out) => warnings.push(format!("node upgrade failed: {}", out.stderr)),
                Err(err) => warnings.push(format!("node upgrade failed: {err}")),
            }
        } else {
            warnings.push(
                "Node.js is below 22 and no winget/choco is available for auto-upgrade."
                    .to_string(),
            );
        }
    } else if has_winget {
        match shell::run_command(
            "winget",
            &[
                "install",
                "--id",
                "OpenJS.NodeJS.LTS",
                "-e",
                "--source",
                "winget",
                "--accept-package-agreements",
                "--accept-source-agreements",
            ],
            None,
            &[],
        ) {
            Ok(out) if out.code == 0 => installed.push("nodejs-lts".to_string()),
            Ok(out) => warnings.push(format!("node install failed: {}", out.stderr)),
            Err(err) => warnings.push(format!("node install failed: {err}")),
        }
    } else if has_choco {
        match shell::run_command("choco", &["install", "nodejs-lts", "-y"], None, &[]) {
            Ok(out) if out.code == 0 => installed.push("nodejs-lts".to_string()),
            Ok(out) => warnings.push(format!("node install failed: {}", out.stderr)),
            Err(err) => warnings.push(format!("node install failed: {err}")),
        }
    } else {
        warnings
            .push("Neither winget nor choco found. Install Node.js or Bun manually.".to_string());
    }

    if has_vcredist {
        skipped.push("vcredist".to_string());
    } else if has_winget {
        match shell::run_command(
            "winget",
            &[
                "install",
                "--id",
                "Microsoft.VCRedist.2015+.x64",
                "-e",
                "--source",
                "winget",
                "--accept-package-agreements",
                "--accept-source-agreements",
            ],
            None,
            &[],
        ) {
            Ok(out) if out.code == 0 => installed.push("vcredist".to_string()),
            Ok(out) => warnings.push(format!("vcredist install failed: {}", out.stderr)),
            Err(err) => warnings.push(format!("vcredist install failed: {err}")),
        }
    } else {
        warnings.push(
            "Visual C++ runtime not detected; install Microsoft VC++ Redistributable x64."
                .to_string(),
        );
    }

    if warnings.is_empty() {
        logger::info("Environment dependency installation completed successfully.");
    } else {
        logger::warn(&format!(
            "Environment installation warnings: {}",
            warnings.join(" | ")
        ));
    }

    Ok(InstallEnvResult {
        installed,
        skipped,
        warnings,
    })
}

fn dependency_status() -> Vec<DependencyStatus> {
    let mut deps: Vec<DependencyStatus> = ["git", "node", "npm", "bun", "winget", "choco"]
        .iter()
        .map(|name| DependencyStatus {
            name: (*name).to_string(),
            found: shell::command_exists(name).is_some(),
            path: shell::command_exists(name),
        })
        .collect();
    deps.push(DependencyStatus {
        name: "vcredist".to_string(),
        found: has_vc_runtime(),
        path: None,
    });
    deps
}

async fn check_network() -> (bool, String) {
    let client = match Client::builder().timeout(Duration::from_secs(5)).build() {
        Ok(c) => c,
        Err(err) => return (false, format!("Failed to init HTTP client: {err}")),
    };

    match client
        .get("https://docs.openclaw.ai")
        .header("User-Agent", "openclaw-installer/0.1.0")
        .send()
        .await
    {
        Ok(resp) => {
            if resp.status().is_success() {
                (true, "docs.openclaw.ai reachable".to_string())
            } else {
                (false, format!("HTTP {}", resp.status()))
            }
        }
        Err(err) => (false, format!("Network check failed: {err}")),
    }
}

fn has_vc_runtime() -> bool {
    let keys = [
        r#"HKLM\SOFTWARE\Microsoft\VisualStudio\14.0\VC\Runtimes\x64"#,
        r#"HKLM\SOFTWARE\WOW6432Node\Microsoft\VisualStudio\14.0\VC\Runtimes\x64"#,
    ];
    for key in keys {
        if let Ok(out) = shell::run_command("reg", &["query", key, "/v", "Installed"], None, &[]) {
            if out.code == 0 && out.stdout.contains("0x1") {
                return true;
            }
        }
    }
    false
}

fn node_major_version() -> Option<u32> {
    let out = shell::run_command("node", &["--version"], None, &[]).ok()?;
    if out.code != 0 {
        return None;
    }
    let raw = out.stdout.trim().trim_start_matches('v');
    raw.split('.').next()?.parse::<u32>().ok()
}
