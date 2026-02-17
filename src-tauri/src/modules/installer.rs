use std::collections::HashSet;
use std::fs;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use reqwest::Client;

use crate::models::{
    InstallResult, InstallState, OpenClawConfigInput, SourceMethod, UninstallResult,
};

use super::{logger, paths, process, shell, state_store};

pub async fn install_openclaw(payload: &OpenClawConfigInput) -> Result<InstallResult> {
    install_openclaw_inner(payload, false).await
}

pub async fn install_openclaw_for_upgrade(payload: &OpenClawConfigInput) -> Result<InstallResult> {
    install_openclaw_inner(payload, true).await
}

async fn install_openclaw_inner(
    payload: &OpenClawConfigInput,
    allow_reinstall: bool,
) -> Result<InstallResult> {
    if !allow_reinstall {
        // Hard lock: once install state exists, installer flow must not reinstall
        // until user explicitly uninstalls from Maintenance.
        if let Some(existing) = state_store::load_install_state()? {
            return Err(anyhow!(
                "OpenClaw is already installed at {} (version {}). Uninstall first before reinstalling.",
                existing.install_dir,
                existing.version
            ));
        }
    }
    let install_dir = paths::normalize_path(&payload.install_dir)?;
    if paths::is_user_profile_default_openclaw_dir(&install_dir) {
        return Err(anyhow!(
            "Unsafe install directory detected: {}. For isolation, choose a different folder (recommended: %LOCALAPPDATA%\\\\OpenClawInstaller\\\\openclaw).",
            install_dir.to_string_lossy()
        ));
    }
    // Keep installer and OpenClaw state strictly bound to the chosen install directory.
    // This prevents mixing with any existing `%USERPROFILE%\\.openclaw` on the machine.
    std::env::set_var(
        "OPENCLAW_INSTALLER_OPENCLAW_HOME",
        install_dir.to_string_lossy().to_string(),
    );
    paths::ensure_dirs()?;
    fs::create_dir_all(&install_dir)?;

    let env_vars = proxy_env(payload);

    match &payload.source_method {
        SourceMethod::Npm => install_from_npm(&install_dir, &env_vars)?,
        SourceMethod::Bun => install_from_bun(&install_dir, &env_vars)?,
        SourceMethod::Git => install_from_git(&install_dir, payload, &env_vars)?,
        SourceMethod::Binary => install_from_binary(&install_dir, payload, &env_vars).await?,
    }

    let command_path = resolve_command_path(
        &install_dir,
        &payload.source_method,
        payload.source_url.clone(),
    )?;
    let version = detect_version(&command_path).unwrap_or_else(|_| "unknown".to_string());
    let install_state = InstallState {
        method: payload.source_method.clone(),
        install_dir: install_dir.to_string_lossy().to_string(),
        source_url: payload.source_url.clone(),
        command_path: command_path.clone(),
        version: version.clone(),
        launch_args: payload.launch_args.clone(),
    };
    state_store::save_install_state(&install_state)?;
    logger::info(&format!(
        "OpenClaw installed using {:?} at {}",
        &payload.source_method, install_state.install_dir
    ));

    Ok(InstallResult {
        method: format!("{:?}", &payload.source_method).to_lowercase(),
        install_dir: install_dir.to_string_lossy().to_string(),
        version,
        command_path,
    })
}

