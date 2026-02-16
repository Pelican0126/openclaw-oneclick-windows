use anyhow::{anyhow, Result};

use crate::models::UpgradeResult;

use super::{backup, config, installer, logger, state_store};

pub async fn upgrade() -> Result<UpgradeResult> {
    let install_state = state_store::load_install_state()?
        .ok_or_else(|| anyhow!("Install state not found. Install OpenClaw first."))?;

    // Rebuild upgrade payload from last config, then fallback to current config if needed.
    let mut payload = state_store::load_last_config()?.unwrap_or_default();
    if payload.install_dir.trim().is_empty() {
        payload.install_dir = install_state.install_dir.clone();
    }
    if payload.launch_args.trim().is_empty() {
        payload.launch_args = install_state.launch_args.clone();
    }
    payload.source_method = install_state.method.clone();
    if payload.source_url.is_none() {
        payload.source_url = install_state.source_url.clone();
    }
    let current = config::read_current_config()?;
    if payload.provider.trim().is_empty() {
        payload.provider = current.provider;
    }
    if payload.model_chain.primary.trim().is_empty() {
        payload.model_chain = current.model_chain;
    }
    if payload.base_url.as_deref().unwrap_or("").trim().is_empty() {
        payload.base_url = current.base_url;
    }
    if payload.proxy.as_deref().unwrap_or("").trim().is_empty() {
        payload.proxy = current.proxy;
    }
    if payload.bind_address.trim().is_empty() {
        payload.bind_address = current.bind_address;
    }
    if payload.port == 0 {
        payload.port = current.port;
    }

    let old_version = install_state.version.clone();
    // Upgrade is guarded by a pre-upgrade snapshot for automatic rollback.
    let pre_upgrade = backup::backup_with_prefix("pre-upgrade")?;
    let backup_id = pre_upgrade.id.clone();

    match installer::install_openclaw_for_upgrade(&payload).await {
        Ok(result) => {
            logger::info(&format!(
                "Upgrade completed from {} to {}",
                old_version, result.version
            ));
            Ok(UpgradeResult {
                old_version,
                new_version: result.version,
                rolled_back: false,
                backup_id,
                message: "Upgrade completed successfully.".to_string(),
            })
        }
        Err(err) => {
            // Any upgrade failure restores the snapshot to keep service continuity.
            logger::error(&format!(
                "Upgrade failed, restoring backup {backup_id}: {err}"
            ));
            backup::restore_backup(&backup_id)?;
            Ok(UpgradeResult {
                old_version,
                new_version: "rollback".to_string(),
                rolled_back: true,
                backup_id,
                message: format!("Upgrade failed and rollback completed: {err}"),
            })
        }
    }
}
