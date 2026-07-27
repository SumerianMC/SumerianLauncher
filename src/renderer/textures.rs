use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use tokio::fs;

pub struct TextureManager {
    pub textures_dir: PathBuf,
}

#[derive(Debug, Clone)]
pub struct TexturePack {
    pub name: String,
    pub path: PathBuf,
}

impl TextureManager {
    pub fn new(base_dir: &Path) -> Self {
        let textures_dir = base_dir.join("textures");
        Self { textures_dir }
    }

    pub async fn init(&self) -> Result<()> {
        for sub in &["default", "packs", "active"] {
            fs::create_dir_all(self.textures_dir.join(sub)).await?;
        }
        Ok(())
    }

    pub async fn list_packs(&self) -> Result<Vec<TexturePack>> {
        let packs_dir = self.textures_dir.join("packs");
        let mut packs = Vec::new();
        let mut dir = match fs::read_dir(&packs_dir).await {
            Ok(d) => d,
            Err(_) => return Ok(packs),
        };
        while let Some(entry) = dir.next_entry().await? {
            let path = entry.path();
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            if path.is_dir() || name.ends_with(".zip") {
                packs.push(TexturePack { name, path });
            }
        }
        Ok(packs)
    }

    /// Import a resource pack (zip or folder) into the packs directory.
    pub async fn import_pack(&self, source: &Path) -> Result<String> {
        let name = source
            .file_name()
            .context("Invalid source path")?
            .to_string_lossy()
            .to_string();
        let dest = self.textures_dir.join("packs").join(&name);
        if source.is_dir() {
            copy_dir_recursive(source, &dest).await?;
        } else {
            fs::copy(source, &dest).await?;
        }
        Ok(name)
    }

    /// Activate a pack: copy it into the active slot and inject into game resourcepacks dir.
    pub async fn activate_pack(&self, pack_name: &str, game_dir: &Path) -> Result<()> {
        let source = self.textures_dir.join("packs").join(pack_name);
        if !source.exists() {
            bail!("Pack '{}' not found", pack_name);
        }

        // Clear active slot
        let active_dir = self.textures_dir.join("active");
        if active_dir.exists() {
            fs::remove_dir_all(&active_dir).await?;
        }
        fs::create_dir_all(&active_dir).await?;

        // Copy to active
        if source.is_dir() {
            copy_dir_recursive(&source, &active_dir.join(pack_name)).await?;
        } else {
            fs::copy(&source, active_dir.join(pack_name)).await?;
        }

        // Inject into game resourcepacks directory
        let rp_dir = game_dir.join("resourcepacks");
        fs::create_dir_all(&rp_dir).await?;
        let dest = rp_dir.join(pack_name);
        if source.is_dir() {
            copy_dir_recursive(&source, &dest).await?;
        } else {
            fs::copy(&source, &dest).await?;
        }

        println!("  Texture pack '{}' activated.", pack_name);
        Ok(())
    }

    pub async fn deactivate(&self, game_dir: &Path) -> Result<()> {
        let active_dir = self.textures_dir.join("active");
        if !active_dir.exists() {
            return Ok(());
        }
        let mut dir = fs::read_dir(&active_dir).await?;
        while let Some(entry) = dir.next_entry().await? {
            let name = entry.file_name().to_string_lossy().to_string();
            let rp_path = game_dir.join("resourcepacks").join(&name);
            if rp_path.exists() {
                if rp_path.is_dir() {
                    fs::remove_dir_all(&rp_path).await?;
                } else {
                    fs::remove_file(&rp_path).await?;
                }
            }
        }
        fs::remove_dir_all(&active_dir).await?;
        fs::create_dir_all(&active_dir).await?;
        Ok(())
    }
}

async fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst).await?;
    let mut dir = fs::read_dir(src).await?;
    while let Some(entry) = dir.next_entry().await? {
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            Box::pin(copy_dir_recursive(&src_path, &dst_path)).await?;
        } else {
            fs::copy(&src_path, &dst_path).await?;
        }
    }
    Ok(())
}
