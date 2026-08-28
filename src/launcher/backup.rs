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
