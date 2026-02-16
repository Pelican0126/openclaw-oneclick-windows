use std::path::Path;
use std::process::Command;

use anyhow::{anyhow, Result};

#[cfg(windows)]
use encoding_rs::GBK;
#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Debug, Clone)]
pub struct CmdOutput {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

pub fn run_command<S: AsRef<str>>(
    exe: S,
    args: &[S],
    cwd: Option<&Path>,
    extra_env: &[(String, String)],
) -> Result<CmdOutput> {
    let exe_ref = exe.as_ref();
    let mut cmd = if is_cmd_script(exe_ref) {
        let mut wrapped = Command::new("cmd");
        wrapped.arg("/D").arg("/C").arg(exe_ref);
        for arg in args {
            wrapped.arg(arg.as_ref());
        }
        wrapped
    } else if is_powershell_script(exe_ref) {
        // Some npm global shims on Windows are .ps1 only.
        // Execute them via PowerShell explicitly to avoid "program not found".
        let mut wrapped = Command::new("powershell");
        wrapped
            .arg("-NoProfile")
            .arg("-NonInteractive")
            .arg("-ExecutionPolicy")
            .arg("Bypass")
            .arg("-File")
            .arg(exe_ref);
        for arg in args {
            wrapped.arg(arg.as_ref());
        }
        wrapped
    } else {
        let mut direct = Command::new(exe_ref);
        for arg in args {
            direct.arg(arg.as_ref());
        }
        direct
    };
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    for (k, v) in extra_env {
        cmd.env(k, v);
    }
    #[cfg(windows)]
    {
        // Prevent console flashing when GUI process invokes CLI tools.
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let output = cmd.output()?;
    Ok(CmdOutput {
        code: output.status.code().unwrap_or(-1),
        stdout: decode_output(&output.stdout),
        stderr: decode_output(&output.stderr),
    })
}

fn is_cmd_script(exe: &str) -> bool {
    let lower = exe.to_ascii_lowercase();
    lower.ends_with(".cmd") || lower.ends_with(".bat")
}

fn is_powershell_script(exe: &str) -> bool {
    exe.to_ascii_lowercase().ends_with(".ps1")
}

fn decode_output(raw: &[u8]) -> String {
    if raw.is_empty() {
        return String::new();
    }
    if let Ok(text) = std::str::from_utf8(raw) {
        return text.trim().to_string();
    }
    #[cfg(windows)]
    {
        // npm/cmd errors on CN Windows are often emitted as GBK/CP936.
        let (decoded, _, _) = GBK.decode(raw);
        return decoded.trim().to_string();
    }
    #[cfg(not(windows))]
    {
        String::from_utf8_lossy(raw).trim().to_string()
    }
}

pub fn command_exists(name: &str) -> Option<String> {
    match run_command("where", &[name], None, &[]) {
        Ok(out) if out.code == 0 => {
            let mut lines: Vec<String> = out
                .stdout
                .lines()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            if !lines.is_empty() {
                lines.sort_by_key(|item| executable_rank(item));
                return lines.into_iter().next();
            }
        }
        _ => {}
    }
    fallback_command_exists(name)
}

#[cfg(windows)]
fn fallback_command_exists(name: &str) -> Option<String> {
    let normalized = name.trim().trim_matches('"').to_ascii_lowercase();
    if normalized.is_empty() {
        return None;
    }

    // On some Windows setups, %APPDATA%\npm is not in PATH for GUI-launched apps.
    // Fall back to npm global shims directly to keep installer behavior stable.
    if let Ok(appdata) = std::env::var("APPDATA") {
        let npm_shim_dir = Path::new(&appdata).join("npm");
        let candidates = [
            npm_shim_dir.join(format!("{normalized}.cmd")),
            npm_shim_dir.join(format!("{normalized}.exe")),
            npm_shim_dir.join(normalized.as_str()),
            npm_shim_dir.join(format!("{normalized}.ps1")),
        ];
        for candidate in candidates {
            if candidate.exists() {
                return Some(candidate.to_string_lossy().to_string());
            }
        }
    }

    // npx often lives in Program Files even when PATH inheritance is incomplete.
    if normalized == "npx" {
        let common = [
            Path::new("C:\\Program Files\\nodejs\\npx.cmd").to_path_buf(),
            Path::new("C:\\Program Files\\nodejs\\npx.exe").to_path_buf(),
            Path::new("C:\\Program Files (x86)\\nodejs\\npx.cmd").to_path_buf(),
            Path::new("C:\\Program Files (x86)\\nodejs\\npx.exe").to_path_buf(),
        ];
        for candidate in common {
            if candidate.exists() {
                return Some(candidate.to_string_lossy().to_string());
            }
        }
    }

    None
}

#[cfg(not(windows))]
fn fallback_command_exists(_name: &str) -> Option<String> {
    None
}

fn executable_rank(path: &str) -> u8 {
    let lower = path.to_ascii_lowercase();
    if lower.ends_with(".exe") {
        return 0;
    }
    if lower.ends_with(".cmd") {
        return 1;
    }
    if lower.ends_with(".bat") {
        return 2;
    }
    if lower.ends_with(".com") {
        return 3;
    }
    if lower.ends_with(".ps1") {
        return 9;
    }
    5
}

pub fn is_admin() -> bool {
    match run_command("net", &["session"], None, &[]) {
        Ok(out) => out.code == 0,
        Err(_) => false,
    }
}

pub fn is_process_alive(pid: u32) -> bool {
    let filter = format!("PID eq {pid}");
    match run_command(
        "tasklist",
        &["/FI", &filter, "/FO", "CSV", "/NH"],
        None,
        &[],
    ) {
        Ok(out) if out.code == 0 => {
            if out.stdout.contains("No tasks are running") {
                return false;
            }
            out.stdout.contains(&pid.to_string())
        }
        _ => false,
    }
}

pub fn process_name_by_pid(pid: u32) -> Option<String> {
    let filter = format!("PID eq {pid}");
    let out = run_command(
        "tasklist",
        &["/FI", &filter, "/FO", "CSV", "/NH"],
        None,
        &[],
    )
    .ok()?;
    if out.code != 0 || out.stdout.contains("No tasks are running") {
        return None;
    }
    let line = out.stdout.lines().next()?.trim().trim_matches('"');
    let mut parts = line.split("\",\"");
    parts.next().map(|s| s.to_string())
}

pub fn ensure_success(op: &str, output: &CmdOutput) -> Result<()> {
    if output.code == 0 {
        Ok(())
    } else {
        Err(anyhow!(
            "{op} failed (code={}): {}",
            output.code,
            if output.stderr.is_empty() {
                output.stdout.clone()
            } else {
                output.stderr.clone()
            }
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::run_command;
    use std::fs;

    #[cfg(windows)]
    #[test]
    fn run_command_handles_cmd_path_with_spaces() {
        let dir = std::env::temp_dir().join("openclaw installer tests");
        fs::create_dir_all(&dir).expect("create temp test dir");
        let script = dir.join("echo test.cmd");
        fs::write(&script, "@echo off\r\necho hello_from_script\r\n")
            .expect("write test cmd script");

        let exe = script.to_string_lossy().to_string();
        let out = run_command(exe.as_str(), &["arg"], None, &[]).expect("invoke test cmd script");
        assert_eq!(out.code, 0, "stdout={}, stderr={}", out.stdout, out.stderr);
        assert!(out
            .stdout
            .to_ascii_lowercase()
            .contains("hello_from_script"));

        let _ = fs::remove_file(script);
    }

    #[cfg(windows)]
    #[test]
    fn run_command_handles_ps1_path_with_spaces() {
        let dir = std::env::temp_dir().join("openclaw installer tests");
        fs::create_dir_all(&dir).expect("create temp test dir");
        let script = dir.join("echo test.ps1");
        fs::write(&script, "Write-Output \"hello_from_ps1\"\r\n").expect("write test ps1 script");

        let exe = script.to_string_lossy().to_string();
        let out = run_command(exe.as_str(), &["arg"], None, &[]).expect("invoke test ps1 script");
        assert_eq!(out.code, 0, "stdout={}, stderr={}", out.stdout, out.stderr);
        assert!(out
            .stdout
            .to_ascii_lowercase()
            .contains("hello_from_ps1"));

        let _ = fs::remove_file(script);
    }
}
