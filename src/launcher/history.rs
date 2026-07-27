use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchRecord {
    pub version_id: String,
    pub username: String,
    pub started_at: DateTime<Utc>,
    pub duration_secs: u64,
    pub exit_code: Option<i32>,
}

pub struct HistoryManager {
    path: PathBuf,
}

impl HistoryManager {
    pub fn new(data_dir: &Path) -> Self {
        Self { path: data_dir.join("launch_history.json") }
    }

    pub async fn load(&self) -> Result<Vec<LaunchRecord>> {
        match fs::read_to_string(&self.path).await {
            Ok(raw) => Ok(serde_json::from_str(&raw).unwrap_or_default()),
            Err(_) => Ok(Vec::new()),
        }
    }

    pub async fn push(&self, record: LaunchRecord) -> Result<()> {
        let mut records = self.load().await?;
        records.push(record);
        // Keep last 100 entries
        if records.len() > 100 {
            records.drain(0..records.len() - 100);
        }
        fs::write(&self.path, serde_json::to_string_pretty(&records)?).await?;
        Ok(())
    }
}
