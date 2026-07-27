use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use tokio::fs;

const MODRINTH_SEARCH: &str = "https://api.modrinth.com/v2/search";
const MODRINTH_VERSIONS: &str = "https://api.modrinth.com/v2/project/{id}/version";

#[derive(Debug, Deserialize)]
pub struct ModrinthHit {
    pub project_id: String,
    pub title: String,
    pub description: String,
    pub downloads: u64,
}

#[derive(Debug, Deserialize)]
pub struct ModrinthVersion {
    pub id: String,
    pub name: String,
    pub game_versions: Vec<String>,
    pub files: Vec<ModrinthFile>,
}

#[derive(Debug, Deserialize)]
pub struct ModrinthFile {
    pub url: String,
    pub filename: String,
    pub primary: bool,
}

#[derive(Debug, Deserialize)]
struct SearchResponse {
    hits: Vec<ModrinthHit>,
}

pub struct ModManager {
    client: reqwest::Client,
}

impl ModManager {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }

    pub async fn search(&self, query: &str, game_version: &str) -> Result<Vec<ModrinthHit>> {
        let resp = self
            .client
            .get(MODRINTH_SEARCH)
            .query(&[
                ("query", query),
                ("facets", &format!("[[\"versions:{game_version}\"],[\"project_type:mod\"]]")),
                ("limit", "10"),
            ])
            .send()
            .await
            .context("Modrinth search failed")?
            .json::<SearchResponse>()
            .await
            .context("Failed to parse Modrinth response")?;
        Ok(resp.hits)
    }

    pub async fn get_versions(&self, project_id: &str, game_version: &str) -> Result<Vec<ModrinthVersion>> {
        let url = MODRINTH_VERSIONS.replace("{id}", project_id);
        let versions = self
            .client
            .get(&url)
            .query(&[("game_versions", &format!("[\"{game_version}\"]"))])
            .send()
            .await?
            .json::<Vec<ModrinthVersion>>()
            .await
            .context("Failed to parse version list")?;
        Ok(versions)
    }

    /// Download the primary file of a ModrinthVersion into the instance mods dir.
    pub async fn download_mod(&self, version: &ModrinthVersion, mods_dir: &Path) -> Result<PathBuf> {
        let file = version
            .files
            .iter()
            .find(|f| f.primary)
            .or_else(|| version.files.first())
            .ok_or_else(|| anyhow::anyhow!("No files for mod version"))?;

        fs::create_dir_all(mods_dir).await?;
        let dest = mods_dir.join(&file.filename);

        if dest.exists() {
            bail!("'{}' is already installed.", file.filename);
        }

        let bytes = self
            .client
            .get(&file.url)
            .send()
            .await?
            .bytes()
            .await?;

        fs::write(&dest, &bytes).await?;
        Ok(dest)
    }

    pub async fn list_installed(mods_dir: &Path) -> Result<Vec<String>> {
        let mut mods = Vec::new();
        let mut dir = match fs::read_dir(mods_dir).await {
            Ok(d) => d,
            Err(_) => return Ok(mods),
        };
        while let Some(entry) = dir.next_entry().await? {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.ends_with(".jar") {
                mods.push(name);
            }
        }
        Ok(mods)
    }

    pub async fn remove_mod(mods_dir: &Path, filename: &str) -> Result<()> {
        let path = mods_dir.join(filename);
        if !path.exists() {
            bail!("Mod '{}' not found.", filename);
        }
        fs::remove_file(&path).await?;
        Ok(())
    }
}
