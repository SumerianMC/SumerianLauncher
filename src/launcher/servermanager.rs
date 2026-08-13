/// Server whitelist & op manager.
///
/// Reads and writes `whitelist.json` and `ops.json` inside an instance or game
/// directory.  These files follow the vanilla server JSON format.
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhitelistEntry {
    pub uuid: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpEntry {
    pub uuid: String,
    pub name: String,
    pub level: u8,
    #[serde(rename = "bypassesPlayerLimit", default)]
    pub bypasses_player_limit: bool,
}

// ── Whitelist ────────────────────────────────────────────────────────────────

pub async fn load_whitelist(dir: &Path) -> Result<Vec<WhitelistEntry>> {
    let path = dir.join("whitelist.json");
    match fs::read_to_string(&path).await {
        Ok(raw) => Ok(serde_json::from_str(&raw).unwrap_or_default()),
        Err(_)  => Ok(Vec::new()),
    }
}

pub async fn save_whitelist(dir: &Path, entries: &[WhitelistEntry]) -> Result<()> {
    fs::write(dir.join("whitelist.json"), serde_json::to_string_pretty(entries)?).await?;
    Ok(())
}

pub async fn add_whitelist(dir: &Path, name: &str, uuid: &str) -> Result<()> {
    let mut list = load_whitelist(dir).await?;
    if list.iter().any(|e| e.name.eq_ignore_ascii_case(name)) {
        bail!("'{}' is already on the whitelist.", name);
    }
    list.push(WhitelistEntry { uuid: uuid.to_string(), name: name.to_string() });
    save_whitelist(dir, &list).await
}

pub async fn remove_whitelist(dir: &Path, name: &str) -> Result<()> {
    let mut list = load_whitelist(dir).await?;
    let before = list.len();
    list.retain(|e| !e.name.eq_ignore_ascii_case(name));
    if list.len() == before { bail!("'{}' not found on whitelist.", name); }
    save_whitelist(dir, &list).await
}

// ── Ops ──────────────────────────────────────────────────────────────────────

pub async fn load_ops(dir: &Path) -> Result<Vec<OpEntry>> {
    let path = dir.join("ops.json");
    match fs::read_to_string(&path).await {
        Ok(raw) => Ok(serde_json::from_str(&raw).unwrap_or_default()),
        Err(_)  => Ok(Vec::new()),
    }
}

pub async fn save_ops(dir: &Path, entries: &[OpEntry]) -> Result<()> {
    fs::write(dir.join("ops.json"), serde_json::to_string_pretty(entries)?).await?;
    Ok(())
}

pub async fn add_op(dir: &Path, name: &str, uuid: &str, level: u8) -> Result<()> {
    let mut ops = load_ops(dir).await?;
    if let Some(e) = ops.iter_mut().find(|e| e.name.eq_ignore_ascii_case(name)) {
        e.level = level;
        return save_ops(dir, &ops).await;
    }
    ops.push(OpEntry { uuid: uuid.to_string(), name: name.to_string(), level, bypasses_player_limit: false });
    save_ops(dir, &ops).await
}

pub async fn remove_op(dir: &Path, name: &str) -> Result<()> {
    let mut ops = load_ops(dir).await?;
    let before = ops.len();
    ops.retain(|e| !e.name.eq_ignore_ascii_case(name));
    if ops.len() == before { bail!("'{}' not found in ops.", name); }
    save_ops(dir, &ops).await
}
