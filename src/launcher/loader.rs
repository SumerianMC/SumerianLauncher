use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};

// ── Fabric ────────────────────────────────────────────────────────────────────

const FABRIC_META: &str = "https://meta.fabricmc.net/v2";

#[derive(Deserialize)]
struct FabricLoader {
    version: String,
}

#[derive(Deserialize)]
struct FabricProfileResponse {
    id: String,
    #[serde(rename = "mainClass")]
    main_class: String,
    libraries: Vec<FabricLibrary>,
    arguments: Option<serde_json::Value>,
    #[serde(rename = "minecraftArguments")]
    minecraft_arguments: Option<String>,
}

#[derive(Deserialize)]
struct FabricLibrary {
    name: String,
    url: String,
}

/// Returns a list of available Fabric loader versions for `mc_version`.
pub async fn fabric_loader_versions(
    http: &reqwest::Client,
    mc_version: &str,
) -> Result<Vec<String>> {
    let url = format!("{}/versions/loader/{}", FABRIC_META, mc_version);
    let loaders: Vec<FabricLoader> = http
        .get(&url)
        .send()
        .await?
        .error_for_status()
        .context("Fabric meta API error — is this version supported by Fabric?")?
        .json()
        .await?;
    Ok(loaders.into_iter().map(|l| l.version).collect())
}

/// Downloads the Fabric profile JSON and writes a merged version JSON to
/// `versions/<mc_version>-fabric-<loader>/`.  Returns the new version id.
pub async fn install_fabric(
    http: &reqwest::Client,
    game_dir: &Path,
    mc_version: &str,
    loader_version: &str,
) -> Result<String> {
    let profile_url = format!(
        "{}/versions/loader/{}/{}/profile/json",
        FABRIC_META, mc_version, loader_version
    );

    let profile: FabricProfileResponse = http
        .get(&profile_url)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let version_id = profile.id.clone();
    let version_dir = game_dir.join("versions").join(&version_id);
    tokio::fs::create_dir_all(&version_dir).await?;

    // Download each Fabric library into the libraries dir
    for lib in &profile.libraries {
        let dest = maven_path(game_dir, &lib.name);
        if dest.exists() {
            continue;
        }
        tokio::fs::create_dir_all(dest.parent().unwrap()).await?;
        let url = maven_url(&lib.url, &lib.name);
        let bytes = http.get(&url).send().await?.error_for_status()?.bytes().await?;
        tokio::fs::write(&dest, &bytes).await?;
    }

    // Build a minimal version JSON that inherits from the base MC version
    let mut json = serde_json::json!({
        "id": version_id,
        "type": "release",
        "mainClass": profile.main_class,
        "inheritsFrom": mc_version,
        "libraries": profile.libraries.iter().map(|l| serde_json::json!({
            "name": l.name,
            "url": l.url,
        })).collect::<Vec<_>>(),
    });

    if let Some(args) = profile.arguments {
        json["arguments"] = args;
    } else if let Some(legacy) = profile.minecraft_arguments {
        json["minecraftArguments"] = serde_json::Value::String(legacy);
    }

    let json_path = version_dir.join(format!("{}.json", version_id));
    tokio::fs::write(&json_path, serde_json::to_string_pretty(&json)?).await?;

    // Symlink / copy the base MC jar so the classpath builder finds it
    let base_jar = game_dir
        .join("versions")
        .join(mc_version)
        .join(format!("{}.jar", mc_version));
    let fabric_jar = version_dir.join(format!("{}.jar", version_id));
    if base_jar.exists() && !fabric_jar.exists() {
        tokio::fs::copy(&base_jar, &fabric_jar).await?;
    }

    Ok(version_id)
}

// ── Forge ─────────────────────────────────────────────────────────────────────

const FORGE_MAVEN: &str = "https://maven.minecraftforge.net/net/minecraftforge/forge";

#[allow(dead_code)]
#[derive(Deserialize)]
struct ForgeMavenMeta {
    versioning: ForgeVersioning,
}

#[allow(dead_code)]
#[derive(Deserialize)]
struct ForgeVersioning {
    versions: ForgeVersionList,
}

