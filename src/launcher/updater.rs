use anyhow::Result;
use serde::Deserialize;
use std::path::PathBuf;

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const RELEASES_API: &str =
    "https://api.github.com/repos/SumerianMC/SumerianLauncher/releases/latest";

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
}

/// Returns `Some((tag, download_url))` if a newer release exists, else `None`.
pub async fn check_for_update(http: &reqwest::Client) -> Result<Option<(String, String)>> {
    let release: Release = http
        .get(RELEASES_API)
        .header("User-Agent", "SumerianClient")
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let latest = release.tag_name.trim_start_matches('v');
    if latest == CURRENT_VERSION {
        return Ok(None);
    }

    let asset = release
        .assets
        .into_iter()
        .find(|a| a.name.ends_with(".exe") || a.name == "sumerian")
        .ok_or_else(|| anyhow::anyhow!("No binary asset found in release {}", release.tag_name))?;

    Ok(Some((release.tag_name, asset.browser_download_url)))
}

/// Downloads `url` and atomically replaces the running executable.
pub async fn apply_update(http: &reqwest::Client, url: &str) -> Result<PathBuf> {
    let current_exe = std::env::current_exe()?;
    let tmp = current_exe.with_extension("tmp");

    let bytes = http
        .get(url)
        .header("User-Agent", "SumerianClient")
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;

    std::fs::write(&tmp, &bytes)?;

    // On Windows we can't overwrite a running exe directly — rename current
    // to .old, move .tmp into place, then delete .old.
    let old = current_exe.with_extension("old");
    let _ = std::fs::remove_file(&old);
    std::fs::rename(&current_exe, &old)?;
    std::fs::rename(&tmp, &current_exe)?;
    let _ = std::fs::remove_file(&old);

    Ok(current_exe)
}

pub fn current_version() -> &'static str {
    CURRENT_VERSION
}
