/// Config Snapshot — save and restore an instance's `config/` directory.
///
/// Each snapshot is stored as a zip archive under:
///   `<data_dir>/config_snapshots/<instance>/<name>.zip`
///
/// This is intentionally separate from `BackupManager` (which targets `saves/`)
/// so users can version-control settings independently of world data.
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use tokio::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotInfo {
    /// User-supplied label.
    pub name: String,
    /// RFC3339 creation timestamp.
    pub created_at: String,
    /// File name of the zip archive (not the full path).
    pub filename: String,
}

pub struct ConfigSnapshotManager {
    snapshots_root: PathBuf,
}

impl ConfigSnapshotManager {
    pub fn new(data_dir: &Path) -> Self {
        Self {
            snapshots_root: data_dir.join("config_snapshots"),
        }
    }

    fn instance_dir(&self, instance: &str) -> PathBuf {
        self.snapshots_root.join(instance)
    }

    fn index_path(&self, instance: &str) -> PathBuf {
        self.instance_dir(instance).join("index.json")
    }

    // ── index helpers ────────────────────────────────────────────────────────

    async fn load_index(&self, instance: &str) -> Vec<SnapshotInfo> {
        match fs::read_to_string(self.index_path(instance)).await {
            Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
            Err(_) => Vec::new(),
        }
    }

    async fn save_index(&self, instance: &str, index: &[SnapshotInfo]) -> Result<()> {
        fs::create_dir_all(self.instance_dir(instance)).await?;
        fs::write(
            self.index_path(instance),
            serde_json::to_string_pretty(index)?,
        )
        .await?;
        Ok(())
    }

    // ── public API ───────────────────────────────────────────────────────────

    /// List all snapshots for `instance`.
    pub async fn list(&self, instance: &str) -> Vec<SnapshotInfo> {
        self.load_index(instance).await
    }

    /// Zip the `config/` directory of `instance_config_dir` and save it as a
    /// named snapshot. `instance_config_dir` should be the `config/` subdirectory
    /// of the instance (e.g. `instances/<name>/config`).
    pub async fn create(
        &self,
        instance: &str,
        instance_config_dir: &Path,
        label: &str,
    ) -> Result<SnapshotInfo> {
        if !instance_config_dir.exists() {
            bail!(
                "Config directory not found: {}",
                instance_config_dir.display()
            );
        }

        let ts = chrono::Utc::now();
        let filename = format!(
            "{}.zip",
            ts.format("%Y%m%d_%H%M%S")
        );
        let zip_path = self.instance_dir(instance).join(&filename);
        fs::create_dir_all(self.instance_dir(instance)).await?;

        // Build zip on a blocking thread — zip crate is sync.
        let config_dir_owned = instance_config_dir.to_path_buf();
        let zip_path_owned = zip_path.clone();
        tokio::task::spawn_blocking(move || -> Result<()> {
            let file = std::fs::File::create(&zip_path_owned)?;
            let mut zip = zip::ZipWriter::new(file);
            let opts = zip::write::FileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
            zip_dir_recursive(&mut zip, &config_dir_owned, &config_dir_owned, opts)?;
            zip.finish()?;
            Ok(())
        })
        .await??;

        let info = SnapshotInfo {
            name: label.to_string(),
            created_at: ts.to_rfc3339(),
            filename,
        };

        let mut index = self.load_index(instance).await;
        index.push(info.clone());
        self.save_index(instance, &index).await?;
        Ok(info)
    }

    /// Restore a snapshot by clearing `instance_config_dir` and extracting
    /// the archived zip into it.
    pub async fn restore(
        &self,
        instance: &str,
        snapshot_name: &str,
        instance_config_dir: &Path,
    ) -> Result<()> {
        let index = self.load_index(instance).await;
        let snap = index
            .iter()
            .find(|s| s.name == snapshot_name)
            .ok_or_else(|| anyhow::anyhow!("Snapshot '{}' not found.", snapshot_name))?;

        let zip_path = self.instance_dir(instance).join(&snap.filename);
        if !zip_path.exists() {
            bail!("Snapshot archive missing: {}", zip_path.display());
        }

        // Clear existing config dir.
        if instance_config_dir.exists() {
            fs::remove_dir_all(instance_config_dir).await?;
        }
        fs::create_dir_all(instance_config_dir).await?;

        let config_dir_owned = instance_config_dir.to_path_buf();
        tokio::task::spawn_blocking(move || -> Result<()> {
            let file = std::fs::File::open(&zip_path)?;
            let mut archive = zip::ZipArchive::new(file)?;
            for i in 0..archive.len() {
                let mut zf = archive.by_index(i)?;
                let out = config_dir_owned.join(zf.name());
                if zf.is_dir() {
                    std::fs::create_dir_all(&out)?;
                } else {
                    if let Some(p) = out.parent() {
                        std::fs::create_dir_all(p)?;
                    }
                    let mut buf = Vec::new();
                    zf.read_to_end(&mut buf)?;
                    std::fs::write(&out, buf)?;
                }
            }
            Ok(())
        })
        .await??;

        Ok(())
    }

    /// Delete a named snapshot.
    pub async fn delete(&self, instance: &str, snapshot_name: &str) -> Result<()> {
        let mut index = self.load_index(instance).await;
        let pos = index
            .iter()
            .position(|s| s.name == snapshot_name)
            .ok_or_else(|| anyhow::anyhow!("Snapshot '{}' not found.", snapshot_name))?;

        let filename = index[pos].filename.clone();
        index.remove(pos);
        self.save_index(instance, &index).await?;

        let zip_path = self.instance_dir(instance).join(filename);
        if zip_path.exists() {
            fs::remove_file(zip_path).await?;
        }
        Ok(())
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn zip_dir_recursive(
    zip: &mut zip::ZipWriter<std::fs::File>,
    base: &Path,
    dir: &Path,
    opts: zip::write::FileOptions,
) -> Result<()> {
    for entry in std::fs::read_dir(dir)?.flatten() {
        let path = entry.path();
        let rel = path.strip_prefix(base).unwrap_or(&path);
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        if path.is_dir() {
            zip.add_directory(&rel_str, opts)?;
            zip_dir_recursive(zip, base, &path, opts)?;
        } else {
            zip.start_file(&rel_str, opts)?;
            let bytes = std::fs::read(&path)?;
            zip.write_all(&bytes)?;
        }
    }
    Ok(())
}
