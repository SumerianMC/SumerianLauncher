use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;

const MODRINTH_SEARCH: &str = "https://api.modrinth.com/v2/search";
const MODRINTH_PROJECT: &str = "https://api.modrinth.com/v2/project/{id}/version";

#[derive(Debug, Deserialize)]
pub struct ModpackHit {
    pub project_id: String,
    pub title: String,
    pub description: String,
    pub downloads: u64,
}

#[derive(Debug, Deserialize)]
pub struct ModpackVersion {
    pub id: String,
    pub name: String,
    pub game_versions: Vec<String>,
    pub files: Vec<ModpackFile>,
}

#[derive(Debug, Deserialize)]
pub struct ModpackFile {
    pub url: String,
    pub filename: String,
    pub primary: bool,
}

#[derive(Debug, Deserialize)]
struct SearchResponse {
    hits: Vec<ModpackHit>,
}

// mrpack index structures
#[derive(Debug, Deserialize)]
struct MrpackIndex {
    files: Vec<MrpackFile>,
    dependencies: Option<MrpackDeps>,
}

#[derive(Debug, Deserialize)]
struct MrpackFile {
    path: String,
    downloads: Vec<String>,
    #[serde(default)]
    env: Option<MrpackEnv>,
}

#[derive(Debug, Deserialize)]
struct MrpackEnv {
    client: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MrpackDeps {
    minecraft: Option<String>,
    #[serde(rename = "fabric-loader")]
    fabric_loader: Option<String>,
    forge: Option<String>,
}

pub struct ModpackInstaller {
    client: reqwest::Client,
}

impl ModpackInstaller {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }

    pub async fn search(&self, query: &str, game_version: &str) -> Result<Vec<ModpackHit>> {
        let resp = self.client
            .get(MODRINTH_SEARCH)
            .query(&[
                ("query", query),
                ("facets", &format!("[[\"versions:{game_version}\"],[\"project_type:modpack\"]]")),
                ("limit", "10"),
            ])
            .send().await?
            .json::<SearchResponse>().await
            .context("Failed to parse Modrinth modpack search")?;
        Ok(resp.hits)
    }

    pub async fn get_versions(&self, project_id: &str, game_version: &str) -> Result<Vec<ModpackVersion>> {
        let url = MODRINTH_PROJECT.replace("{id}", project_id);
        let versions = self.client
            .get(&url)
            .query(&[("game_versions", &format!("[\"{game_version}\"]"))])
            .send().await?
            .json::<Vec<ModpackVersion>>().await
            .context("Failed to parse modpack versions")?;
        Ok(versions)
    }

    /// Download and install an mrpack into `instance_dir`.
    /// Returns (minecraft_version, optional_fabric_version, optional_forge_version).
    pub async fn install_mrpack(
        &self,
        version: &ModpackVersion,
        instance_dir: &Path,
    ) -> Result<(String, Option<String>, Option<String>)> {
        let file = version.files.iter().find(|f| f.primary || f.filename.ends_with(".mrpack"))
            .or_else(|| version.files.first())
            .ok_or_else(|| anyhow::anyhow!("No mrpack file found"))?;

        println!("  → Downloading modpack {}...", version.name);
        let bytes = self.client.get(&file.url).send().await?.bytes().await?;

        let cursor = std::io::Cursor::new(&bytes);
        let mut zip = zip::ZipArchive::new(cursor)?;

        // Parse modrinth.index.json
        let index: MrpackIndex = {
            let mut f = zip.by_name("modrinth.index.json")
                .context("Not a valid mrpack (missing modrinth.index.json)")?;
            use std::io::Read;
            let mut s = String::new();
            f.read_to_string(&mut s)?;
            serde_json::from_str(&s).context("Failed to parse modrinth.index.json")?
        };

        let mc_version = index.dependencies.as_ref()
            .and_then(|d| d.minecraft.clone())
            .unwrap_or_else(|| "unknown".to_string());
        let fabric_version = index.dependencies.as_ref().and_then(|d| d.fabric_loader.clone());
        let forge_version = index.dependencies.as_ref().and_then(|d| d.forge.clone());

        tokio::fs::create_dir_all(instance_dir).await?;

        // Extract overrides/ into instance_dir
        for i in 0..zip.len() {
            let mut zf = zip.by_index(i)?;
            let name = zf.name().to_string();
            let strip = if name.starts_with("overrides/") {
                name.strip_prefix("overrides/").unwrap_or(&name).to_string()
            } else if name.starts_with("client-overrides/") {
                name.strip_prefix("client-overrides/").unwrap_or(&name).to_string()
            } else {
                continue;
            };
            if strip.is_empty() { continue; }
            let dest = instance_dir.join(&strip);
            if zf.is_dir() {
                tokio::fs::create_dir_all(&dest).await?;
            } else {
                if let Some(p) = dest.parent() { tokio::fs::create_dir_all(p).await?; }
                use std::io::Read;
                let mut buf = Vec::new();
                zf.read_to_end(&mut buf)?;
                tokio::fs::write(&dest, buf).await?;
            }
        }

        // Download mod files listed in the index
        let mods_dir = instance_dir.join("mods");
        tokio::fs::create_dir_all(&mods_dir).await?;

        for mf in &index.files {
            // Skip server-only files
            if let Some(env) = &mf.env {
                if env.client.as_deref() == Some("unsupported") { continue; }
            }
            let dest = instance_dir.join(&mf.path);
            if dest.exists() { continue; }
            if let Some(p) = dest.parent() { tokio::fs::create_dir_all(p).await?; }
            let url = mf.downloads.first().ok_or_else(|| anyhow::anyhow!("No download URL for {}", mf.path))?;
            let data = self.client.get(url).send().await?.bytes().await?;
            tokio::fs::write(&dest, data).await?;
        }

        Ok((mc_version, fabric_version, forge_version))
    }
}
