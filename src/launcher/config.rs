use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs;

use crate::optimizer::OptimizationProfile;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LauncherConfig {
    #[serde(default)]
    pub default_optimization: OptimizationProfile,
    #[serde(default)]
    pub auto_backup_on_launch: bool,
    #[serde(default)]
    pub java8_path: Option<String>,
    #[serde(default)]
    pub java21_path: Option<String>,
    #[serde(default)]
    pub java25_path: Option<String>,
    #[serde(default)]
    pub default_width: Option<u32>,
    #[serde(default)]
    pub default_height: Option<u32>,
    #[serde(default)]
    pub close_on_launch: bool,
    #[serde(default)]
    pub check_updates_on_start: bool,
    #[serde(default)]
    pub discord_rpc: bool,
}

impl Default for LauncherConfig {
    fn default() -> Self {
        Self {
            default_optimization: OptimizationProfile::Balanced,
            auto_backup_on_launch: false,
            java8_path: None,
            java21_path: None,
            java25_path: None,
            default_width: None,
            default_height: None,
            close_on_launch: false,
            check_updates_on_start: true,
            discord_rpc: true,
        }
    }
}

pub struct ConfigManager {
    path: PathBuf,
}

impl ConfigManager {
    pub fn new(data_dir: &Path) -> Self {
        Self { path: data_dir.join("config").join("launcher.json") }
    }

    pub async fn load(&self) -> LauncherConfig {
        match fs::read_to_string(&self.path).await {
            Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
            Err(_) => LauncherConfig::default(),
        }
    }

    pub async fn save(&self, cfg: &LauncherConfig) -> Result<()> {
        if let Some(p) = self.path.parent() { fs::create_dir_all(p).await?; }
        fs::write(&self.path, serde_json::to_string_pretty(cfg)?).await?;
        Ok(())
    }
}
