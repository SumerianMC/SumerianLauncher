/// JVM Flag Presets — user-defined named sets of extra JVM arguments.
///
/// These complement (rather than replace) the built-in `OptimizationProfile`
/// flags.  A preset is appended to the JVM arg list at launch so users can
/// store things like GraalVM hints, agent flags, or module opens without
/// editing JSON by hand.
///
/// Storage: `<data_dir>/config/jvm_presets.json`
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs;

/// A single named collection of extra JVM arguments.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JvmPreset {
    /// Display name (e.g. "GraalVM Native", "Debug Agent").
    pub name: String,
    /// The raw JVM flags, one per element (e.g. ["-XX:+UnlockDiagnosticVMOptions"]).
    pub flags: Vec<String>,
    /// Optional description shown in the menu.
    pub description: String,
    /// RFC3339 last-modified timestamp.
    pub updated_at: String,
}

pub struct JvmPresetManager {
    path: PathBuf,
}

impl JvmPresetManager {
    pub fn new(data_dir: &Path) -> Self {
        Self {
            path: data_dir.join("config").join("jvm_presets.json"),
        }
    }

    // ── persistence ───────────────────────────────────────────────────────────

    pub async fn load_all(&self) -> Vec<JvmPreset> {
        match fs::read_to_string(&self.path).await {
            Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
            Err(_) => Vec::new(),
        }
    }

    async fn save_all(&self, presets: &[JvmPreset]) -> Result<()> {
        if let Some(p) = self.path.parent() {
            fs::create_dir_all(p).await?;
        }
        fs::write(&self.path, serde_json::to_string_pretty(presets)?).await?;
        Ok(())
    }

    // ── CRUD ──────────────────────────────────────────────────────────────────

    /// Create a new preset. Fails if the name is already taken.
    pub async fn create(
        &self,
        name: &str,
        flags: Vec<String>,
        description: &str,
    ) -> Result<JvmPreset> {
        let mut presets = self.load_all().await;
        if presets.iter().any(|p| p.name.eq_ignore_ascii_case(name)) {
            bail!("A JVM preset named '{}' already exists.", name);
        }
        let preset = JvmPreset {
            name: name.to_string(),
            flags,
            description: description.to_string(),
            updated_at: chrono::Utc::now().to_rfc3339(),
        };
        presets.push(preset.clone());
        self.save_all(&presets).await?;
        Ok(preset)
    }

    /// Update flags and/or description of an existing preset.
    pub async fn update(
        &self,
        name: &str,
        new_flags: Option<Vec<String>>,
        new_description: Option<&str>,
    ) -> Result<()> {
        let mut presets = self.load_all().await;
        let preset = presets
            .iter_mut()
            .find(|p| p.name.eq_ignore_ascii_case(name))
            .ok_or_else(|| anyhow::anyhow!("JVM preset '{}' not found.", name))?;

        if let Some(flags) = new_flags {
            preset.flags = flags;
        }
        if let Some(desc) = new_description {
            preset.description = desc.to_string();
        }
        preset.updated_at = chrono::Utc::now().to_rfc3339();
        self.save_all(&presets).await
    }

    /// Delete a preset by name.
    pub async fn delete(&self, name: &str) -> Result<()> {
        let mut presets = self.load_all().await;
        let before = presets.len();
        presets.retain(|p| !p.name.eq_ignore_ascii_case(name));
        if presets.len() == before {
            bail!("JVM preset '{}' not found.", name);
        }
        self.save_all(&presets).await
    }

    /// Return a preset by name.
    pub async fn get(&self, name: &str) -> Option<JvmPreset> {
        self.load_all()
            .await
            .into_iter()
            .find(|p| p.name.eq_ignore_ascii_case(name))
    }
}
