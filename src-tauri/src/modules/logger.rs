use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use anyhow::Result;
use chrono::Local;
use once_cell::sync::Lazy;
use std::sync::Mutex;

use crate::models::LogSummary;

use super::paths;

static LOG_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

pub fn info(message: &str) {
    let _ = write_line("INFO", message);
}

pub fn warn(message: &str) {
    let _ = write_line("WARN", message);
}

pub fn error(message: &str) {
    let _ = write_line("ERROR", message);
}

fn write_line(level: &str, message: &str) -> Result<()> {
    let _guard = LOG_LOCK
        .lock()
        .map_err(|_| anyhow::anyhow!("failed to lock logger"))?;
    paths::ensure_dirs()?;
    let log_file = paths::logs_dir().join(format!("{}.log", Local::now().format("%Y-%m-%d")));
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_file)?;
    let line = format!(
        "{} [{}] {}\n",
        Local::now().format("%Y-%m-%d %H:%M:%S"),
        level,
        message
    );
    file.write_all(line.as_bytes())?;
    Ok(())
}

pub fn list_logs() -> Result<Vec<LogSummary>> {
    paths::ensure_dirs()?;
    let mut out = Vec::new();
    for entry in fs::read_dir(paths::logs_dir())? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let metadata = entry.metadata()?;
        let modified = metadata
            .modified()
            .ok()
            .map(|ts| {
                let dt: chrono::DateTime<Local> = ts.into();
                dt.format("%Y-%m-%d %H:%M:%S").to_string()
            })
            .unwrap_or_else(|| "-".to_string());
        out.push(LogSummary {
            name: path
                .file_name()
                .map(|v| v.to_string_lossy().to_string())
                .unwrap_or_else(|| "unknown.log".to_string()),
            path: path.to_string_lossy().to_string(),
            size: metadata.len(),
            modified_at: modified,
        });
    }
    out.sort_by(|a, b| b.modified_at.cmp(&a.modified_at));
    Ok(out)
}

pub fn read_log(name: &str, max_lines: usize) -> Result<String> {
    let path = paths::logs_dir().join(name);
    if !path.exists() {
        return Ok(String::new());
    }
    let content = fs::read_to_string(path)?;
    let lines: Vec<&str> = content.lines().collect();
    if lines.len() <= max_lines {
        return Ok(content);
    }
    let start = lines.len().saturating_sub(max_lines);
    Ok(lines[start..].join("\n"))
}

pub fn export_log(name: &str, output: &Path) -> Result<String> {
    let src = paths::logs_dir().join(name);
    if !src.exists() {
        anyhow::bail!("Log file not found: {}", src.to_string_lossy());
    }
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(&src, output)?;
    Ok(output.to_string_lossy().to_string())
}

pub fn logs_dir_path() -> Result<String> {
    paths::ensure_dirs()?;
    Ok(paths::logs_dir().to_string_lossy().to_string())
}
