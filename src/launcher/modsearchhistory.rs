/// Mod Search History — persist the last N Modrinth search queries per
/// instance so users can re-run them without retyping.
///
/// Storage: `<data_dir>/mod_search_history.json`
/// Format:  `{ "<instance_or_default>": ["query1", "query2", ...] }`
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;

const MAX_HISTORY: usize = 20;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SearchHistory {
    /// Map of instance name (or "default") → ordered list (newest last).
    #[serde(flatten)]
    pub entries: HashMap<String, Vec<String>>,
}

pub struct ModSearchHistoryManager {
    path: PathBuf,
}

impl ModSearchHistoryManager {
    pub fn new(data_dir: &Path) -> Self {
        Self { path: data_dir.join("mod_search_history.json") }
    }

    async fn load(&self) -> SearchHistory {
        match fs::read_to_string(&self.path).await {
            Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
            Err(_) => SearchHistory::default(),
        }
    }

    async fn save(&self, history: &SearchHistory) -> Result<()> {
        fs::write(&self.path, serde_json::to_string_pretty(history)?).await?;
        Ok(())
    }

    /// Record a query for the given instance key.
    pub async fn push(&self, instance: &str, query: &str) -> Result<()> {
        let mut h = self.load().await;
        let list = h.entries.entry(instance.to_string()).or_default();
        // Remove duplicate if already present, then push to end.
        list.retain(|q| q != query);
        list.push(query.to_string());
        if list.len() > MAX_HISTORY {
            list.drain(0..list.len() - MAX_HISTORY);
        }
        self.save(&h).await
    }

    /// Return the query list for an instance (newest last → display reversed).
    pub async fn get(&self, instance: &str) -> Vec<String> {
        let h = self.load().await;
        h.entries.get(instance).cloned().unwrap_or_default()
    }

    /// Clear history for a specific instance.
    pub async fn clear(&self, instance: &str) -> Result<()> {
        let mut h = self.load().await;
        h.entries.remove(instance);
        self.save(&h).await
    }
}
