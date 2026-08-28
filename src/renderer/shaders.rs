use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs;

use crate::client::injection::VersionEra;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShaderConfig {
    pub name: String,
    pub shadow_quality: u8,
    pub water_reflections: bool,
    pub bloom: bool,
    pub ambient_occlusion: bool,
    pub antialiasing: String,
    pub render_distance_boost: i32,
}

impl Default for ShaderConfig {
    fn default() -> Self {
        Self {
            name: "Vanilla Plus".into(),
            shadow_quality: 2,
            water_reflections: true,
            bloom: false,
            ambient_occlusion: true,
            antialiasing: "FXAA".into(),
            render_distance_boost: 0,
        }
    }
}

/// Which GLSL API tier a Minecraft version supports.
#[derive(Debug, Clone, PartialEq)]
pub enum ShaderTier {
    /// Classic / Alpha / Beta — no shader mod existed, shaders cannot be injected.
    None,
    /// Beta 1.7 through 1.6.4 — GLSL Shaders Mod (Karyonix). No MRT, no mc_Entity,
    /// no shadow uniforms. Pure OpenGL 2.1 post-processing only.
    Legacy,
    /// 1.7.2+ with OptiFine or Iris — full gbuffers pipeline, MRT, shadow map, mc_Entity.
    Modern,
}

impl ShaderTier {
    pub fn for_era(era: &VersionEra) -> Self {
        match era {
            VersionEra::Classic | VersionEra::Alpha | VersionEra::Beta => ShaderTier::None,
            VersionEra::ReleaseLegacy => ShaderTier::Legacy,
            VersionEra::Release | VersionEra::Snapshot => ShaderTier::Modern,
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            ShaderTier::None   => "no shader support (Classic/Alpha/Beta)",
            ShaderTier::Legacy => "GLSL Shaders Mod (1.0–1.6.4, OpenGL 2.1, no MRT/shadows)",
            ShaderTier::Modern => "OptiFine/Iris (1.7.2+, full gbuffers pipeline)",
        }
    }
}

pub struct ShaderManager {
    pub shaders_dir: PathBuf,
}

impl ShaderManager {
    pub fn new(shaders_dir: &Path) -> Self {
        Self {
            shaders_dir: shaders_dir.to_path_buf(),
        }
    }

    pub async fn list_presets(&self) -> Result<Vec<String>> {
        let mut presets = Vec::new();
        let mut dir = match fs::read_dir(&self.shaders_dir).await {
            Ok(d) => d,
            Err(_) => return Ok(presets),
        };
        while let Some(entry) = dir.next_entry().await? {
            let path = entry.path();
            if path.is_dir() {
                presets.push(
                    path.file_name()
                        .unwrap()
                        .to_string_lossy()
                        .to_string(),
                );
            }
        }
        Ok(presets)
    }

    pub async fn load_preset(&self, preset_name: &str) -> Result<ShaderConfig> {
        let config_path = self.shaders_dir.join(preset_name).join("shader.json");
        let raw = fs::read_to_string(&config_path)
            .await
            .with_context(|| format!("Shader preset '{}' not found", preset_name))?;
        serde_json::from_str(&raw).context("Invalid shader.json")
    }

    /// Inject the correct GLSL tier for the running Minecraft version.
    /// Returns Ok(false) if the era cannot support shaders at all.
    pub async fn inject_shader_for_era(
        &self,
        preset_name: &str,
        game_dir: &Path,
        era: &VersionEra,
    ) -> Result<bool> {
        let tier = ShaderTier::for_era(era);

        if tier == ShaderTier::None {
            return Ok(false);
        }

        let preset_root = self.shaders_dir.join(preset_name);
        if !preset_root.exists() {
            bail!("Shader preset '{}' not found", preset_name);
        }

        // Legacy tier uses the shaders/legacy/ subfolder; modern uses shaders/
        let glsl_src = match tier {
            ShaderTier::Legacy => preset_root.join("legacy"),
            ShaderTier::Modern => preset_root.join("shaders"),
            ShaderTier::None   => unreachable!(),
        };

        if !glsl_src.exists() {
            bail!(
                "Preset '{}' has no '{}' folder. Cannot inject shaders for {}.",
                preset_name,
                if tier == ShaderTier::Legacy { "legacy" } else { "shaders" },
                tier.description()
            );
        }

        let dest = game_dir.join("shaderpacks").join(preset_name);
        if dest.exists() {
            fs::remove_dir_all(&dest).await?;
        }

        // Copy shader.json + the correct GLSL subfolder
        fs::create_dir_all(&dest).await?;
        let json_src = preset_root.join("shader.json");
        if json_src.exists() {
            fs::copy(&json_src, dest.join("shader.json")).await?;
        }
        copy_dir_recursive(&glsl_src, &dest.join("shaders")).await?;

        let options_path = game_dir.join("optionsshaders.txt");
        fs::write(&options_path, format!("shaderPack={}\n", preset_name)).await?;

        Ok(true)
    }

    /// Legacy entry point kept for the standalone shader manager menu (no era context).
    pub async fn inject_shader(&self, preset_name: &str, game_dir: &Path) -> Result<()> {
        let preset_dir = self.shaders_dir.join(preset_name);
        if !preset_dir.exists() {
            bail!("Shader preset '{}' not found", preset_name);
        }
        let dest = game_dir.join("shaderpacks").join(preset_name);
        if dest.exists() {
            fs::remove_dir_all(&dest).await?;
        }
        copy_dir_recursive(&preset_dir, &dest).await?;
        let options_path = game_dir.join("optionsshaders.txt");
        fs::write(&options_path, format!("shaderPack={}\n", preset_name)).await?;
        println!("  Shader preset '{}' injected.", preset_name);
        Ok(())
    }

    pub async fn disable_shaders(&self, game_dir: &Path) -> Result<()> {
        let options_path = game_dir.join("optionsshaders.txt");
        fs::write(&options_path, "shaderPack=(internal)\n").await?;
        Ok(())
    }
}

async fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst).await?;
    let mut dir = fs::read_dir(src).await?;
    while let Some(entry) = dir.next_entry().await? {
        let s = entry.path();
        let d = dst.join(entry.file_name());
        if s.is_dir() {
            Box::pin(copy_dir_recursive(&s, &d)).await?;
        } else {
            fs::copy(&s, &d).await?;
        }
    }
    Ok(())
}
