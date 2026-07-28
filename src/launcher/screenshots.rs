use anyhow::Result;
use std::path::{Path, PathBuf};
use tokio::fs;

pub struct ScreenshotGallery;

impl ScreenshotGallery {
    /// List all PNG screenshots in the given game/instance dir's screenshots folder.
    pub async fn list(game_dir: &Path) -> Result<Vec<PathBuf>> {
        let dir = game_dir.join("screenshots");
        let mut shots = Vec::new();
        let mut rd = match fs::read_dir(&dir).await {
            Ok(d) => d,
            Err(_) => return Ok(shots),
        };
        while let Some(entry) = rd.next_entry().await? {
            let p = entry.path();
            if p.extension().and_then(|e| e.to_str()) == Some("png") {
                shots.push(p);
            }
        }
        // Sort newest first by filename (Minecraft names them by date)
        shots.sort_by(|a, b| b.file_name().cmp(&a.file_name()));
        Ok(shots)
    }

    /// Open a screenshot with the system default image viewer.
    pub fn open(path: &PathBuf) -> anyhow::Result<()> {
        open::that(path)?;
        Ok(())
    }

    /// Open the screenshots folder in the system file explorer.
    pub fn open_folder(game_dir: &Path) -> anyhow::Result<()> {
        open::that(game_dir.join("screenshots"))?;
        Ok(())
    }
}
