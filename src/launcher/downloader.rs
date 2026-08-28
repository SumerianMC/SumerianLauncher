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

/// Max concurrent asset downloads.
const ASSET_CONCURRENCY: usize = 64;
/// Max concurrent library downloads.
const LIB_CONCURRENCY: usize = 16;

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
        // Collect all (artifact, dest_path, is_native) tuples first
        struct LibTask {
            artifact: crate::launcher::manifest::Artifact,
            path: PathBuf,
            is_native: bool,
        }
        let mut tasks: Vec<LibTask> = Vec::new();
        for lib in &meta.libraries {
            if !lib.is_allowed_on_current_os() { continue; }
            let Some(downloads) = &lib.downloads else { continue; };
            if let Some(artifact) = &downloads.artifact {
                tasks.push(LibTask {
                    artifact: artifact.clone(),
                    path: self.library_path_from_name(&lib.name),
                    is_native: false,
                });
            }
            if let Some(classifier_key) = lib.native_classifier() {
                if let Some(classifiers) = &downloads.classifiers {
                    if let Some(native_artifact) = classifiers.get(&classifier_key) {
                        tasks.push(LibTask {
                            artifact: native_artifact.clone(),
                            path: self.library_path_from_name(&format!("{}-{}", lib.name, classifier_key)),
                            is_native: true,
                        });
                    }
                }
            }
        }

        let pb = self.multi.add(ProgressBar::new(tasks.len() as u64));
        pb.set_style(progress_style());
        pb.set_message("Downloading libraries");

        let sem = Arc::new(Semaphore::new(LIB_CONCURRENCY));
        let client = self.client.clone();
        let multi = self.multi.clone();
        let version_id = meta.id.clone();
        let game_dir = self.game_dir.clone();

        let mut handles = Vec::new();
        for task in tasks {
            let sem = sem.clone();
            let client = client.clone();
            let multi = multi.clone();
            let version_id = version_id.clone();
            let game_dir = game_dir.clone();
            handles.push(tokio::spawn(async move {
                let _permit = sem.acquire().await.unwrap();
                // Inline download to avoid borrowing self
                download_artifact_standalone(&client, &multi, &task.artifact, &task.path, "").await?;
                if task.is_native {
                    extract_natives_standalone(&task.path, &game_dir, &version_id).await?;
                }
                Ok::<PathBuf, anyhow::Error>(task.path)
            }));
        }

        let mut paths = Vec::new();
        for h in handles {
            let path = h.await??;
            paths.push(path);
            pb.inc(1);
        }
        pb.finish_with_message("Libraries downloaded");
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

        // Filter to only assets that are actually missing or corrupt
        let base = self.game_dir.join("assets").join("objects");
        let mut needed: Vec<(String, AssetObject)> = Vec::new();
        for (name, obj) in &objects.objects {
            let dest = base.join(&obj.hash[..2]).join(&obj.hash);
            // Fast path: if size matches, skip SHA1 check
            let already_ok = tokio::fs::metadata(&dest).await
                .map(|m| m.len() == obj.size)
                .unwrap_or(false);
            if !already_ok {
                needed.push((name.clone(), obj.clone()));
            }
        }

        if needed.is_empty() {
            return Ok(());
        }

        let pb = self.multi.add(ProgressBar::new(needed.len() as u64));
        pb.set_style(progress_style());
        pb.set_message(format!("Downloading {} assets", needed.len()));

        let sem = Arc::new(Semaphore::new(ASSET_CONCURRENCY));
        let mut tasks = Vec::new();
        for (name, obj) in needed {
            let client = self.client.clone();
            let base = base.clone();
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
        pb.finish_with_message("Assets up to date");
        Ok(())
    }

    pub async fn extract_natives(&self, zip_path: &Path, meta: &VersionMeta) -> Result<()> {
        extract_natives_standalone(zip_path, &self.game_dir, &meta.id).await
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

    // Stream directly to disk while hashing — no full-file buffer
    let mut file = fs::File::create(&dest).await?;
    let mut stream = resp.bytes_stream();
    let mut hasher = Sha1::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        hasher.update(&chunk);
        file.write_all(&chunk).await?;
    }
    file.flush().await?;
    drop(file);

    let computed = hex::encode(hasher.finalize());
    if computed != obj.hash {
        fs::remove_file(&dest).await.ok();
        bail!("Asset SHA1 mismatch: expected {}, got {}", obj.hash, computed);
    }
    Ok(())
}

/// Standalone artifact downloader (no &self borrow needed for parallel tasks).
async fn download_artifact_standalone(
    client: &Client,
    multi: &MultiProgress,
    artifact: &crate::launcher::manifest::Artifact,
    dest: &Path,
    label: &str,
) -> Result<()> {
    if dest.exists() && verify_sha1(dest, &artifact.sha1).await? {
        return Ok(());
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).await?;
    }
    let pb = multi.add(ProgressBar::new(artifact.size));
    pb.set_style(progress_style());
    if !label.is_empty() { pb.set_message(format!("Downloading {}", label)); }

    let resp = client.get(&artifact.url).send().await
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
    pb.finish_and_clear();

    let computed = hex::encode(hasher.finalize());
    if computed != artifact.sha1 {
        fs::remove_file(dest).await.ok();
        bail!("SHA1 mismatch for {}: expected {}, got {}", artifact.url, artifact.sha1, computed);
    }
    Ok(())
}

async fn extract_natives_standalone(zip_path: &Path, game_dir: &Path, version_id: &str) -> Result<()> {
    let natives_dir = game_dir.join("versions").join(version_id).join("natives");
    fs::create_dir_all(&natives_dir).await?;
    let zip_data = fs::read(zip_path).await?;
    let cursor = std::io::Cursor::new(zip_data);
    let mut archive = zip::ZipArchive::new(cursor)?;
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let name = file.name().to_string();
        if name.ends_with('/') || name.starts_with("META-INF") { continue; }
        let out_path = natives_dir.join(&name);
        if let Some(parent) = out_path.parent() { std::fs::create_dir_all(parent)?; }
        let mut out = std::fs::File::create(&out_path)?;
        std::io::copy(&mut file, &mut out)?;
    }
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