fn install_from_npm(install_dir: &Path, env_vars: &[(String, String)]) -> Result<()> {
    let npm_exe = shell::command_exists("npm")
        .ok_or_else(|| anyhow!("npm not found. Please install Node.js first."))?;
    ensure_local_package_json(install_dir)?;

    // IMPORTANT: Never install globally. Global installs can overwrite an existing OpenClaw
    // the user is already using on this machine.
    let dir = install_dir.to_string_lossy().to_string();
    logger::info(&format!(
        "Installing OpenClaw locally: npm --prefix \"{}\" install openclaw@latest",
        dir
    ));
    let install_args: Vec<&str> = vec![
        "--prefix",
        dir.as_str(),
        "install",
        "openclaw@latest",
        "--no-audit",
        "--no-fund",
        "--loglevel",
        "error",
    ];
    let attempts = npm_install_attempts(env_vars);
    let mut out: Option<shell::CmdOutput> = None;
    for attempt in attempts {
        logger::info(&format!("npm install attempt: {}", attempt.label));
        let current = shell::run_command(
            npm_exe.as_str(),
            &install_args,
            None,
            attempt.env.as_slice(),
        )
        .with_context(|| format!("failed to start npm executable: {npm_exe}"))?;
        log_command_output(
            &format!("npm install openclaw@latest (local) [{}]", attempt.label),
            &current,
        );
        if current.code == 0 {
            return Ok(());
        }
        let retry_with_next_route = is_npm_git_fetch_failure(&current);
        out = Some(current);
        if !retry_with_next_route {
            break;
        }
        logger::warn(&format!(
            "npm install attempt '{}' failed with git transport/auth issue; trying next fallback route.",
            attempt.label
        ));
    }
    let out = out.ok_or_else(|| anyhow!("npm install openclaw@latest did not run."))?;

    if let Some(existing) = shell::command_exists("openclaw") {
        if command_is_usable(existing.as_str()) {
            logger::warn(&format!(
                "npm local install failed, fallback to existing openclaw binary: {existing}"
            ));
            return Ok(());
        }
        logger::warn(&format!(
            "Found global openclaw but it is not runnable: {}",
            existing
        ));
    }
    if is_npm_git_fetch_failure(&out) {
        return Err(anyhow!(
            "npm install openclaw@latest (local) failed after mirror retries. Git dependencies from GitHub are unreachable or unauthorized in current network. Configure a working HTTP(S) proxy in Wizard -> Advanced, or allow access to github.com / gitclone.com / gh.llkk.cc. Last error: {}",
            if out.stderr.is_empty() {
                out.stdout.clone()
            } else {
                out.stderr.clone()
            }
        ));
    }
    shell::ensure_success("npm install openclaw@latest (local)", &out)?;
    Ok(())
}

fn ensure_local_package_json(install_dir: &Path) -> Result<()> {
    let path = install_dir.join("package.json");
    if path.exists() {
        return Ok(());
    }
    // Minimal package.json to make `npm --prefix <dir> install ...` deterministic.
    let content = "{\n  \"name\": \"openclaw-installer-local\",\n  \"private\": true\n}\n";
    fs::write(&path, content)?;
    Ok(())
}

fn is_npm_git_fetch_failure(out: &shell::CmdOutput) -> bool {
    let text = merged_output_lower(out);
    let has_git_error = text.contains("npm error code 128")
        || text.contains("unknown git error")
        || text.contains("ls-remote")
        || text.contains("git --no-replace-objects");
    if !has_git_error {
        return false;
    }
    let has_transport_or_auth = text.contains("failed to connect")
        || text.contains("could not connect to server")
        || text.contains("timed out")
        || text.contains("connection reset")
        || text.contains("recv failure")
        || text.contains("permission denied (publickey)")
        || text.contains("could not read from remote repository")
        || text.contains("unable to access");
    has_transport_or_auth || text.contains("github.com") || text.contains("libsignal-node")
}

fn merged_output_lower(out: &shell::CmdOutput) -> String {
    let mut merged = out.stdout.clone();
    merged.push('\n');
    merged.push_str(&out.stderr);
    merged.to_ascii_lowercase()
}

#[derive(Debug, Clone)]
struct NpmInstallAttempt {
    label: String,
    env: Vec<(String, String)>,
}

