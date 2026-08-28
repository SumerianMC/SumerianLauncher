use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs;

use crate::optimizer::OptimizationProfile;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchPreset {
    pub name: String,
    pub version_id: String,
    pub optimization: OptimizationProfile,
    pub texture_pack: Option<String>,
    pub shader_preset: Option<String>,
    #[serde(default)]
    pub custom_jvm_args: Vec<String>,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
    #[serde(default)]
    pub server: Option<String>,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub instance: Option<String>,
}

impl LaunchPreset {
    pub fn summary(&self) -> String {
        let tex = self.texture_pack.as_deref().unwrap_or("none");
        let shd = self.shader_preset.as_deref().unwrap_or("none");
        let res = match (self.width, self.height) {
            (Some(w), Some(h)) => format!("{}x{}", w, h),
            _ => "default".into(),
        };
        format!(
            "{} | {} | tex:{} shd:{} res:{}",
            self.version_id, self.optimization, tex, shd, res
        )
    }
}

pub struct PresetManager {
    path: PathBuf,
}

impl PresetManager {
    pub fn new(data_dir: &Path) -> Self {
        Self {
            path: data_dir.join("presets.json"),
        }
    }

    pub async fn load_all(&self) -> Result<Vec<LaunchPreset>> {
        match fs::read_to_string(&self.path).await {
            Ok(raw) => Ok(serde_json::from_str(&raw).unwrap_or_default()),
            Err(_) => Ok(Vec::new()),
        }
    }

    pub async fn save_all(&self, presets: &[LaunchPreset]) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).await?;
        }
        fs::write(&self.path, serde_json::to_string_pretty(presets)?).await?;
        Ok(())
    }

    pub async fn add(&self, preset: LaunchPreset) -> Result<()> {
        let mut presets = self.load_all().await?;
        if presets.iter().any(|p| p.name.eq_ignore_ascii_case(&preset.name)) {
            bail!("A preset named '{}' already exists.", preset.name);
        }
        presets.push(preset);
        self.save_all(&presets).await
    }

    pub async fn remove(&self, name: &str) -> Result<()> {
        let mut presets = self.load_all().await?;
        let before = presets.len();
        presets.retain(|p| !p.name.eq_ignore_ascii_case(name));
        if presets.len() == before {
            bail!("No preset named '{}' found.", name);
        }
        self.save_all(&presets).await
    }

    pub async fn update(&self, preset: LaunchPreset) -> Result<()> {
        let mut presets = self.load_all().await?;
        match presets.iter_mut().find(|p| p.name.eq_ignore_ascii_case(&preset.name)) {
            Some(p) => *p = preset,
            None => bail!("No preset named '{}' found.", preset.name),
        }
        self.save_all(&presets).await
    }
}
