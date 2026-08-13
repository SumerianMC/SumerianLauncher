/// Advancement / achievement tracker.
///
/// Reads `<saves_dir>/<world>/advancements/<uuid>.json` and aggregates
/// completed vs total advancements per category (namespace).
use anyhow::Result;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Deserialize)]
struct AdvancementFile(HashMap<String, serde_json::Value>);

#[derive(Debug, Clone)]
pub struct CategoryProgress {
    pub namespace: String,
    pub done: usize,
    pub total: usize,
}

impl CategoryProgress {
    pub fn pct(&self) -> f64 {
        if self.total == 0 { 0.0 } else { self.done as f64 / self.total as f64 * 100.0 }
    }
}

/// Parse an advancements JSON file and return per-namespace progress.
pub fn parse_advancements(path: &Path) -> Result<Vec<CategoryProgress>> {
    let raw = std::fs::read_to_string(path)?;
    let map: HashMap<String, serde_json::Value> = serde_json::from_str(&raw)?;

    let mut buckets: HashMap<String, (usize, usize)> = HashMap::new();

    for (key, val) in &map {
        // Skip the DataVersion entry and recipe advancements
        if key == "DataVersion" { continue; }
        if key.contains("recipes/") { continue; }

        // Namespace is the part before the first `/` or the whole key
        let ns = key.split('/').next().unwrap_or(key).to_string();
        let entry = buckets.entry(ns).or_default();
        entry.1 += 1; // total

        // An advancement is "done" when its "done" field is true
        if val.get("done").and_then(|v| v.as_bool()).unwrap_or(false) {
            entry.0 += 1;
        }
    }

    let mut result: Vec<CategoryProgress> = buckets
        .into_iter()
        .map(|(ns, (done, total))| CategoryProgress { namespace: ns, done, total })
        .collect();
    result.sort_by(|a, b| b.done.cmp(&a.done));
    Ok(result)
}

/// Find the most-recently-modified advancements file in a world's advancements dir.
pub fn find_latest_advancements(world_dir: &Path) -> Option<std::path::PathBuf> {
    let adv_dir = world_dir.join("advancements");
    std::fs::read_dir(&adv_dir).ok()?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("json"))
        .max_by_key(|e| e.metadata().and_then(|m| m.modified()).ok())
        .map(|e| e.path())
}
