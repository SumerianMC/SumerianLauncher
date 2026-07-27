use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const MANIFEST_URL: &str =
    "https://piston-meta.mojang.com/mc/game/version_manifest.json";

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VersionManifest {
    pub latest: LatestVersions,
    pub versions: Vec<VersionEntry>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LatestVersions {
    pub release: String,
    pub snapshot: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VersionEntry {
    pub id: String,
    #[serde(rename = "type")]
    pub version_type: String,
    pub url: String,
    pub time: String,
    #[serde(rename = "releaseTime")]
    pub release_time: String,
    pub sha1: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VersionMeta {
    pub id: String,
    #[serde(rename = "type")]
    pub version_type: String,
    #[serde(rename = "mainClass")]
    pub main_class: String,
    #[serde(rename = "minecraftArguments")]
    pub minecraft_arguments: Option<String>,
    pub arguments: Option<Arguments>,
    pub downloads: Option<Downloads>,
    pub libraries: Vec<Library>,
    #[serde(rename = "assetIndex")]
    pub asset_index: Option<AssetIndex>,
    pub assets: Option<String>,
    #[serde(rename = "javaVersion")]
    pub java_version: Option<JavaVersion>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Arguments {
    pub game: Option<Vec<serde_json::Value>>,
    pub jvm: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Downloads {
    pub client: Option<Artifact>,
    pub server: Option<Artifact>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Artifact {
    pub url: String,
    pub sha1: String,
    pub size: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Library {
    pub name: String,
    pub downloads: Option<LibraryDownloads>,
    pub rules: Option<Vec<Rule>>,
    pub natives: Option<HashMap<String, String>>,
    pub extract: Option<Extract>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LibraryDownloads {
    pub artifact: Option<Artifact>,
    pub classifiers: Option<HashMap<String, Artifact>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Rule {
    pub action: String,
    pub os: Option<OsRule>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OsRule {
    pub name: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Extract {
    pub exclude: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AssetIndex {
    pub id: String,
    pub url: String,
    pub sha1: String,
    pub size: u64,
    #[serde(rename = "totalSize")]
    pub total_size: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct JavaVersion {
    pub component: String,
    #[serde(rename = "majorVersion")]
    pub major_version: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AssetObjects {
    pub objects: HashMap<String, AssetObject>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AssetObject {
    pub hash: String,
    pub size: u64,
}

impl VersionManifest {
    pub async fn fetch(client: &reqwest::Client) -> Result<Self> {
        let manifest = client
            .get(MANIFEST_URL)
            .send()
            .await
            .context("Failed to fetch version manifest")?
            .json::<VersionManifest>()
            .await
            .context("Failed to parse version manifest")?;
        Ok(manifest)
    }

    pub fn filter_by_type(&self, version_type: &str) -> Vec<&VersionEntry> {
        self.versions
            .iter()
            .filter(|v| v.version_type == version_type)
            .collect()
    }

    pub fn get_version(&self, id: &str) -> Option<&VersionEntry> {
        self.versions.iter().find(|v| v.id == id)
    }

    pub fn type_counts(&self) -> HashMap<String, usize> {
        let mut counts = HashMap::new();
        for v in &self.versions {
            *counts.entry(v.version_type.clone()).or_insert(0) += 1;
        }
        counts
    }
}

impl VersionEntry {
    pub async fn fetch_meta(&self, client: &reqwest::Client) -> Result<VersionMeta> {
        let meta = client
            .get(&self.url)
            .send()
            .await
            .with_context(|| format!("Failed to fetch metadata for {}", self.id))?
            .json::<VersionMeta>()
            .await
            .with_context(|| format!("Failed to parse metadata for {}", self.id))?;
        Ok(meta)
    }
}

impl Library {
    pub fn is_allowed_on_current_os(&self) -> bool {
        let Some(rules) = &self.rules else {
            return true;
        };
        let os_name = current_os_name();
        let mut allowed = false;
        for rule in rules {
            let matches = rule
                .os
                .as_ref()
                .map(|o| o.name.as_deref() == Some(os_name))
                .unwrap_or(true);
            if matches {
                allowed = rule.action == "allow";
            }
        }
        allowed
    }

    pub fn native_classifier(&self) -> Option<String> {
        let natives = self.natives.as_ref()?;
        let os = current_os_name();
        let arch_suffix = if cfg!(target_arch = "x86_64") {
            "64"
        } else {
            "32"
        };
        let key = natives.get(os)?;
        Some(key.replace("${arch}", arch_suffix))
    }
}

pub fn current_os_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "osx"
    } else {
        "linux"
    }
}
