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

    /// Returns the directory where texture packs are stored.
    pub fn packs_dir(&self) -> PathBuf {
        self.textures_dir.join("packs")
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

    /// Return a human-readable preview of a pack: format version and description
    /// from `pack.mcmeta`, without extracting or activating the pack.
    pub async fn preview_pack(&self, pack_name: &str) -> Result<PackPreview> {
        let pack_path = self.textures_dir.join("packs").join(pack_name);
        if !pack_path.exists() {
            bail!("Pack '{}' not found.", pack_name);
        }
        read_pack_meta(&pack_path)
    }

    pub async fn deactivate(&self, game_dir: &Path) -> Result<()> {        let active_dir = self.textures_dir.join("active");
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

// ── Resource pack preview ─────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct PackPreview {
    /// `pack_format` integer from pack.mcmeta.
    pub format: i64,
    /// `description` string from pack.mcmeta (may contain § colour codes).
    pub description: String,
}

impl PackPreview {
    /// Map pack_format → approximate MC version range.
    pub fn format_label(&self) -> &'static str {
        match self.format {
            1  => "1.6–1.8",
            2  => "1.9–1.10",
            3  => "1.11–1.12",
            4  => "1.13–1.14",
            5  => "1.15–1.16",
            6  => "1.16.2–1.16.5",
            7  => "1.17",
            8  => "1.18–1.18.2",
            9  => "1.19–1.19.2",
            12 => "1.19.3",
            13 => "1.19.4",
            15 => "1.20–1.20.1",
            18 => "1.20.2",
            22 => "1.20.3–1.20.4",
            32 => "1.20.5–1.20.6",
            34 => "1.21",
            _  => "unknown",
        }
    }

    /// Strip Minecraft § colour codes for clean terminal display.
    pub fn clean_description(&self) -> String {
        let mut out = String::new();
        let mut chars = self.description.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '§' {
                chars.next(); // skip the formatting code character
            } else {
                out.push(c);
            }
        }
        out
    }
}

/// Read `pack.mcmeta` from either a zip archive or a directory.
fn read_pack_meta(pack_path: &std::path::Path) -> Result<PackPreview> {
    let raw = if pack_path.is_dir() {
        let meta_path = pack_path.join("pack.mcmeta");
        if !meta_path.exists() {
            bail!("pack.mcmeta not found in '{}'", pack_path.display());
        }
        std::fs::read_to_string(meta_path)?
    } else {
        // ZIP archive
        let file = std::fs::File::open(pack_path)?;
        let mut archive = zip::ZipArchive::new(file)?;
        let mut zf = archive.by_name("pack.mcmeta")
            .map_err(|_| anyhow::anyhow!("pack.mcmeta not found in zip '{}'", pack_path.display()))?;
        use std::io::Read;
        let mut s = String::new();
        zf.read_to_string(&mut s)?;
        s
    };

    let v: serde_json::Value = serde_json::from_str(&raw)
        .context("Failed to parse pack.mcmeta as JSON")?;

    let format = v["pack"]["pack_format"].as_i64().unwrap_or(0);
    let description = v["pack"]["description"]
        .as_str()
        .unwrap_or("")
        .to_string();

    Ok(PackPreview { format, description })
}
