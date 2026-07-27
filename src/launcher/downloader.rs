use anyhow::{bail, Context, Result};
use futures_util::StreamExt;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use reqwest::Client;
use sha1::{Digest, Sha1};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio::sync::Semaphore;

use crate::launcher::manifest::{
    AssetIndex, AssetObject, AssetObjects, Artifact, VersionMeta,
};

pub struct Downloader {
    client: Client,
    pub game_dir: PathBuf,
    multi: MultiProgress,
}

impl Downloader {
    pub fn new(client: Client, game_dir: PathBuf) -> Self {
        Self {
            client,
            game_dir,
            multi: MultiProgress::new(),
        }
    }

    pub async fn download_version(&self, meta: &VersionMeta) -> Result<PathBuf> {
        let downloads = meta
            .downloads
            .as_ref()
            .context("Version has no downloads section")?;
        let artifact = downloads
            .client
            .as_ref()
            .context("Version has no client download")?;

        let jar_path = self
            .game_dir
            .join("versions")
            .join(&meta.id)
            .join(format!("{}.jar", meta.id));

        self.download_artifact(artifact, &jar_path, &format!("{}.jar", meta.id))
            .await?;
        Ok(jar_path)
    }

    pub async fn download_libraries(&self, meta: &VersionMeta) -> Result<Vec<PathBuf>> {
        let mut paths = Vec::new();
        for lib in &meta.libraries {
            if !lib.is_allowed_on_current_os() {
                continue;
            }
            let Some(downloads) = &lib.downloads else {
                continue;
            };

            // Main artifact
            if let Some(artifact) = &downloads.artifact {
                let path = self.library_path_from_name(&lib.name);
                self.download_artifact(artifact, &path, &lib.name).await?;
                paths.push(path);
            }

            // Natives
            if let Some(classifier_key) = lib.native_classifier() {
                if let Some(classifiers) = &downloads.classifiers {
                    if let Some(native_artifact) = classifiers.get(&classifier_key) {
                        let path = self.library_path_from_name(&format!(
                            "{}-{}",
                            lib.name, classifier_key
                        ));
                        self.download_artifact(native_artifact, &path, &lib.name)
                            .await?;
                        self.extract_natives(&path, meta).await?;
                        paths.push(path);
                    }
                }
            }
        }
        Ok(paths)
    }

    pub async fn download_assets(&self, asset_index: &AssetIndex) -> Result<()> {
        let index_path = self
            .game_dir
            .join("assets")
            .join("indexes")
            .join(format!("{}.json", asset_index.id));

        self.download_artifact(
            &crate::launcher::manifest::Artifact {
                url: asset_index.url.clone(),
                sha1: asset_index.sha1.clone(),
                size: asset_index.size,
            },
            &index_path,
            &format!("asset index {}", asset_index.id),
        )
        .await?;

        let content = fs::read_to_string(&index_path).await?;
        let objects: AssetObjects = serde_json::from_str(&content)?;

        let pb = self.multi.add(ProgressBar::new(objects.objects.len() as u64));
        pb.set_style(progress_style());
        pb.set_message("Downloading assets");

        let sem = Arc::new(Semaphore::new(32));
        let mut tasks = Vec::new();
        for (name, obj) in &objects.objects {
            let obj = obj.clone();
            let name = name.clone();
            let client = self.client.clone();
            let base = self.game_dir.join("assets").join("objects");
            let sem = sem.clone();
            tasks.push(tokio::spawn(async move {
                let _permit = sem.acquire().await.unwrap();
                download_asset_object(&client, &obj, &base, &name).await
            }));
        }

        for task in tasks {
            task.await??;
            pb.inc(1);
        }
        pb.finish_with_message("Assets downloaded");
        Ok(())
    }

