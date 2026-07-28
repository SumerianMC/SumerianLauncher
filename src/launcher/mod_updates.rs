use anyhow::Result;
use serde::Deserialize;
use std::path::Path;

const MODRINTH_HASH_URL: &str = "https://api.modrinth.com/v2/version_file/{hash}";
const MODRINTH_UPDATE_URL: &str = "https://api.modrinth.com/v2/version_file/{hash}/update";

#[derive(Debug, Deserialize)]
struct VersionFile {
    hashes: FileHashes,
}

#[derive(Debug, Deserialize)]
struct FileHashes {
    sha1: String,
}

#[derive(Debug, Deserialize)]
pub struct ModVersion {
    pub version_number: String,
    pub name: String,
    pub files: Vec<UpdateFile>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateFile {
    pub url: String,
    pub filename: String,
    pub primary: bool,
}

pub struct ModUpdate {
    pub filename: String,
    pub current_version: String,
    pub latest_version: String,
    pub download_url: String,
    pub new_filename: String,
}

pub async fn check_updates(
    client: &reqwest::Client,
    mods_dir: &Path,
    game_version: &str,
    loader: &str,
) -> Result<Vec<ModUpdate>> {
    let mut updates = Vec::new();

    let mut rd = match tokio::fs::read_dir(mods_dir).await {
        Ok(d) => d,
        Err(_) => return Ok(updates),
    };

    while let Some(entry) = rd.next_entry().await? {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("jar") {
            continue;
        }
        let filename = path.file_name().unwrap_or_default().to_string_lossy().to_string();

        // Compute SHA1 of the jar
        let bytes = match tokio::fs::read(&path).await {
            Ok(b) => b,
            Err(_) => continue,
        };
        use sha1::Digest;
        let hash = format!("{:x}", sha1::Sha1::digest(&bytes));

        // Look up current version on Modrinth by hash
        let current_url = MODRINTH_HASH_URL.replace("{hash}", &hash);
        let current: serde_json::Value = match client.get(&current_url).send().await {
            Ok(r) if r.status().is_success() => match r.json().await {
                Ok(v) => v,
                Err(_) => continue,
            },
            _ => continue, // not on Modrinth
        };
        let current_version = current["version_number"].as_str().unwrap_or("unknown").to_string();

        // Check for update
        let update_url = MODRINTH_UPDATE_URL.replace("{hash}", &hash);
        let update_resp = client
            .post(&update_url)
            .json(&serde_json::json!({
                "loaders": [loader],
                "game_versions": [game_version]
            }))
            .send().await;

        let update: ModVersion = match update_resp {
            Ok(r) if r.status().is_success() => match r.json().await {
                Ok(v) => v,
                Err(_) => continue,
            },
            _ => continue,
        };

        if update.version_number != current_version {
            let file = update.files.iter().find(|f| f.primary).or_else(|| update.files.first());
            if let Some(file) = file {
                updates.push(ModUpdate {
                    filename,
                    current_version,
                    latest_version: update.version_number,
                    download_url: file.url.clone(),
                    new_filename: file.filename.clone(),
                });
            }
        }
    }

    Ok(updates)
}

/// Download the updated jar, remove the old one.
pub async fn apply_update(
    client: &reqwest::Client,
    mods_dir: &Path,
    update: &ModUpdate,
) -> Result<()> {
    let bytes = client.get(&update.download_url).send().await?.bytes().await?;
    tokio::fs::write(mods_dir.join(&update.new_filename), &bytes).await?;
    tokio::fs::remove_file(mods_dir.join(&update.filename)).await?;
    Ok(())
}