fn npm_install_attempts(base_env: &[(String, String)]) -> Vec<NpmInstallAttempt> {
    let mut attempts = Vec::new();
    attempts.push(NpmInstallAttempt {
        label: "direct-github".to_string(),
        env: npm_git_env(base_env),
    });
    for mirror in [
        "https://gitclone.com/github.com/",
        "https://gh.llkk.cc/https://github.com/",
    ] {
        attempts.push(NpmInstallAttempt {
            label: format!("mirror:{mirror}"),
            env: npm_git_env_with_mirror(base_env, mirror),
        });
    }
    attempts
}

fn install_from_bun(install_dir: &Path, env_vars: &[(String, String)]) -> Result<()> {
    let bun_exe = shell::command_exists("bun").ok_or_else(|| anyhow!("bun not found."))?;
    let dir = install_dir.to_string_lossy().to_string();
    let out = shell::run_command(
        bun_exe.as_str(),
        &["add", "--cwd", dir.as_str(), "openclaw@latest"],
        None,
        env_vars,
    )
    .with_context(|| format!("failed to start bun executable: {bun_exe}"))?;
    log_command_output("bun add openclaw@latest", &out);
    shell::ensure_success("bun add openclaw@latest", &out)?;
    Ok(())
}

fn install_from_git(
    install_dir: &Path,
    payload: &OpenClawConfigInput,
    env_vars: &[(String, String)],
) -> Result<()> {
    let git_exe = shell::command_exists("git").ok_or_else(|| anyhow!("git not found."))?;
    let git_url = payload
        .source_url
        .clone()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "https://github.com/openclaw/openclaw.git".to_string());
    let git_dir = install_dir.join(".git");
    if git_dir.exists() {
        let dir = install_dir.to_string_lossy().to_string();
        let out = shell::run_command(
            git_exe.as_str(),
            &["-C", dir.as_str(), "pull", "--ff-only"],
            None,
            env_vars,
        )
        .with_context(|| format!("failed to start git executable: {git_exe}"))?;
        log_command_output("git pull --ff-only", &out);
        shell::ensure_success("git pull", &out)?;
    } else {
        let dir = install_dir.to_string_lossy().to_string();
        let out = shell::run_command(
            git_exe.as_str(),
            &["clone", git_url.as_str(), dir.as_str()],
            None,
            env_vars,
        )
        .with_context(|| format!("failed to start git executable: {git_exe}"))?;
        log_command_output("git clone", &out);
        shell::ensure_success("git clone", &out)?;
    }
    if install_dir.join("package.json").exists() {
        let npm_exe = shell::command_exists("npm");
        if let Some(npm_exe) = npm_exe {
            let dir = install_dir.to_string_lossy().to_string();
            let out = shell::run_command(
                npm_exe.as_str(),
                &["install", "--prefix", dir.as_str()],
                None,
                env_vars,
            )
            .with_context(|| format!("failed to start npm executable: {npm_exe}"))?;
            log_command_output("npm install --prefix", &out);
            shell::ensure_success("npm install", &out)?;
        }
    }
    Ok(())
}

async fn install_from_binary(
    install_dir: &Path,
    payload: &OpenClawConfigInput,
    env_vars: &[(String, String)],
) -> Result<()> {
    let url = payload
        .source_url
        .clone()
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| anyhow!("Binary source_url is required."))?;
    let mut client = Client::builder();
    if let Some(proxy) = env_vars
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("HTTPS_PROXY"))
        .map(|(_, v)| v.to_string())
    {
        client = client.proxy(reqwest::Proxy::https(proxy)?);
    }
    let client = client.build()?;
    let resp = client.get(url.clone()).send().await?;
    if !resp.status().is_success() {
        return Err(anyhow!("Binary download failed: HTTP {}", resp.status()));
    }
    let bytes = resp.bytes().await?;
    let out = install_dir.join("openclaw.exe");
    fs::write(out, &bytes)?;
    logger::info("Binary download complete.");
    Ok(())
}

