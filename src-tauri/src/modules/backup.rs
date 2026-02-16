use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use chrono::Local;
use uuid::Uuid;
use walkdir::WalkDir;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::models::{BackupInfo, BackupResult, RollbackResult};

use super::{logger, paths};

pub fn backup() -> Result<BackupResult> {
    let info = backup_with_prefix("manual")?;
    Ok(BackupResult { backup: info })
}

pub fn backup_with_prefix(prefix: &str) -> Result<BackupInfo> {
    paths::ensure_dirs()?;
    let id = format!("{}-{}", prefix, Local::now().format("%Y%m%d-%H%M%S"));
    let zip_path = paths::backups_dir().join(format!("{id}.zip"));
    let file = File::create(&zip_path)?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    // Backup includes OpenClaw runtime data + installer state for full rollback.
    add_folder_to_zip(&mut zip, &paths::openclaw_home(), "openclaw_home", options)?;
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    add_folder_to_zip(&mut zip, &paths::state_dir(), "installer_state", options)?;
    zip.finish()?;

    let size = fs::metadata(&zip_path)?.len();
    logger::info(&format!("Backup created: {}", zip_path.to_string_lossy()));
    Ok(BackupInfo {
        id,
        path: zip_path.to_string_lossy().to_string(),
        created_at: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        size,
    })
}

pub fn list_backups() -> Result<Vec<BackupInfo>> {
    paths::ensure_dirs()?;
    let mut out = Vec::new();
    for entry in fs::read_dir(paths::backups_dir())? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path
            .extension()
            .map(|v| v.to_string_lossy().to_ascii_lowercase())
            != Some("zip".to_string())
        {
            continue;
        }
        let metadata = entry.metadata()?;
        let id = path
            .file_stem()
            .map(|v| v.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());
        let created = metadata
            .modified()
            .ok()
            .map(|m| {
                let dt: chrono::DateTime<Local> = m.into();
                dt.format("%Y-%m-%d %H:%M:%S").to_string()
            })
            .unwrap_or_else(|| "-".to_string());
        out.push(BackupInfo {
            id,
            path: path.to_string_lossy().to_string(),
            created_at: created,
            size: metadata.len(),
        });
    }
    out.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(out)
}

pub fn rollback(backup_id: &str) -> Result<RollbackResult> {
    // Safety guard: always snapshot current state before restore.
    let auto = backup_with_prefix("pre-rollback")?;
    restore_backup(backup_id)?;
    logger::warn(&format!("Rollback finished from backup {backup_id}."));
    Ok(RollbackResult {
        from_backup: backup_id.to_string(),
        auto_backup: auto,
    })
}

pub fn restore_backup(backup_id_or_path: &str) -> Result<()> {
    let backup_file = resolve_backup_path(backup_id_or_path)?;
    let temp_dir = std::env::temp_dir().join(format!("openclaw-restore-{}", Uuid::new_v4()));
    fs::create_dir_all(&temp_dir)?;
    extract_zip(&backup_file, &temp_dir)?;

    let restored_home = temp_dir.join("openclaw_home");
    if restored_home.exists() {
        copy_dir_overwrite(&restored_home, &paths::openclaw_home())?;
    }
    let restored_state = temp_dir.join("installer_state");
    if restored_state.exists() {
        copy_dir_overwrite(&restored_state, &paths::state_dir())?;
    }
    let _ = fs::remove_dir_all(temp_dir);
    Ok(())
}

fn resolve_backup_path(value: &str) -> Result<PathBuf> {
    let path = PathBuf::from(value);
    if path.exists() {
        return Ok(path);
    }
    let candidate = paths::backups_dir().join(format!("{value}.zip"));
    if candidate.exists() {
        return Ok(candidate);
    }
    Err(anyhow!("Backup not found: {value}"))
}

fn add_folder_to_zip(
    zip: &mut ZipWriter<File>,
    folder: &Path,
    prefix: &str,
    options: SimpleFileOptions,
) -> Result<()> {
    if !folder.exists() {
        return Ok(());
    }
    for entry in WalkDir::new(folder).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        let rel = path.strip_prefix(folder)?;
        if rel.as_os_str().is_empty() {
            continue;
        }
        let zip_name = format!("{prefix}/{}", rel.to_string_lossy().replace('\\', "/"));
        if path.is_dir() {
            zip.add_directory(zip_name, options)?;
            continue;
        }
        zip.start_file(zip_name, options)?;
        let mut file = File::open(path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;
        zip.write_all(&buffer)?;
    }
    Ok(())
}

fn extract_zip(archive_file: &Path, destination: &Path) -> Result<()> {
    let file = File::open(archive_file)?;
    let mut archive = ZipArchive::new(file)?;
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        // Reject zip-slip style traversal paths.
        let enclosed = file
            .enclosed_name()
            .ok_or_else(|| anyhow!("Invalid zip path detected"))?;
        let out_path = destination.join(enclosed);
        if file.is_dir() {
            fs::create_dir_all(&out_path)?;
        } else {
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut out = File::create(&out_path)?;
            std::io::copy(&mut file, &mut out)?;
        }
    }
    Ok(())
}

fn copy_dir_overwrite(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;
    for entry in WalkDir::new(src).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        let rel = path.strip_prefix(src)?;
        if rel.as_os_str().is_empty() {
            continue;
        }
        let target = dst.join(rel);
        if path.is_dir() {
            fs::create_dir_all(&target)?;
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(path, target)?;
    }
    Ok(())
}
