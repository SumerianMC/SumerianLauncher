/// Custom splash screen editor.
///
/// Manages the `assets/minecraft/texts/splashes.txt` file inside a resource
/// pack, letting the user view, add, remove, and import splash text lines.
use anyhow::{bail, Result};
use std::path::Path;
use tokio::fs;

/// Path to the splashes file inside a resource pack directory.
pub fn splashes_path(pack_dir: &Path) -> std::path::PathBuf {
    pack_dir
        .join("assets")
        .join("minecraft")
        .join("texts")
        .join("splashes.txt")
}

/// Read all splash lines from the pack's splashes.txt.
/// Returns an empty vec if the file does not exist.
pub async fn load(pack_dir: &Path) -> Result<Vec<String>> {
    let path = splashes_path(pack_dir);
    match fs::read_to_string(&path).await {
        Ok(raw) => Ok(raw
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect()),
        Err(_) => Ok(Vec::new()),
    }
}

/// Write the splash list back to disk, creating parent directories as needed.
pub async fn save(pack_dir: &Path, lines: &[String]) -> Result<()> {
    let path = splashes_path(pack_dir);
    if let Some(p) = path.parent() { fs::create_dir_all(p).await?; }
    fs::write(&path, lines.join("\n") + "\n").await?;
    Ok(())
}

/// Add a splash line (reject duplicates and lines > 256 chars).
pub async fn add(pack_dir: &Path, line: &str) -> Result<()> {
    let line = line.trim();
    if line.is_empty() { bail!("Splash text cannot be empty."); }
    if line.len() > 256 { bail!("Splash text must be 256 characters or fewer."); }
    let mut lines = load(pack_dir).await?;
    if lines.iter().any(|l| l.eq_ignore_ascii_case(line)) {
        bail!("That splash already exists.");
    }
    lines.push(line.to_string());
    save(pack_dir, &lines).await
}

/// Remove a splash line by index.
pub async fn remove(pack_dir: &Path, index: usize) -> Result<()> {
    let mut lines = load(pack_dir).await?;
    if index >= lines.len() { bail!("Index out of range."); }
    lines.remove(index);
    save(pack_dir, &lines).await
}

/// Import splashes from a plain-text file (one per line), merging with existing.
pub async fn import_file(pack_dir: &Path, src_path: &Path) -> Result<usize> {
    let raw = fs::read_to_string(src_path).await?;
    let mut existing = load(pack_dir).await?;
    let before = existing.len();
    for line in raw.lines().map(str::trim).filter(|l| !l.is_empty()) {
        if line.len() <= 256 && !existing.iter().any(|e| e.eq_ignore_ascii_case(line)) {
            existing.push(line.to_string());
        }
    }
    let added = existing.len() - before;
    save(pack_dir, &existing).await?;
    Ok(added)
}
