use anyhow::{Context, Result};
use std::path::PathBuf;

const PROFILE_URL: &str = "https://api.minecraftservices.com/minecraft/profile";
const SKIN_URL: &str = "https://api.minecraftservices.com/minecraft/profile/skins";

#[derive(serde::Deserialize)]
pub struct MinecraftProfile {
    pub id: String,
    pub name: String,
    pub skins: Vec<SkinEntry>,
}

#[derive(serde::Deserialize)]
pub struct SkinEntry {
    pub id: String,
    pub state: String,
    pub url: String,
    pub variant: String,
}

pub struct SkinManager {
    client: reqwest::Client,
}

impl SkinManager {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }

    pub async fn get_profile(&self, access_token: &str) -> Result<MinecraftProfile> {
        self.client
            .get(PROFILE_URL)
            .bearer_auth(access_token)
            .send().await?
            .json::<MinecraftProfile>().await
            .context("Failed to fetch Minecraft profile")
    }

    /// Upload a skin file (PNG). variant = "classic" or "slim"
    pub async fn upload_skin(&self, access_token: &str, path: &PathBuf, variant: &str) -> Result<()> {
        let bytes = tokio::fs::read(path).await.context("Failed to read skin file")?;
        let filename = path.file_name().unwrap_or_default().to_string_lossy().to_string();

        let part = reqwest::multipart::Part::bytes(bytes)
            .file_name(filename)
            .mime_str("image/png")?;
        let form = reqwest::multipart::Form::new()
            .text("variant", variant.to_string())
            .part("file", part);

        let resp = self.client
            .post(SKIN_URL)
            .bearer_auth(access_token)
            .multipart(form)
            .send().await?;

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("Skin upload failed: {}", text);
        }
        Ok(())
    }

    /// Reset skin back to the default Steve/Alex.
    pub async fn reset_skin(&self, access_token: &str, uuid: &str) -> Result<()> {
        let url = format!("https://api.minecraftservices.com/minecraft/profile/skins/active");
        let resp = self.client
            .delete(&url)
            .bearer_auth(access_token)
            .send().await?;
        if !resp.status().is_success() {
            anyhow::bail!("Failed to reset skin (status {}). UUID: {}", resp.status(), uuid);
        }
        Ok(())
    }
}
