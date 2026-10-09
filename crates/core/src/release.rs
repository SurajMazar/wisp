//! Backups, data export and update checks (spec §117, Phase 21).

use std::path::PathBuf;

use wisp_ipc::{
    ErrorCode, IpcError,
    types::release::{BackupInfo, BackupList, ExportResult, UpdateInfo},
};

use crate::Core;

const KEEP_BACKUPS: usize = 7;
const DAY_MS: i64 = 24 * 60 * 60 * 1000;
const RELEASES_URL: &str = "https://api.github.com/repos/SurajMazar/wisp/releases/latest";

/// "0.1.10" > "0.1.9"; a pre-release suffix sorts before the release.
pub fn newer(latest: &str, current: &str) -> bool {
    let parse = |v: &str| {
        let v = v.trim().trim_start_matches('v');
        let (core, pre) = v
            .split_once('-')
            .map_or((v, None), |(c, p)| (c, Some(p.to_owned())));
        let nums: Vec<u64> = core.split('.').map(|n| n.parse().unwrap_or(0)).collect();
        (nums, pre)
    };
    let (a, ap) = parse(latest);
    let (b, bp) = parse(current);
    match a.cmp(&b) {
        std::cmp::Ordering::Greater => true,
        std::cmp::Ordering::Less => false,
        std::cmp::Ordering::Equal => match (ap, bp) {
            (None, Some(_)) => true,
            (Some(x), Some(y)) => x > y,
            _ => false,
        },
    }
}

impl Core {
    fn backups_dir(&self) -> PathBuf {
        self.data_dir().join("backups")
    }

    pub(crate) fn backup_list_sync(&self) -> BackupList {
        let dir = self.backups_dir();
        let mut backups: Vec<BackupInfo> = std::fs::read_dir(&dir)
            .map(|it| {
                it.flatten()
                    .filter(|e| e.file_name().to_string_lossy().ends_with(".sqlite3"))
                    .filter_map(|e| {
                        let m = e.metadata().ok()?;
                        let created = m
                            .modified()
                            .ok()?
                            .duration_since(std::time::UNIX_EPOCH)
                            .ok()?
                            .as_millis() as i64;
                        Some(BackupInfo {
                            name: e.file_name().to_string_lossy().into_owned(),
                            size_bytes: m.len(),
                            created_at_ms: created,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        backups.sort_by_key(|b| std::cmp::Reverse(b.created_at_ms));
        BackupList {
            backups,
            folder: dir.display().to_string(),
        }
    }

    /// A consistent snapshot of the database (SQLite `VACUUM INTO`), keeping the newest seven.
    pub(crate) async fn backup_create(&self) -> Result<BackupInfo, IpcError> {
        let dir = self.backups_dir();
        std::fs::create_dir_all(&dir).map_err(|e| IpcError::internal(e.to_string()))?;
        let name = format!("wisp-{}.sqlite3", wisp_common::time::now_ms());
        let path = dir.join(&name);
        let target = path.clone();
        self.blocking(move |db| db.backup_to(&target)).await?;
        let list = self.backup_list_sync();
        for old in list.backups.iter().skip(KEEP_BACKUPS) {
            let _ = std::fs::remove_file(dir.join(&old.name));
        }
        tracing::info!("database backed up");
        list.backups
            .into_iter()
            .find(|b| b.name == name)
            .ok_or_else(|| IpcError::internal("backup missing"))
    }

    /// Daily automatic backup (at startup when the last one is older than a day).
    pub fn backup_if_due(&self) {
        let latest = self
            .backup_list_sync()
            .backups
            .first()
            .map(|b| b.created_at_ms);
        if latest.is_some_and(|t| wisp_common::time::now_ms() - t < DAY_MS) {
            return;
        }
        let core = self.clone();
        tokio::spawn(async move {
            if let Err(e) = core.backup_create().await {
                tracing::warn!(error = %e.message, "automatic backup failed");
            }
        });
    }

    /// Everything the user made, as JSON in the data folder's `exports/`.
    pub(crate) async fn data_export(&self) -> Result<ExportResult, IpcError> {
        let doc = self.blocking(|db| db.export_all()).await?;
        let count = |k: &str| doc[k].as_array().map_or(0, |a| a.len() as u32);
        let dir = self.data_dir().join("exports");
        std::fs::create_dir_all(&dir).map_err(|e| IpcError::internal(e.to_string()))?;
        let path = dir.join(format!("wisp-export-{}.json", wisp_common::time::now_ms()));
        let json =
            serde_json::to_vec_pretty(&doc).map_err(|e| IpcError::internal(e.to_string()))?;
        std::fs::write(&path, json).map_err(|e| IpcError::internal(e.to_string()))?;
        Ok(ExportResult {
            path: path.display().to_string(),
            conversations: count("conversations"),
            memories: count("memories"),
            skills: count("skills"),
            agents: count("agents"),
            routines: count("routines"),
        })
    }

    /// Ask GitHub for the latest release. Only runs when the user asks.
    pub(crate) async fn update_check(&self) -> Result<UpdateInfo, IpcError> {
        let current = env!("CARGO_PKG_VERSION").to_owned();
        let client = reqwest::Client::builder()
            .user_agent(format!("Wisp/{current}"))
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|e| IpcError::internal(e.to_string()))?;
        let v: serde_json::Value = client
            .get(RELEASES_URL)
            .send()
            .await
            .map_err(|_| {
                IpcError::new(
                    ErrorCode::Unavailable,
                    "Couldn't reach GitHub to check for updates.",
                )
            })?
            .json()
            .await
            .map_err(|_| IpcError::new(ErrorCode::Unavailable, "No releases were found."))?;
        let latest = v["tag_name"]
            .as_str()
            .map(|t| t.trim_start_matches('v').to_owned());
        let url = v["html_url"]
            .as_str()
            .filter(|u| u.starts_with("https://github.com/"))
            .map(str::to_owned);
        Ok(UpdateInfo {
            update_available: latest.as_deref().is_some_and(|l| newer(l, &current)),
            current,
            latest,
            url,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_versions() {
        assert!(newer("0.1.10", "0.1.9"));
        assert!(newer("v0.2.0", "0.1.0"));
        assert!(newer("0.1.0", "0.1.0-alpha.1"));
        assert!(newer("0.1.0-alpha.2", "0.1.0-alpha.1"));
        assert!(!newer("0.1.0", "0.1.0"));
        assert!(!newer("0.1.0-alpha.1", "0.1.0"));
        assert!(!newer("0.0.9", "0.1.0"));
    }
}
