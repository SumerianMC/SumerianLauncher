use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs;

/// A named instance is an isolated copy of a version's game directory.
/// Each instance has its own mods/, saves/, resourcepacks/, shaderpacks/.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Instance {
    pub name: String,
    pub version_id: String,
    pub created_at: String,
}

pub struct InstanceManager {
    instances_dir: PathBuf,
    index_path: PathBuf,
}

impl InstanceManager {
    pub fn new(data_dir: &Path) -> Self {
        Self {
            instances_dir: data_dir.join("instances"),
            index_path: data_dir.join("instances.json"),
        }
    }

    pub async fn load_all(&self) -> Result<Vec<Instance>> {
        match fs::read_to_string(&self.index_path).await {
            Ok(raw) => Ok(serde_json::from_str(&raw).unwrap_or_default()),
            Err(_) => Ok(Vec::new()),
        }
    }

    async fn save_all(&self, instances: &[Instance]) -> Result<()> {
        fs::create_dir_all(&self.instances_dir).await?;
        fs::write(&self.index_path, serde_json::to_string_pretty(instances)?).await?;
        Ok(())
    }

    /// Create a new instance directory for the given version.
    pub async fn create(&self, name: &str, version_id: &str) -> Result<Instance> {
        let mut all = self.load_all().await?;
        if all.iter().any(|i| i.name.eq_ignore_ascii_case(name)) {
            bail!("Instance '{}' already exists.", name);
        }
        let dir = self.instance_dir(name);
        for sub in &["mods", "saves", "resourcepacks", "shaderpacks", "config"] {
            fs::create_dir_all(dir.join(sub)).await?;
        }
        let instance = Instance {
            name: name.to_string(),
            version_id: version_id.to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        all.push(instance.clone());
        self.save_all(&all).await?;
        Ok(instance)
    }

    pub async fn delete(&self, name: &str) -> Result<()> {
        let mut all = self.load_all().await?;
        let before = all.len();
        all.retain(|i| !i.name.eq_ignore_ascii_case(name));
        if all.len() == before {
            bail!("Instance '{}' not found.", name);
        }
        let dir = self.instance_dir(name);
        if dir.exists() {
            fs::remove_dir_all(&dir).await?;
        }
        self.save_all(&all).await
    }

    /// Returns the game directory for an instance (passed as --gameDir to Minecraft).
    pub fn instance_dir(&self, name: &str) -> PathBuf {
        self.instances_dir.join(name)
    }
}
