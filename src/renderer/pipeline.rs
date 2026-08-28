use anyhow::Result;
use console::style;
use std::path::Path;

use crate::client::injection::VersionEra;
use crate::renderer::shaders::{ShaderManager, ShaderTier};
use crate::renderer::textures::TextureManager;

pub struct RenderPipeline<'a> {
    pub textures: &'a TextureManager,
    pub shaders: &'a ShaderManager,
}

impl<'a> RenderPipeline<'a> {
    pub fn new(textures: &'a TextureManager, shaders: &'a ShaderManager) -> Self {
        Self { textures, shaders }
    }

    /// Apply selected texture pack and shader preset before launching.
    /// `era` is used to select the correct GLSL tier and warn when shaders
    /// are not supported by the target version.
    pub async fn apply(
        &self,
        texture_pack: Option<&str>,
        shader_preset: Option<&str>,
        game_dir: &Path,
        era: &VersionEra,
    ) -> Result<()> {
        self.textures.deactivate(game_dir).await?;

        if let Some(pack) = texture_pack {
            self.textures.activate_pack(pack, game_dir).await?;
        }

        if let Some(preset) = shader_preset {
            let tier = ShaderTier::for_era(era);
            match tier {
                ShaderTier::None => {
                    println!(
                        "  {} Shaders skipped: {} does not support any shader mod.",
                        style("⚠").yellow(),
                        tier.description()
                    );
                    self.shaders.disable_shaders(game_dir).await?;
                }
                ShaderTier::Legacy => {
                    println!(
                        "  {} Using legacy GLSL tier: {}",
                        style("→").cyan(),
                        tier.description()
                    );
                    match self.shaders.inject_shader_for_era(preset, game_dir, era).await {
                        Ok(true)  => println!("  {} Shader preset '{}' injected (legacy).", style("✓").green(), preset),
                        Ok(false) => {}
                        Err(e)    => println!("  {} Shader injection failed: {}", style("✗").red(), e),
                    }
                }
                ShaderTier::Modern => {
                    match self.shaders.inject_shader_for_era(preset, game_dir, era).await {
                        Ok(true)  => println!("  {} Shader preset '{}' injected.", style("✓").green(), preset),
                        Ok(false) => {}
                        Err(e)    => println!("  {} Shader injection failed: {}", style("✗").red(), e),
                    }
                }
            }
        } else {
            self.shaders.disable_shaders(game_dir).await?;
        }

        Ok(())
    }
}