fn resolve_command_path(
    install_dir: &Path,
    method: &SourceMethod,
    source_url: Option<String>,
) -> Result<String> {
    match method {
        SourceMethod::Binary => Ok(install_dir
            .join("openclaw.exe")
            .to_string_lossy()
            .to_string()),
        SourceMethod::Git => {
            let candidates = [
                install_dir.join("openclaw.exe"),
                install_dir
                    .join("node_modules")
                    .join(".bin")
                    .join("openclaw.cmd"),
                install_dir
                    .join("node_modules")
                    .join(".bin")
                    .join("openclaw"),
            ];
            for candidate in candidates {
                if candidate.exists() {
                    let text = candidate.to_string_lossy().to_string();
                    if command_is_usable(&text) {
                        return Ok(text);
                    }
                    logger::warn(&format!(
                        "Detected unusable OpenClaw command candidate: {text}"
                    ));
                }
            }
            if let Some(global) = shell::command_exists("openclaw") {
                if command_is_usable(global.as_str()) {
                    return Ok(global);
                }
            }
            if let Some(url) = source_url {
                logger::warn(&format!(
                    "Could not detect command path from git source {url}, fallback to npx."
                ));
            }
            Ok("npx".to_string())
        }
        SourceMethod::Npm => {
            // Prefer the locally installed shim under install_dir so we stay isolated and
            // do not depend on (or override) any global OpenClaw installation.
            let candidates = [
                install_dir
                    .join("node_modules")
                    .join(".bin")
                    .join("openclaw.cmd"),
                install_dir
                    .join("node_modules")
                    .join(".bin")
                    .join("openclaw"),
                install_dir
                    .join("node_modules")
                    .join(".bin")
                    .join("openclaw.ps1"),
                install_dir.join("openclaw.exe"),
            ];
            for candidate in candidates {
                if candidate.exists() {
                    let text = candidate.to_string_lossy().to_string();
                    if command_is_usable(&text) {
                        return Ok(text);
                    }
                    logger::warn(&format!(
                        "Detected unusable OpenClaw command candidate: {text}"
                    ));
                }
            }

            if let Some(global) = resolve_global_openclaw() {
                return Ok(global);
            }
            if let Some(local_home_cmd) = resolve_local_home_openclaw() {
                return Ok(local_home_cmd);
            }
            Ok("npx".to_string())
        }
        SourceMethod::Bun => {
            if let Some(global) = resolve_global_openclaw() {
                return Ok(global);
            }
            let candidates = [
                install_dir.join("openclaw.cmd"),
                install_dir.join("openclaw"),
                install_dir
                    .join("node_modules")
                    .join(".bin")
                    .join("openclaw.cmd"),
                install_dir
                    .join("node_modules")
                    .join(".bin")
                    .join("openclaw"),
            ];
            for candidate in candidates {
                if candidate.exists() {
                    let text = candidate.to_string_lossy().to_string();
                    if command_is_usable(&text) {
                        return Ok(text);
                    }
                    logger::warn(&format!(
                        "Detected unusable OpenClaw command candidate: {text}"
                    ));
                }
            }
            Ok("npx".to_string())
        }
    }
}

fn detect_version(command_path: &str) -> Result<String> {
    if command_path.eq_ignore_ascii_case("npx") {
        let Some(npx_exe) = shell::command_exists("npx") else {
            return Ok("unknown".to_string());
        };
        let out = shell::run_command(
            npx_exe.as_str(),
            &["--yes", "openclaw", "--version"],
            None,
            &[],
        )?;
        if out.code == 0 {
            return Ok(first_line_or_unknown(&out.stdout));
        }
        return Ok("unknown".to_string());
    }
    match shell::run_command(command_path, &["--version"], None, &[]) {
        Ok(out) if out.code == 0 => Ok(first_line_or_unknown(&out.stdout)),
        _ => Ok("unknown".to_string()),
    }
}