#[allow(dead_code)]
#[derive(Deserialize)]
struct ForgeVersionList {
    version: Vec<String>,
}

/// Returns available Forge versions for `mc_version` (newest first).
pub async fn forge_versions(
    http: &reqwest::Client,
    mc_version: &str,
) -> Result<Vec<String>> {
    let url = format!("{}/maven-metadata.xml", FORGE_MAVEN);
    let xml = http.get(&url).send().await?.error_for_status()?.text().await?;

    // Simple prefix filter — no full XML parser needed
    let prefix = format!("{}-", mc_version);
    let mut versions: Vec<String> = xml
        .lines()
        .filter_map(|l| {
            let t = l.trim();
            if t.starts_with("<version>") && t.ends_with("</version>") {
                let v = t.trim_start_matches("<version>").trim_end_matches("</version>");
                if v.starts_with(&prefix) {
                    return Some(v.to_string());
                }
            }
            None
        })
        .collect();
    versions.reverse(); // newest first
    Ok(versions)
}

/// Downloads the Forge installer jar and runs it with `--installClient`.
/// Returns the installed Forge version id (e.g. `1.21-forge-47.3.0`).
pub async fn install_forge(
    http: &reqwest::Client,
    game_dir: &Path,
    forge_version: &str, // e.g. "1.21-47.3.0"
) -> Result<String> {
    let installer_url = format!(
        "{}/{forge_version}/forge-{forge_version}-installer.jar",
        FORGE_MAVEN,
        forge_version = forge_version
    );

    let tmp_dir = std::env::temp_dir();
    let installer_path = tmp_dir.join(format!("forge-{}-installer.jar", forge_version));

    println!("  Downloading Forge installer...");
    let bytes = http
        .get(&installer_url)
        .send()
        .await?
        .error_for_status()
        .context("Forge installer not found — check the version string")?
        .bytes()
        .await?;
    tokio::fs::write(&installer_path, &bytes).await?;

    // Run the installer headlessly
    let java = find_java();
    let status = std::process::Command::new(&java)
        .arg("-jar")
        .arg(&installer_path)
        .arg("--installClient")
        .arg(game_dir)
        .status()
        .context("Failed to run Forge installer — is Java on PATH?")?;

    let _ = std::fs::remove_file(&installer_path);

    if !status.success() {
        bail!("Forge installer exited with status {}", status);
    }

    // Forge names the version like "1.21-forge-47.3.0"
    let parts: Vec<&str> = forge_version.splitn(2, '-').collect();
    let version_id = if parts.len() == 2 {
        format!("{}-forge-{}", parts[0], parts[1])
    } else {
        format!("forge-{}", forge_version)
    };

    Ok(version_id)
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn find_java() -> PathBuf {
    // Delegate to the full discovery chain in injection.rs
    crate::client::injection::find_java_for_major(21)
        .or_else(|| crate::client::injection::find_java_for_major(17))
        .or_else(|| crate::client::injection::find_java_for_major(8))
        .unwrap_or_else(|| PathBuf::from("java"))
}

fn maven_path(game_dir: &Path, name: &str) -> PathBuf {
    let parts: Vec<&str> = name.splitn(3, ':').collect();
    if parts.len() < 3 {
        return game_dir.join("libraries").join(name);
    }
    let group = parts[0].replace('.', "/");
    let artifact = parts[1];
    let version = parts[2];
    game_dir
        .join("libraries")
        .join(group)
        .join(artifact)
        .join(version)
        .join(format!("{}-{}.jar", artifact, version))
}

fn maven_url(base_url: &str, name: &str) -> String {
    let parts: Vec<&str> = name.splitn(3, ':').collect();
    if parts.len() < 3 {
        return format!("{}{}", base_url, name);
    }
    let group = parts[0].replace('.', "/");
    let artifact = parts[1];
    let version = parts[2];
    let base = base_url.trim_end_matches('/');
    format!("{}/{}/{}/{}/{}-{}.jar", base, group, artifact, version, artifact, version)
}