    pub async fn extract_natives(&self, zip_path: &Path, meta: &VersionMeta) -> Result<()> {
        let natives_dir = self
            .game_dir
            .join("versions")
            .join(&meta.id)
            .join("natives");
        fs::create_dir_all(&natives_dir).await?;

        let zip_data = fs::read(zip_path).await?;
        let cursor = std::io::Cursor::new(zip_data);
        let mut archive = zip::ZipArchive::new(cursor)?;

        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let name = file.name().to_string();
            if name.ends_with('/') || name.starts_with("META-INF") {
                continue;
            }
            let out_path = natives_dir.join(&name);
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut out = std::fs::File::create(&out_path)?;
            std::io::copy(&mut file, &mut out)?;
        }
        Ok(())
    }

    pub async fn download_artifact(
        &self,
        artifact: &Artifact,
        dest: &Path,
        label: &str,
    ) -> Result<()> {
        if dest.exists() && verify_sha1(dest, &artifact.sha1).await? {
            return Ok(());
        }

        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).await?;
        }

        let pb = self.multi.add(ProgressBar::new(artifact.size));
        pb.set_style(progress_style());
        pb.set_message(format!("Downloading {}", label));

        let resp = self
            .client
            .get(&artifact.url)
            .send()
            .await
            .with_context(|| format!("GET {} failed", artifact.url))?;

        if !resp.status().is_success() {
            bail!("HTTP {} for {}", resp.status(), artifact.url);
        }

        let mut file = fs::File::create(dest).await?;
        let mut stream = resp.bytes_stream();
        let mut hasher = Sha1::new();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            hasher.update(&chunk);
            file.write_all(&chunk).await?;
            pb.inc(chunk.len() as u64);
        }
        file.flush().await?;
        drop(file);

        let computed = hex::encode(hasher.finalize());
        if computed != artifact.sha1 {
            fs::remove_file(dest).await.ok();
            bail!(
                "SHA1 mismatch for {}: expected {}, got {}",
                label,
                artifact.sha1,
                computed
            );
        }

        pb.finish_with_message(format!("{} done", label));
        Ok(())
    }

    fn library_path_from_name(&self, name: &str) -> PathBuf {
        // name format: group:artifact:version or group:artifact:version:classifier
        let parts: Vec<&str> = name.splitn(4, ':').collect();
        if parts.len() < 3 {
            return self.game_dir.join("libraries").join(name);
        }
        let group = parts[0].replace('.', "/");
        let artifact = parts[1];
        let version = parts[2];
        let classifier = parts.get(3).copied().unwrap_or("");
        let filename = if classifier.is_empty() {
            format!("{}-{}.jar", artifact, version)
        } else {
            format!("{}-{}-{}.jar", artifact, version, classifier)
        };
        self.game_dir
            .join("libraries")
            .join(group)
            .join(artifact)
            .join(version)
            .join(filename)
    }

    pub fn natives_dir(&self, version_id: &str) -> PathBuf {
        self.game_dir
            .join("versions")
            .join(version_id)
            .join("natives")
    }
}

async fn download_asset_object(
    client: &Client,
    obj: &AssetObject,
    base: &Path,
    _name: &str,
) -> Result<()> {
    let prefix = &obj.hash[..2];
    let dest = base.join(prefix).join(&obj.hash);

    // Verify existing file — re-download if missing or corrupt
    if dest.exists() && verify_sha1(&dest, &obj.hash).await.unwrap_or(false) {
        return Ok(());
    }

    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).await?;
    }

    let url = format!(
        "https://resources.download.minecraft.net/{}/{}",
        prefix, obj.hash
    );

    let resp = client.get(&url).send().await?;
    if !resp.status().is_success() {
        bail!("Asset download failed: HTTP {} for {}", resp.status(), obj.hash);
    }

    let bytes = resp.bytes().await?;
    let mut hasher = Sha1::new();
    hasher.update(&bytes);
    let computed = hex::encode(hasher.finalize());
    if computed != obj.hash {
        bail!("Asset SHA1 mismatch: expected {}, got {}", obj.hash, computed);
    }

    fs::write(&dest, &bytes).await?;
    Ok(())
}

async fn verify_sha1(path: &Path, expected: &str) -> Result<bool> {
    let data = fs::read(path).await?;
    let mut hasher = Sha1::new();
    hasher.update(&data);
    Ok(hex::encode(hasher.finalize()) == expected)
}

fn progress_style() -> ProgressStyle {
    ProgressStyle::with_template(
        "{msg:.cyan} [{bar:40.green/white}] {bytes}/{total_bytes} ({eta})",
    )
    .unwrap()
    .progress_chars("=>-")
}
