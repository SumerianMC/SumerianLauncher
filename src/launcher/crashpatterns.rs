/// Crash pattern learner.
///
/// Maintains a persistent `crash_patterns.json` in the data directory.
/// Each time a game session ends with a crash, the parser's suspected mods and
/// the crash type are recorded.  Over time this builds a ranked table of
/// "mods most associated with crashes" for this user's setup.
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CrashPatternStore {
    /// mod_fragment → number of crashes where this mod appeared in the stack trace
    pub mod_crash_count: HashMap<String, u32>,
    /// crash type label → count
    pub cause_count: HashMap<String, u32>,
    /// total crashes recorded
    pub total_crashes: u32,
}

impl CrashPatternStore {
    /// Return mods sorted by crash count descending (top N).
    pub fn top_suspects(&self, n: usize) -> Vec<(&str, u32)> {
        let mut v: Vec<_> = self
            .mod_crash_count
            .iter()
            .map(|(k, &c)| (k.as_str(), c))
            .collect();
        v.sort_by(|a, b| b.1.cmp(&a.1));
        v.truncate(n);
        v
    }

    /// Return cause labels sorted by frequency descending.
    pub fn top_causes(&self, n: usize) -> Vec<(&str, u32)> {
        let mut v: Vec<_> = self
            .cause_count
            .iter()
            .map(|(k, &c)| (k.as_str(), c))
            .collect();
        v.sort_by(|a, b| b.1.cmp(&a.1));
        v.truncate(n);
        v
    }
}

pub struct CrashPatternManager {
    path: PathBuf,
}

impl CrashPatternManager {
    pub fn new(data_dir: &Path) -> Self {
        Self { path: data_dir.join("crash_patterns.json") }
    }

    pub async fn load(&self) -> CrashPatternStore {
        match fs::read_to_string(&self.path).await {
            Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
            Err(_)  => CrashPatternStore::default(),
        }
    }

    pub async fn save(&self, store: &CrashPatternStore) -> Result<()> {
        if let Some(p) = self.path.parent() { fs::create_dir_all(p).await?; }
        fs::write(&self.path, serde_json::to_string_pretty(store)?).await?;
        Ok(())
    }

    /// Record a crash event from a parsed crash report.
    pub async fn record(
        &self,
        suspected_mods: &[String],
        cause_labels: &[&str],
    ) -> Result<()> {
        let mut store = self.load().await;
        store.total_crashes += 1;
        for m in suspected_mods {
            // Use only the first two package segments as the "mod fingerprint"
            let fragment = m.splitn(3, '.').take(2).collect::<Vec<_>>().join(".");
            if !fragment.is_empty() {
                *store.mod_crash_count.entry(fragment).or_default() += 1;
            }
        }
        for c in cause_labels {
            *store.cause_count.entry(c.to_string()).or_default() += 1;
        }
        self.save(&store).await
    }

    /// Clear all recorded patterns.
    pub async fn reset(&self) -> Result<()> {
        self.save(&CrashPatternStore::default()).await
    }
}
