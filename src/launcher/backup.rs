use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use tokio::fs;

pub struct BackupManager {
    backups_dir: PathBuf,
}

impl BackupManager {
    pub fn new(data_dir: &Path) -> Self {
        Self { backups_dir: data_dir.join("backups") }
    }

    /// Zip the saves/ directory of a game dir and store it as a timestamped backup.
    pub async fn create_backup(&self, instance_name: &str, game_dir: &Path) -> Result<PathBuf> {
        fs::create_dir_all(&self.backups_dir).await?;

        let saves_dir = game_dir.join("saves");
        if !saves_dir.exists() {
            anyhow::bail!("No saves directory found at {}", saves_dir.display());
        }

        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let zip_name = format!("{instance_name}_{timestamp}.zip");
        let zip_path = self.backups_dir.join(&zip_name);

        let zip_file = std::fs::File::create(&zip_path)
            .context("Failed to create backup zip")?;
        let mut zip = zip::ZipWriter::new(zip_file);
        let options = zip::write::FileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        add_dir_to_zip(&mut zip, &saves_dir, &saves_dir, &options)?;
        zip.finish()?;

        Ok(zip_path)
    }

    pub async fn list_backups(&self, instance_name: &str) -> Result<Vec<PathBuf>> {
        let mut backups = Vec::new();
        let mut dir = match fs::read_dir(&self.backups_dir).await {
            Ok(d) => d,
            Err(_) => return Ok(backups),
        };
        while let Some(entry) = dir.next_entry().await? {
            let path = entry.path();
            let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            if name.starts_with(instance_name) && name.ends_with(".zip") {
                backups.push(path);
            }
        }
        backups.sort();
        Ok(backups)
    }

    /// Restore a backup zip into the game dir's saves/ folder.
    pub async fn restore_backup(&self, zip_path: &Path, game_dir: &Path) -> Result<()> {
        let saves_dir = game_dir.join("saves");
        if saves_dir.exists() {
            fs::remove_dir_all(&saves_dir).await?;
        }
        fs::create_dir_all(&saves_dir).await?;

        let data = fs::read(zip_path).await?;
        let cursor = std::io::Cursor::new(data);
        let mut archive = zip::ZipArchive::new(cursor)?;

        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let name = file.name().to_string();
            if name.ends_with('/') { continue; }
            let out = saves_dir.join(&name);
            if let Some(p) = out.parent() { std::fs::create_dir_all(p)?; }
            let mut out_file = std::fs::File::create(&out)?;
            std::io::copy(&mut file, &mut out_file)?;
        }
        Ok(())
    }
}

fn add_dir_to_zip(
    zip: &mut zip::ZipWriter<std::fs::File>,
    base: &Path,
    dir: &Path,
    options: &zip::write::FileOptions,
) -> Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let rel = path.strip_prefix(base).unwrap_or(&path);
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        if path.is_dir() {
            zip.add_directory(&rel_str, *options)?;
            add_dir_to_zip(zip, base, &path, options)?;
        } else {
            zip.start_file(&rel_str, *options)?;
            let data = std::fs::read(&path)?;
            use std::io::Write;
            zip.write_all(&data)?;
        }
    }
    Ok(())
}

// ── Scheduled backup ──────────────────────────────────────────────────────────

/// Tracker stored at `<data_dir>/scheduled_backup_state.json`.
/// Records cumulative playtime since the last scheduled backup so we can
/// fire when the threshold is crossed.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ScheduledBackupState {
    /// Seconds of playtime accumulated since the last scheduled backup.
    pub secs_since_last_backup: u64,
    /// RFC3339 timestamp of the last scheduled backup (informational).
    #[serde(default)]
    pub last_backup_at: Option<String>,
}

pub struct ScheduledBackupManager {
    backups_dir: PathBuf,
    state_path: PathBuf,
}

impl ScheduledBackupManager {
    pub fn new(data_dir: &Path) -> Self {
        Self {
            backups_dir: data_dir.join("backups"),
            state_path: data_dir.join("scheduled_backup_state.json"),
        }
    }

    async fn load_state(&self) -> ScheduledBackupState {
        match tokio::fs::read_to_string(&self.state_path).await {
            Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
            Err(_) => ScheduledBackupState::default(),
        }
    }

    async fn save_state(&self, state: &ScheduledBackupState) -> anyhow::Result<()> {
        tokio::fs::write(
            &self.state_path,
            serde_json::to_string_pretty(state)?,
        )
        .await?;
        Ok(())
    }

    /// Call after every session with the session's duration.
    /// If cumulative playtime since last backup exceeds `threshold_hours`,
    /// triggers a backup and resets the counter.
    /// Returns the backup path if a backup was created.
    pub async fn tick(
        &self,
        instance_name: &str,
        game_dir: &Path,
        session_secs: u64,
        threshold_hours: u64,
    ) -> anyhow::Result<Option<PathBuf>> {
        if threshold_hours == 0 {
            return Ok(None);
        }

        let mut state = self.load_state().await;
        state.secs_since_last_backup = state.secs_since_last_backup.saturating_add(session_secs);

        let threshold_secs = threshold_hours * 3600;
        if state.secs_since_last_backup >= threshold_secs {
            // Create a BackupManager and fire.
            let bm = BackupManager { backups_dir: self.backups_dir.clone() };
            let path = bm.create_backup(instance_name, game_dir).await?;
            state.secs_since_last_backup = 0;
            state.last_backup_at = Some(chrono::Utc::now().to_rfc3339());
            self.save_state(&state).await?;
            return Ok(Some(path));
        }

        self.save_state(&state).await?;
        Ok(None)
    }
}
