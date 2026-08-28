use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs;

use crate::launcher::manifest::VersionMeta;
use crate::optimizer::OptimizationProfile;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledVersion {
    pub id: String,
    pub version_type: String,
    pub jar_path: PathBuf,
    pub meta_path: PathBuf,
}

pub struct VersionManager {
    pub versions_dir: PathBuf,
}

impl VersionManager {
    pub fn new(game_dir: &Path) -> Self {
        Self {
            versions_dir: game_dir.join("versions"),
        }
    }

    pub async fn list_installed(&self) -> Result<Vec<InstalledVersion>> {
        let mut dir = match fs::read_dir(&self.versions_dir).await {
            Ok(d) => d,
            Err(_) => return Ok(Vec::new()),
        };

        // Collect all candidate dirs first, then read their JSON in parallel
        let mut candidates: Vec<(String, PathBuf, PathBuf)> = Vec::new();
        while let Some(entry) = dir.next_entry().await? {
            let path = entry.path();
            if !path.is_dir() { continue; }
            let id = path.file_name().unwrap().to_string_lossy().to_string();
            let jar = path.join(format!("{}.jar", id));
            let meta = path.join(format!("{}.json", id));
            if jar.exists() && meta.exists() {
                candidates.push((id, jar, meta));
            }
        }

        let handles: Vec<_> = candidates.into_iter().map(|(id, jar, meta_path)| {
            tokio::spawn(async move {
                let raw = fs::read_to_string(&meta_path).await?;
                let version_meta: VersionMeta = serde_json::from_str(&raw)?;
                Ok::<InstalledVersion, anyhow::Error>(InstalledVersion {
                    id,
                    version_type: version_meta.version_type,
                    jar_path: jar,
                    meta_path,
                })
            })
        }).collect();

        let mut installed = Vec::with_capacity(handles.len());
        for h in handles {
            installed.push(h.await??);
        }
        Ok(installed)
    }

    pub async fn save_meta(&self, meta: &VersionMeta) -> Result<()> {
        let dir = self.versions_dir.join(&meta.id);
        fs::create_dir_all(&dir).await?;
        let path = dir.join(format!("{}.json", meta.id));
        let json = serde_json::to_string_pretty(meta)?;
        fs::write(path, json).await?;
        Ok(())
    }

    pub async fn load_meta(&self, version_id: &str) -> Result<VersionMeta> {
        let path = self
            .versions_dir
            .join(version_id)
            .join(format!("{}.json", version_id));
        let raw = fs::read_to_string(&path)
            .await
            .with_context(|| format!("Version {} not installed", version_id))?;
        Ok(serde_json::from_str(&raw)?)
    }

    pub fn build_classpath(
        &self,
        meta: &VersionMeta,
        game_dir: &Path,
        jar_path: &Path,
    ) -> String {
        let sep = if cfg!(target_os = "windows") { ";" } else { ":" };
        let mut entries: Vec<String> = Vec::new();

        for lib in &meta.libraries {
            if !lib.is_allowed_on_current_os() {
                continue;
            }
            let Some(downloads) = &lib.downloads else {
                continue;
            };
            if let Some(artifact) = &downloads.artifact {
                let path = maven_path(game_dir, &lib.name);
                if path.exists() {
                    entries.push(path.to_string_lossy().to_string());
                } else {
                    // Try to derive path from artifact URL
                    let _ = artifact;
                }
            }
        }

        entries.push(jar_path.to_string_lossy().to_string());
        entries.join(sep)
    }

    pub fn build_jvm_args(
        &self,
        meta: &VersionMeta,
        game_dir: &Path,
        natives_dir: &Path,
        profile: &OptimizationProfile,
        required_java: u32,
    ) -> Vec<String> {
        let mut args = profile.jvm_flags();

        args.push(format!(
            "-Djava.library.path={}",
            to_forward_slashes(natives_dir)
        ));
        args.push(format!(
            "-Dminecraft.launcher.brand=SumerianClient"
        ));
        args.push(format!("-Dminecraft.launcher.version=0.1.0"));

        // Modern versions use arguments.jvm
        if let Some(arguments) = &meta.arguments {
            if let Some(jvm_args) = &arguments.jvm {
                for arg in jvm_args {
                    if let Some(s) = arg.as_str() {
                        if s.starts_with("--sun-misc-unsafe-memory-access") && required_java < 23 {
                            continue;
                        }
                        let resolved = resolve_arg(s, meta, game_dir, natives_dir);
                        args.push(resolved);
                    }
                }
            }
        }

        args
    }

    pub fn build_game_args(
        &self,
        meta: &VersionMeta,
        game_dir: &Path,
        username: &str,
        access_token: &str,
        uuid: &str,
        user_type: &str,
    ) -> Vec<String> {
        let assets_dir = game_dir.join("assets");
        let asset_index = meta
            .asset_index
            .as_ref()
            .map(|a| a.id.as_str())
            .unwrap_or("legacy");

        // Legacy versions use minecraftArguments string
        if let Some(legacy_args) = &meta.minecraft_arguments {
            return legacy_args
                .split_whitespace()
                .map(|s| {
                    s.replace("${auth_player_name}", username)
                        .replace("${version_name}", &meta.id)
                        .replace("${game_directory}", &to_forward_slashes(game_dir))
                        .replace("${assets_root}", &to_forward_slashes(&assets_dir))
                        .replace("${assets_index_name}", asset_index)
                        .replace("${auth_uuid}", uuid)
                        .replace("${auth_access_token}", access_token)
                        .replace("${user_type}", user_type)
                        .replace("${version_type}", &meta.version_type)
                        .replace("${user_properties}", "{}")
                })
                .collect();
        }

        // Modern versions use arguments.game
        let mut args = Vec::new();
        if let Some(arguments) = &meta.arguments {
            if let Some(game_args) = &arguments.game {
                for arg in game_args {
                    if let Some(s) = arg.as_str() {
                        let resolved = s
                            .replace("${auth_player_name}", username)
                            .replace("${version_name}", &meta.id)
                            .replace("${game_directory}", &to_forward_slashes(game_dir))
                            .replace("${assets_root}", &to_forward_slashes(&assets_dir))
                            .replace("${assets_index_name}", asset_index)
                            .replace("${auth_uuid}", uuid)
                            .replace("${auth_access_token}", access_token)
                            .replace("${user_type}", user_type)
                            .replace("${version_type}", &meta.version_type);
                        args.push(resolved);
                    }
                }
            }
        }
        args
    }
}

fn resolve_arg(s: &str, meta: &VersionMeta, game_dir: &Path, natives_dir: &Path) -> String {
    let assets_dir = game_dir.join("assets");
    s.replace(
        "${natives_directory}",
        &to_forward_slashes(natives_dir),
    )
    .replace("${launcher_name}", "SumerianClient")
    .replace("${launcher_version}", "0.1.0")
    .replace("${classpath}", "")
    .replace("${assets_root}", &to_forward_slashes(&assets_dir))
    .replace("${game_directory}", &to_forward_slashes(game_dir))
    .replace("${version_name}", &meta.id)
}

fn to_forward_slashes(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

pub fn maven_path(game_dir: &Path, name: &str) -> PathBuf {
    let parts: Vec<&str> = name.splitn(4, ':').collect();
    if parts.len() < 3 {
        return game_dir.join("libraries").join(name);
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
    game_dir
        .join("libraries")
        .join(group)
        .join(artifact)
        .join(version)
        .join(filename)
}
