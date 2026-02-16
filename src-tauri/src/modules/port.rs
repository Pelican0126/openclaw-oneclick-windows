use std::thread;
use std::time::Duration;

use anyhow::{anyhow, Result};

use crate::models::PortStatus;

use super::shell;

pub fn check_port(port: u16) -> Result<PortStatus> {
    let target = format!(":{port}");
    let out = shell::run_command("netstat", &["-ano", "-p", "tcp"], None, &[])?;
    if out.code != 0 {
        return Ok(PortStatus {
            port,
            in_use: false,
            pid: None,
            process_name: None,
        });
    }

    for line in out.stdout.lines() {
        let compact = line.trim();
        if !(compact.contains(&target) && compact.contains("LISTENING")) {
            continue;
        }
        let parts: Vec<&str> = compact.split_whitespace().collect();
        if let Some(last) = parts.last() {
            if let Ok(pid) = last.parse::<u32>() {
                return Ok(PortStatus {
                    port,
                    in_use: true,
                    pid: Some(pid),
                    process_name: shell::process_name_by_pid(pid),
                });
            }
        }
    }

    Ok(PortStatus {
        port,
        in_use: false,
        pid: None,
        process_name: None,
    })
}

pub fn release_port(port: u16) -> Result<String> {
    let status = check_port(port)?;
    if !status.in_use {
        return Ok(format!("Port {port} is already free."));
    }
    let pid = status
        .pid
        .ok_or_else(|| anyhow!("Port {port} is in use but PID cannot be resolved."))?;
    let pid_text = pid.to_string();
    let out = shell::run_command("taskkill", &["/PID", &pid_text, "/T", "/F"], None, &[])?;
    if out.code != 0 {
        return Err(anyhow!(
            "Failed to stop process PID {pid} for port {port}: {}",
            if out.stderr.is_empty() {
                out.stdout
            } else {
                out.stderr
            }
        ));
    }

    // Wait for kernel socket table to settle before re-check.
    for _ in 0..8 {
        thread::sleep(Duration::from_millis(250));
        if !check_port(port)?.in_use {
            return Ok(format!("Released port {port} by terminating PID {pid}."));
        }
    }

    Err(anyhow!(
        "Port {port} is still in use after terminating PID {pid}."
    ))
}
