use anyhow::{Context, Result};
use base64::Engine;
use std::path::PathBuf;

const PROFILE_URL: &str = "https://api.minecraftservices.com/minecraft/profile";
const SKIN_URL: &str = "https://api.minecraftservices.com/minecraft/profile/skins";

// ely.by skin API
const ELY_SESSION_URL: &str = "https://sessionserver.ely.by/session/minecraft/profile";
const ELY_SKIN_UPLOAD_URL: &str = "https://skinsystem.ely.by/api/skins";

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

#[derive(serde::Deserialize)]
struct ElySessionProfile {
    name: String,
    properties: Vec<ElyProperty>,
}

#[derive(serde::Deserialize)]
struct ElyProperty {
    name: String,
    value: String,
}

#[derive(serde::Deserialize)]
struct ElyTexturesWrapper {
    textures: ElyTextureMap,
}

#[derive(serde::Deserialize)]
struct ElyTextureMap {
    #[serde(rename = "SKIN")]
    skin: Option<ElyTexture>,
}

#[derive(serde::Deserialize)]
struct ElyTexture {
    url: String,
    metadata: Option<ElyTextureMetadata>,
}

#[derive(serde::Deserialize)]
struct ElyTextureMetadata {
    model: Option<String>,
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

    pub async fn get_profile_ely(&self, uuid: &str) -> Result<MinecraftProfile> {
        // sessionserver expects UUID without dashes
        let uuid_clean = uuid.replace('-', "");
        let url = format!("{}/{}", ELY_SESSION_URL, uuid_clean);
        let ely: ElySessionProfile = self.client
            .get(&url)
            .send().await?
            .json::<ElySessionProfile>().await
            .context("Failed to fetch ely.by session profile")?;

        let textures_b64 = ely.properties.iter()
            .find(|p| p.name == "textures")
            .map(|p| p.value.as_str())
            .unwrap_or("");

        let skin_entry = if !textures_b64.is_empty() {
            if let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(textures_b64) {
                if let Ok(tex) = serde_json::from_slice::<ElyTexturesWrapper>(&decoded) {
                    tex.textures.skin.map(|s| {
                        let variant = s.metadata
                            .and_then(|m| m.model)
                            .unwrap_or_else(|| "classic".into());
                        SkinEntry {
                            id: String::new(),
                            state: "ACTIVE".into(),
                            url: s.url,
                            variant,
                        }
                    })
                } else { None }
            } else { None }
        } else { None };

        Ok(MinecraftProfile {
            id: uuid.to_string(),
            name: ely.name,
            skins: skin_entry.into_iter().collect(),
        })
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

    pub async fn upload_skin_ely(&self, access_token: &str, path: &PathBuf, variant: &str) -> Result<()> {
        let bytes = tokio::fs::read(path).await.context("Failed to read skin file")?;
        let filename = path.file_name().unwrap_or_default().to_string_lossy().to_string();

        let part = reqwest::multipart::Part::bytes(bytes)
            .file_name(filename)
            .mime_str("image/png")?;
        let form = reqwest::multipart::Form::new()
            .text("model", if variant == "slim" { "slim" } else { "" }.to_string())
            .part("skin", part);

        let resp = self.client
            .post(ELY_SKIN_UPLOAD_URL)
            .bearer_auth(access_token)
            .multipart(form)
            .send().await?;

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("ely.by skin upload failed: {}", text);
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

    pub async fn reset_skin_ely(&self, access_token: &str) -> Result<()> {
        let resp = self.client
            .delete(ELY_SKIN_UPLOAD_URL)
            .bearer_auth(access_token)
            .send().await?;
        if !resp.status().is_success() {
            anyhow::bail!("ely.by skin reset failed (status {})", resp.status());
        }
        Ok(())
    }
}
