use std::collections::HashMap;
use chrono::{Datelike, Utc};
use crate::launcher::history::HistoryManager;

pub struct PlaytimeTracker<'a> {
    pub mgr: &'a HistoryManager,
}

pub struct PlaytimeSummary {
    /// Total seconds ever recorded.
    pub total_secs: u64,
    /// Seconds in the current calendar week (Mon–Sun).
    pub this_week_secs: u64,
    /// Seconds per version_id, sorted descending.
    pub by_version: Vec<(String, u64)>,
    /// Seconds per username, sorted descending.
    pub by_account: Vec<(String, u64)>,
    /// Seconds per (ISO year, ISO week number), sorted ascending — last 8 weeks.
    pub weekly: Vec<((i32, u32), u64)>,
}

impl<'a> PlaytimeTracker<'a> {
    pub fn new(mgr: &'a HistoryManager) -> Self {
        Self { mgr }
    }

    pub async fn summary(&self) -> anyhow::Result<PlaytimeSummary> {
        let records = self.mgr.load().await?;

        let now = Utc::now();
        let this_iso_week = now.iso_week();

        let mut total_secs: u64 = 0;
        let mut this_week_secs: u64 = 0;
        let mut by_version: HashMap<String, u64> = HashMap::new();
        let mut by_account: HashMap<String, u64> = HashMap::new();
        let mut weekly: HashMap<(i32, u32), u64> = HashMap::new();

        for r in &records {
            let d = r.duration_secs;
            total_secs += d;

            let iw = r.started_at.iso_week();
            if iw.year() == this_iso_week.year() && iw.week() == this_iso_week.week() {
                this_week_secs += d;
            }

            *by_version.entry(r.version_id.clone()).or_default() += d;
            *by_account.entry(r.username.clone()).or_default() += d;
            *weekly.entry((iw.year(), iw.week())).or_default() += d;
        }

        let mut by_version: Vec<_> = by_version.into_iter().collect();
        by_version.sort_by(|a, b| b.1.cmp(&a.1));

        let mut by_account: Vec<_> = by_account.into_iter().collect();
        by_account.sort_by(|a, b| b.1.cmp(&a.1));

        // Keep last 8 weeks, sorted ascending
        let mut weekly: Vec<_> = weekly.into_iter().collect();
        weekly.sort_by_key(|&(k, _)| k);
        if weekly.len() > 8 {
            weekly.drain(0..weekly.len() - 8);
        }

        Ok(PlaytimeSummary { total_secs, this_week_secs, by_version, by_account, weekly })
    }
}

pub fn fmt_duration(secs: u64) -> String {
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    if h > 0 { format!("{}h {}m", h, m) } else { format!("{}m", m) }
}

/// Build a simple ASCII bar scaled to `max_secs`.
pub fn bar(secs: u64, max_secs: u64, width: usize) -> String {
    if max_secs == 0 { return " ".repeat(width); }
    let filled = ((secs as f64 / max_secs as f64) * width as f64).round() as usize;
    let filled = filled.min(width);
    format!("{}{}", "█".repeat(filled), "░".repeat(width - filled))
}

/// Return the Monday date string for a given ISO (year, week).
pub fn week_label(year: i32, week: u32) -> String {
    // Find the Monday of that ISO week
    use chrono::{NaiveDate, Weekday};
    NaiveDate::from_isoywd_opt(year, week, Weekday::Mon)
        .map(|d| d.format("%b %d").to_string())
        .unwrap_or_else(|| format!("W{}", week))
}
