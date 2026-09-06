/// Named Mod Profiles — save and switch between named sets of enabled/disabled
/// mods for a given instance.
///
/// A profile records which mod filenames are *disabled* (renamed to `.jar.disabled`
/// on disk by `InstanceManager::disable_mod`). Switching profiles applies the
/// stored enabled/disabled state to the instance's `mods/` directory.
///
/// Storage: `<instances_dir>/<instance>/named_mod_profiles.json`
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs;

/// A single named mod profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamedModProfile {
    /// Display name (e.g. "speedrun", "casual", "benchmark").
    pub name: String,
    /// Filenames of mods that should be *disabled* when this profile is active.
    pub disabled: Vec<String>,
    /// RFC3339 creation/last-modified timestamp.
    pub updated_at: String,
}

pub struct ModProfileManager {
    instances_dir: PathBuf,
}

impl ModProfileManager {
    pub fn new(instances_dir: &Path) -> Self {
        Self {
            instances_dir: instances_dir.to_path_buf(),
        }
    }

    fn profiles_path(&self, instance: &str) -> PathBuf {
        self.instances_dir
            .join(instance)
            .join("named_mod_profiles.json")
    }

    fn mods_dir(&self, instance: &str) -> PathBuf {
        self.instances_dir.join(instance).join("mods")
    }

    // ── index helpers ────────────────────────────────────────────────────────

    pub async fn load_all(&self, instance: &str) -> Vec<NamedModProfile> {
        match fs::read_to_string(self.profiles_path(instance)).await {
            Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
            Err(_) => Vec::new(),
        }
    }

    async fn save_all(&self, instance: &str, profiles: &[NamedModProfile]) -> Result<()> {
        let path = self.profiles_path(instance);
        if let Some(p) = path.parent() {
            fs::create_dir_all(p).await?;
        }
        fs::write(path, serde_json::to_string_pretty(profiles)?).await?;
        Ok(())
    }

    // ── public API ───────────────────────────────────────────────────────────

    /// Snapshot the current enabled/disabled state of the instance's mods/
    /// directory as a new named profile.
    pub async fn save_current(&self, instance: &str, profile_name: &str) -> Result<NamedModProfile> {
        let mods_dir = self.mods_dir(instance);
        let disabled = collect_disabled_mods(&mods_dir).await;

        let mut profiles = self.load_all(instance).await;
        // Replace if name already exists.
        profiles.retain(|p| p.name != profile_name);

        let profile = NamedModProfile {
            name: profile_name.to_string(),
            disabled,
            updated_at: chrono::Utc::now().to_rfc3339(),
        };
        profiles.push(profile.clone());
        self.save_all(instance, &profiles).await?;
        Ok(profile)
    }

    /// Apply a named profile to the instance: enable all mods first, then
    /// disable only the ones listed in the profile.
    pub async fn apply(&self, instance: &str, profile_name: &str) -> Result<()> {
        let profiles = self.load_all(instance).await;
        let profile = profiles
            .iter()
            .find(|p| p.name == profile_name)
            .ok_or_else(|| anyhow::anyhow!("Mod profile '{}' not found.", profile_name))?
            .clone();

        let mods_dir = self.mods_dir(instance);
        if !mods_dir.exists() {
            bail!("Mods directory not found for instance '{}'.", instance);
        }

        // Step 1 — enable everything (rename *.jar.disabled → *.jar).
        enable_all_mods(&mods_dir).await?;

        // Step 2 — disable the mods listed in the profile.
        for filename in &profile.disabled {
            let src = mods_dir.join(filename);
            let dst = mods_dir.join(format!("{}.disabled", filename));
            if src.exists() {
                fs::rename(&src, &dst).await?;
            }
            // If already disabled or not present, silently skip.
        }

        Ok(())
    }

    /// Delete a named mod profile (does not change disk state of mods).
    pub async fn delete(&self, instance: &str, profile_name: &str) -> Result<()> {
        let mut profiles = self.load_all(instance).await;
        let before = profiles.len();
        profiles.retain(|p| p.name != profile_name);
        if profiles.len() == before {
            bail!("Mod profile '{}' not found.", profile_name);
        }
        self.save_all(instance, &profiles).await
    }

    /// Return a human-readable diff between two profiles: mods that differ.
    pub async fn diff(
        &self,
        instance: &str,
        a: &str,
        b: &str,
    ) -> Result<Vec<String>> {
        let profiles = self.load_all(instance).await;
        let pa = profiles
            .iter()
            .find(|p| p.name == a)
            .ok_or_else(|| anyhow::anyhow!("Profile '{}' not found.", a))?;
        let pb = profiles
            .iter()
            .find(|p| p.name == b)
            .ok_or_else(|| anyhow::anyhow!("Profile '{}' not found.", b))?;

        let mut lines = Vec::new();
        // Mods disabled in A but not B.
        for m in &pa.disabled {
            if !pb.disabled.contains(m) {
                lines.push(format!("  enabled  in '{}', disabled in '{}': {}", b, a, m));
            }
        }
        // Mods disabled in B but not A.
        for m in &pb.disabled {
            if !pa.disabled.contains(m) {
                lines.push(format!("  disabled in '{}', enabled  in '{}': {}", b, a, m));
            }
        }
        Ok(lines)
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

/// Collect basenames of all `.jar.disabled` files in `mods_dir`.
async fn collect_disabled_mods(mods_dir: &Path) -> Vec<String> {
    let mut disabled = Vec::new();
    if let Ok(mut rd) = fs::read_dir(mods_dir).await {
        while let Ok(Some(entry)) = rd.next_entry().await {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.ends_with(".jar.disabled") {
                // Strip the `.disabled` suffix to get the canonical filename.
                disabled.push(name.trim_end_matches(".disabled").to_string());
            }
        }
    }
    disabled
}

/// Rename every `*.jar.disabled` → `*.jar` in `mods_dir`.
async fn enable_all_mods(mods_dir: &Path) -> Result<()> {
    let mut rd = fs::read_dir(mods_dir).await?;
    while let Ok(Some(entry)) = rd.next_entry().await {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name.ends_with(".jar.disabled") {
            let new_name = name.trim_end_matches(".disabled");
            fs::rename(&path, mods_dir.join(new_name)).await?;
        }
    }
    Ok(())
}