fn command_is_usable(command_path: &str) -> bool {
    if command_path.eq_ignore_ascii_case("npx") {
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
    let Ok(out) = shell::run_command(command_path, &["--version"], None, &[]) else {
        return false;
    };
    out.code == 0
}

fn resolve_global_openclaw() -> Option<String> {
    let global = shell::command_exists("openclaw")?;
    if command_is_usable(global.as_str()) {
        return Some(global);
    }
    None
}

fn resolve_local_home_openclaw() -> Option<String> {
    let candidates = [
        paths::openclaw_home().join("openclaw.cmd"),
        paths::openclaw_home().join("openclaw"),
        paths::openclaw_home().join("openclaw.exe"),
    ];
    for candidate in candidates {
        if candidate.exists() {
            let text = candidate.to_string_lossy().to_string();
            if command_is_usable(text.as_str()) {
                return Some(text);
            }
        }
    }
    None
}

fn log_command_output(op: &str, out: &shell::CmdOutput) {
    logger::info(&format!("{op} finished with code={}", out.code));
    if !out.stdout.trim().is_empty() {
        logger::info(&format!("{op} stdout: {}", compact_text(&out.stdout, 2800)));
    }
    if !out.stderr.trim().is_empty() {
        logger::warn(&format!("{op} stderr: {}", compact_text(&out.stderr, 2800)));
    }
}

fn compact_text(raw: &str, max_len: usize) -> String {
    let mut text = raw.replace('\r', "");
    if text.len() > max_len {
        text.truncate(max_len);
        text.push_str(" ...<truncated>");
    }
    text
}

fn first_line_or_unknown(raw: &str) -> String {
    raw.lines()
        .next()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

fn proxy_env(payload: &OpenClawConfigInput) -> Vec<(String, String)> {
    let mut envs = Vec::new();
    if let Some(proxy) = payload.proxy.clone().filter(|s| !s.trim().is_empty()) {
        envs.push(("HTTP_PROXY".to_string(), proxy.clone()));
        envs.push(("HTTPS_PROXY".to_string(), proxy.clone()));
        envs.push(("ALL_PROXY".to_string(), proxy));
    }
    envs
}

fn npm_git_env(base: &[(String, String)]) -> Vec<(String, String)> {
    npm_git_env_with_mirror(base, "")
}

fn npm_git_env_with_mirror(
    base: &[(String, String)],
    mirror_prefix: &str,
) -> Vec<(String, String)> {
    let mut out = base.to_vec();
    out.push(("GIT_TERMINAL_PROMPT".to_string(), "0".to_string()));
    // Force git+ssh dependencies to fallback to HTTPS and optionally redirect GitHub to a mirror.
    let mut configs = vec![
        (
            "url.https://github.com/.insteadof".to_string(),
            "ssh://git@github.com/".to_string(),
        ),
        (
            "url.https://github.com/.insteadof".to_string(),
            "git@github.com:".to_string(),
        ),
        ("http.version".to_string(), "HTTP/1.1".to_string()),
    ];
    let mirror = mirror_prefix.trim();
    if !mirror.is_empty() {
        let normalized = if mirror.ends_with('/') {
            mirror.to_string()
        } else {
            format!("{mirror}/")
        };
        configs.push((
            format!("url.{normalized}.insteadof"),
            "https://github.com/".to_string(),
        ));
        configs.push((
            format!("url.{normalized}.insteadof"),
            "ssh://git@github.com/".to_string(),
        ));
        configs.push((
            format!("url.{normalized}.insteadof"),
            "git@github.com:".to_string(),
        ));
    }
    out.push(("GIT_CONFIG_COUNT".to_string(), configs.len().to_string()));
    for (idx, (key, value)) in configs.into_iter().enumerate() {
        out.push((format!("GIT_CONFIG_KEY_{idx}"), key));
        out.push((format!("GIT_CONFIG_VALUE_{idx}"), value));
    }
    out
}

pub fn uninstall_openclaw() -> Result<UninstallResult> {
    paths::ensure_dirs()?;
    logger::info("OpenClaw uninstall started.");

    let mut warnings = Vec::<String>::new();
    let mut removed_paths = Vec::<String>::new();
    let mut stopped_process = false;

    match process::stop() {
        Ok(_) => {
            stopped_process = true;
        }
        Err(err) => warnings.push(format!("Failed to stop running process: {err}")),
    }

    let install_state = state_store::load_install_state()?;
    // IMPORTANT: Never uninstall global OpenClaw automatically.
    // Users may have their own global OpenClaw installation unrelated to this installer.

    let mut targets = HashSet::<String>::new();
    if let Some(state) = install_state.as_ref() {
        let dir = paths::normalize_path(&state.install_dir)?;
        targets.insert(dir.to_string_lossy().to_string());
    }
    targets.insert(paths::openclaw_home().to_string_lossy().to_string());
    targets.insert(paths::run_dir().to_string_lossy().to_string());
    targets.insert(paths::state_dir().to_string_lossy().to_string());
    targets.insert(paths::appdata_root().to_string_lossy().to_string());

    for target in targets {
        remove_dir_best_effort(Path::new(&target), &mut removed_paths, &mut warnings);
    }

    // Ensure state files are removed even if the state dir still exists.
    if let Err(err) = state_store::clear_install_state() {
        warnings.push(format!("Failed to clear install state file: {err}"));
    }
    if let Err(err) = state_store::clear_last_config() {
        warnings.push(format!("Failed to clear last config file: {err}"));
    }
    if let Err(err) = state_store::clear_run_prefs() {
        warnings.push(format!("Failed to clear run prefs file: {err}"));
    }

    Ok(UninstallResult {
        stopped_process,
        removed_paths,
        warnings,
    })
}

fn remove_dir_best_effort(
    path: &Path,
    removed_paths: &mut Vec<String>,
    warnings: &mut Vec<String>,
) {
    if !path.exists() {
        return;
    }
    match fs::remove_dir_all(path) {
        Ok(_) => removed_paths.push(path.to_string_lossy().to_string()),
        Err(err) => warnings.push(format!(
            "Failed to remove directory '{}': {}",
            path.to_string_lossy(),
            err
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::{is_npm_git_fetch_failure, npm_git_env, npm_git_env_with_mirror};
    use crate::modules::shell::CmdOutput;

    #[test]
    fn npm_git_env_includes_direct_rewrite_rules() {
        let env = npm_git_env(&[]);
        let joined = format!("{env:?}").to_ascii_lowercase();
        assert!(joined.contains("git_terminal_prompt"));
        assert!(joined.contains("url.https://github.com/.insteadof"));
        assert!(joined.contains("ssh://git@github.com/"));
        assert!(joined.contains("git@github.com:"));
    }

    #[test]
    fn npm_git_env_with_mirror_includes_mirror_rewrite_rules() {
        let env = npm_git_env_with_mirror(&[], "https://gitclone.com/github.com/");
        let joined = format!("{env:?}").to_ascii_lowercase();
        assert!(joined.contains("url.https://gitclone.com/github.com/.insteadof"));
        assert!(joined.contains("https://github.com/"));
    }

    #[test]
    fn git_fetch_failure_detection_covers_network_and_auth_cases() {
        let network = CmdOutput {
            code: 1,
            stdout: String::new(),
            stderr: "npm error code 128\nnpm error command git --no-replace-objects ls-remote ssh://git@github.com/whiskeysockets/libsignal-node.git\nfatal: unable to access 'https://github.com/whiskeysockets/libsignal-node.git/': Failed to connect to github.com port 443".to_string(),
        };
        assert!(is_npm_git_fetch_failure(&network));

        let auth = CmdOutput {
            code: 1,
            stdout: String::new(),
            stderr: "npm error code 128\nnpm error command git --no-replace-objects ls-remote ssh://git@github.com/whiskeysockets/libsignal-node.git\nnpm error git@github.com: Permission denied (publickey).".to_string(),
        };
        assert!(is_npm_git_fetch_failure(&auth));
    }
}
